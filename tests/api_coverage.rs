/// API 覆盖率测试
///
/// 专门用于测试 api.rs 中的所有公开 API，确保覆盖率
use dm_database_parser_sqllog::*;
use std::io::Write;
use tempfile::NamedTempFile;

/// 测试 for_each_sqllog_in_string 函数
#[test]
fn test_for_each_sqllog_in_string() {
    let log = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:09.549 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2
2025-08-12 10:57:09.550 (EP[2] sess:0x125 thrd:458 user:charlie trxid:791 stmt:1001 appname:app) SELECT 3"#;

    let mut count = 0;
    let result = for_each_sqllog_in_string(log, |sqllog| {
        count += 1;
        assert!(sqllog.body.starts_with("SELECT"));
    });

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);
    assert_eq!(count, 3);
}

/// 测试 for_each_sqllog_in_string 空输入
#[test]
fn test_for_each_sqllog_in_string_empty() {
    let count = for_each_sqllog_in_string("", |_| {}).unwrap();
    assert_eq!(count, 0);
}

/// 测试 for_each_sqllog_in_string 无效输入
#[test]
fn test_for_each_sqllog_in_string_invalid() {
    let log = "invalid line 1\ninvalid line 2";
    let count = for_each_sqllog_in_string(log, |_| {}).unwrap();
    assert_eq!(count, 0);
}

/// 测试 for_each_sqllog_from_file 函数
#[test]
fn test_for_each_sqllog_from_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
    )?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.549 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
    )?;
    temp_file.flush()?;

    let mut usernames = Vec::new();
    let count = for_each_sqllog_from_file(temp_file.path(), |sqllog| {
        usernames.push(sqllog.meta.username.clone());
    })?;

    assert_eq!(count, 2);
    assert_eq!(usernames, vec!["alice", "bob"]);
    Ok(())
}

/// 测试 for_each_sqllog_from_file 文件不存在
#[test]
fn test_for_each_sqllog_from_file_not_found() {
    let result = for_each_sqllog_from_file("nonexistent_file.log", |_| {});
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, ParseError::FileNotFound(_)));
    }
}

/// 测试 for_each_sqllog 函数
#[test]
fn test_for_each_sqllog() {
    let log = b"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n";
    let reader = std::io::Cursor::new(log);

    let mut ep_values = Vec::new();
    let count = for_each_sqllog(reader, |sqllog| {
        ep_values.push(sqllog.meta.ep);
    })
    .unwrap();

    assert_eq!(count, 1);
    assert_eq!(ep_values, vec![0]);
}

/// 测试 parse_records_from_file 函数（一次性加载模式）
#[test]
fn test_parse_records_from_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
    )?;
    writeln!(temp_file, "invalid line here")?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.549 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
    )?;
    temp_file.flush()?;

    let (records, _errors) = parse_records_from_file(temp_file.path())?;

    // 无效行会被自动跳过
    assert_eq!(records.len(), 2);
    Ok(())
}

/// 测试 parse_records_from_file 文件不存在
#[test]
fn test_parse_records_from_file_not_found() {
    let result = parse_records_from_file("nonexistent.log");
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, ParseError::FileNotFound(_)));
    }
}

/// 测试 parse_records_from_file 空文件
#[test]
fn test_parse_records_from_file_empty() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;

    let (records, errors) = parse_records_from_file(temp_file.path())?;

    assert_eq!(records.len(), 0);
    assert_eq!(errors.len(), 0);
    Ok(())
}

/// 测试 parse_sqllogs_from_file 函数（一次性加载模式）
#[test]
fn test_parse_sqllogs_from_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
    )?;
    writeln!(temp_file, "invalid line")?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.549 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
    )?;
    temp_file.flush()?;

    let (sqllogs, _errors) = parse_sqllogs_from_file(temp_file.path())?;

    // 无效行会被自动跳过
    assert_eq!(sqllogs.len(), 2);
    assert_eq!(sqllogs[0].meta.username, "alice");
    assert_eq!(sqllogs[1].meta.username, "bob");
    Ok(())
}

/// 测试 parse_sqllogs_from_file 文件不存在
#[test]
fn test_parse_sqllogs_from_file_not_found() {
    let result = parse_sqllogs_from_file("nonexistent.log");
    assert!(result.is_err());
}

/// 测试 parse_sqllogs_from_file 空文件
#[test]
fn test_parse_sqllogs_from_file_empty() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;

    let (sqllogs, errors) = parse_sqllogs_from_file(temp_file.path())?;

    assert_eq!(sqllogs.len(), 0);
    assert_eq!(errors.len(), 0);
    Ok(())
}

/// 测试 iter_records_from_file 文件不存在错误
#[test]
fn test_iter_records_from_file_not_found() {
    let result = iter_records_from_file("nonexistent.log");
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, ParseError::FileNotFound(_)));
    }
}

/// 测试 iter_sqllogs_from_file 文件不存在错误
#[test]
fn test_iter_sqllogs_from_file_not_found() {
    let result = iter_sqllogs_from_file("nonexistent.log");
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, ParseError::FileNotFound(_)));
    }
}

/// 测试 parse_records_from_file 处理多行记录
#[test]
fn test_parse_records_from_file_multiline() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *"
    )?;
    writeln!(temp_file, "FROM users")?;
    writeln!(temp_file, "WHERE id = 1")?;
    temp_file.flush()?;

    let (records, errors) = parse_records_from_file(temp_file.path())?;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].lines.len(), 3);
    assert_eq!(errors.len(), 0);
    Ok(())
}

/// 测试 parse_sqllogs_from_file 处理多行 SQL
#[test]
fn test_parse_sqllogs_from_file_multiline() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *"
    )?;
    writeln!(temp_file, "FROM users")?;
    writeln!(temp_file, "WHERE id = 1")?;
    temp_file.flush()?;

    let (sqllogs, _) = parse_sqllogs_from_file(temp_file.path())?;

    assert_eq!(sqllogs.len(), 1);
    // 多行 SQL 会被合并，但可能包含换行符
    assert!(sqllogs[0].body.contains("SELECT *"));
    assert!(sqllogs[0].body.contains("FROM users"));
    assert!(sqllogs[0].body.contains("WHERE id = 1"));
    Ok(())
}

/// 测试 for_each_sqllog_from_file 处理多行 SQL
#[test]
fn test_for_each_sqllog_from_file_multiline() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) INSERT INTO t"
    )?;
    writeln!(temp_file, "VALUES (1, 2,")?;
    writeln!(temp_file, "3, 4)")?;
    temp_file.flush()?;

    let mut bodies = Vec::new();
    let count = for_each_sqllog_from_file(temp_file.path(), |sqllog| {
        bodies.push(sqllog.body.clone());
    })?;

    assert_eq!(count, 1);
    assert!(bodies[0].contains("INSERT INTO t"));
    assert!(bodies[0].contains("VALUES (1, 2,"));
    assert!(bodies[0].contains("3, 4)"));
    Ok(())
}

/// 测试 deprecated 的 records_from_file 函数
#[test]
#[allow(deprecated)]
fn test_deprecated_records_from_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
    )?;
    temp_file.flush()?;

    let parser = records_from_file(temp_file.path())?;
    let records: Vec<_> = parser.filter_map(Result::ok).collect();

    assert_eq!(records.len(), 1);
    Ok(())
}

/// 测试 deprecated 的 sqllogs_from_file 函数
#[test]
#[allow(deprecated)]
fn test_deprecated_sqllogs_from_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
    )?;
    temp_file.flush()?;

    let parser = sqllogs_from_file(temp_file.path())?;
    let sqllogs: Vec<_> = parser.filter_map(Result::ok).collect();

    assert_eq!(sqllogs.len(), 1);
    Ok(())
}

/// 测试 for_each_sqllog_in_string 处理带性能指标的记录
#[test]
fn test_for_each_sqllog_in_string_with_indicators() {
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME:123.0 ROWCOUNT:10 EXEC_ID:abc123";

    let mut has_indicators = false;
    let count = for_each_sqllog_in_string(log, |sqllog| {
        // 检查 body 中是否包含性能指标
        if sqllog.body.contains("EXECTIME:") {
            has_indicators = true;
        }
        // execute_time() 方法从 Sqllog 结构体的字段中读取，而不是从 body 中解析
        // 所以这里只检查 body 是否包含指标信息
    })
    .unwrap();

    assert_eq!(count, 1);
    assert!(has_indicators);
}

/// 测试 parse_records_from_file 处理混合有效和无效行
#[test]
fn test_parse_records_from_file_mixed() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
    )?;
    writeln!(temp_file, "garbage line 1")?;
    writeln!(temp_file, "garbage line 2")?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.549 (EP[1] sess:0x124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
    )?;
    writeln!(temp_file, "more garbage")?;
    temp_file.flush()?;

    let (records, _errors) = parse_records_from_file(temp_file.path())?;

    // 无效行会被自动跳过
    assert_eq!(records.len(), 2);
    Ok(())
}
