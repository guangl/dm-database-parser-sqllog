pub mod builder;
pub mod iterator;
pub(crate) mod encoding;

pub use builder::LogParserBuilder;
pub use iterator::LogIterator;
pub use encoding::FileEncodingHint;

use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
use std::str;
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::record::{self, Sqllog};
use ::encoding::all::GB18030;
use ::encoding::{DecoderTrap, Encoding};

/// Pre-built SIMD searcher for the `") "` meta-close pattern.
static FINDER_CLOSE_META: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b") "));

/// SQL 日志文件解析器。
///
/// 通过 [`LogParserBuilder`] 构建实例。内部将整个文件读入内存，
/// 自动检测文件编码（UTF-8 或 GB18030）。
pub struct LogParser {
    pub(super) data: Vec<u8>,
    pub(super) encoding: FileEncodingHint,
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

/// 从原始字节解析单条 SQL 日志记录。
///
/// 自动检测多行模式。适合已从文件中读出完整记录的调用方。
pub(crate) fn parse_record(record_bytes: &[u8]) -> Result<Sqllog, ParseError> {
    parse_record_with_hint(record_bytes, FileEncodingHint::Auto, 0)
}

/// 核心解析函数：从原始字节一次性解析全部字段到 Sqllog。
pub(super) fn parse_record_with_hint(
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
                record::parse_meta_from_bytes(meta_bytes)
            }
            FileEncodingHint::Auto => {
                // Auto: try UTF-8 first, then GB18030 fallback
                match str::from_utf8(meta_bytes) {
                    Ok(_) => record::parse_meta_from_bytes(meta_bytes),
                    Err(_) => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
                        Ok(decoded) => record::parse_meta_from_bytes(decoded.as_bytes()),
                        Err(_) => {
                            let lossy = String::from_utf8_lossy(meta_bytes).into_owned();
                            record::parse_meta_from_bytes(lossy.as_bytes())
                        }
                    },
                }
            }
            FileEncodingHint::Gb18030 => {
                match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
                    Ok(decoded) => record::parse_meta_from_bytes(decoded.as_bytes()),
                    Err(_) => {
                        let lossy = String::from_utf8_lossy(meta_bytes).into_owned();
                        record::parse_meta_from_bytes(lossy.as_bytes())
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
    let split = record::find_indicators_split(content_slice);
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
    let (exectime, rowcount, exec_id) = record::parse_indicators_from_bytes(ind_bytes);

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
