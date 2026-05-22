use memchr::memchr;
use memchr::memmem::Finder;
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::filter::adapter;
use crate::filter::builder::Filter;
use crate::parser::encoding::FileEncodingHint;
use crate::record::Sqllog;

/// Pre-built SIMD searcher for the `"\n20"` record-start pattern.
static FINDER_RECORD_START: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"\n20"));

// ── 时间戳验证常量 ──────────────────────────────────────────────────────────────

const LO_MASK: u64 = 0xFF0000FF0000FFFF;
const LO_EXPECTED: u64 = 0x2D00002D00003032;
const HI_MASK: u64 = 0x0000FF0000FF0000;
const HI_EXPECTED: u64 = 0x00003A0000200000;

/// 检查 bytes[0..23] 是否符合时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"。
#[inline(always)]
fn is_timestamp_start(bytes: &[u8]) -> bool {
    debug_assert!(bytes.len() >= 23);
    let lo = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let hi = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    (lo & LO_MASK == LO_EXPECTED)
        && (hi & HI_MASK == HI_EXPECTED)
        && bytes[16] == b':'
        && bytes[19] == b'.'
}

/// SQL 日志记录的顺序迭代器。
pub struct LogIterator<'a> {
    pub(super) data: &'a [u8],
    pub(super) pos: usize,
    pub(super) encoding: FileEncodingHint,
    pub(super) line_number: u64,
}

impl<'a> LogIterator<'a> {
    /// 返回一个跳过解析错误的迭代器。
    pub fn skip_errors(self) -> impl Iterator<Item = Sqllog> + 'a {
        self.filter_map(Result::ok)
    }

    /// 过滤出执行时间大于等于 `min_ms` 毫秒的记录。
    ///
    /// 解析错误在迭代过程中**静默丢弃**。若需保留错误，请使用 [`apply_filter_keep_errors`]。
    pub fn filter_by_exec_time(
        self,
        min_ms: f32,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::filter_by_exec_time(self, min_ms)
    }

    /// 过滤出 SQL 语句体包含指定 `pattern` 的记录。
    ///
    /// 解析错误在迭代过程中**静默丢弃**。若需保留错误，请使用 [`apply_filter_keep_errors`]。
    pub fn filter_by_sql_contains(
        self,
        pattern: &str,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::filter_by_sql_contains(self, pattern)
    }

    /// 应用 FilterBuilder 产出的组合过滤器，错误记录被丢弃（与 filter_by_exec_time 一致）。
    pub fn apply_filter(
        self,
        filter: Filter,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::apply_filter(self, filter)
    }

    /// 应用 FilterBuilder 产出的组合过滤器，错误记录透传。
    pub fn apply_filter_keep_errors(
        self,
        filter: Filter,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::apply_filter_keep_errors(self, filter)
    }
}

impl<'a> Iterator for LogIterator<'a> {
    type Item = Result<Sqllog, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.pos >= self.data.len() {
                return None;
            }

            let data = &self.data[self.pos..];
            let current_line = self.line_number;

            let (record_end, next_start) = match memchr(b'\n', data) {
                None => (data.len(), data.len()),
                Some(first_nl) => {
                    let ts_start = first_nl + 1;
                    if ts_start + 23 <= data.len()
                        && is_timestamp_start(&data[ts_start..ts_start + 23])
                    {
                        (first_nl, ts_start)
                    } else {
                        // 多行记录：用 memmem 跳过嵌入换行继续搜索
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
                            Some(idx) => (idx, idx + 1),
                            None => (data.len(), data.len()),
                        }
                    }
                }
            };

            let record_slice = &data[..record_end];
            self.pos += next_start;

            self.line_number += data[..next_start].iter().filter(|&&b| b == b'\n').count() as u64;

            // Trim trailing CR
            let record_slice = if record_slice.ends_with(b"\r") {
                &record_slice[..record_slice.len() - 1]
            } else {
                record_slice
            };

            if record_slice.is_empty() {
                continue;
            }

            return Some(super::parse_record_with_hint(
                record_slice,
                self.encoding,
                current_line,
            ));
        }
    }
}

// ── 测试 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_timestamp_start_valid() {
        let ts = b"2025-11-17 16:09:41.123";
        assert!(is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_year_prefix() {
        let ts = b"1025-11-17 16:09:41.123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_month_separator() {
        let ts = b"2025X11-17 16:09:41.123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_second_separator() {
        let ts = b"2025-11-17 16:09X41.123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_millis_separator() {
        let ts = b"2025-11-17 16:09:41X123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_exactly_23_bytes() {
        let ts = b"2025-11-17 16:09:41.123";
        assert_eq!(ts.len(), 23);
        assert!(is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_trailing_garbage() {
        let ts = b"2025-11-17 16:09:41.123extra_garbage_here";
        assert!(is_timestamp_start(ts));
    }
}
