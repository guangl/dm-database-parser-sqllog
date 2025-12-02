use dm_database_parser_sqllog::LogParser;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_parser_lazy_loading() {
    let mut file = NamedTempFile::new().unwrap();
    let log_content = "2025-11-17 16:09:41.123 (sess:123 thrd:456 user:SYSDBA trxid:789 stmt:0 appname:disql ip:::127.0.0.1) SELECT * FROM DUAL; EXECTIME: 1.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 100.\n";
    file.write_all(log_content.as_bytes()).unwrap();
    let path = file.path();

    let parser = LogParser::from_path(path).unwrap();
    let mut iter = parser.iter();

    let record = iter.next().unwrap().unwrap();

    // Check timestamp (parsed immediately)
    assert_eq!(record.ts, "2025-11-17 16:09:41.123");

    // Check body (parsed lazily)
    assert_eq!(record.body(), "SELECT * FROM DUAL; ");

    // Check indicators (parsed lazily)
    let indicators = record.parse_indicators().unwrap();
    assert_eq!(indicators.execute_time, 1.0);
    assert_eq!(indicators.row_count, 1);
    assert_eq!(indicators.execute_id, 100);
}

#[test]
fn test_parser_multiline() {
    let mut file = NamedTempFile::new().unwrap();
    let log_content = "2025-11-17 16:09:41.124 (sess:124 thrd:457 user:USER1 trxid:790 stmt:1 appname:manager ip:::192.168.1.1) SELECT *\nFROM USERS\nWHERE ID = 1;\nEXECTIME: 2.5(ms) ROWCOUNT: 5(rows) EXEC_ID: 101.\n";
    file.write_all(log_content.as_bytes()).unwrap();
    let path = file.path();

    let parser = LogParser::from_path(path).unwrap();
    let mut iter = parser.iter();

    let record = iter.next().unwrap().unwrap();

    assert_eq!(record.ts, "2025-11-17 16:09:41.124");
    assert_eq!(record.body(), "SELECT *\nFROM USERS\nWHERE ID = 1;\n");

    let indicators = record.parse_indicators().unwrap();
    assert_eq!(indicators.execute_time, 2.5);
    assert_eq!(indicators.row_count, 5);
}
