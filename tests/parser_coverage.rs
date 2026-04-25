use dm_database_parser_sqllog::{parse_record, LogParser};
use std::io::Write;
use tempfile::NamedTempFile;

/// parse_record with no embedded newline → hits the None branch in is_multiline=true path
#[test]
fn parse_record_single_line_no_newline() {
    let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:a) SELECT 1";
    let rec = parse_record(raw).unwrap();
    assert_eq!(rec.ts, "2025-11-17 16:09:41.123");
    assert!(rec.body().contains("SELECT"));
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

/// File starting with a leading newline → record_slice is empty on first iteration → line 172 continue
#[test]
#[cfg(not(miri))]
fn iterator_skips_leading_blank_line() {
    let mut file = NamedTempFile::new().unwrap();
    // Leading \n before the first record causes an empty record_slice → hits the `continue` path
    let content = "\n2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:a) SELECT 1\n";
    file.write_all(content.as_bytes()).unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let records: Vec<_> = parser.iter().filter_map(|r| r.ok()).collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].ts, "2025-11-17 16:09:41.123");
}

/// CRLF in multiline record first line → covers lines 223-224 in parse_record_with_hint
#[test]
#[cfg(not(miri))]
fn crlf_in_multiline_first_line() {
    let mut file = NamedTempFile::new().unwrap();
    // Two records: first is multiline with CRLF, second is normal
    let content = concat!(
        "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:a) SELECT\r\n",
        "  col1\r\n",
        "  FROM t\r\n",
        "2025-11-17 16:09:42.000 (EP[0] sess:2 thrd:2 user:U trxid:4 stmt:5 appname:a) SELECT 2\n",
    );
    file.write_all(content.as_bytes()).unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let records: Vec<_> = parser.iter().filter_map(|r| r.ok()).collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].ts, "2025-11-17 16:09:41.123");
    assert_eq!(records[1].ts, "2025-11-17 16:09:42.000");
}
