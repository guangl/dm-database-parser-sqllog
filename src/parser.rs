use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
use memmap2::Mmap;
use simdutf8::basic::from_utf8 as simd_from_utf8;
use std::borrow::Cow;
use std::fs::File;
use std::path::Path;
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::sqllog::Sqllog;
use encoding::all::GB18030;
use encoding::{DecoderTrap, Encoding};

/// Pre-built SIMD searcher for the `") "` meta-close pattern.
/// Avoids rebuilding the Finder on every record parse.
static FINDER_CLOSE_META: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b") "));

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
        let encoding = if simd_from_utf8(sample).is_ok() {
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

    /// Returns a Rayon parallel iterator over all log records.
    ///
    /// Splits the file into CPU-count chunks at record boundaries and
    /// processes each chunk on a separate thread.
    pub fn par_iter(
        &self,
    ) -> impl rayon::iter::ParallelIterator<Item = Result<Sqllog<'_>, ParseError>> + '_ {
        use rayon::prelude::*;

        let data: &[u8] = &self.mmap;
        let encoding = self.encoding;
        let num_threads = rayon::current_num_threads().max(1);

        // Find chunk start positions at record boundaries
        let mut starts: Vec<usize> = vec![0];
        if !data.is_empty() {
            let chunk_size = (data.len() / num_threads).max(1);
            for i in 1..num_threads {
                let boundary = find_next_record_start(data, i * chunk_size);
                if boundary < data.len() {
                    starts.push(boundary);
                }
            }
        }
        starts.push(data.len());
        starts.dedup();

        // Pair up (start, end) boundaries, collect to Vec so we can par_iter
        let bounds: Vec<(usize, usize)> = starts.windows(2).map(|w| (w[0], w[1])).collect();

        bounds
            .into_par_iter()
            .flat_map_iter(move |(start, end)| LogIterator {
                data: &data[start..end],
                pos: 0,
                encoding,
            })
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

/// Find the position of the next record start at or after `from`.
/// A record start is a line beginning with a timestamp pattern.
fn find_next_record_start(data: &[u8], from: usize) -> usize {
    let mut pos = from;
    // Skip to start of next line
    if let Some(nl) = memchr(b'\n', &data[pos..]) {
        pos += nl + 1;
    } else {
        return data.len();
    }
    // Scan forward for a line starting with timestamp
    loop {
        if pos + 23 > data.len() {
            return data.len();
        }
        let peek = &data[pos..pos + 23];
        if peek[0] == b'2'
            && peek[1] == b'0'
            && peek[4] == b'-'
            && peek[7] == b'-'
            && peek[10] == b' '
            && peek[13] == b':'
            && peek[16] == b':'
            && peek[19] == b'.'
        {
            return pos;
        }
        // Skip to next line
        match memchr(b'\n', &data[pos..]) {
            Some(nl) => pos += nl + 1,
            None => return data.len(),
        }
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

    // Find closing ')' for meta using pre-built SIMD Finder.
    let meta_end = match FINDER_CLOSE_META.find(&first_line[meta_start..]) {
        Some(idx) => Some(meta_start + idx),
        None => memrchr(b')', &first_line[meta_start..]).map(|idx| meta_start + idx),
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
        FileEncodingHint::Utf8 => match simd_from_utf8(meta_bytes) {
            Ok(s) => {
                // SAFETY: meta_bytes is a sub-slice of first_line which lives for 'a
                unsafe {
                    Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        s.as_ptr(),
                        s.len(),
                    )))
                }
            }
            Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
        },
        FileEncodingHint::Gb18030 => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
            Ok(s) => Cow::Owned(s),
            Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
        },
        FileEncodingHint::Auto => match simd_from_utf8(meta_bytes) {
            Ok(s) => {
                // SAFETY: meta_bytes is a sub-slice of first_line which lives for 'a
                unsafe {
                    Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        s.as_ptr(),
                        s.len(),
                    )))
                }
            }
            Err(_) => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
                Ok(s) => Cow::Owned(s),
                Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
            },
        },
    };

    // 3. Body & 4. Indicators
    let body_start_in_first_line = meta_end + 1;

    // The ") " pattern guarantees one space; skip it directly.
    let content_start = if body_start_in_first_line < first_line.len()
        && first_line[body_start_in_first_line] == b' '
    {
        body_start_in_first_line + 1
    } else {
        body_start_in_first_line
    };

    // Extract optional leading tag like [SEL] or [ORA]
    let mut tag: Option<Cow<'a, str>> = None;
    let content_slice = if content_start < record_bytes.len() {
        let mut s = &record_bytes[content_start..];
        // If it starts with '[', try to find matching ']' and treat inner token as tag
        if !s.is_empty()
            && s[0] == b'['
            && let Some(end_idx) = memchr(b']', s)
            && end_idx >= 1
        {
            let inner = &s[1..end_idx];
            // Accept token without spaces and reasonable length
            if !inner.contains(&b' ') && inner.len() <= 32 {
                tag = match simd_from_utf8(inner) {
                    Ok(st) => Some(unsafe {
                        // SAFETY: inner is a sub-slice of record_bytes which lives for 'a
                        Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                            st.as_ptr(),
                            st.len(),
                        )))
                    }),
                    Err(_) => match encoding_hint {
                        FileEncodingHint::Gb18030 => {
                            match GB18030.decode(inner, DecoderTrap::Strict) {
                                Ok(s) => Some(Cow::Owned(s)),
                                Err(_) => {
                                    Some(Cow::Owned(String::from_utf8_lossy(inner).into_owned()))
                                }
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
