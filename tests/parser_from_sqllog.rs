use dm_database_parser_sqllog::{FromSqllog, LogParserBuilder, Sqllog, parse_record};
use std::io::Write;
use tempfile::NamedTempFile;

struct TestRecord {
    timestamp: String,
    sql: String,
}

impl FromSqllog for TestRecord {
    fn from_sqllog(s: Sqllog<'_>) -> Self {
        TestRecord {
            timestamp: s.ts.to_string(),
            sql: s.body().to_string(),
        }
    }
}

fn build_record(tag_and_body: &str, tail: &str) -> Vec<u8> {
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

#[cfg(not(miri))]
#[test]
fn test_from_sqllog_maps_record() {
    let content =
        "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1\n";
    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(content.as_bytes()).unwrap();
    let parser = LogParserBuilder::new(tmp.path()).build().unwrap();
    let records: Vec<TestRecord> = parser
        .iter()
        .filter_map(|r| r.ok())
        .map(TestRecord::from_sqllog)
        .collect();
    assert_eq!(records.len(), 1);
    assert!(records[0].timestamp.contains("2025"));
    assert!(records[0].sql.contains("SELECT 1"));
}

#[cfg(not(miri))]
#[test]
fn test_exec_time_returns_value() {
    let raw = build_record(
        "SELECT * FROM T ",
        "EXECTIME: 200(ms) ROWCOUNT: 100(rows) EXEC_ID: 999.",
    );
    let rec = parse_record(&raw).unwrap();
    let result = rec.exec_time();
    assert_eq!(result, Ok(Some(200)));
}

#[cfg(not(miri))]
#[test]
fn test_exec_time_returns_none_when_missing() {
    let raw = build_record("SELECT 1;", "");
    let rec = parse_record(&raw).unwrap();
    let result = rec.exec_time();
    assert_eq!(result, Ok(None));
}

#[cfg(not(miri))]
#[test]
fn test_row_count_returns_value() {
    let raw = build_record(
        "SELECT * FROM T ",
        "EXECTIME: 10(ms) ROWCOUNT: 42(rows) EXEC_ID: 999.",
    );
    let rec = parse_record(&raw).unwrap();
    let result = rec.row_count();
    assert_eq!(result, Ok(Some(42)));
}

#[test]
fn test_from_sqllog_trait_object_safety() {
    // Compile-time verification: FromSqllog can be used as a generic trait bound.
    fn _use_trait_bound<T: FromSqllog>(_t: T) {}
    // This function exists purely for compile-time verification.
}
