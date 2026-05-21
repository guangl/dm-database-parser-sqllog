use dm_database_parser_sqllog::{LogParserBuilder, parse_record};
use std::io::Write;
use tempfile::NamedTempFile;

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

#[test]
#[cfg(not(miri))]
fn probable_record_start_line_and_iterator_singleline_detection() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:1 user:u trxid:1 stmt:1 appname:a) A\n";
    let r2 = "2025-11-17 16:09:41.124 (EP[0] sess:2 thrd:2 user:u trxid:2 stmt:2 appname:b) B EXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 2.\n";
    write!(file, "{}{}", r1, r2).unwrap();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let v: Vec<_> = parser.iter().collect();
    assert_eq!(v.len(), 2);
    let s2 = v[1].as_ref().unwrap();
    assert_eq!(s2.ts, "2025-11-17 16:09:41.124");
}
