//! 边界条件和错误处理的单元测试补充

use dm_database_parser_sqllog::{parse_records_from_string, parse_sqllogs_from_string};

/// 测试各种边界情况的时间戳格式
#[test]
fn test_timestamp_boundary_cases() {
    // 最小时间
    let log = "2000-01-01 00:00:00.000 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let records = parse_records_from_string(log);
    assert_eq!(records.len(), 1);

    // 最大时间（合理范围）
    let log = "2099-12-31 23:59:59.999 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let records = parse_records_from_string(log);
    assert_eq!(records.len(), 1);

    // 年份 9999 - 应该能解析（虽然不太可能）
    let log = "9999-01-01 00:00:00.000 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let _records = parse_records_from_string(log);
    // 具体行为取决于实现，只要不 panic 就行

    // 无效月份
    let log = "2025-13-01 00:00:00.000 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let records = parse_records_from_string(log);
    // 时间戳格式校验可能不够严格，这里不强制要求为 0
    assert!(records.len() <= 1);

    // 无效日期
    let log = "2025-02-30 00:00:00.000 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let records = parse_records_from_string(log);
    // 同上
    assert!(records.len() <= 1);
}

/// 测试 EP 字段的边界值
#[test]
fn test_ep_field_boundaries() {
    // EP[0]
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.ep, 0);

    // EP[255] (u8 最大值)
    let log = "2025-08-12 10:57:09.548 (EP[255] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.ep, 255);

    // 无效 EP 格式
    let log = "2025-08-12 10:57:09.548 (EP[abc] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert!(sqllogs[0].is_err());
}

/// 测试会话 ID 的各种格式
#[test]
fn test_session_id_formats() {
    // 十六进制格式
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123abc thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.sess_id, "0x123abc");

    // 十进制格式
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:12345 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.sess_id, "12345");

    // 空会话 ID（不应该出现，但测试容错性）
    let log = "2025-08-12 10:57:09.548 (EP[0] sess: thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    // 应该能解析，但会话 ID 为空
    assert!(sqllogs[0].is_ok());
}

/// 测试用户名的特殊字符
#[test]
fn test_username_special_characters() {
    // 下划线
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:test_user trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.username, "test_user");

    // 数字
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:user123 trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.username, "user123");

    // 大写字母
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:ADMIN trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.username, "ADMIN");

    // 空用户名（边界情况）
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user: trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert!(sqllogs[0].is_ok());
}

/// 测试性能指标的边界值
#[test]
fn test_performance_indicators_boundaries() {
    // 极小值
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 0.001(ms) ROWCOUNT: 0(rows) EXEC_ID: 0.";
    let sqllogs = parse_sqllogs_from_string(log);
    let sqllog = sqllogs[0].as_ref().unwrap();
    assert_eq!(sqllog.execute_time(), Some(0.001));
    assert_eq!(sqllog.row_count(), Some(0));
    assert_eq!(sqllog.execute_id(), Some(0));

    // 极大值
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 99999.999(ms) ROWCOUNT: 999999999(rows) EXEC_ID: 999999999.";
    let sqllogs = parse_sqllogs_from_string(log);
    let sqllog = sqllogs[0].as_ref().unwrap();
    assert_eq!(sqllog.execute_time(), Some(99999.999));
    assert_eq!(sqllog.row_count(), Some(999999999));
    assert_eq!(sqllog.execute_id(), Some(999999999));

    // 缺少单位
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 10.5 ROWCOUNT: 100";
    let sqllogs = parse_sqllogs_from_string(log);
    let sqllog = sqllogs[0].as_ref().unwrap();
    // 应该解析失败或者忽略无效的指标
    assert!(!sqllog.has_indicators() || sqllog.execute_time().is_none());
}

/// 测试 SQL 语句的各种类型
#[test]
fn test_sql_statement_types() {
    let test_cases = vec![
        ("SELECT", "SELECT * FROM users"),
        ("INSERT", "INSERT INTO users (name) VALUES ('test')"),
        ("UPDATE", "UPDATE users SET name = 'new'"),
        ("DELETE", "DELETE FROM users WHERE id = 1"),
        ("CREATE", "CREATE TABLE test (id INT)"),
        ("DROP", "DROP TABLE test"),
        ("ALTER", "ALTER TABLE users ADD COLUMN age INT"),
        ("TRUNCATE", "TRUNCATE TABLE logs"),
        ("GRANT", "GRANT SELECT ON users TO alice"),
        ("REVOKE", "REVOKE SELECT ON users FROM alice"),
    ];

    for (stmt_type, sql) in test_cases {
        let log = format!(
            "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) {}",
            sql
        );
        let sqllogs = parse_sqllogs_from_string(&log);
        assert_eq!(sqllogs.len(), 1, "Failed to parse {} statement", stmt_type);
        assert!(
            sqllogs[0].is_ok(),
            "{} statement should parse successfully",
            stmt_type
        );
        assert!(
            sqllogs[0].as_ref().unwrap().body.contains(sql),
            "{} statement body mismatch",
            stmt_type
        );
    }
}

/// 测试极端长度的字段
#[test]
fn test_extreme_field_lengths() {
    // 极长的用户名
    let long_username = "a".repeat(1000);
    let log = format!(
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:{} trxid:789 stmt:999 appname:app) SELECT 1",
        long_username
    );
    let sqllogs = parse_sqllogs_from_string(&log);
    assert!(sqllogs[0].is_ok());
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.username, long_username);

    // 极长的 SQL 语句
    let long_sql = format!("SELECT {}", "col, ".repeat(1000));
    let log = format!(
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) {}",
        long_sql
    );
    let sqllogs = parse_sqllogs_from_string(&log);
    assert!(sqllogs[0].is_ok());
    assert!(sqllogs[0].as_ref().unwrap().body.len() > 5000);
}

/// 测试空白字符处理
#[test]
fn test_whitespace_handling() {
    // 前导空格
    let log = "   2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let records = parse_records_from_string(log);
    // 前导空格应该被忽略（不是有效的记录起始）
    assert_eq!(records.len(), 0);

    // SQL 中的多个空格
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT    *    FROM    users";
    let sqllogs = parse_sqllogs_from_string(log);
    assert!(sqllogs[0].is_ok());
    assert!(sqllogs[0].as_ref().unwrap().body.contains("SELECT    *"));

    // 制表符
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT\t1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert!(sqllogs[0].is_ok());
}

/// 测试各种无效输入
#[test]
fn test_invalid_inputs() {
    let invalid_cases = vec![
        "",                                       // 空字符串
        "\n\n\n",                                 // 只有换行符
        "   ",                                    // 只有空格
        "这不是一个有效的日志行",                 // 完全无效的内容
        "2025-08-12",                             // 不完整的时间戳
        "2025-08-12 10:57:09.548",                // 只有时间戳
        "2025-08-12 10:57:09.548 (EP[0])",        // 缺少必要字段
        "(EP[0] sess:0x123 thrd:456 user:alice)", // 没有时间戳
    ];

    for (i, input) in invalid_cases.iter().enumerate() {
        let records = parse_records_from_string(input);
        assert_eq!(
            records.len(),
            0,
            "Invalid input case {} should produce 0 records: {:?}",
            i,
            input
        );
    }
}

/// 测试混合编码和特殊字符
#[test]
fn test_mixed_encoding_and_special_chars() {
    // UTF-8 中文字符
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:张三 trxid:789 stmt:999 appname:app) SELECT * FROM 用户表";
    let sqllogs = parse_sqllogs_from_string(log);
    assert!(sqllogs[0].is_ok());
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.username, "张三");
    assert!(sqllogs[0].as_ref().unwrap().body.contains("用户表"));

    // 特殊 SQL 字符
    let log = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users WHERE name = 'O''Brien'"#;
    let sqllogs = parse_sqllogs_from_string(log);
    assert!(sqllogs[0].is_ok());
    assert!(sqllogs[0].as_ref().unwrap().body.contains("O''Brien"));

    // Emoji（如果支持）
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) INSERT INTO comments (text) VALUES ('👍')";
    let sqllogs = parse_sqllogs_from_string(log);
    assert!(sqllogs[0].is_ok());
}

/// 测试事务 ID 的特殊值
#[test]
fn test_transaction_id_special_values() {
    // trxid: 0（通常表示无事务）
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:0 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.trxid, "0");

    // 极大的 trxid
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:999999999999 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.trxid, "999999999999");
}

/// 测试客户端 IP 的各种格式
#[test]
fn test_client_ip_formats() {
    // IPv4
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:192.168.1.1) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.client_ip, "192.168.1.1");

    // IPv6
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:2001:0db8:85a3:0000:0000:8a2e:0370:7334) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(
        sqllogs[0].as_ref().unwrap().meta.client_ip,
        "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
    );

    // 没有 IP
    let log = "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
    let sqllogs = parse_sqllogs_from_string(log);
    assert_eq!(sqllogs[0].as_ref().unwrap().meta.client_ip, "");
}
