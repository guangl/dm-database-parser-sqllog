use dm_database_parser_sqllog::LogParser;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn iterator_handles_crlf_and_eof_without_newline() {
    let mut file = NamedTempFile::new().unwrap();
    let rec1 = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app) SELECT 1\r\n";
    let rec2_no_nl =
        "2025-11-17 16:09:42.123 (EP[0] sess:2 thrd:3 user:u trxid:3 stmt:4 appname:app) SELECT 2"; // 无结尾换行
    write!(file, "{}{}", rec1, rec2_no_nl).unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let mut it = parser.iter();

    let r1 = it.next().unwrap().unwrap();
    assert_eq!(r1.ts, "2025-11-17 16:09:41.123");
    assert_eq!(r1.body(), "SELECT 1");

    let r2 = it.next().unwrap().unwrap();
    assert_eq!(r2.ts, "2025-11-17 16:09:42.123");
    assert_eq!(r2.body(), "SELECT 2");

    assert!(it.next().is_none());
}

#[test]
fn iterator_multiline_detection() {
    let mut file = NamedTempFile::new().unwrap();
    let content = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app) SELECT\n  *\n  FROM dual\nEXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    file.write_all(content.as_bytes()).unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let mut it = parser.iter();
    let r = it.next().unwrap().unwrap();
    assert!(r.body().contains("FROM dual"));
    let ind = r.parse_indicators().unwrap();
    assert_eq!(ind.row_count, 1);
}
