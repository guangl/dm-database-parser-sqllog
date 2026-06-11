pub mod builder;
pub(crate) mod encoding;
pub mod iterator;

pub use builder::LogParserBuilder;
pub use encoding::FileEncodingHint;
pub use iterator::LogIterator;

use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
use std::fs::File;
use std::path::PathBuf;
use std::str;
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::record::{self, Sqllog};
use ::encoding::all::GB18030;
use ::encoding::{DecoderTrap, Encoding};

/// Pre-built SIMD searcher for the `") "` meta-close pattern.
static FINDER_CLOSE_META: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b") "));

/// SQL 日志文件解析器。
///
/// 通过 [`LogParserBuilder`] 构建实例。每次调用 [`iter`](LogParser::iter) 时
/// 打开文件并以流式方式逐行读取，内存占用与文件大小无关。
pub struct LogParser {
    pub(super) path: PathBuf,
    pub(super) encoding: FileEncodingHint,
}

impl LogParser {
    /// 打开文件并返回流式迭代器。
    pub fn iter(&self) -> Result<LogIterator, ParseError> {
        let file = File::open(&self.path).map_err(|e| ParseError::IoError(e.to_string()))?;
        Ok(LogIterator::new(file, self.encoding))
    }
}

/// 从原始字节解析单条 SQL 日志记录。
///
/// 自动检测多行模式。适合已从文件中读出完整记录的调用方。
#[cfg(test)]
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
    let (ep, sess_id, thrd_id, username, trxid, statement, appname, client_ip) = match encoding_hint
    {
        FileEncodingHint::Utf8 => record::parse_meta_from_bytes(meta_bytes),
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
        FileEncodingHint::Gb18030 => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
            Ok(decoded) => record::parse_meta_from_bytes(decoded.as_bytes()),
            Err(_) => {
                let lossy = String::from_utf8_lossy(meta_bytes).into_owned();
                record::parse_meta_from_bytes(lossy.as_bytes())
            }
        },
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
                    FileEncodingHint::Utf8 => str::from_utf8(inner).ok().map(|t| t.to_string()),
                    FileEncodingHint::Auto => str::from_utf8(inner)
                        .ok()
                        .map(|t| t.to_string())
                        .or_else(|| GB18030.decode(inner, DecoderTrap::Strict).ok()),
                    FileEncodingHint::Gb18030 => GB18030
                        .decode(inner, DecoderTrap::Strict)
                        .ok()
                        .or_else(|| str::from_utf8(inner).ok().map(|s| s.to_string())),
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
        FileEncodingHint::Utf8 => String::from_utf8_lossy(body_bytes).into_owned(),
        FileEncodingHint::Auto => match str::from_utf8(body_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => match GB18030.decode(body_bytes, DecoderTrap::Strict) {
                Ok(s) => s,
                Err(_) => String::from_utf8_lossy(body_bytes).into_owned(),
            },
        },
        FileEncodingHint::Gb18030 => match GB18030.decode(body_bytes, DecoderTrap::Strict) {
            Ok(s) => s,
            Err(_) => String::from_utf8_lossy(body_bytes).into_owned(),
        },
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
        let record = parser.iter().unwrap().next().unwrap().unwrap();
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

    // ── 从 tests/performance_metrics.rs 迁入 ──────────────────────────────

    fn build_perf_record(tag_and_body: &str, tail: &str) -> Vec<u8> {
        let header =
            b"2025-11-17 16:09:41.123 (EP[1] sess:123 thrd:456 user:alice trxid:789 stmt:0x1 appname:bench) ";
        let mut v = Vec::new();
        v.extend_from_slice(header);
        v.extend_from_slice(tag_and_body.as_bytes());
        if !tail.is_empty() {
            v.extend_from_slice(tail.as_bytes());
        }
        v
    }

    #[test]
    fn performance_metrics_full() {
        let raw = build_perf_record(
            "SELECT * FROM T ",
            "EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 999.",
        );
        let rec = parse_record(&raw).unwrap();
        assert!((rec.exectime - 10.5).abs() < 1e-6);
        assert_eq!(rec.rowcount, 100);
        assert_eq!(rec.exec_id, 999);
        assert_eq!(rec.sql, "SELECT * FROM T ");
    }

    #[test]
    fn performance_metrics_no_indicators() {
        let raw = build_perf_record("SELECT 1;", "");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.exectime, 0.0);
        assert_eq!(rec.rowcount, 0);
        assert_eq!(rec.exec_id, 0);
        assert_eq!(rec.sql, "SELECT 1;");
    }

    #[test]
    fn performance_metrics_ora_tag_strips_colon_space_prefix() {
        let raw = build_perf_record(
            "[ORA] : SELECT 1 FROM DUAL ",
            "EXECTIME: 5.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 42.",
        );
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.tag.as_deref(), Some("ORA"));
        assert_eq!(rec.sql, "SELECT 1 FROM DUAL ");
        assert!((rec.exectime - 5.0).abs() < 1e-6);
        assert_eq!(rec.rowcount, 1);
        assert_eq!(rec.exec_id, 42);
    }

    #[test]
    fn performance_metrics_ora_tag_no_prefix_unchanged() {
        let raw = build_perf_record(
            "[ORA] SELECT 1 FROM DUAL ",
            "EXECTIME: 5.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 42.",
        );
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.tag.as_deref(), Some("ORA"));
        assert_eq!(rec.sql, "SELECT 1 FROM DUAL ");
    }

    #[test]
    fn performance_metrics_non_ora_tag_keeps_prefix_intact() {
        let raw = build_perf_record("[SEL] : SELECT 1 ", "EXEC_ID: 7.");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.tag.as_deref(), Some("SEL"));
        assert_eq!(rec.sql, ": SELECT 1 ");
    }

    #[test]
    fn performance_metrics_no_tag_keeps_prefix_intact() {
        let raw = build_perf_record(": SELECT 1 ", "EXEC_ID: 7.");
        let rec = parse_record(&raw).unwrap();
        assert!(rec.tag.is_none());
        assert_eq!(rec.sql, ": SELECT 1 ");
    }

    #[test]
    fn performance_metrics_exectime_only() {
        let raw = build_perf_record("DELETE FROM T; ", "EXECTIME: 3.5(ms)");
        let rec = parse_record(&raw).unwrap();
        assert!((rec.exectime - 3.5).abs() < 1e-6);
        assert_eq!(rec.rowcount, 0);
        assert_eq!(rec.exec_id, 0);
        assert_eq!(rec.sql, "DELETE FROM T; ");
    }

    #[test]
    fn performance_metrics_rowcount_only() {
        let raw = build_perf_record("UPDATE T SET A=1; ", "ROWCOUNT: 10(rows)");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.exectime, 0.0);
        assert_eq!(rec.rowcount, 10);
        assert_eq!(rec.exec_id, 0);
    }

    #[test]
    fn performance_metrics_exec_id_only() {
        let raw = build_perf_record("SELECT 1; ", "EXEC_ID: 42.");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.exectime, 0.0);
        assert_eq!(rec.rowcount, 0);
        assert_eq!(rec.exec_id, 42);
    }

    #[test]
    fn performance_metrics_ora_tag_only_colon_space_sql_empty_after_strip() {
        let raw = build_perf_record("[ORA] : ", "EXEC_ID: 1.");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.tag.as_deref(), Some("ORA"));
        assert_eq!(rec.sql, "");
    }

    #[test]
    fn early_exit_no_dot_suffix() {
        let raw = build_perf_record("SELECT * FROM users WHERE id = 1;", "");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.exectime, 0.0);
        assert_eq!(rec.rowcount, 0);
        assert_eq!(rec.exec_id, 0);
    }

    #[test]
    fn dot_suffix_no_real_indicators_guarded() {
        let raw = build_perf_record("SELECT url FROM t WHERE url = 'http://example.com'.", "");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.exec_id, 0);
        assert_eq!(rec.exectime, 0.0);
    }

    #[test]
    fn dot_suffix_with_real_indicators() {
        let raw = build_perf_record(
            "SELECT 1 FROM T ",
            "EXECTIME: 2.5(ms) ROWCOUNT: 5(rows) EXEC_ID: 77.",
        );
        let rec = parse_record(&raw).unwrap();
        assert!((rec.exectime - 2.5).abs() < 1e-6);
        assert_eq!(rec.rowcount, 5);
        assert_eq!(rec.exec_id, 77);
        assert_eq!(rec.sql, "SELECT 1 FROM T ");
    }

    #[test]
    fn fake_keyword_in_body_plus_real_indicators() {
        let raw = build_perf_record(
            "SELECT 'EXECTIME: fake' FROM T ",
            "EXECTIME: 1.0(ms) ROWCOUNT: 3(rows) EXEC_ID: 55.",
        );
        let rec = parse_record(&raw).unwrap();
        assert!((rec.exectime - 1.0).abs() < 1e-6);
        assert_eq!(rec.rowcount, 3);
        assert_eq!(rec.exec_id, 55);
        assert!(rec.sql.contains("EXECTIME: fake"));
    }

    #[test]
    fn multiple_colons_in_body() {
        let raw = build_perf_record(
            "SELECT 'http://example.com:8080/path' FROM T ",
            "EXECTIME: 3.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 99.",
        );
        let rec = parse_record(&raw).unwrap();
        assert!((rec.exectime - 3.0).abs() < 1e-6);
        assert_eq!(rec.rowcount, 1);
        assert_eq!(rec.exec_id, 99);
        assert!(rec.sql.contains("http://example.com:8080/path"));
    }

    #[test]
    fn exec_id_only_split_correct() {
        let raw = build_perf_record("INSERT INTO T VALUES (1); ", "EXEC_ID: 123.");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.exec_id, 123);
        assert_eq!(rec.exectime, 0.0);
        assert_eq!(rec.rowcount, 0);
        assert_eq!(rec.sql, "INSERT INTO T VALUES (1); ");
    }

    // ── 从 tests/sqllog_additional.rs 迁入 ───────────────────────────────

    fn build_additional_record(line1_body: &str, tail: &str) -> Vec<u8> {
        let header = b"2025-11-17 16:09:41.123 (EP[1] sess:123 thrd:456 user:alice trxid:789 stmt:0x1 appname:bench) ";
        let mut v = Vec::new();
        v.extend_from_slice(header);
        v.extend_from_slice(line1_body.as_bytes());
        if !tail.is_empty() {
            v.extend_from_slice(tail.as_bytes());
        }
        v
    }

    #[test]
    fn body_without_indicators() {
        let raw = build_additional_record("SELECT 1;", "");
        let rec = parse_record(&raw).expect("parse ok");
        assert_eq!(rec.sql, "SELECT 1;");
        assert_eq!(rec.exec_id, 0);
        assert_eq!(rec.exectime, 0.0);
        assert_eq!(rec.rowcount, 0);
    }

    #[test]
    fn indicators_exec_id_only() {
        let raw = build_additional_record("SELECT 1; ", "EXEC_ID: 42.");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.sql, "SELECT 1; ");
        assert_eq!(rec.exec_id, 42);
    }

    #[test]
    fn indicators_rowcount_only() {
        let raw = build_additional_record("UPDATE T SET A=1; ", "ROWCOUNT: 10(rows)");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.sql, "UPDATE T SET A=1; ");
        assert_eq!(rec.rowcount, 10);
    }

    #[test]
    fn indicators_exectime_only() {
        let raw = build_additional_record("DELETE FROM T; ", "EXECTIME: 3.5(ms)");
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.sql, "DELETE FROM T; ");
        assert!((rec.exectime - 3.5).abs() < 1e-6);
    }

    #[test]
    fn indicators_permutation_all() {
        let tail = "ROWCOUNT: 5(rows) EXECTIME: 12.25(ms) EXEC_ID: 999.";
        let raw = build_additional_record("SELECT * FROM T ", tail);
        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.sql, "SELECT * FROM T ");
        assert_eq!(rec.rowcount, 5);
        assert!((rec.exectime - 12.25).abs() < 1e-6);
        assert_eq!(rec.exec_id, 999);
    }

    #[test]
    fn meta_parsing_basic() {
        let raw = b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:SYSDBA trxid:0 stmt:0x2 appname:cli) SELECT";
        let rec = parse_record(raw).unwrap();
        assert_eq!(rec.ep, 2);
        assert_eq!(rec.sess_id, "0xABC");
        assert_eq!(rec.thrd_id, "777");
        assert_eq!(rec.username, "SYSDBA");
        assert_eq!(rec.trxid, "0");
        assert_eq!(rec.statement, "0x2");
        assert_eq!(rec.appname, "cli");
    }

    #[test]
    fn meta_parsing_empty_appname() {
        let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:) X";
        let rec = parse_record(raw).unwrap();
        assert_eq!(rec.appname, "");
    }

    #[test]
    fn appname_empty_followed_by_ip_colon_single_should_keep_appname_empty() {
        let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname: ip:10.1.1.1) X";
        let rec = parse_record(raw).unwrap();
        assert_eq!(rec.appname, "");
        assert_eq!(rec.client_ip, "10.1.1.1");
    }

    #[test]
    fn appname_empty_followed_by_ip_triple_colon_should_keep_appname_empty() {
        let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname: ip:::ffff:10.3.100.68) X";
        let rec = parse_record(raw).unwrap();
        assert_eq!(rec.appname, "");
        assert_eq!(rec.client_ip, "::ffff:10.3.100.68");
    }

    #[test]
    fn meta_parsing_gb18030_username() {
        use ::encoding::all::GB18030;
        use ::encoding::{EncoderTrap, Encoding};

        let username = "用户";
        let user_bytes = GB18030
            .encode(username, EncoderTrap::Strict)
            .expect("encode");

        let mut raw: Vec<u8> = b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:".to_vec();
        raw.extend_from_slice(&user_bytes);
        raw.extend_from_slice(b" trxid:0 stmt:0x2 appname:cli) SELECT");

        let rec = parse_record(&raw).unwrap();
        assert_eq!(rec.username, username);
    }

    #[test]
    fn tag_extraction_and_body_trim() {
        let raw = b"2025-11-17 16:09:41.123 (EP[1] sess:123 thrd:456 user:u trxid:3 stmt:4 appname:bench) [SEL] SELECT 1; EXEC_ID: 42.";
        let rec = parse_record(raw).unwrap();
        assert_eq!(rec.tag.as_deref(), Some("SEL"));
        assert_eq!(rec.sql, "SELECT 1; ");
    }

    #[test]
    #[cfg(not(miri))]
    fn file_encoding_detection_gb18030() {
        use ::encoding::all::GB18030;
        use ::encoding::{EncoderTrap, Encoding};
        use std::io::Write;
        use tempfile::NamedTempFile;

        let username = "用户";
        let user_bytes = GB18030
            .encode(username, EncoderTrap::Strict)
            .expect("encode");

        let mut line: Vec<u8> =
            b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:".to_vec();
        line.extend_from_slice(&user_bytes);
        line.extend_from_slice(b" trxid:0 stmt:0x2 appname:cli) SELECT\n");

        let mut tmp = NamedTempFile::new().expect("tmp");
        tmp.write_all(&line).expect("write");
        tmp.as_file().sync_all().expect("sync");

        let parser = LogParserBuilder::new(tmp.path()).build().expect("open");
        let rec = parser.iter().unwrap().next().unwrap().unwrap();
        assert_eq!(rec.username, username);
    }

    #[test]
    fn find_indicators_split_exectime_keyword_in_sql_body_no_indicators() {
        let raw = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:0 stmt:0 appname:a) SELECT * FROM t WHERE col = 'EXECTIME: slow'\n";
        let record = parse_record(raw.as_bytes()).unwrap();
        assert_eq!(record.sql, "SELECT * FROM t WHERE col = 'EXECTIME: slow'\n");
        assert_eq!(record.exec_id, 0);
        assert_eq!(record.exectime, 0.0);
    }

    #[test]
    fn find_indicators_split_rowcount_keyword_in_sql_body_no_indicators() {
        let raw = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:0 stmt:0 appname:a) SELECT * FROM t WHERE cnt = 'ROWCOUNT: many'\n";
        let record = parse_record(raw.as_bytes()).unwrap();
        assert_eq!(record.sql, "SELECT * FROM t WHERE cnt = 'ROWCOUNT: many'\n");
        assert_eq!(record.rowcount, 0);
    }

    #[test]
    fn find_indicators_split_exec_id_keyword_in_sql_body_no_indicators() {
        let raw = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:0 stmt:0 appname:a) SELECT EXEC_ID: foo FROM dual\n";
        let record = parse_record(raw.as_bytes()).unwrap();
        assert_eq!(record.sql, "SELECT EXEC_ID: foo FROM dual\n");
        assert_eq!(record.exec_id, 0);
    }

    #[test]
    fn find_indicators_split_keyword_in_body_plus_real_indicators() {
        let raw = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:0 stmt:0 appname:a) SELECT EXECTIME: slow\nEXECTIME: 5.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 99.\n";
        let record = parse_record(raw.as_bytes()).unwrap();
        assert!((record.exectime - 5.0).abs() < 1e-6);
        assert!(record.sql.contains("SELECT"));
    }

    #[test]
    fn find_indicators_split_multiple_keywords_in_body_no_indicators() {
        let raw = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:0 stmt:0 appname:a) EXECTIME: x ROWCOUNT: y EXEC_ID: z\n";
        let record = parse_record(raw.as_bytes()).unwrap();
        assert_eq!(record.sql, "EXECTIME: x ROWCOUNT: y EXEC_ID: z\n");
        assert_eq!(record.exec_id, 0);
        assert_eq!(record.exectime, 0.0);
    }

    #[test]
    #[cfg(not(miri))]
    fn encoding_detection_gb18030_after_64kb_boundary() {
        use ::encoding::all::GB18030;
        use ::encoding::{EncoderTrap, Encoding};
        use std::io::Write;
        use tempfile::NamedTempFile;

        let ascii_record = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:ascii trxid:0 stmt:0 appname:app) SELECT 1;\n";
        let repeat_count = 65536 / ascii_record.len() + 2;

        let username = "用户";
        let user_bytes = GB18030.encode(username, EncoderTrap::Strict).unwrap();
        let mut gb_line: Vec<u8> = b"2025-11-17 16:09:42.000 (EP[0] sess:2 thrd:2 user:".to_vec();
        gb_line.extend_from_slice(&user_bytes);
        gb_line.extend_from_slice(b" trxid:0 stmt:0 appname:app) SELECT 2;\n");

        let mut tmp = NamedTempFile::new().unwrap();
        for _ in 0..repeat_count {
            tmp.write_all(ascii_record.as_bytes()).unwrap();
        }
        tmp.write_all(&gb_line).unwrap();
        tmp.as_file().sync_all().unwrap();

        let parser = LogParserBuilder::new(tmp.path()).build().unwrap();
        let records: Vec<_> = parser.iter().unwrap().collect();
        let last = records.last().unwrap().as_ref().unwrap();
        assert_eq!(last.username, username);
    }

    #[test]
    #[cfg(not(miri))]
    fn file_encoding_detection_utf8() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let username = "用户";
        let user_bytes = username.as_bytes();

        let mut line: Vec<u8> =
            b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:".to_vec();
        line.extend_from_slice(user_bytes);
        line.extend_from_slice(b" trxid:0 stmt:0x2 appname:cli) SELECT\n");

        let mut tmp = NamedTempFile::new().expect("tmp");
        tmp.write_all(&line).expect("write");
        tmp.as_file().sync_all().expect("sync");

        let parser = LogParserBuilder::new(tmp.path()).build().expect("open");
        let rec = parser.iter().unwrap().next().unwrap().unwrap();
        assert_eq!(rec.username, username);
    }

    // ── 从 tests/parser_coverage.rs 迁入 ─────────────────────────────────

    /// parse_record with no embedded newline → hits the None branch in is_multiline=true path
    #[test]
    fn parse_record_single_line_no_newline() {
        let raw =
            b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:a) SELECT 1";
        let rec = parse_record(raw).unwrap();
        assert_eq!(rec.ts, "2025-11-17 16:09:41.123");
        assert!(rec.sql.contains("SELECT"));
    }

    /// Record >= 23 bytes with valid timestamp but no `(` → InvalidFormat at meta_start
    #[test]
    fn parse_record_no_meta_open_paren() {
        let raw = b"2025-11-17 16:09:41.123 NO_OPEN_PAREN_AT_ALL_HERE body";
        let result = parse_record(raw);
        assert!(result.is_err());
    }

    /// Record with `(` but no closing `)` → InvalidFormat at meta_end
    #[test]
    fn parse_record_no_meta_close_paren() {
        let raw = b"2025-11-17 16:09:41.123 (UNCLOSED_META body";
        let result = parse_record(raw);
        assert!(result.is_err());
    }

    // ── 从 tests/edge_cases.rs 迁入 ──────────────────────────────────────

    #[test]
    fn meta_closing_paren_without_space_then_body_on_next_line() {
        let content = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app)\nSELECT * FROM T\nEXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 7.\n";
        let rec = parse_record(content).expect("parse ok");
        assert!(rec.sql.trim_start().starts_with("SELECT * FROM T"));
        assert_eq!(rec.exec_id, 7);
    }

    #[test]
    fn appname_empty_then_take_next_token_as_appname_not_ip() {
        let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname: [SEL] ip:::ffff:10.0.0.1) X";
        let rec = parse_record(raw).unwrap();
        assert_eq!(rec.appname, "[SEL]");
        assert_eq!(rec.client_ip, "::ffff:10.0.0.1");
    }

    #[test]
    fn indicators_not_strictly_formatted_should_not_split_body() {
        let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app) SELECT 1; EXEC_ID:123";
        let rec = parse_record(raw).unwrap();
        // EXEC_ID:123 无点号结尾，不会被识别为指标，整段作为 SQL body
        assert_eq!(rec.exec_id, 0);
        assert!(rec.sql.ends_with("EXEC_ID:123"));
    }

    // ── 从 tests/parser_errors.rs 迁入 ───────────────────────────────────

    #[test]
    fn test_parse_record_timestamp_validation() {
        use crate::error::ParseError;

        let valid = b"2025-11-17 16:09:41.123 (EP[0]) SELECT";
        let result = parse_record(valid);
        assert!(result.is_ok());

        let bad_ts_no_meta = b"2025-11-17 16:09:41.123 INVALID NO META";
        let result = parse_record(bad_ts_no_meta);
        assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));

        let short = b"2025-11-17 16:0";
        let result = parse_record(short);
        assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));
    }
}
