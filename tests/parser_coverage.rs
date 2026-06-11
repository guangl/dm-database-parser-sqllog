use dm_database_parser_sqllog::LogParserBuilder;
use std::io::Write;
use tempfile::NamedTempFile;

/// File starting with a leading newline → record_slice is empty on first iteration → continue
#[test]
#[cfg(not(miri))]
fn iterator_skips_leading_blank_line() {
    let mut file = NamedTempFile::new().unwrap();
    let content = "\n2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:a) SELECT 1\n";
    file.write_all(content.as_bytes()).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let records: Vec<_> = parser.iter().unwrap().filter_map(|r| r.ok()).collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].ts, "2025-11-17 16:09:41.123");
}

/// CRLF in multiline record first line
#[test]
#[cfg(not(miri))]
fn crlf_in_multiline_first_line() {
    let mut file = NamedTempFile::new().unwrap();
    let content = concat!(
        "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:a) SELECT\r\n",
        "  col1\r\n",
        "  FROM t\r\n",
        "2025-11-17 16:09:42.000 (EP[0] sess:2 thrd:2 user:U trxid:4 stmt:5 appname:a) SELECT 2\n",
    );
    file.write_all(content.as_bytes()).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let records: Vec<_> = parser.iter().unwrap().filter_map(|r| r.ok()).collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].ts, "2025-11-17 16:09:41.123");
    assert_eq!(records[1].ts, "2025-11-17 16:09:42.000");
}
