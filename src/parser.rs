use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::sqllog;
use crate::sqllog::Sqllog;
use encoding::all::GB18030;
use encoding::{DecoderTrap, Encoding};

/// Pre-built SIMD searcher for the `") "` meta-close pattern.
static FINDER_CLOSE_META: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b") "));

/// Pre-built SIMD searcher for the `"\n20"` record-start pattern.
static FINDER_RECORD_START: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"\n20"));

/// 文件编码提示，用于指示日志文件的字符编码。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FileEncodingHint {
    /// 自动探测编码（默认行为）
    #[default]
    Auto,
    /// 文件使用 UTF-8 编码
    Utf8,
    /// 文件使用 GB18030 编码
    Gb18030,
}

/// SQL 日志文件解析器。
///
/// 通过 [`LogParserBuilder`] 构建实例。内部将整个文件读入内存，
/// 自动检测文件编码（UTF-8 或 GB18030）。
pub struct LogParser {
    data: Vec<u8>,
    encoding: FileEncodingHint,
}

/// 配置并构建 [`LogParser`] 的构建器模式 API。
pub struct LogParserBuilder {
    path: PathBuf,
    encoding_hint: Option<FileEncodingHint>,
}

impl LogParserBuilder {
    /// 创建一个新的 `LogParserBuilder`。
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            encoding_hint: None,
        }
    }

    /// 设置文件编码提示。
    pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
        self.encoding_hint = Some(hint);
        self
    }

    /// 构建并返回 [`LogParser`] 实例。
    pub fn build(self) -> Result<LogParser, ParseError> {
        let data = fs::read(&self.path)
            .map_err(|e| ParseError::IoError(e.to_string()))?;

        let encoding = match self.encoding_hint {
            Some(hint) => hint,
            None => {
                // 自动编码探测：采样头部 64KB 和尾部 4KB
                let head_size = data.len().min(64 * 1024);
                let head_ok = str::from_utf8(&data[..head_size]).is_ok();
                let tail_start = data.len().saturating_sub(4 * 1024).max(head_size);
                let tail_ok = tail_start >= data.len()
                    || str::from_utf8(&data[tail_start..]).is_ok();
                if head_ok && tail_ok {
                    FileEncodingHint::Utf8
                } else {
                    FileEncodingHint::Gb18030
                }
            }
        };

        Ok(LogParser { data, encoding })
    }
}

impl LogParser {
    /// 返回顺序迭代器。
    pub fn iter(&self) -> LogIterator<'_> {
        LogIterator {
            data: &self.data,
            pos: 0,
            encoding: self.encoding,
            line_number: 1,
        }
    }
}

/// SQL 日志记录的顺序迭代器。
pub struct LogIterator<'a> {
    data: &'a [u8],
    pos: usize,
    encoding: FileEncodingHint,
    line_number: u64,
}

impl<'a> LogIterator<'a> {
    /// 返回一个跳过解析错误的迭代器。
    pub fn skip_errors(self) -> impl Iterator<Item = Sqllog> + 'a {
        self.filter_map(Result::ok)
    }

    /// 过滤出执行时间大于等于 `min_ms` 毫秒的记录。
    pub fn filter_by_exec_time(
        self,
        min_ms: u64,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        let threshold = min_ms as f32;
        self.filter(move |item| match item {
            Ok(sqllog) => sqllog.exectime >= threshold,
            Err(_) => false,
        })
    }

    /// 过滤出 SQL 语句体包含指定 `pattern` 的记录。
    pub fn filter_by_sql_contains(
        self,
        pattern: &str,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        let pattern = pattern.to_string();
        self.filter(move |item| match item {
            Ok(sqllog) => sqllog.sql.contains(&pattern),
            Err(_) => false,
        })
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

            return Some(parse_record_with_hint(
                record_slice,
                self.encoding,
                current_line,
            ));
        }
    }
}

/// 从原始字节解析单条 SQL 日志记录。
///
/// 自动检测多行模式。适合已从文件中读出完整记录的调用方。
pub fn parse_record(record_bytes: &[u8]) -> Result<Sqllog, ParseError> {
    parse_record_with_hint(record_bytes, FileEncodingHint::Auto, 0)
}

/// 核心解析函数：从原始字节一次性解析全部字段到 Sqllog。
fn parse_record_with_hint(
    record_bytes: &[u8],
    encoding_hint: FileEncodingHint,
    line_number: u64,
) -> Result<Sqllog, ParseError> {
    // 检测是否多行
    let is_multiline = memchr(b'\n', record_bytes).is_some();

    // 找到第一行
    let first_line = if is_multiline {
        match memchr(b'\n', record_bytes) {
            Some(idx) => {
                let mut line = &record_bytes[..idx];
                if line.ends_with(b"\r") {
                    line = &line[..line.len() - 1];
                }
                line
            }
            None => {
                let mut line = record_bytes;
                if line.ends_with(b"\r") {
                    line = &line[..line.len() - 1];
                }
                line
            }
        }
    } else {
        let mut line = record_bytes;
        if line.ends_with(b"\r") {
            line = &line[..line.len() - 1];
        }
        line
    };

    // ── 1. 时间戳 ──
    if first_line.len() < 23 {
        return Err(make_invalid_format_error(first_line, line_number));
    }
    let ts = match str::from_utf8(&first_line[0..23]) {
        Ok(s) => s.to_string(),
        Err(_) => return Err(make_invalid_format_error(first_line, line_number)),
    };

    // ── 2. 元数据 ──
    let meta_start = match memchr(b'(', &first_line[23..]) {
        Some(idx) => 23 + idx,
        None => return Err(make_invalid_format_error(first_line, line_number)),
    };

    let meta_end = match FINDER_CLOSE_META.find(&first_line[meta_start..]) {
        Some(idx) => Some(meta_start + idx),
        None => memrchr(b')', &first_line[meta_start..]).map(|idx| meta_start + idx),
    };

    let meta_end = match meta_end {
        Some(idx) => idx,
        None => return Err(make_invalid_format_error(first_line, line_number)),
    };

    let meta_bytes = &first_line[meta_start + 1..meta_end];

    // 解析元数据（考虑编码）
    let (ep, sess_id, thrd_id, username, trxid, statement, appname, client_ip) =
        match encoding_hint {
            FileEncodingHint::Utf8 => {
                sqllog::parse_meta_from_bytes(meta_bytes)
            }
            FileEncodingHint::Auto => {
                // Auto: try UTF-8 first, then GB18030 fallback
                match str::from_utf8(meta_bytes) {
                    Ok(_) => sqllog::parse_meta_from_bytes(meta_bytes),
                    Err(_) => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
                        Ok(decoded) => sqllog::parse_meta_from_bytes(decoded.as_bytes()),
                        Err(_) => {
                            let lossy = String::from_utf8_lossy(meta_bytes).into_owned();
                            sqllog::parse_meta_from_bytes(lossy.as_bytes())
                        }
                    },
                }
            }
            FileEncodingHint::Gb18030 => {
                match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
                    Ok(decoded) => sqllog::parse_meta_from_bytes(decoded.as_bytes()),
                    Err(_) => {
                        let lossy = String::from_utf8_lossy(meta_bytes).into_owned();
                        sqllog::parse_meta_from_bytes(lossy.as_bytes())
                    }
                }
            }
        };

    // ── 3. Body 和 Indicators ──
    let body_start_in_first_line = meta_end + 1;

    let content_start = if body_start_in_first_line < first_line.len()
        && first_line[body_start_in_first_line] == b' '
    {
        body_start_in_first_line + 1
    } else {
        body_start_in_first_line
    };

    // 提取可选的标签 [SEL] / [ORA]
    let mut tag: Option<String> = None;
    let content_slice = if content_start < record_bytes.len() {
        let mut s = &record_bytes[content_start..];
        if !s.is_empty()
            && s[0] == b'['
            && let Some(end_idx) = memchr(b']', s)
            && end_idx >= 1
        {
            let inner = &s[1..end_idx];
            if !inner.contains(&b' ') && inner.len() <= 32 {
                tag = match encoding_hint {
                    FileEncodingHint::Utf8 => {
                        str::from_utf8(inner).ok().map(|t| t.to_string())
                    }
                    FileEncodingHint::Auto => {
                        str::from_utf8(inner).ok().map(|t| t.to_string())
                            .or_else(|| {
                                GB18030.decode(inner, DecoderTrap::Strict)
                                    .ok()
                            })
                    }
                    FileEncodingHint::Gb18030 => {
                        GB18030.decode(inner, DecoderTrap::Strict)
                            .ok()
                            .or_else(|| str::from_utf8(inner).ok().map(|s| s.to_string()))
                    }
                };
                // 跳过 ']' 及后续空白
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

    // 分割 body 和 indicators
    let split = sqllog::find_indicators_split(content_slice);
    let body_bytes = &content_slice[..split];
    let ind_bytes = &content_slice[split..];

    // 解码 body
    let sql_raw = match encoding_hint {
        FileEncodingHint::Utf8 => {
            String::from_utf8_lossy(body_bytes).into_owned()
        }
        FileEncodingHint::Auto => {
            match str::from_utf8(body_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => match GB18030.decode(body_bytes, DecoderTrap::Strict) {
                    Ok(s) => s,
                    Err(_) => String::from_utf8_lossy(body_bytes).into_owned(),
                },
            }
        }
        FileEncodingHint::Gb18030 => {
            match GB18030.decode(body_bytes, DecoderTrap::Strict) {
                Ok(s) => s,
                Err(_) => String::from_utf8_lossy(body_bytes).into_owned(),
            }
        }
    };

    // 处理 ORA 前缀
    let sql = if tag.as_deref() == Some("ORA") {
        sql_raw.strip_prefix(": ").unwrap_or(&sql_raw).to_string()
    } else {
        sql_raw
    };

    // 解析性能指标
    let (exectime, rowcount, exec_id) = sqllog::parse_indicators_from_bytes(ind_bytes);

    Ok(Sqllog {
        ts,
        tag,
        ep,
        sess_id,
        thrd_id,
        username,
        trxid,
        statement,
        appname,
        client_ip,
        sql,
        exectime,
        rowcount,
        exec_id,
    })
}

// ── 时间戳验证 ──────────────────────────────────────────────────────────────

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

#[cold]
fn make_invalid_format_error(raw_bytes: &[u8], line_number: u64) -> ParseError {
    ParseError::InvalidFormat {
        raw: String::from_utf8_lossy(raw_bytes).to_string(),
        line_number,
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

    #[cfg(not(miri))]
    #[test]
    fn test_builder_encoding_hint_utf8() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(
            tmp,
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1"
        )
        .unwrap();
        tmp.as_file().sync_all().unwrap();

        let parser = LogParserBuilder::new(tmp.path())
            .encoding_hint(FileEncodingHint::Utf8)
            .build()
            .expect("build");
        let record = parser.iter().next().unwrap().unwrap();
        assert_eq!(record.ts, "2025-11-17 16:09:41.123");
        assert!(record.sql.contains("SELECT 1"));
    }

    #[cfg(not(miri))]
    #[test]
    fn test_builder_file_not_found() {
        let result = LogParserBuilder::new("/nonexistent/path.log").build();
        assert!(result.is_err());
        match result {
            Err(ParseError::IoError(_)) => {}
            _ => panic!("Expected IoError on nonexistent file"),
        }
    }
}
