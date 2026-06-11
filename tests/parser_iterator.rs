use dm_database_parser_sqllog::LogParserBuilder;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
#[cfg(not(miri))]
fn iterator_handles_crlf_and_eof_without_newline() {
    let mut file = NamedTempFile::new().unwrap();
    let rec1 = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app) SELECT 1\r\n";
    let rec2_no_nl =
        "2025-11-17 16:09:42.123 (EP[0] sess:2 thrd:3 user:u trxid:3 stmt:4 appname:app) SELECT 2";
    write!(file, "{}{}", rec1, rec2_no_nl).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter().unwrap();

    let r1 = it.next().unwrap().unwrap();
    assert_eq!(r1.ts, "2025-11-17 16:09:41.123");
    assert_eq!(r1.sql, "SELECT 1");

    let r2 = it.next().unwrap().unwrap();
    assert_eq!(r2.ts, "2025-11-17 16:09:42.123");
    assert_eq!(r2.sql, "SELECT 2");

    assert!(it.next().is_none());
}

#[test]
#[cfg(not(miri))]
fn iterator_multiline_detection() {
    let mut file = NamedTempFile::new().unwrap();
    let content = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app) SELECT\n  *\n  FROM dual\nEXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    file.write_all(content.as_bytes()).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter().unwrap();
    let r = it.next().unwrap().unwrap();
    assert!(r.sql.contains("FROM dual"));
    assert_eq!(r.rowcount, 1);
}
