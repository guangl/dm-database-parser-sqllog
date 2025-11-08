//! 测试新的四部分记录结构

use dm_database_parser_sqllog::parse_record;

#[test]
fn test_record_four_parts_basic() {
    let log_text = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1\nEXECTIME: 10ms ROWCOUNT: 5 EXEC_ID: 100";
    let parsed = parse_record(log_text);

    // 验证四部分
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.meta, " (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)");
    assert_eq!(parsed.body, "SELECT 1");
    assert_eq!(parsed.end, Some("EXECTIME: 10ms ROWCOUNT: 5 EXEC_ID: 100"));

    // 验证解析字段
    assert_eq!(parsed.user, "admin");
    assert_eq!(parsed.execute_time_ms, Some(10));
    assert_eq!(parsed.row_count, Some(5));
    assert_eq!(parsed.execute_id, Some(100));
}

#[test]
fn test_record_four_parts_no_end() {
    let log_text = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:guest trxid:0 stmt:1 appname:MyApp) INSERT INTO logs VALUES ('test')";
    let parsed = parse_record(log_text);

    // 验证四部分
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.meta, " (EP[0] sess:1 thrd:1 user:guest trxid:0 stmt:1 appname:MyApp)");
    assert_eq!(parsed.body, "INSERT INTO logs VALUES ('test')");
    assert_eq!(parsed.end, None);

    // 验证解析字段
    assert_eq!(parsed.user, "guest");
    assert!(parsed.execute_time_ms.is_none());
}

#[test]
fn test_record_four_parts_multiline_body() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:test trxid:0 stmt:1 appname:MyApp) SELECT *
FROM users
WHERE id = 1
EXECTIME: 25ms ROWCOUNT: 1 EXEC_ID: 200"#;
    
    let parsed = parse_record(log_text);

    // 验证四部分
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.meta, " (EP[0] sess:1 thrd:1 user:test trxid:0 stmt:1 appname:MyApp)");
    assert_eq!(parsed.body, "SELECT *\nFROM users\nWHERE id = 1");
    assert_eq!(parsed.end, Some("EXECTIME: 25ms ROWCOUNT: 1 EXEC_ID: 200"));

    // 验证解析字段
    assert_eq!(parsed.user, "test");
    assert_eq!(parsed.execute_time_ms, Some(25));
}

#[test]
fn test_record_four_parts_body_contains_exectime_keyword() {
    // body 中包含 EXECTIME 关键字，但不是 end 行
    let log_text = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT * FROM logs WHERE message LIKE '%EXECTIME%'";
    let parsed = parse_record(log_text);

    // 应该没有 end 部分，因为 EXECTIME 在 body 中
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.body, "SELECT * FROM logs WHERE message LIKE '%EXECTIME%'");
    assert!(parsed.end.is_none() || parsed.end == Some("SELECT * FROM logs WHERE message LIKE '%EXECTIME%'"));
}

#[test]
fn test_record_four_parts_only_end_line() {
    // 只有 EXECTIME 行，没有 SQL body
    let log_text = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 5ms ROWCOUNT: 0 EXEC_ID: 300";
    let parsed = parse_record(log_text);

    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.meta, " (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)");
    // body 应该为空，end 应该包含 EXECTIME 行
    assert_eq!(parsed.body, "");
    assert_eq!(parsed.end, Some("EXECTIME: 5ms ROWCOUNT: 0 EXEC_ID: 300"));
    assert_eq!(parsed.execute_time_ms, Some(5));
}

#[test]
fn test_record_four_parts_meta_extraction() {
    let log_text = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:100 stmt:1 appname:TestApp ip:192.168.1.1) SELECT 1";
    let parsed = parse_record(log_text);

    // meta 应该包含完整的括号及内容
    assert!(parsed.meta.starts_with(" ("));
    assert!(parsed.meta.ends_with(')'));
    assert!(parsed.meta.contains("EP[0]"));
    assert!(parsed.meta.contains("user:admin"));
    
    // meta_raw 是括号内的内容
    assert!(!parsed.meta_raw.contains('('));
    assert!(!parsed.meta_raw.contains(')'));
}

#[test]
fn test_record_structure_invariants() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) UPDATE table SET x=1
WHERE y=2
EXECTIME: 15ms ROWCOUNT: 10 EXEC_ID: 400"#;
    
    let parsed = parse_record(log_text);

    // 验证不变式
    // 1. ts 必定在首行
    assert!(log_text.starts_with(parsed.ts));
    
    // 2. meta 必定在首行
    let first_line = log_text.lines().next().unwrap();
    assert!(first_line.contains(parsed.meta));
    
    // 3. body 可能跨多行
    assert!(parsed.body.contains('\n') || !parsed.body.is_empty());
    
    // 4. end 如果存在，必定在最后一行
    if let Some(end) = parsed.end {
        let last_line = log_text.lines().last().unwrap();
        assert!(last_line.contains(end));
        assert!(end.starts_with("EXECTIME:"));
    }
}
