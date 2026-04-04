use memchr::{memchr, memrchr};
use memmap2::Mmap;
use std::borrow::Cow;
use std::fs::File;
use std::path::Path;

use crate::error::ParseError;
use crate::sqllog::Sqllog;
use encoding::all::GB18030;
use encoding::{DecoderTrap, Encoding};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FileEncodingHint {
    #[default]
    Auto,
    Utf8,
    Gb18030,
}

pub struct LogParser {
    mmap: Mmap,
    encoding: FileEncodingHint,
}

impl LogParser {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
        let file = File::open(path).map_err(|e| ParseError::IoError(e.to_string()))?;
        let mmap = unsafe { Mmap::map(&file).map_err(|e| ParseError::IoError(e.to_string()))? };

        // Sample the first 64 KB to determine encoding.
        let sample = &mmap[..mmap.len().min(65536)];
        let encoding = if std::str::from_utf8(sample).is_ok() {
            FileEncodingHint::Utf8
        } else {
            FileEncodingHint::Gb18030
        };

        Ok(Self { mmap, encoding })
    }

    pub fn iter(&self) -> LogIterator<'_> {
        LogIterator { data: &self.mmap, pos: 0, encoding: self.encoding }
    }
}

pub struct LogIterator<'a> {
    data: &'a [u8],
    pos: usize,
    encoding: FileEncodingHint,
}

impl<'a> Iterator for LogIterator<'a> {
    type Item = Result<Sqllog<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data.len() {
            return None;
        }

        let data = &self.data[self.pos..];
        let mut scan_pos = 0;
        let mut found_next = None;
        let mut is_multiline = false;

        while let Some(idx) = memchr(b'\n', &data[scan_pos..]) {
            let newline_idx = scan_pos + idx;
            let next_line_start = newline_idx + 1;

            if next_line_start >= data.len() {
                break;
            }

            // Check if next line starts with timestamp
            let check_len = std::cmp::min(23, data.len() - next_line_start);
            if check_len == 23 {
                let next_bytes = &data[next_line_start..next_line_start + 23];
                // Fast check: 20xx and separators
                if next_bytes[0] == b'2'
                    && next_bytes[1] == b'0'
                    && next_bytes[4] == b'-'
                    && next_bytes[7] == b'-'
                    && next_bytes[10] == b' '
                    && next_bytes[13] == b':'
                    && next_bytes[16] == b':'
                    && next_bytes[19] == b'.'
                {
                    found_next = Some(newline_idx);
                    break;
                }
            }

            is_multiline = true;
            scan_pos = next_line_start;
        }

        let (record_end, next_start) = match found_next {
            Some(idx) => (idx, idx + 1),
            None => (data.len(), data.len()),
        };

        let record_slice = &data[..record_end];
        self.pos += next_start;

        // Trim trailing CR
        let record_slice = record_slice
            .strip_suffix(b"\r")
            .unwrap_or(record_slice);

        if record_slice.is_empty() {
            return self.next();
        }

        Some(parse_record_with_hint(record_slice, is_multiline, self.encoding))
    }
}

/// Parse a raw record byte slice. Exposed for testing.
pub fn parse_record<'a>(record_bytes: &'a [u8]) -> Result<Sqllog<'a>, ParseError> {
    parse_record_with_hint(record_bytes, true, FileEncodingHint::Auto)
}

fn parse_record_with_hint<'a>(
    record_bytes: &'a [u8],
    is_multiline: bool,
    encoding_hint: FileEncodingHint,
) -> Result<Sqllog<'a>, ParseError> {
    // For single-line records the record_slice IS the first line — skip the scan.
    let (first_line, _rest) = if is_multiline {
        match memchr(b'\n', record_bytes) {
            Some(idx) => {
                let mut line = &record_bytes[..idx];
                if line.ends_with(b"\r") {
                    line = &line[..line.len() - 1];
                }
                (line, &record_bytes[idx + 1..])
            }
            None => {
                let mut line = record_bytes;
                if line.ends_with(b"\r") {
                    line = &line[..line.len() - 1];
                }
                (line, &[] as &[u8])
            }
        }
    } else {
        let mut line = record_bytes;
        if line.ends_with(b"\r") {
            line = &line[..line.len() - 1];
        }
        (line, &[] as &[u8])
    };

    if first_line.len() < 23 {
        return Err(ParseError::InvalidFormat {
            raw: String::from_utf8_lossy(first_line).to_string(),
        });
    }

    // SAFETY: timestamp bytes are ASCII digits and separators, validated by the iterator.
    let ts = unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(&first_line[0..23])) };

    // Find the opening '(' for meta
    let meta_start = match memchr(b'(', &first_line[23..]) {
        Some(idx) => 23 + idx,
        None => {
            return Err(ParseError::InvalidFormat {
                raw: String::from_utf8_lossy(first_line).to_string(),
            });
        }
    };

    // Find the closing ") " for meta
    let mut search_pos = meta_start;
    let meta_end = loop {
        match memchr(b')', &first_line[search_pos..]) {
            Some(idx) => {
                let abs_idx = search_pos + idx;
                if abs_idx + 1 < first_line.len() && first_line[abs_idx + 1] == b' ' {
                    break Some(abs_idx);
                }
                search_pos = abs_idx + 1;
            }
            None => {
                break memrchr(b')', &first_line[meta_start..]).map(|idx| meta_start + idx);
            }
        }
    };

    let meta_end = match meta_end {
        Some(idx) => idx,
        None => {
            return Err(ParseError::InvalidFormat {
                raw: String::from_utf8_lossy(first_line).to_string(),
            });
        }
    };

    let meta_bytes = &first_line[meta_start + 1..meta_end];
    let meta_raw = match encoding_hint {
        FileEncodingHint::Utf8 => match std::str::from_utf8(meta_bytes) {
            Ok(s) => Cow::Borrowed(s),
            Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
        },
        FileEncodingHint::Gb18030 => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
            Ok(s) => Cow::Owned(s),
            Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
        },
        FileEncodingHint::Auto => match std::str::from_utf8(meta_bytes) {
            Ok(s) => Cow::Borrowed(s),
            Err(_) => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
                Ok(s) => Cow::Owned(s),
                Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
            },
        },
    };

    // Body starts after ") "
    let body_start_in_first_line = meta_end + 1;
    let first_line_body = if body_start_in_first_line < first_line.len() {
        &first_line[body_start_in_first_line..]
    } else {
        &[]
    };
    let start_idx = first_line_body
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(first_line_body.len());
    let content_start = body_start_in_first_line + start_idx;

    // Extract optional leading tag like [SEL] or [ORA]
    let mut tag: Option<Cow<'a, str>> = None;
    let content_slice = if content_start < record_bytes.len() {
        let mut s = &record_bytes[content_start..];
        if !s.is_empty()
            && s[0] == b'['
            && let Some(end_idx) = memchr(b']', s)
            && end_idx >= 1
        {
            let inner = &s[1..end_idx];
            if !inner.contains(&b' ') && inner.len() <= 32 {
                tag = match std::str::from_utf8(inner) {
                    Ok(st) => Some(Cow::Borrowed(st)),
                    Err(_) => match encoding_hint {
                        FileEncodingHint::Gb18030 => {
                            match GB18030.decode(inner, DecoderTrap::Strict) {
                                Ok(s) => Some(Cow::Owned(s)),
                                Err(_) => Some(Cow::Owned(
                                    String::from_utf8_lossy(inner).into_owned(),
                                )),
                            }
                        }
                        _ => Some(Cow::Owned(String::from_utf8_lossy(inner).into_owned())),
                    },
                };
                s = &s[end_idx + 1..];
                let mut skip = 0usize;
                while skip < s.len() && s[skip].is_ascii_whitespace() {
                    skip += 1;
                }
                s = &s[skip..];
            }
        }
        s
    } else {
        &[] as &[u8]
    };

    Ok(Sqllog {
        ts,
        meta_raw,
        content_raw: Cow::Borrowed(content_slice),
        tag,
        encoding: encoding_hint,
    })
}

