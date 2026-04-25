use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
#[cfg(unix)]
use memmap2::Advice;
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

/// Pre-built SIMD searcher for the `"\n20"` record-start pattern.
/// Shared across threads via LazyLock; constructed once on first use.
static FINDER_RECORD_START: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"\n20"));

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum FileEncodingHint {
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

        // HOT-04: 告知 OS 以顺序模式预读 mmap 页面，减少 page fault 开销
        // Unix-only；Windows 上 advise() 方法不存在，cfg 门控跳过
        // 失败（如内核不支持）静默忽略，不影响正确性
        #[cfg(unix)]
        let _ = mmap.advise(Advice::Sequential);

        // Detect encoding by sampling the first 64 KB and the last 4 KB.
        // Sampling both ends catches the rare case where GB18030 content only
        // appears after the initial UTF-8 section (e.g. late-joined non-ASCII
        // usernames), while keeping the cost well below a full-file scan.
        let head_size = mmap.len().min(64 * 1024);
        let tail_start = mmap.len().saturating_sub(4 * 1024).max(head_size);
        let head_ok = simd_from_utf8(&mmap[..head_size]).is_ok();
        let tail_ok = tail_start >= mmap.len() || simd_from_utf8(&mmap[tail_start..]).is_ok();
        let encoding = if head_ok && tail_ok {
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
        loop {
            if self.pos >= self.data.len() {
                return None;
            }

            let data = &self.data[self.pos..];

            // 快速路径：先用 memchr 找第一个 '\n'，若下一行即是时间戳则为单行记录
            // 慢速路径（多行）：用 FINDER_RECORD_START.find_iter 跳过嵌入换行
            let (record_end, next_start, is_multiline) = match memchr(b'\n', data) {
                None => (data.len(), data.len(), false),
                Some(first_nl) => {
                    let ts_start = first_nl + 1;
                    if ts_start + 23 <= data.len()
                        && is_timestamp_start(&data[ts_start..ts_start + 23])
                    {
                        // 单行记录：边界就是第一个 '\n'
                        (first_nl, ts_start, false)
                    } else {
                        // 多行记录：用 memmem 跳过嵌入换行继续搜索
                        // ALGO-01: find_iter 替代逐行 while-memchr 循环
                        let mut found_boundary: Option<usize> = None;
                        for candidate in FINDER_RECORD_START.find_iter(&data[ts_start..]) {
                            let abs_ts = ts_start + candidate + 1;
                            if abs_ts + 23 <= data.len()
                                && is_timestamp_start(&data[abs_ts..abs_ts + 23])
                            {
                                found_boundary = Some(ts_start + candidate);
                                break;
                            }
                        }
                        match found_boundary {
                            Some(idx) => (idx, idx + 1, true),
                            None => (data.len(), data.len(), true),
                        }
                    }
                }
            };

            let record_slice = &data[..record_end];
            self.pos += next_start;

            // Trim trailing CR if present
            let record_slice = if record_slice.ends_with(b"\r") {
                &record_slice[..record_slice.len() - 1]
            } else {
                record_slice
            };

            // Skip empty slices iteratively instead of recursing to avoid stack overflow
            // when the file contains many consecutive blank lines.
            if record_slice.is_empty() {
                continue;
            }

            return Some(parse_record_with_hint(
                record_slice,
                is_multiline,
                self.encoding,
            ));
        }
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
    // 先检查 pos 本身是否是时间戳行（Finder 不会命中无前置 '\n' 的行首）
    if pos + 23 <= data.len() && is_timestamp_start(&data[pos..pos + 23]) {
        return pos;
    }

    // ALGO-01: memmem 单次扫描替代逐行 memchr loop
    for candidate in FINDER_RECORD_START.find_iter(&data[pos..]) {
        let ts_start = pos + candidate + 1;
        if ts_start + 23 <= data.len() && is_timestamp_start(&data[ts_start..ts_start + 23]) {
            return ts_start;
        }
    }
    data.len()
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
        return Err(make_invalid_format_error(first_line));
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
            return Err(make_invalid_format_error(first_line));
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
            return Err(make_invalid_format_error(first_line));
        }
    };

    let meta_bytes = &first_line[meta_start + 1..meta_end];
    // Lazy parsing: store raw bytes as a Cow<'a, str>.
    // For Utf8 / Auto-UTF8 encoding: meta_bytes is a sub-slice of the memory-mapped buffer
    // (raw UTF-8 bytes) that lives for 'a — borrowing is sound.
    // For Gb18030 / Auto-GB18030 encoding: GB18030.decode() produces a new owned String, so
    // meta_raw becomes Cow::Owned; the 'a lifetime is NOT extended to that allocation.
    let meta_raw = match encoding_hint {
        FileEncodingHint::Utf8 => {
            // File already validated as UTF-8 during `from_path`; skip per-slice re-validation.
            // SAFETY: meta_bytes is a sub-slice of record_bytes which lives for 'a.
            // No lifetime extension via from_raw_parts needed — meta_bytes already carries 'a.
            unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(meta_bytes)) }
        }
        FileEncodingHint::Gb18030 => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
            Ok(s) => Cow::Owned(s),
            Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
        },
        FileEncodingHint::Auto => match simd_from_utf8(meta_bytes) {
            Ok(_) => {
                // SAFETY: meta_bytes is a sub-slice of record_bytes which lives for 'a;
                // simd_from_utf8 confirmed it is valid UTF-8.
                unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(meta_bytes)) }
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
                tag = match encoding_hint {
                    FileEncodingHint::Utf8 => {
                        // File already validated as UTF-8; skip re-validation.
                        // SAFETY: inner is a sub-slice of record_bytes which lives for 'a.
                        // No from_raw_parts needed — inner already carries 'a lifetime.
                        Some(unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(inner)) })
                    }
                    _ => match simd_from_utf8(inner) {
                        Ok(_) => Some(unsafe {
                            // SAFETY: inner is a sub-slice of record_bytes which lives for 'a;
                            // simd_from_utf8 confirmed it is valid UTF-8.
                            Cow::Borrowed(std::str::from_utf8_unchecked(inner))
                        }),
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

// u64 掩码常量：验证时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"
// 字节位置：0('2'), 1('0'), 4('-'), 7('-'), 10(' '), 13(':'), 16(':'), 19('.')
const LO_MASK: u64 = 0xFF0000FF0000FFFF; // data[0..8]：位置 0,1,4,7
const LO_EXPECTED: u64 = 0x2D00002D00003032; // LE: '2'=0x32,'0'=0x30,'-'=0x2D,'-'=0x2D
const HI_MASK: u64 = 0x0000FF0000FF0000; // data[8..16]：位置 10,13（偏移 2,5）
const HI_EXPECTED: u64 = 0x00003A0000200000; // LE: ' '=0x20,':'=0x3A

/// 检查 bytes[0..23] 是否符合时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"。
/// 调用前需确保 bytes.len() >= 23（由调用方做长度检查）。
#[inline(always)]
fn is_timestamp_start(bytes: &[u8]) -> bool {
    debug_assert!(bytes.len() >= 23);
    let lo = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let hi = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    // 位置 16(':') 和 19('.') 用两次单字节比较（比第三次 u64 load 更清晰）
    (lo & LO_MASK == LO_EXPECTED)
        && (hi & HI_MASK == HI_EXPECTED)
        && bytes[16] == b':'
        && bytes[19] == b'.'
}

/// 将原始字节转换为 InvalidFormat 错误（错误路径，标注 cold 避免影响热路径代码布局）
#[cold]
fn make_invalid_format_error(raw_bytes: &[u8]) -> ParseError {
    ParseError::InvalidFormat {
        raw: String::from_utf8_lossy(raw_bytes).to_string(),
    }
}
