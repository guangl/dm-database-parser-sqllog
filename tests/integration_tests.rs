//! 集成测试 - 测试完整的端到端场景

use dm_database_parser_sqllog::{
    iter_records_from_file, iter_sqllogs_from_file, parse_records_from_string,
    parse_sqllogs_from_string,
};
use std::io::Write;
use tempfile::NamedTempFile;

/// 测试基本的字符串解析工作流
#[test]
fn test_end_to_end_string_parsing() {
    let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users
2025-08-12 10:57:10.000 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) INSERT INTO logs VALUES (1, 'test')"#;

    // 测试 Record 解析
    let records = parse_records_from_string(log_content);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].lines.len(), 1);
    assert_eq!(records[1].lines.len(), 1);

    // 测试 Sqllog 解析
    let sqllogs = parse_sqllogs_from_string(log_content);
    assert_eq!(sqllogs.len(), 2);

    let first_log = sqllogs[0].as_ref().unwrap();
    assert_eq!(first_log.meta.username, "alice");
    assert_eq!(first_log.meta.ep, 0);
    assert!(first_log.body.contains("SELECT * FROM users"));

    let second_log = sqllogs[1].as_ref().unwrap();
    assert_eq!(second_log.meta.username, "bob");
    assert_eq!(second_log.meta.ep, 1);
    assert!(second_log.body.contains("INSERT INTO logs"));
}

/// 测试多行 SQL 语句的解析
#[test]
fn test_multiline_sql_parsing() {
    let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT u.id,
       u.name,
       u.email
FROM users u
WHERE u.status = 'active'
ORDER BY u.created_at DESC"#;

    let records = parse_records_from_string(log_content);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].lines.len(), 6);

    let sqllogs = parse_sqllogs_from_string(log_content);
    assert_eq!(sqllogs.len(), 1);

    let log = sqllogs[0].as_ref().unwrap();
    assert!(log.body.contains("SELECT u.id"));
    assert!(log.body.contains("FROM users u"));
    assert!(log.body.contains("WHERE u.status"));
}

/// 测试带性能指标的 SQL 解析
#[test]
fn test_sql_with_performance_indicators() {
    let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM large_table EXECTIME: 150.5(ms) ROWCOUNT: 1000(rows) EXEC_ID: 12345."#;

    let sqllogs = parse_sqllogs_from_string(log_content);
    assert_eq!(sqllogs.len(), 1);

    let log = sqllogs[0].as_ref().unwrap();
    assert!(log.has_indicators());
    assert_eq!(log.execute_time(), Some(150.5));
    assert_eq!(log.row_count(), Some(1000));
    assert_eq!(log.execute_id(), Some(12345));
}

/// 测试文件读取和迭代器模式
#[test]
fn test_file_reading_iterator() -> Result<(), Box<dyn std::error::Error>> {
    // 创建临时测试文件
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
    )?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:10.000 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
    )?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:11.000 (EP[2] sess:0x125 thrd:458 user:charlie trxid:791 stmt:1001 appname:app) SELECT 3"
    )?;
    temp_file.flush()?;

    // 测试 Records 迭代器
    let mut record_count = 0;
    for result in iter_records_from_file(temp_file.path())? {
        result?;
        record_count += 1;
    }
    assert_eq!(record_count, 3);

    // 测试 Sqllogs 迭代器
    let mut sqllog_count = 0;
    let mut users = Vec::new();

    for result in iter_sqllogs_from_file(temp_file.path())? {
        let sqllog = result?;
        users.push(sqllog.meta.username.to_string());
        sqllog_count += 1;
    }

    assert_eq!(sqllog_count, 3);
    assert_eq!(users, vec!["alice", "bob", "charlie"]);

    Ok(())
}

/// 测试大文件处理（内存效率）
#[test]
fn test_large_file_processing() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;

    // 生成 1000 条记录
    for i in 0..1000 {
        writeln!(
            temp_file,
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT {}",
            i / 1000,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i
        )?;
    }
    temp_file.flush()?;

    // 使用迭代器处理，不应该占用大量内存
    let mut count = 0;
    for result in iter_sqllogs_from_file(temp_file.path())? {
        result?;
        count += 1;
    }

    assert_eq!(count, 1000);

    Ok(())
}

/// 测试混合内容（有效和无效行）
#[test]
fn test_mixed_valid_invalid_lines() {
    let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
这是一行无效的日志
Another invalid line
2025-08-12 10:57:10.000 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2
More garbage data
2025-08-12 10:57:11.000 (EP[2] sess:0x125 thrd:458 user:charlie trxid:791 stmt:1001 appname:app) SELECT 3"#;

    let records = parse_records_from_string(log_content);
    // 应该只解析出 3 条有效记录
    assert_eq!(records.len(), 3);

    let sqllogs = parse_sqllogs_from_string(log_content);
    // 应该有 3 条成功解析
    let successful = sqllogs.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successful, 3);
}

/// 测试空输入
#[test]
fn test_empty_input() {
    let records = parse_records_from_string("");
    assert_eq!(records.len(), 0);

    let sqllogs = parse_sqllogs_from_string("");
    assert_eq!(sqllogs.len(), 0);
}

/// 测试只有无效行
#[test]
fn test_only_invalid_lines() {
    let log_content = r#"This is not a valid log line
Another invalid line
Yet another one"#;

    let records = parse_records_from_string(log_content);
    assert_eq!(records.len(), 0);

    let sqllogs = parse_sqllogs_from_string(log_content);
    assert_eq!(sqllogs.len(), 0);
}

/// 测试特殊字符处理
#[test]
fn test_special_characters() {
    let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users WHERE name = '中文测试' AND code = 'UTF-8测试'"#;

    let sqllogs = parse_sqllogs_from_string(log_content);
    assert_eq!(sqllogs.len(), 1);

    let log = sqllogs[0].as_ref().unwrap();
    assert!(log.body.contains("中文测试"));
    assert!(log.body.contains("UTF-8测试"));
}

/// 测试边界情况 - 极长的 SQL 语句
#[test]
fn test_very_long_sql() {
    let mut sql = String::from("SELECT ");
    for i in 0..1000 {
        sql.push_str(&format!("col{}, ", i));
    }
    sql.push_str("id FROM table");

    let log_content = format!(
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) {}",
        sql
    );

    let sqllogs = parse_sqllogs_from_string(&log_content);
    assert_eq!(sqllogs.len(), 1);

    let log = sqllogs[0].as_ref().unwrap();
    assert!(log.body.len() > 1000);
}

/// 测试并发场景 - 多线程访问
#[test]
fn test_concurrent_parsing() {
    use std::sync::Arc;
    use std::thread;

    let log_content = Arc::new(
        r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.000 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"#
            .to_string(),
    );

    let mut handles = vec![];

    for _ in 0..10 {
        let content = Arc::clone(&log_content);
        let handle = thread::spawn(move || {
            let records = parse_records_from_string(&content);
            assert_eq!(records.len(), 2);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
