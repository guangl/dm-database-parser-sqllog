use memchr::{memchr, memrchr};
use memmap2::Mmap;
use std::borrow::Cow;
use std::fs::File;
use std::path::Path;

use crate::error::ParseError;
use crate::sqllog::Sqllog;
use encoding::all::GB18030;
use encoding::{DecoderTrap, Encoding};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileEncodingHint {
    /// Unknown / detect per-record (backward compatible)
    Auto,
    /// The file is UTF-8
    Utf8,
    /// The file is GB18030
    Gb18030,
}

impl Default for FileEncodingHint {
    fn default() -> Self {
        FileEncodingHint::Auto
    }
}

pub struct LogParser {
    mmap: Mmap,
    encoding: FileEncodingHint,
}

impl LogParser {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
        let file = File::open(path).map_err(|e| ParseError::IoError(e.to_string()))?;
        let mmap = unsafe { Mmap::map(&file).map_err(|e| ParseError::IoError(e.to_string()))? };

        // Sample the beginning of the file to determine if it's valid UTF-8.
        // If it's valid UTF-8, treat whole file as UTF-8 for speed. Otherwise assume GB18030.
        let sample_len = std::cmp::min(65536, mmap.len());
        let sample = &mmap[..sample_len];
        let encoding = if std::str::from_utf8(sample).is_ok() {
            FileEncodingHint::Utf8
        } else {
            FileEncodingHint::Gb18030
        };

        Ok(Self { mmap, encoding })
    }

    pub fn iter(&self) -> LogIterator<'_> {
        LogIterator {
            data: &self.mmap,
            pos: 0,
            encoding: self.encoding,
        }
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

        let (record_end, next_start) = if let Some(idx) = found_next {
            (idx, idx + 1)
        } else {
            (data.len(), data.len())
        };

        let record_slice = &data[..record_end];
        self.pos += next_start;

        // Trim trailing CR if present
        let record_slice = if record_slice.ends_with(b"\r") {
            &record_slice[..record_slice.len() - 1]
        } else {
            record_slice
        };

        if record_slice.is_empty() {
            return self.next();
        }

        Some(parse_record_with_hint(
            record_slice,
            is_multiline,
            self.encoding,
        ))
    }
}

pub fn parse_record<'a>(record_bytes: &'a [u8]) -> Result<Sqllog<'a>, ParseError> {
    parse_record_with_hint(record_bytes, true, FileEncodingHint::Auto)
}

fn parse_record_with_hint<'a>(
    record_bytes: &'a [u8],
    is_multiline: bool,
    encoding_hint: FileEncodingHint,
) -> Result<Sqllog<'a>, ParseError> {
    // Find end of first line
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

    // 1. Timestamp
    if first_line.len() < 23 {
        return Err(ParseError::InvalidFormat {
            raw: String::from_utf8_lossy(first_line).to_string(),
        });
    }
    // We assume ASCII/UTF-8 for timestamp
    // SAFETY: We validated the timestamp format in LogIterator::next using is_ts_millis_bytes,
    // which ensures it contains only digits and separators.
    let ts = unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(&first_line[0..23])) };

    // 2. Meta
    // Format: TS (META) BODY
    // Find first '(' after TS
    let meta_start = match memchr(b'(', &first_line[23..]) {
        Some(idx) => 23 + idx,
        None => {
            return Err(ParseError::InvalidFormat {
                raw: String::from_utf8_lossy(first_line).to_string(),
            });
        }
    };

    // Find closing ')' for meta.
    // We search for ") " starting from meta_start.
    // Optimization: use memchr loop instead of windows(2)
    let mut search_pos = meta_start;
    let meta_end = loop {
        match memchr(b')', &first_line[search_pos..]) {
            Some(idx) => {
                let abs_idx = search_pos + idx;
                // Check if followed by space
                if abs_idx + 1 < first_line.len() && first_line[abs_idx + 1] == b' ' {
                    break Some(abs_idx);
                }
                // If not, continue searching after this ')'
                search_pos = abs_idx + 1;
            }
            None => {
                // Fallback: find last ')' if ") " not found (robustness)
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
    // Lazy parsing: store raw bytes
    // SAFETY: meta_bytes is a sub-slice of first_line, which is 'a.
    // Use the provided encoding hint (file-level autodetection) to decide how to decode meta bytes.
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

    // 3. Body & 4. Indicators
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
        // If it starts with '[', try to find matching ']' and treat inner token as tag
        if !s.is_empty() && s[0] == b'[' {
            if let Some(end_idx) = memchr(b']', s) {
                if end_idx >= 1 {
                    let inner = &s[1..end_idx];
                    // Accept token without spaces and reasonable length
                    if !inner.iter().any(|&b| b == b' ') && inner.len() <= 32 {
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
                        // Move past the closing ']' and any following ASCII whitespace
                        s = &s[end_idx + 1..];
                        let mut skip = 0usize;
                        while skip < s.len() && s[skip].is_ascii_whitespace() {
                            skip += 1;
                        }
                        s = &s[skip..];
                    }
                }
            }
        }
        s
    } else {
        &[] as &[u8]
    };

    let content_raw = Cow::Borrowed(content_slice);

    Ok(Sqllog {
        ts,
        meta_raw,
        content_raw,
        tag,
        encoding: encoding_hint,
    })
}
