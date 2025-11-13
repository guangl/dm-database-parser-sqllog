use dm_database_parser_sqllog::parser::Record;

#[test]
fn test_record_new() {
    let record = Record::new("test line".to_string());
    assert_eq!(record.lines.len(), 1);
    assert_eq!(record.start_line(), "test line");
}

#[test]
fn test_record_add_line() {
    let mut record = Record::new("first line".to_string());
    record.add_line("second line".to_string());
    record.add_line("third line".to_string());

    assert_eq!(record.lines.len(), 3);
    assert_eq!(record.all_lines().len(), 3);
}

#[test]
fn test_record_full_content() {
    let mut record = Record::new("line 1".to_string());
    record.add_line("line 2".to_string());
    record.add_line("line 3".to_string());

    let full = record.full_content();
    assert_eq!(full, "line 1\nline 2\nline 3");
}

#[test]
fn test_record_has_continuation_lines() {
    let single = Record::new("only line".to_string());
    assert!(!single.has_continuation_lines());

    let mut multi = Record::new("first line".to_string());
    multi.add_line("second line".to_string());
    assert!(multi.has_continuation_lines());
}

#[test]
fn test_record_parse_to_sqllog_success() {
    let record = Record::new(
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string()
    );

    let result = record.parse_to_sqllog();
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.meta.username, "alice");
    assert!(sqllog.body.contains("SELECT"));
}

#[test]
fn test_record_parse_to_sqllog_with_continuation() {
    let mut record = Record::new(
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string()
    );
    record.add_line("FROM users".to_string());
    record.add_line("WHERE id > 0".to_string());

    let result = record.parse_to_sqllog();
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert!(sqllog.body.contains("FROM users"));
    assert!(sqllog.body.contains("WHERE id > 0"));
}

#[test]
fn test_record_parse_to_sqllog_invalid() {
    let record = Record::new("invalid log line".to_string());
    let result = record.parse_to_sqllog();
    assert!(result.is_err());
}

#[test]
fn test_record_clone() {
    let mut original = Record::new("first".to_string());
    original.add_line("second".to_string());

    let cloned = original.clone();
    assert_eq!(original.lines, cloned.lines);
}

#[test]
fn test_record_equality() {
    let record1 = Record::new("test".to_string());
    let record2 = Record::new("test".to_string());
    let record3 = Record::new("different".to_string());

    assert_eq!(record1, record2);
    assert_ne!(record1, record3);
}
