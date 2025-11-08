//! 完整的解析器集成测试
//!
//! 测试所有公共 API 的功能和边界情况

use dm_database_parser_sqllog::{
    for_each_record, parse_all, parse_into, parse_record, parse_records_with,
    split_by_ts_records_with_errors, split_into, RecordSplitter,
};

#[test]
fn test_split_by_ts_records_with_errors_basic() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let (records, errors) = split_by_ts_records_with_errors(log_text);
    assert_eq!(records.len(), 2);
    assert_eq!(errors.len(), 0);
    assert!(records[0].contains("SELECT 1"));
    assert!(records[1].contains("SELECT 2"));
}

#[test]
fn test_split_by_ts_records_with_errors_leading_garbage() {
    let log_text = r#"garbage line 1
garbage line 2
2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#;

    let (records, errors) = split_by_ts_records_with_errors(log_text);
    assert_eq!(records.len(), 1);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0], "garbage line 1");
    assert_eq!(errors[1], "garbage line 2");
}

#[test]
fn test_split_into_reuse_buffers() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#;

    let mut records = Vec::new();
    let mut errors = Vec::new();

    // 第一次调用
    split_into(log_text, &mut records, &mut errors);
    assert_eq!(records.len(), 1);
    assert_eq!(errors.len(), 0);

    // 第二次调用，重用缓冲区
    split_into(log_text, &mut records, &mut errors);
    assert_eq!(records.len(), 1);
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_for_each_record_basic() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let mut count = 0;
    for_each_record(log_text, |rec| {
        assert!(rec.contains("SELECT"));
        count += 1;
    });
    assert_eq!(count, 2);
}

#[test]
fn test_parse_record_complete() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[12345] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:688489653 stmt:0x7fb236077b70 appname: ip:::ffff:10.3.100.68) EXECTIME: 100ms ROWCOUNT: 5 EXEC_ID: 289655185
SELECT * FROM users WHERE id = 1
"#;

    let parsed = parse_record(record_text);
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.ep, "EP[12345]");
    assert_eq!(parsed.sess, "0x7fb24f392a30");
    assert_eq!(parsed.thrd, "757794");
    assert_eq!(parsed.user, "HBTCOMS_V3_PROD");
    assert_eq!(parsed.trxid, "688489653");
    assert_eq!(parsed.stmt, "0x7fb236077b70");
    assert_eq!(parsed.appname, "");
    assert_eq!(parsed.ip, Some("10.3.100.68"));
    assert_eq!(parsed.execute_time_ms, Some(100));
    assert_eq!(parsed.row_count, Some(5));
    assert_eq!(parsed.execute_id, Some(289655185));
    assert!(parsed.body.contains("SELECT * FROM users"));
}

#[test]
fn test_parse_record_without_metrics() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) TRX: START
BEGIN TRANSACTION
"#;

    let parsed = parse_record(record_text);
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.user, "admin");
    assert_eq!(parsed.execute_time_ms, None);
    assert_eq!(parsed.row_count, None);
    assert_eq!(parsed.execute_id, None);
    assert!(parsed.body.contains("TRX: START"));
    assert!(parsed.body.contains("BEGIN TRANSACTION"));
}

#[test]
fn test_parse_all_basic() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let records = parse_all(log_text);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].user, "admin");
    assert_eq!(records[1].user, "guest");
}

#[test]
fn test_parse_into_reuse_buffer() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#;

    let mut parsed_records = Vec::new();

    // 第一次调用
    parse_into(log_text, &mut parsed_records);
    assert_eq!(parsed_records.len(), 1);
    assert_eq!(parsed_records[0].user, "admin");

    // 第二次调用，重用缓冲区
    parse_into(log_text, &mut parsed_records);
    assert_eq!(parsed_records.len(), 1);
    assert_eq!(parsed_records[0].user, "admin");
}

#[test]
fn test_parse_records_with_callback() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let mut users = Vec::new();
    parse_records_with(log_text, |parsed| {
        users.push(parsed.user.to_string());
    });
    assert_eq!(users.len(), 2);
    assert_eq!(users[0], "admin");
    assert_eq!(users[1], "guest");
}

#[test]
fn test_record_splitter_iterator() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:0 stmt:3 appname:MyApp) SELECT 3
"#;

    let splitter = RecordSplitter::new(log_text);
    let records: Vec<&str> = splitter.collect();
    assert_eq!(records.len(), 3);
}

#[test]
fn test_record_splitter_leading_errors() {
    let log_text = r#"garbage line
2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#;

    let splitter = RecordSplitter::new(log_text);
    let leading_errors = splitter.leading_errors_slice();
    assert!(leading_errors.is_some());
    assert!(leading_errors.unwrap().contains("garbage line"));
}

#[test]
fn test_parse_record_all_fields() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[123] sess:abc thrd:456 user:testuser trxid:789 stmt:stmt123 appname:MyApp) EXECTIME: 50ms ROWCOUNT: 10 EXEC_ID: 999
SELECT * FROM table
"#;

    let parsed = parse_record(record_text);
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert_eq!(parsed.ep, "EP[123]");
    assert_eq!(parsed.sess, "abc");
    assert_eq!(parsed.thrd, "456");
    assert_eq!(parsed.user, "testuser");
    assert_eq!(parsed.trxid, "789");
    assert_eq!(parsed.stmt, "stmt123");
    assert_eq!(parsed.appname, "MyApp");
    assert_eq!(parsed.execute_time_ms, Some(50));
    assert_eq!(parsed.row_count, Some(10));
    assert_eq!(parsed.execute_id, Some(999));
    assert!(parsed.body.contains("SELECT * FROM table"));
}

#[test]
fn test_parse_record_with_ip() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname: ip:::ffff:192.168.1.1) SELECT 1
"#;

    let parsed = parse_record(record_text);
    assert_eq!(parsed.ip, Some("192.168.1.1"));
    assert_eq!(parsed.appname, "");
}

#[test]
fn test_parse_record_with_ip_ffff_prefix() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname: ip:::ffff:ffff:192.168.1.1) SELECT 1
"#;

    let parsed = parse_record(record_text);
    assert_eq!(parsed.ip, Some("192.168.1.1"));
}

#[test]
fn test_multiline_record_body() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 0ms ROWCOUNT: 1 EXEC_ID: 1
SELECT 
    column1,
    column2,
    column3
FROM 
    table1
WHERE 
    id = 1
"#;

    let parsed = parse_record(record_text);
    assert!(parsed.body.contains("SELECT"));
    assert!(parsed.body.contains("column1"));
    assert!(parsed.body.contains("column2"));
    assert!(parsed.body.contains("FROM"));
    assert!(parsed.body.contains("WHERE"));
}

#[test]
fn test_empty_log() {
    let log_text = "";
    let (records, errors) = split_by_ts_records_with_errors(log_text);
    assert_eq!(records.len(), 0);
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_only_garbage_lines() {
    let log_text = "garbage line 1\ngarbage line 2\n";
    let (records, errors) = split_by_ts_records_with_errors(log_text);
    assert_eq!(records.len(), 0);
    assert_eq!(errors.len(), 0); // 找不到第一个有效记录时，errors 为空
}

#[test]
fn test_record_with_complex_sql() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 100ms ROWCOUNT: 1000 EXEC_ID: 12345
INSERT INTO orders (id, customer_id, product_id, quantity, price) 
VALUES (1, 100, 200, 5, 99.99), (2, 101, 201, 3, 149.99), (3, 102, 202, 10, 79.99)
"#;

    let parsed = parse_record(record_text);
    assert_eq!(parsed.execute_time_ms, Some(100));
    assert_eq!(parsed.row_count, Some(1000));
    assert_eq!(parsed.execute_id, Some(12345));
    assert!(parsed.body.contains("INSERT INTO orders"));
    assert!(parsed.body.contains("VALUES"));
}

#[test]
fn test_multiple_records_with_various_sql_types() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 10ms ROWCOUNT: 1 EXEC_ID: 1
SELECT * FROM users

2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:admin trxid:0 stmt:2 appname:MyApp) EXECTIME: 5ms ROWCOUNT: 0 EXEC_ID: 2
INSERT INTO users (name, email) VALUES ('John', 'john@example.com')

2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:admin trxid:0 stmt:3 appname:MyApp) EXECTIME: 3ms ROWCOUNT: 1 EXEC_ID: 3
UPDATE users SET email = 'newemail@example.com' WHERE id = 1

2025-08-12 10:57:12.789 (EP[0] sess:4 thrd:4 user:admin trxid:0 stmt:4 appname:MyApp) EXECTIME: 2ms ROWCOUNT: 1 EXEC_ID: 4
DELETE FROM users WHERE id = 1
"#;

    let records = parse_all(log_text);
    assert_eq!(records.len(), 4);
    
    assert!(records[0].body.contains("SELECT"));
    assert_eq!(records[0].row_count, Some(1));
    
    assert!(records[1].body.contains("INSERT"));
    assert_eq!(records[1].row_count, Some(0));
    
    assert!(records[2].body.contains("UPDATE"));
    assert_eq!(records[2].row_count, Some(1));
    
    assert!(records[3].body.contains("DELETE"));
    assert_eq!(records[3].row_count, Some(1));
}

#[test]
fn test_record_with_special_characters() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 0ms ROWCOUNT: 1 EXEC_ID: 1
SELECT * FROM users WHERE name = 'O''Brien' AND email LIKE '%@example.com'
"#;

    let parsed = parse_record(record_text);
    assert!(parsed.body.contains("O''Brien"));
    assert!(parsed.body.contains("%@example.com"));
}

#[test]
fn test_record_with_unicode() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 0ms ROWCOUNT: 1 EXEC_ID: 1
SELECT * FROM users WHERE name = '测试用户' AND description LIKE '%中文%'
"#;

    let parsed = parse_record(record_text);
    assert!(parsed.body.contains("测试用户"));
    assert!(parsed.body.contains("中文"));
}

#[test]
fn test_parse_record_edge_cases() {
    // 测试空 body
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)
"#;
    let parsed = parse_record(record_text);
    assert_eq!(parsed.ts, "2025-08-12 10:57:09.562");
    assert!(parsed.body.is_empty() || parsed.body.trim().is_empty());

    // 测试只有时间戳和元数据，没有 body
    let record_text2 = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)";
    let parsed2 = parse_record(record_text2);
    assert_eq!(parsed2.ts, "2025-08-12 10:57:09.562");
}

#[test]
fn test_large_number_of_records() {
    let mut log_text = String::new();
    for i in 0..1000 {
        log_text.push_str(&format!(
            "2025-08-12 10:57:09.562 (EP[{}] sess:{} thrd:{} user:user{} trxid:{} stmt:{} appname:App) SELECT {}\n",
            i % 10, i, i, i, i, i, i
        ));
    }

    let (records, errors) = split_by_ts_records_with_errors(&log_text);
    assert_eq!(records.len(), 1000);
    assert_eq!(errors.len(), 0);

    let parsed = parse_all(&log_text);
    assert_eq!(parsed.len(), 1000);
    for (i, p) in parsed.iter().enumerate() {
        assert_eq!(p.user, format!("user{}", i));
    }
}

#[test]
fn test_record_splitter_empty() {
    let log_text = "";
    let splitter = RecordSplitter::new(log_text);
    assert!(splitter.leading_errors_slice().is_none());
    let records: Vec<&str> = splitter.collect();
    assert_eq!(records.len(), 0);
}

#[test]
fn test_parse_record_metrics_order() {
    // 测试不同的指标顺序
    let record_text1 = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 10ms ROWCOUNT: 5 EXEC_ID: 100
SELECT 1
"#;
    let parsed1 = parse_record(record_text1);
    assert_eq!(parsed1.execute_time_ms, Some(10));
    assert_eq!(parsed1.row_count, Some(5));
    assert_eq!(parsed1.execute_id, Some(100));

    // 测试只有部分指标
    let record_text2 = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 20ms
SELECT 2
"#;
    let parsed2 = parse_record(record_text2);
    assert_eq!(parsed2.execute_time_ms, Some(20));
    assert_eq!(parsed2.row_count, None);
    assert_eq!(parsed2.execute_id, None);
}

#[test]
fn test_for_each_record_empty() {
    let log_text = "";
    let mut count = 0;
    for_each_record(log_text, |_rec| {
        count += 1;
    });
    assert_eq!(count, 0);
}

#[test]
fn test_parse_records_with_empty() {
    let log_text = "";
    let mut count = 0;
    parse_records_with(log_text, |_parsed| {
        count += 1;
    });
    assert_eq!(count, 0);
}

#[test]
fn test_record_with_appname_and_ip_separate() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp ip:::ffff:10.3.100.68) SELECT 1
"#;
    let parsed = parse_record(record_text);
    // 注意：根据当前实现，ip 可能在 appname 字段中，需要检查实际行为
    assert_eq!(parsed.appname, "MyApp");
}

#[test]
fn test_record_with_null_stmt() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:NULL appname:MyApp) TRX: START
"#;
    let parsed = parse_record(record_text);
    assert_eq!(parsed.stmt, "NULL");
}

#[test]
fn test_parse_all_vs_for_each_record_consistency() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let records_all = parse_all(log_text);
    let mut record_texts = Vec::new();
    for_each_record(log_text, |rec| {
        record_texts.push(rec.to_string());
    });

    assert_eq!(records_all.len(), record_texts.len());
    for (i, rec_text) in record_texts.iter().enumerate() {
        let parsed = parse_record(rec_text);
        assert_eq!(parsed.user, records_all[i].user);
    }
}

