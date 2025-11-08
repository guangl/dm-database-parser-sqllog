use super::constants::BODY_OFFSET;
use super::parse_functions::{
    build_body, extract_field_value, extract_indicator, extract_sql_body, parse_ep_field,
    parse_indicators, parse_meta,
};
use super::*;
use crate::ParseError;

#[test]
fn test_single_line_record() {
    let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let records = parse_records_from_string(input);

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].lines.len(), 1);
    assert_eq!(records[0].start_line(), input);
    assert!(!records[0].has_continuation_lines());
}

#[test]
fn test_multi_line_record() {
    let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
WHERE id = 1"#;

    let records = parse_records_from_string(input);

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].lines.len(), 3);
    assert!(records[0].has_continuation_lines());
    assert!(records[0].start_line().starts_with("2025-08-12"));
    assert_eq!(records[0].all_lines()[1], "FROM users");
    assert_eq!(records[0].all_lines()[2], "WHERE id = 1");
}

#[test]
fn test_multiple_records() {
    let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) INSERT INTO table"#;

    let records = parse_records_from_string(input);

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].lines.len(), 1);
    assert_eq!(records[1].lines.len(), 1);
}

#[test]
fn test_multiple_records_with_continuation() {
    let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM table1
WHERE id = 1
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE table2
SET name = 'test'
WHERE id = 2"#;

    let records = parse_records_from_string(input);

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].lines.len(), 3);
    assert_eq!(records[1].lines.len(), 3);
    assert!(records[0].has_continuation_lines());
    assert!(records[1].has_continuation_lines());
}

#[test]
fn test_skip_invalid_lines_at_start() {
    let input = r#"Some garbage line
Another invalid line
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"#;

    let records = parse_records_from_string(input);

    assert_eq!(records.len(), 1);
    assert!(records[0].start_line().starts_with("2025-08-12"));
}

#[test]
fn test_empty_input() {
    let input = "";
    let records = parse_records_from_string(input);
    assert_eq!(records.len(), 0);
}

#[test]
fn test_full_content() {
    let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
WHERE id = 1"#;

    let records = parse_records_from_string(input);
    assert_eq!(records[0].full_content(), input);
}

#[test]
fn test_parse_record_single_line() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:0x999 appname:app ip:::ffff:10.0.0.1) SELECT 1",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.ts, "2025-08-12 10:57:09.548");
    assert_eq!(sqllog.meta.ep, 0);
    assert_eq!(sqllog.meta.sess_id, "0x123");
    assert_eq!(sqllog.meta.thrd_id, "456");
    assert_eq!(sqllog.meta.username, "alice");
    assert_eq!(sqllog.meta.trxid, "789");
    assert_eq!(sqllog.meta.statement, "0x999");
    assert_eq!(sqllog.meta.appname, "app");
    assert_eq!(sqllog.meta.client_ip, "10.0.0.1");
    assert_eq!(sqllog.body, "SELECT 1");
    assert!(sqllog.indicators.is_none());
}

#[test]
fn test_parse_record_with_indicators() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.body, "SELECT 1");

    assert!(sqllog.indicators.is_some());
    let indicators = sqllog.indicators.unwrap();
    assert_eq!(indicators.execute_time, 10.0);
    assert_eq!(indicators.row_count, 5);
    assert_eq!(indicators.execute_id, 12345);
}

#[test]
fn test_parse_record_multiline() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *",
        "FROM users",
        "WHERE id = 1",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.body, "SELECT *\nFROM users\nWHERE id = 1");
}

#[test]
fn test_parse_record_empty_input() {
    let lines: Vec<&str> = vec![];
    let result = parse_record(&lines);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ParseError::EmptyInput));
}

#[test]
fn test_parse_record_invalid_format() {
    let lines = vec!["not a valid log line"];
    let result = parse_record(&lines);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::InvalidRecordStartLine { .. }
    ));
}

#[test]
fn test_parse_record_without_ip() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.meta.client_ip, "");
}

#[test]
fn test_record_parse_to_sqllog() {
    let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let records = parse_records_from_string(input);

    assert_eq!(records.len(), 1);
    let sqllog = records[0].parse_to_sqllog().unwrap();
    assert_eq!(sqllog.meta.username, "alice");
    assert_eq!(sqllog.body, "SELECT 1");
}

#[test]
fn test_sqllog_parser() {
    let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) INSERT INTO table"#;

    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = SqllogParser::new(cursor);
    let sqllogs: Vec<_> = parser.collect();

    assert_eq!(sqllogs.len(), 2);
    assert!(sqllogs[0].is_ok());
    assert!(sqllogs[1].is_ok());

    let sqllog1 = sqllogs[0].as_ref().unwrap();
    let sqllog2 = sqllogs[1].as_ref().unwrap();

    assert_eq!(sqllog1.meta.username, "alice");
    assert_eq!(sqllog2.meta.username, "bob");
}

#[test]
fn test_parse_sqllogs_from_string() {
    let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE table"#;

    let results = parse_sqllogs_from_string(input);
    assert_eq!(results.len(), 2);

    let sqllog1 = results[0].as_ref().unwrap();
    assert_eq!(sqllog1.body, "SELECT *\nFROM users");
    assert_eq!(sqllog1.meta.username, "alice");
}

// ==================== 辅助函数测试 ====================

#[test]
fn test_build_body_single_line() {
    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let closing_paren = first_line.find(')').unwrap();
    let body_start = closing_paren + BODY_OFFSET;
    let continuation: &[&str] = &[];

    let body = build_body(first_line, body_start, continuation);
    assert_eq!(body, "SELECT 1");
}

#[test]
fn test_build_body_multi_line() {
    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *";
    let closing_paren = first_line.find(')').unwrap();
    let body_start = closing_paren + BODY_OFFSET;
    let continuation = &["FROM users", "WHERE id = 1"];

    let body = build_body(first_line, body_start, continuation);
    assert_eq!(body, "SELECT *\nFROM users\nWHERE id = 1");
}

#[test]
fn test_build_body_empty_first_line() {
    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app)";
    let body_start = first_line.len();
    let continuation = &["SELECT 1"];

    let body = build_body(first_line, body_start, continuation);
    assert_eq!(body, "SELECT 1");
}

#[test]
fn test_extract_sql_body_with_exectime() {
    let full_body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, "SELECT 1");
}

#[test]
fn test_extract_sql_body_with_rowcount_first() {
    let full_body = "SELECT 1 ROWCOUNT: 5(rows) EXECTIME: 10(ms) EXEC_ID: 12345.";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, "SELECT 1");
}

#[test]
fn test_extract_sql_body_without_indicators() {
    let full_body = "SELECT 1 FROM users";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, "SELECT 1 FROM users");
}

#[test]
fn test_parse_ep_field_valid() {
    let raw = "EP[0]";
    let result = parse_ep_field(raw, raw);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    let raw = "EP[15]";
    let result = parse_ep_field(raw, raw);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 15);
}

#[test]
fn test_parse_ep_field_invalid_format() {
    let raw = "EP0";
    let result = parse_ep_field(raw, raw);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::InvalidEpFormat { .. }
    ));

    let raw = "[0]";
    let result = parse_ep_field(raw, raw);
    assert!(result.is_err());

    let raw = "EP[";
    let result = parse_ep_field(raw, raw);
    assert!(result.is_err());
}

#[test]
fn test_parse_ep_field_invalid_number() {
    let raw = "EP[abc]";
    let result = parse_ep_field(raw, raw);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::EpParseError { .. }
    ));

    let raw = "EP[256]"; // 超过 u8 范围
    let result = parse_ep_field(raw, raw);
    assert!(result.is_err());
}

#[test]
fn test_extract_field_value_valid() {
    let raw = "sess:123";
    let result = extract_field_value(raw, "sess:", raw);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "123");

    let raw = "user:alice";
    let result = extract_field_value(raw, "user:", raw);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "alice");
}

#[test]
fn test_extract_field_value_invalid_prefix() {
    let raw = "sess:123";
    let result = extract_field_value(raw, "user:", raw);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::InvalidFieldFormat { .. }
    ));
}

#[test]
fn test_extract_field_value_empty_value() {
    let raw = "sess:";
    let result = extract_field_value(raw, "sess:", raw);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_parse_meta_valid() {
    let meta_str = "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app";
    let result = parse_meta(meta_str);
    assert!(result.is_ok());

    let meta = result.unwrap();
    assert_eq!(meta.ep, 0);
    assert_eq!(meta.sess_id, "123");
    assert_eq!(meta.thrd_id, "456");
    assert_eq!(meta.username, "alice");
    assert_eq!(meta.trxid, "789");
    assert_eq!(meta.statement, "999");
    assert_eq!(meta.appname, "app");
    assert_eq!(meta.client_ip, "");
}

#[test]
fn test_parse_meta_with_ip() {
    let meta_str =
        "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:10.0.0.1";
    let result = parse_meta(meta_str);
    assert!(result.is_ok());

    let meta = result.unwrap();
    assert_eq!(meta.client_ip, "10.0.0.1");
}

#[test]
fn test_parse_meta_insufficient_fields() {
    let meta_str = "EP[0] sess:123 thrd:456";
    let result = parse_meta(meta_str);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::InsufficientMetaFields { .. }
    ));
}

#[test]
fn test_parse_meta_invalid_ep() {
    let meta_str = "EP0 sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app";
    let result = parse_meta(meta_str);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::InvalidEpFormat { .. }
    ));
}

#[test]
fn test_parse_indicators_valid() {
    let body = "SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";
    let result = parse_indicators(body);
    assert!(result.is_ok());

    let indicators = result.unwrap();
    assert_eq!(indicators.execute_time, 10.5);
    assert_eq!(indicators.row_count, 100);
    assert_eq!(indicators.execute_id, 12345);
}

#[test]
fn test_parse_indicators_integer_exectime() {
    let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.";
    let result = parse_indicators(body);
    assert!(result.is_ok());

    let indicators = result.unwrap();
    assert_eq!(indicators.execute_time, 10.0);
}

#[test]
fn test_parse_indicators_missing_exectime() {
    let body = "SELECT 1 ROWCOUNT: 5(rows) EXEC_ID: 12345.";
    let result = parse_indicators(body);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::IndicatorsParseError { .. }
    ));
}

#[test]
fn test_parse_indicators_missing_rowcount() {
    let body = "SELECT 1 EXECTIME: 10(ms) EXEC_ID: 12345.";
    let result = parse_indicators(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_indicators_missing_exec_id() {
    let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows)";
    let result = parse_indicators(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_indicators_invalid_exectime_format() {
    let body = "SELECT 1 EXECTIME: abc(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.";
    let result = parse_indicators(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_indicators_invalid_rowcount_format() {
    let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: xyz(rows) EXEC_ID: 12345.";
    let result = parse_indicators(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_indicators_invalid_exec_id_format() {
    let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: abc.";
    let result = parse_indicators(body);
    assert!(result.is_err());
}

#[test]
fn test_extract_indicator_valid() {
    let text = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows)";
    let result = extract_indicator(text, "EXECTIME: ", "(ms)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "10");

    let result = extract_indicator(text, "ROWCOUNT: ", "(rows)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "5");
}

#[test]
fn test_extract_indicator_missing_prefix() {
    let text = "SELECT 1 ROWCOUNT: 5(rows)";
    let result = extract_indicator(text, "EXECTIME: ", "(ms)");
    assert!(result.is_err());
}

#[test]
fn test_extract_indicator_missing_suffix() {
    let text = "SELECT 1 EXECTIME: 10 ROWCOUNT: 5(rows)";
    let result = extract_indicator(text, "EXECTIME: ", "(ms)");
    assert!(result.is_err());
}

// ==================== RecordParser 边界测试 ====================

#[test]
fn test_record_parser_empty_input() {
    let input = "";
    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = RecordParser::new(cursor);
    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 0);
}

#[test]
fn test_record_parser_only_invalid_lines() {
    let input = r#"garbage line 1
garbage line 2
not a valid record"#;
    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = RecordParser::new(cursor);
    let records: Vec<_> = parser.collect();
    assert_eq!(records.len(), 0);
}

#[test]
fn test_record_parser_mixed_valid_invalid() {
    let input = r#"garbage line
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
more garbage
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2
invalid line again"#;

    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = RecordParser::new(cursor);
    let records: Vec<_> = parser.collect();

    assert_eq!(records.len(), 2);
    assert!(records[0].is_ok());
    assert!(records[1].is_ok());
}

#[test]
fn test_record_parser_windows_line_endings() {
    let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\r\n2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2\r\n";

    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = RecordParser::new(cursor);
    let records: Vec<_> = parser.collect();

    assert_eq!(records.len(), 2);
    let record1 = records[0].as_ref().unwrap();
    let record2 = records[1].as_ref().unwrap();

    // 验证换行符已被正确移除
    assert!(!record1.start_line().contains('\r'));
    assert!(!record2.start_line().contains('\r'));
}

#[test]
fn test_record_parser_unix_line_endings() {
    let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2\n";

    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = RecordParser::new(cursor);
    let records: Vec<_> = parser.collect();

    assert_eq!(records.len(), 2);
}

// ==================== SqllogParser 边界测试 ====================

#[test]
fn test_sqllog_parser_empty_input() {
    let input = "";
    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = SqllogParser::new(cursor);
    let sqllogs: Vec<_> = parser.collect();
    assert_eq!(sqllogs.len(), 0);
}

#[test]
fn test_sqllog_parser_mixed_valid_invalid() {
    let input = r#"garbage
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.000 (EP[999] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"#;

    let cursor = std::io::Cursor::new(input.as_bytes());
    let parser = SqllogParser::new(cursor);
    let sqllogs: Vec<_> = parser.collect();

    assert_eq!(sqllogs.len(), 2);
    assert!(sqllogs[0].is_ok());
    // EP[999] 超过 u8 范围，应该解析失败
    assert!(sqllogs[1].is_err());
}

// ==================== 边界情况和错误处理 ====================

#[test]
fn test_parse_record_line_too_short() {
    // 这行太短，is_record_start_line 会拒绝
    let lines = vec!["2025-08-12 10:57:09"];
    let result = parse_record(&lines);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::InvalidRecordStartLine { .. }
    ));
}

#[test]
fn test_parse_record_missing_closing_paren() {
    // 缺少右括号，is_record_start_line 会拒绝
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app SELECT 1",
    ];
    let result = parse_record(&lines);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ParseError::InvalidRecordStartLine { .. }
    ));
}

#[test]
fn test_parse_record_insufficient_meta_fields() {
    // meta 字段不足（少于 5 个必需字段），is_record_start_line 会拒绝
    let lines = vec!["2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456) SELECT 1"];
    let result = parse_record(&lines);
    assert!(result.is_err());
    // 可能得到 InvalidRecordStartLine, InsufficientMetaFields 或 EmptyInput
    match result.unwrap_err() {
        ParseError::InvalidRecordStartLine { .. }
        | ParseError::InsufficientMetaFields { .. }
        | ParseError::EmptyInput => {}
        e => panic!(
            "Expected InvalidRecordStartLine, InsufficientMetaFields or EmptyInput, got: {:?}",
            e
        ),
    }
}

#[test]
fn test_parse_record_with_hex_values() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:0xABCD thrd:0x1234 user:alice trxid:0x789 stmt:0xFFFF appname:app) SELECT 1",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.meta.sess_id, "0xABCD");
    assert_eq!(sqllog.meta.thrd_id, "0x1234");
    assert_eq!(sqllog.meta.trxid, "0x789");
    assert_eq!(sqllog.meta.statement, "0xFFFF");
}

#[test]
fn test_parse_record_multiline_with_indicators() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *",
        "FROM users",
        "WHERE id = 1 EXECTIME: 15.5(ms) ROWCOUNT: 10(rows) EXEC_ID: 99999.",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.body, "SELECT *\nFROM users\nWHERE id = 1");

    assert!(sqllog.indicators.is_some());
    let indicators = sqllog.indicators.unwrap();
    assert_eq!(indicators.execute_time, 15.5);
    assert_eq!(indicators.row_count, 10);
    assert_eq!(indicators.execute_id, 99999);
}

#[test]
fn test_parse_record_empty_body() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app)",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.body, "");
}

#[test]
fn test_parse_record_special_characters_in_fields() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:user@domain.com trxid:789 stmt:999 appname:my-app-v1.0) SELECT 1",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.meta.username, "user@domain.com");
    assert_eq!(sqllog.meta.appname, "my-app-v1.0");
}

#[test]
fn test_record_equality() {
    let record1 = Record::new("line1".to_string());
    let mut record2 = Record::new("line1".to_string());

    assert_eq!(record1, record2);

    record2.add_line("line2".to_string());
    assert_ne!(record1, record2);
}

#[test]
fn test_record_clone() {
    let mut record1 = Record::new("line1".to_string());
    record1.add_line("line2".to_string());

    let record2 = record1.clone();

    assert_eq!(record1, record2);
    assert_eq!(record1.lines, record2.lines);
}

#[test]
fn test_build_body_with_empty_continuation() {
    use super::parse_functions::build_body;

    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    // 找到 ') ' 后的位置
    let close_paren = first_line.find(')').unwrap();
    let body_start = close_paren + 2; // ') ' 之后
    let continuation_lines = &["", "  ", "WHERE id = 1"];

    let body = build_body(first_line, body_start, continuation_lines);
    assert!(body.contains("SELECT 1"));
    assert!(body.contains("WHERE id = 1"));
}

#[test]
fn test_build_body_start_beyond_length() {
    use super::parse_functions::build_body;

    let first_line = "Short";
    let body_start = 100; // 超过字符串长度
    let continuation_lines = &["Continuation line"];

    let body = build_body(first_line, body_start, continuation_lines);
    assert_eq!(body, "Continuation line");
}

#[test]
fn test_extract_sql_body_no_indicators() {
    use super::parse_functions::extract_sql_body;

    let full_body = "SELECT * FROM users WHERE id = 1";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, full_body);
}

#[test]
fn test_extract_sql_body_multiple_indicators() {
    use super::parse_functions::extract_sql_body;

    let full_body = "SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, "SELECT 1");
}

#[test]
fn test_extract_field_value_various_prefixes() {
    use super::parse_functions::extract_field_value;

    // 测试不同前缀
    let result1 = extract_field_value("user:alice", "user:", "meta");
    assert_eq!(result1, Ok("alice".to_string()));

    let result2 = extract_field_value("trxid:12345", "trxid:", "meta");
    assert_eq!(result2, Ok("12345".to_string()));

    let result3 = extract_field_value("appname:my app", "appname:", "meta");
    assert_eq!(result3, Ok("my app".to_string()));
}

#[test]
fn test_parse_ep_field_boundaries() {
    use super::parse_functions::parse_ep_field;

    // 测试边界值
    let result0 = parse_ep_field("EP[0]", "raw");
    assert_eq!(result0, Ok(0));

    let result255 = parse_ep_field("EP[255]", "raw");
    assert_eq!(result255, Ok(255));

    // 超出范围
    let result256 = parse_ep_field("EP[256]", "raw");
    assert!(result256.is_err());
}

#[test]
fn test_extract_indicator_edge_cases() {
    use super::parse_functions::extract_indicator;

    // 测试带小数点的值
    let result1 = extract_indicator("EXECTIME: 0.5(ms)", "EXECTIME:", "(ms)");
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), "0.5");

    // 测试大数值
    let result2 = extract_indicator("ROWCOUNT: 999999999(rows)", "ROWCOUNT:", "(rows)");
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), "999999999");

    // 测试缺少后缀
    let result3 = extract_indicator("EXECTIME: 10.5", "EXECTIME:", "(ms)");
    assert!(result3.is_err());
}

#[test]
fn test_parse_meta_edge_spaces() {
    use super::parse_functions::parse_meta;

    // appname 中包含多个空格
    let meta_str =
        "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:my application name";
    let result = parse_meta(meta_str);
    assert!(result.is_ok());

    let meta = result.unwrap();
    assert_eq!(meta.appname, "my application name");
}

#[test]
fn test_parse_indicators_partial() {
    use super::parse_functions::parse_indicators;

    // 只有 EXECTIME
    let body1 = "SELECT 1 EXECTIME: 10.5(ms)";
    assert!(parse_indicators(body1).is_err()); // 缺少其他必需字段

    // 只有 ROWCOUNT
    let body2 = "SELECT 1 ROWCOUNT: 100(rows)";
    assert!(parse_indicators(body2).is_err());
}

#[test]
fn test_parse_record_with_empty_lines() {
    use super::parse_record;

    // 包含空行的多行记录
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *",
        "FROM users",
        "",
        "WHERE id = 1",
    ];

    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert!(sqllog.body.contains("FROM users"));
    assert!(sqllog.body.contains("WHERE id = 1"));
}
