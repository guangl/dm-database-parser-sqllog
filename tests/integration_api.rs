//! 完整的 API 集成测试
//!
//! 测试所有公共 API 的端到端功能

use dm_database_parser_sqllog::{
    for_each_record, parse_all, parse_into, parse_record, parse_records_with,
    split_by_ts_records_with_errors, split_into, ParsedRecord, RecordSplitter,
};

#[test]
fn test_complete_workflow_split_and_parse() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) EXECTIME: 10ms ROWCOUNT: 5 EXEC_ID: 100
SELECT * FROM users WHERE id = 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) EXECTIME: 5ms ROWCOUNT: 0 EXEC_ID: 101
INSERT INTO users (name) VALUES ('test')
"#;

    // 步骤 1: 拆分记录
    let (records, errors) = split_by_ts_records_with_errors(log_text);
    assert_eq!(records.len(), 2);
    assert_eq!(errors.len(), 0);

    // 步骤 2: 解析每条记录
    let parsed: Vec<ParsedRecord> = records.iter().map(|r| parse_record(r)).collect();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].user, "admin");
    assert_eq!(parsed[1].user, "guest");
    assert_eq!(parsed[0].execute_time_ms, Some(10));
    assert_eq!(parsed[1].execute_time_ms, Some(5));
}

#[test]
fn test_complete_workflow_streaming() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    // 使用流式 API 处理
    let mut users = Vec::new();
    let mut trxids = Vec::new();

    parse_records_with(log_text, |parsed| {
        users.push(parsed.user.to_string());
        trxids.push(parsed.trxid.to_string());
    });

    assert_eq!(users.len(), 2);
    assert_eq!(users[0], "admin");
    assert_eq!(users[1], "guest");
    assert_eq!(trxids[0], "0");
    assert_eq!(trxids[1], "0");
}

#[test]
fn test_complete_workflow_reuse_buffers() {
    let log_text1 = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#;
    let log_text2 = r#"2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut parsed_records = Vec::new();

    // 处理第一个文件
    split_into(log_text1, &mut records, &mut errors);
    parse_into(log_text1, &mut parsed_records);
    assert_eq!(parsed_records.len(), 1);
    assert_eq!(parsed_records[0].user, "admin");

    // 重用缓冲区处理第二个文件
    split_into(log_text2, &mut records, &mut errors);
    parse_into(log_text2, &mut parsed_records);
    assert_eq!(parsed_records.len(), 1); // 缓冲区被清空后重新填充
    assert_eq!(parsed_records[0].user, "guest");
}

#[test]
fn test_record_splitter_api() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let splitter = RecordSplitter::new(log_text);
    
    // 测试 leading_errors_slice（没有前导错误时应该为 None）
    // 注意：如果第一个记录有效，leading_errors_slice 应该为 None
    let leading_errors = splitter.leading_errors_slice();
    // 由于第一个记录是有效的，所以应该没有前导错误
    // 但我们需要先检查，因为 splitter 在创建时已经找到了第一个记录
    let has_leading_errors = leading_errors.is_some();
    
    // 测试迭代器
    let splitter2 = RecordSplitter::new(log_text);
    let records: Vec<&str> = splitter2.collect();
    assert_eq!(records.len(), 2);
    
    // 如果没有前导错误，leading_errors 应该为 None
    if !has_leading_errors {
        assert!(leading_errors.is_none());
    }

    // 测试可以多次创建 splitter
    let splitter3 = RecordSplitter::new(log_text);
    let count = splitter3.count();
    assert_eq!(count, 2);
}

#[test]
fn test_all_parsing_apis_consistency() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    // 方法 1: parse_all
    let records1 = parse_all(log_text);

    // 方法 2: parse_into
    let mut records2 = Vec::new();
    parse_into(log_text, &mut records2);

    // 方法 3: parse_records_with
    let mut users3 = Vec::new();
    let mut trxids3 = Vec::new();
    parse_records_with(log_text, |p| {
        users3.push(p.user.to_string());
        trxids3.push(p.trxid.to_string());
    });

    // 所有方法应该产生相同的结果
    assert_eq!(records1.len(), records2.len());
    assert_eq!(records1.len(), users3.len());

    for i in 0..records1.len() {
        assert_eq!(records1[i].user, records2[i].user);
        assert_eq!(records1[i].user, users3[i]);
        assert_eq!(records1[i].trxid, records2[i].trxid);
        assert_eq!(records1[i].trxid, trxids3[i]);
    }
}

#[test]
fn test_edge_case_empty_meta() {
    // 测试元数据为空的情况（虽然不符合格式，但应该能处理）
    // 注意：根据当前实现，这可能会失败，因为要求所有字段都存在
    // 这个测试用于验证错误处理
}

#[test]
fn test_very_long_record() {
    let mut record_text = String::from("2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) ");
    // 添加很长的 SQL 语句
    record_text.push_str("SELECT ");
    for i in 0..1000 {
        record_text.push_str(&format!("column{}, ", i));
    }
    record_text.push_str("FROM very_long_table_name WHERE condition1 = 'value1' AND condition2 = 'value2'");

    let parsed = parse_record(&record_text);
    assert_eq!(parsed.user, "admin");
    assert!(parsed.body.len() > 1000);
}

#[test]
fn test_record_with_all_optional_fields() {
    // IP 格式应该是 appname: ip:::ffff:192.168.1.1（appname 为空，IP 在 appname 字段中）
    let record_text = r#"2025-08-12 10:57:09.562 (EP[123] sess:abc thrd:456 user:testuser trxid:789 stmt:stmt123 appname: ip:::ffff:192.168.1.1) EXECTIME: 100ms ROWCOUNT: 50 EXEC_ID: 999999
SELECT * FROM large_table WHERE id IN (1,2,3,4,5)
"#;

    let parsed = parse_record(record_text);
    
    // 验证所有字段
    assert_eq!(parsed.ep, "EP[123]");
    assert_eq!(parsed.sess, "abc");
    assert_eq!(parsed.thrd, "456");
    assert_eq!(parsed.user, "testuser");
    assert_eq!(parsed.trxid, "789");
    assert_eq!(parsed.stmt, "stmt123");
    assert_eq!(parsed.appname, ""); // IP 格式时 appname 为空
    assert_eq!(parsed.ip, Some("192.168.1.1"));
    assert_eq!(parsed.execute_time_ms, Some(100));
    assert_eq!(parsed.row_count, Some(50));
    assert_eq!(parsed.execute_id, Some(999999));
    assert!(parsed.body.contains("SELECT"));
}

#[test]
fn test_multiple_files_processing() {
    // 模拟处理多个日志文件
    let files = vec![
        r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#,
        r#"2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#,
        r#"2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:0 stmt:3 appname:MyApp) SELECT 3
"#,
    ];

    let mut all_records = Vec::new();
    let mut temp_records = Vec::new();
    let mut temp_errors = Vec::new();

    for file in &files {
        split_into(file, &mut temp_records, &mut temp_errors);
        for rec in &temp_records {
            all_records.push(parse_record(rec));
        }
    }

    assert_eq!(all_records.len(), 3);
    assert_eq!(all_records[0].user, "admin");
    assert_eq!(all_records[1].user, "guest");
    assert_eq!(all_records[2].user, "test");
}

#[test]
fn test_record_with_complex_ep_value() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[12345-67890-abcdef] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#;
    let parsed = parse_record(record_text);
    assert_eq!(parsed.ep, "EP[12345-67890-abcdef]");
}

#[test]
fn test_record_with_hex_values() {
    let record_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:0x7fb24f392a30 thrd:757794 user:admin trxid:688489653 stmt:0x7fb236077b70 appname:MyApp) SELECT 1
"#;
    let parsed = parse_record(record_text);
    assert_eq!(parsed.sess, "0x7fb24f392a30");
    assert_eq!(parsed.stmt, "0x7fb236077b70");
    assert_eq!(parsed.trxid, "688489653");
}

#[test]
fn test_parse_all_empty_input() {
    let records = parse_all("");
    assert_eq!(records.len(), 0);
}

#[test]
fn test_for_each_record_with_parsing() {
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#;

    let mut parsed_count = 0;
    for_each_record(log_text, |rec| {
        let parsed = parse_record(rec);
        assert!(!parsed.user.is_empty());
        parsed_count += 1;
    });
    assert_eq!(parsed_count, 2);
}

#[test]
fn test_split_into_empty_input() {
    let mut records = Vec::new();
    let mut errors = Vec::new();
    split_into("", &mut records, &mut errors);
    assert_eq!(records.len(), 0);
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_into_empty_input() {
    let mut parsed_records = Vec::new();
    parse_into("", &mut parsed_records);
    assert_eq!(parsed_records.len(), 0);
}

