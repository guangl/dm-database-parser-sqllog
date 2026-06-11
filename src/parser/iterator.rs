use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::error::ParseError;
use crate::filter::adapter;
use crate::filter::builder::Filter;
use crate::parser::encoding::FileEncodingHint;
use crate::record::Sqllog;

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

#[inline(always)]
fn trailing_newline_len(s: &[u8]) -> usize {
    if s.ends_with(b"\r\n") {
        2
    } else if s.ends_with(b"\n") {
        1
    } else {
        0
    }
}

/// SQL 日志记录的流式迭代器。
///
/// 内部使用 `BufReader` 逐行读取，内存峰值约等于单条最大记录的大小。
pub struct LogIterator {
    reader: BufReader<File>,
    encoding: FileEncodingHint,
    pending: Vec<u8>,
    pending_line_number: u64,
    next_line_number: u64,
    line_buf: Vec<u8>,
    done: bool,
}

impl LogIterator {
    pub(super) fn new(file: File, encoding: FileEncodingHint) -> Self {
        Self {
            reader: BufReader::new(file),
            encoding,
            pending: Vec::new(),
            pending_line_number: 1,
            next_line_number: 1,
            line_buf: Vec::new(),
            done: false,
        }
    }

    /// 返回一个跳过解析错误的迭代器。
    pub fn skip_errors(self) -> impl Iterator<Item = Sqllog> {
        self.filter_map(Result::ok)
    }

    /// 过滤出执行时间大于等于 `min_ms` 毫秒的记录。
    pub fn filter_by_exec_time(
        self,
        min_ms: f32,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> {
        adapter::filter_by_exec_time(self, min_ms)
    }

    /// 过滤出 SQL 语句体包含指定 `pattern` 的记录。
    pub fn filter_by_sql_contains(
        self,
        pattern: &str,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> {
        adapter::filter_by_sql_contains(self, pattern)
    }

    /// 应用 FilterBuilder 产出的组合过滤器，错误记录被丢弃。
    pub fn apply_filter(
        self,
        filter: Filter,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> {
        adapter::apply_filter(self, filter)
    }

    /// 应用 FilterBuilder 产出的组合过滤器，错误记录透传。
    pub fn apply_filter_keep_errors(
        self,
        filter: Filter,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> {
        adapter::apply_filter_keep_errors(self, filter)
    }
}

impl Iterator for LogIterator {
    type Item = Result<Sqllog, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.done {
                return None;
            }

            self.line_buf.clear();
            let bytes_read = match self.reader.read_until(b'\n', &mut self.line_buf) {
                Ok(n) => n,
                Err(e) => {
                    self.done = true;
                    return Some(Err(ParseError::IoError(e.to_string())));
                }
            };

            if bytes_read == 0 {
                self.done = true;
                if self.pending.is_empty() {
                    return None;
                }
                let trim = trailing_newline_len(&self.pending);
                let end = self.pending.len() - trim;
                if end == 0 {
                    return None;
                }
                return Some(super::parse_record_with_hint(
                    &self.pending[..end],
                    self.encoding,
                    self.pending_line_number,
                ));
            }

            let current_line = self.next_line_number;
            self.next_line_number += 1;

            let is_new_record =
                self.line_buf.len() >= 23 && is_timestamp_start(&self.line_buf);

            if is_new_record && !self.pending.is_empty() {
                let trim = trailing_newline_len(&self.pending);
                let end = self.pending.len() - trim;

                if end == 0 {
                    // blank line between records — skip silently
                    self.pending.clear();
                    self.pending_line_number = current_line;
                    self.pending.extend_from_slice(&self.line_buf);
                    continue;
                }

                let result = super::parse_record_with_hint(
                    &self.pending[..end],
                    self.encoding,
                    self.pending_line_number,
                );
                self.pending.clear();
                self.pending_line_number = current_line;
                self.pending.extend_from_slice(&self.line_buf);
                return Some(result);
            }

            if is_new_record {
                self.pending_line_number = current_line;
                self.pending.extend_from_slice(&self.line_buf);
            } else if !self.pending.is_empty() {
                self.pending.extend_from_slice(&self.line_buf);
            } else {
                // non-record line before any record start: buffer as a (likely invalid) record
                self.pending_line_number = current_line;
                self.pending.extend_from_slice(&self.line_buf);
            }
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
