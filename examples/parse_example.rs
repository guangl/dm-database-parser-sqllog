use dm_database_parser_sqllog::{ParseError, parse_record};

fn main() -> Result<(), ParseError> {
    // 示例 1: 单行记录（带 indicators）
    println!("=== 示例 1: 单行完整记录 ===");
    let lines1 = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:myapp ip:::ffff:10.3.100.68) [SEL] select 1 from dual EXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.",
    ];

    let record1 = parse_record(&lines1)?;
    println!("时间戳: {}", record1.ts);
    println!("EP: {}", record1.meta.ep);
    println!("会话ID: {}", record1.meta.sess_id);
    println!("线程ID: {}", record1.meta.thrd_id);
    println!("用户名: {}", record1.meta.username);
    println!("事务ID: {}", record1.meta.trxid);
    println!("语句: {}", record1.meta.statement);
    println!("应用名: {}", record1.meta.appname);
    println!("客户端IP: {}", record1.meta.client_ip);
    println!("SQL内容: {}", record1.body);

    if let Some(indicators) = record1.indicators {
        println!("执行时间: {} ms", indicators.execute_time);
        println!("影响行数: {}", indicators.row_count);
        println!("执行ID: {}", indicators.execute_id);
    }

    // 示例 2: 多行记录（带续行）
    println!("\n=== 示例 2: 多行记录 ===");
    let lines2 = vec![
        "2025-08-12 10:58:15.123 (EP[1] sess:0xABCD1234 thrd:123456 user:TEST_USER trxid:100 stmt:0x12345678 appname:testapp ip:::ffff:192.168.1.100) SELECT u.id, u.name, u.email,",
        "       o.order_id, o.total, o.created_at",
        "FROM users u",
        "JOIN orders o ON u.id = o.user_id",
        "WHERE u.status = 'active' EXECTIME: 15.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 999888.",
    ];

    let record2 = parse_record(&lines2)?;
    println!("时间戳: {}", record2.ts);
    println!("用户名: {}", record2.meta.username);
    println!("SQL内容:\n{}", record2.body);

    if let Some(indicators) = record2.indicators {
        println!("\n执行时间: {} ms", indicators.execute_time);
        println!("影响行数: {}", indicators.row_count);
        println!("执行ID: {}", indicators.execute_id);
    }

    // 示例 3: 没有 indicators 的记录
    println!("\n=== 示例 3: 无 indicators 记录 ===");
    let lines3 = vec![
        "2025-08-12 11:00:00.000 (EP[2] sess:xyz thrd:789 user:ADMIN trxid:200 stmt:abc appname:admin) UPDATE settings SET value = 'new_value' WHERE key = 'config'",
    ];

    let record3 = parse_record(&lines3)?;
    println!("时间戳: {}", record3.ts);
    println!("SQL内容: {}", record3.body);
    println!("包含 indicators: {}", record3.indicators.is_some());

    // 示例 4: 不带 IP 的记录
    println!("\n=== 示例 4: 不带 IP 的记录 ===");
    let lines4 = vec![
        "2025-08-12 11:05:30.555 (EP[3] sess:test123 thrd:111 user:GUEST trxid:0 stmt:stmt1 appname:) SELECT version()",
    ];

    let record4 = parse_record(&lines4)?;
    println!("时间戳: {}", record4.ts);
    println!("用户名: {}", record4.meta.username);
    println!("应用名: '{}'", record4.meta.appname);
    println!("客户端IP: '{}'", record4.meta.client_ip);
    println!("SQL内容: {}", record4.body);

    Ok(())
}
