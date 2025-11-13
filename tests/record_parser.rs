use dm_database_parser_sqllog::parser::RecordParser;

#[test]
fn test_parser_single_line() {
    let data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 1);
    assert!(records[0].is_ok());
}

#[test]
fn test_parser_multiple_records() {
    let data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n\
                2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 2);
    assert!(records[0].is_ok());
    assert!(records[1].is_ok());
}

#[test]
fn test_parser_multiline_record() {
    let data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\n\
                FROM users\n\
                WHERE id > 0";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 1);

    let record = records[0].as_ref().unwrap();
    assert_eq!(record.lines.len(), 3);
    assert!(record.full_content().contains("FROM users"));
}

#[test]
fn test_parser_skip_invalid_lines() {
    let data = "invalid line\n\
                another invalid\n\
                2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n\
                2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 2);
}

#[test]
fn test_parser_empty_input() {
    let data = "";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 0);
}

#[test]
fn test_parser_only_whitespace() {
    let data = "   \n\n  \n";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 0);
}

#[test]
fn test_parser_windows_line_endings() {
    let data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\r\n\
                2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2\r\n";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 2);
}

#[test]
fn test_parser_mixed_line_endings() {
    let data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\r\n\
                FROM table1\n\
                WHERE x > 0\r\n\
                2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) INSERT";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].as_ref().unwrap().lines.len(), 3);
}

#[test]
fn test_parser_continuation_between_records() {
    let data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\n\
                FROM users\n\
                2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) UPDATE logs\n\
                SET status = 'done'";
    let parser = RecordParser::new(data.as_bytes());

    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].as_ref().unwrap().lines.len(), 2);
    assert_eq!(records[1].as_ref().unwrap().lines.len(), 2);
}
