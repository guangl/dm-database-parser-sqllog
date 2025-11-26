#![cfg(feature = "test-helpers")]
use dm_database_parser_sqllog::__test_helpers::*;
use dm_database_parser_sqllog::error::ParseError;

#[test]
fn test_parse_record_success() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1",
    ];
    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.ts, "2025-08-12 10:57:09.548");
    assert_eq!(sqllog.meta.username, "alice");
    assert!(sqllog.body.contains("SELECT"));
}

#[test]
fn test_parse_record_empty_input() {
    let lines: Vec<&str> = vec![];
    let result = parse_record(&lines);
    assert!(matches!(result, Err(ParseError::EmptyInput)));
}

#[test]
fn test_parse_record_invalid_start_line() {
    let lines = vec!["not a valid log line"];
    let result = parse_record(&lines);
    assert!(matches!(
        result,
        Err(ParseError::InvalidRecordStartLine { .. })
    ));
}

#[test]
fn test_parse_record_line_too_short() {
    // 短行不会被 is_record_start_line 识别,返回 InvalidRecordStartLine
    let lines = vec!["2025-08-12"];
    let result = parse_record(&lines);
    assert!(matches!(
        result,
        Err(ParseError::InvalidRecordStartLine { .. })
    ));
}

#[test]
fn test_parse_record_missing_closing_paren() {
    // 缺少右括号不会被 is_record_start_line 识别,返回 InvalidRecordStartLine
    let lines = vec!["2025-08-12 10:57:09.548 (EP[0] sess:123 SELECT"];
    let result = parse_record(&lines);
    assert!(matches!(
        result,
        Err(ParseError::InvalidRecordStartLine { .. })
    ));
}

#[test]
fn test_parse_record_multiline() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *",
        "FROM users",
        "WHERE id > 0",
    ];
    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert!(sqllog.body.contains("FROM users"));
    assert!(sqllog.body.contains("WHERE id > 0"));
}

#[test]
fn test_parse_record_with_indicators() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 5.5(ms) ROWCOUNT: 10(rows) EXEC_ID: 999.",
    ];
    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert!(sqllog.indicators.is_some());
    let indicators = sqllog.indicators.unwrap();
    assert_eq!(indicators.execute_time, 5.5);
    assert_eq!(indicators.row_count, 10);
    assert_eq!(indicators.execute_id, 999);
}

#[test]
fn test_parse_meta_success() {
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
}

#[test]
fn test_parse_meta_with_client_ip() {
    let meta_str =
        "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:192.168.1.1";
    let result = parse_meta(meta_str);
    assert!(result.is_ok());

    let meta = result.unwrap();
    assert_eq!(meta.client_ip, "192.168.1.1");
}

#[test]
fn test_parse_meta_insufficient_fields() {
    let meta_str = "EP[0] sess:123";
    let result = parse_meta(meta_str);
    assert!(matches!(
        result,
        Err(ParseError::InsufficientMetaFields { .. })
    ));
}

#[test]
fn test_parse_meta_only_required_fields() {
    // 测试只有必需字段(无 appname 和 clientip)
    let meta_str = "EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222";
    let result = parse_meta(meta_str);
    assert!(result.is_ok());

    let meta = result.unwrap();
    assert_eq!(meta.ep, 1);
    assert_eq!(meta.sess_id, "456");
    assert_eq!(meta.thrd_id, "789");
    assert_eq!(meta.username, "bob");
    assert_eq!(meta.trxid, "111");
    assert_eq!(meta.statement, "222");
    assert_eq!(meta.appname, "");
    assert_eq!(meta.client_ip, "");
}

#[test]
fn test_parse_meta_with_appname_no_ip() {
    let meta_str = "EP[2] sess:789 thrd:123 user:charlie trxid:222 stmt:333 appname:myapp";
    let result = parse_meta(meta_str);
    assert!(result.is_ok());

    let meta = result.unwrap();
    assert_eq!(meta.appname, "myapp");
    assert_eq!(meta.client_ip, "");
}

#[test]
fn test_parse_meta_ep_parse_error() {
    let meta_str = "EP[abc] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app";
    let result = parse_meta(meta_str);
    assert!(matches!(result, Err(ParseError::EpParseError { .. })));
}

#[test]
fn test_parse_meta_invalid_field_prefix() {
    let meta_str = "EP[0] session:123 thrd:456 user:alice trxid:789 stmt:999";
    let result = parse_meta(meta_str);
    assert!(matches!(result, Err(ParseError::InvalidFieldFormat { .. })));
}

#[test]
fn test_build_body_single_line() {
    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let body_start = 92; // ") " 之后的位置
    let continuation_lines: Vec<&str> = vec![];

    let body = build_body(first_line, body_start, &continuation_lines);
    assert_eq!(body, "SELECT 1");
}

#[test]
fn test_build_body_multiline() {
    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *";
    let body_start = 92; // ") " 之后的位置
    let continuation_lines = vec!["FROM users", "WHERE id > 0"];

    let body = build_body(first_line, body_start, &continuation_lines);
    assert_eq!(body, "SELECT *\nFROM users\nWHERE id > 0");
}

#[test]
fn test_build_body_empty_first_part() {
    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app)";
    let body_start = 94;
    let continuation_lines = vec!["SELECT 1"];

    let body = build_body(first_line, body_start, &continuation_lines);
    assert_eq!(body, "SELECT 1");
}

#[test]
fn test_build_body_body_start_out_of_bounds() {
    let first_line = "short";
    let body_start = 100;
    let continuation_lines: Vec<&str> = vec![];

    let body = build_body(first_line, body_start, &continuation_lines);
    assert_eq!(body, "");
}

#[test]
fn test_extract_sql_body_with_all_indicators() {
    let full_body =
        "UPDATE users SET status='active' EXECTIME: 15.5(ms) ROWCOUNT: 50(rows) EXEC_ID: 98765.";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, "UPDATE users SET status='active'");
}

#[test]
fn test_extract_indicator_success() {
    let body = "SELECT 1 EXECTIME: 25.5(ms) ROWCOUNT: 200(rows)";
    let result = extract_indicator(body, "EXECTIME:", "(ms)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "25.5");
}

#[test]
fn test_extract_indicator_prefix_not_found() {
    let body = "SELECT 1";
    let result = extract_indicator(body, "EXECTIME:", "(ms)");
    assert!(result.is_err());
}

#[test]
fn test_extract_indicator_suffix_not_found() {
    let body = "SELECT 1 EXECTIME: 25.5";
    let result = extract_indicator(body, "EXECTIME:", "(ms)");
    assert!(result.is_err());
}

#[test]
fn test_parse_indicators_invalid_exectime() {
    let body = "SELECT 1 EXECTIME: abc(ms) ROWCOUNT: 10(rows) EXEC_ID: 123.";
    let result = parse_indicators(body);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("执行时间解析失败"));
    }
}

#[test]
fn test_parse_indicators_invalid_rowcount() {
    let body = "SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: xyz(rows) EXEC_ID: 123.";
    let result = parse_indicators(body);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("行数解析失败"));
    }
}

#[test]
fn test_parse_indicators_invalid_exec_id() {
    let body = "SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: 10(rows) EXEC_ID: notanumber.";
    let result = parse_indicators(body);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("执行 ID 解析失败"));
    }
}

#[test]
fn test_parse_record_without_indicators() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) UPDATE logs SET status='done'",
    ];
    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert!(sqllog.indicators.is_none());
    assert!(sqllog.body.contains("UPDATE"));
}

#[test]
fn test_parse_record_with_only_appname() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:test app) DELETE FROM cache",
    ];
    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.meta.appname, "test app");
}

#[test]
fn test_parse_ep_field_valid() {
    let result = parse_ep_field("EP[5]", "raw");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);
}

#[test]
fn test_parse_ep_field_invalid_format() {
    let result = parse_ep_field("EPX", "raw");
    assert!(matches!(result, Err(ParseError::InvalidEpFormat { .. })));
}

#[test]
fn test_parse_ep_field_no_brackets() {
    let result = parse_ep_field("EP5", "raw");
    assert!(matches!(result, Err(ParseError::InvalidEpFormat { .. })));
}

#[test]
fn test_parse_ep_field_missing_closing_bracket() {
    let result = parse_ep_field("EP[5", "raw");
    assert!(matches!(result, Err(ParseError::InvalidEpFormat { .. })));
}

#[test]
fn test_extract_field_value_valid() {
    let result = extract_field_value("sess:123", "sess:", "raw");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "123");
}

#[test]
fn test_extract_field_value_invalid_prefix() {
    let result = extract_field_value("session:123", "sess:", "raw");
    assert!(matches!(result, Err(ParseError::InvalidFieldFormat { .. })));
}

#[test]
fn test_extract_field_value_empty_value() {
    let result = extract_field_value("sess:", "sess:", "raw");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_parse_record_minimal_body() {
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app)",
    ];
    let result = parse_record(&lines);
    assert!(result.is_ok());

    let sqllog = result.unwrap();
    assert_eq!(sqllog.body, "");
}

#[test]
fn test_extract_sql_body_with_rowcount_only() {
    let full_body = "INSERT INTO logs VALUES (1) ROWCOUNT: 5(rows) EXEC_ID: 111.";
    let sql_body = extract_sql_body(full_body);
    // extract_sql_body 优先查找 EXECTIME,没有则查找其他指标
    assert!(sql_body.contains("INSERT"));
    assert!(!sql_body.contains("ROWCOUNT"));
}

#[test]
fn test_extract_sql_body_with_exec_id_only() {
    let full_body = "DELETE FROM temp EXEC_ID: 999.";
    let sql_body = extract_sql_body(full_body);
    assert!(sql_body.contains("DELETE"));
    assert!(!sql_body.contains("EXEC_ID"));
}

#[test]
fn test_parse_meta_ep_max_value() {
    let meta_str = "EP[255] sess:999 thrd:888 user:test trxid:777 stmt:666 appname:app";
    let result = parse_meta(meta_str);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().ep, 255);
}

#[test]
fn test_parse_meta_ep_overflow() {
    let meta_str = "EP[256] sess:999 thrd:888 user:test trxid:777 stmt:666 appname:app";
    let result = parse_meta(meta_str);
    assert!(matches!(result, Err(ParseError::EpParseError { .. })));
}

#[test]
fn test_build_body_multiple_continuation_no_first_part() {
    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app)";
    let body_start = first_line.len(); // body_start 等于长度,无第一部分
    let continuation_lines = vec!["SELECT *", "FROM users", "WHERE id > 0"];

    let body = build_body(first_line, body_start, &continuation_lines);
    assert_eq!(body, "SELECT *\nFROM users\nWHERE id > 0");
}

#[test]
fn test_parse_meta_invalid_ep_format() {
    let meta_str = "EPx sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app";
    let result = parse_meta(meta_str);
    assert!(matches!(result, Err(ParseError::InvalidEpFormat { .. })));
}

#[test]
fn test_parse_indicators_all_fields() {
    let body = "SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";
    let result = parse_indicators(body);
    assert!(result.is_ok());

    let indicators = result.unwrap();
    assert_eq!(indicators.execute_time, 10.5);
    assert_eq!(indicators.row_count, 100);
    assert_eq!(indicators.execute_id, 12345);
}

#[test]
fn test_parse_indicators_partial_fields() {
    let body = "SELECT 1 EXECTIME: 5.5(ms)";
    let result = parse_indicators(body);
    // 注意:parse_indicators 需要所有三个字段,否则会失败
    assert!(result.is_err());
}

#[test]
fn test_parse_indicators_no_indicators() {
    let body = "SELECT 1";
    let result = parse_indicators(body);
    assert!(result.is_err());
}

#[test]
fn test_extract_sql_body() {
    let full_body = "SELECT * FROM users EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, "SELECT * FROM users");
}

#[test]
fn test_extract_sql_body_no_indicators() {
    let full_body = "SELECT * FROM users";
    let sql_body = extract_sql_body(full_body);
    assert_eq!(sql_body, "SELECT * FROM users");
}
