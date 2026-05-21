use dm_database_parser_sqllog::LogParserBuilder;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
#[cfg(not(miri))]
fn test_parser() {
    let mut file = NamedTempFile::new().unwrap();
    let log_content = "2025-11-17 16:09:41.123 (sess:123 thrd:456 user:SYSDBA trxid:789 stmt:0 appname:disql ip:::127.0.0.1) SELECT * FROM DUAL; EXECTIME: 1.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 100.\n";
    file.write_all(log_content.as_bytes()).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut iter = parser.iter();

    let record = iter.next().unwrap().unwrap();

    assert_eq!(record.ts, "2025-11-17 16:09:41.123");
    assert_eq!(record.sql, "SELECT * FROM DUAL; ");
    assert_eq!(record.exectime, 1.0);
    assert_eq!(record.rowcount, 1);
    assert_eq!(record.exec_id, 100);
}

#[test]
#[cfg(not(miri))]
fn test_parser_multiline() {
    let mut file = NamedTempFile::new().unwrap();
    let log_content = "2025-11-17 16:09:41.124 (sess:124 thrd:457 user:USER1 trxid:790 stmt:1 appname:manager ip:::192.168.1.1) SELECT *\nFROM USERS\nWHERE ID = 1;\nEXECTIME: 2.5(ms) ROWCOUNT: 5(rows) EXEC_ID: 101.\n";
    file.write_all(log_content.as_bytes()).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut iter = parser.iter();

    let record = iter.next().unwrap().unwrap();

    assert_eq!(record.ts, "2025-11-17 16:09:41.124");
    assert_eq!(record.sql, "SELECT *\nFROM USERS\nWHERE ID = 1;\n");
    assert_eq!(record.exectime, 2.5);
    assert_eq!(record.rowcount, 5);
}
