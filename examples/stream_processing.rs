use dm_database_parser_sqllog::{for_each_sqllog, for_each_sqllog_in_string};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 流式处理 Sqllog 示例 ===\n");

    // 示例 1: 从字符串流式处理
    println!("### 示例 1: 从字符串流式解析");
    let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:ALICE trxid:100 stmt:0x285eb060 appname:app1 ip:::ffff:192.168.1.100) SELECT * FROM users WHERE id = 1
2025-08-12 10:57:10.123 (EP[1] sess:0x178ebca1 thrd:757456 user:BOB trxid:101 stmt:0x285eb061 appname:app2) INSERT INTO orders (user_id, total) VALUES (1, 100.50)
2025-08-12 10:57:11.456 (EP[0] sess:0x178ebca2 thrd:757457 user:CHARLIE trxid:102 stmt:0x285eb062 appname:app1 ip:::ffff:192.168.1.101) UPDATE products
SET price = price * 1.1
WHERE category = 'electronics' EXECTIME: 15.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 123456."#;

    let count = for_each_sqllog_in_string(log_content, |sqllog| {
        println!("📊 记录:");
        println!("  时间戳: {}", sqllog.ts);
        println!("  EP: {}", sqllog.meta.ep);
        println!("  会话ID: {}", sqllog.meta.sess_id);
        println!("  线程ID: {}", sqllog.meta.thrd_id);
        println!("  用户: {}", sqllog.meta.username);
        println!("  事务ID: {}", sqllog.meta.trxid);
        println!("  语句: {}", sqllog.meta.statement);
        println!("  应用: {}", sqllog.meta.appname);

        if !sqllog.meta.client_ip.is_empty() {
            println!("  客户端IP: {}", sqllog.meta.client_ip);
        }

        println!("  SQL: {}", sqllog.body.lines().next().unwrap_or(""));

        if let Some(indicators) = &sqllog.indicators {
            println!("  执行时间: {} ms", indicators.execute_time);
            println!("  影响行数: {}", indicators.row_count);
            println!("  执行ID: {}", indicators.execute_id);
        }
        println!();
    })?;

    println!("✅ 共处理 {} 条记录\n", count);

    // 示例 2: 统计分析
    println!("### 示例 2: 统计分析");

    let mut stats = Statistics::new();

    for_each_sqllog_in_string(log_content, |sqllog| {
        stats.total_records += 1;

        // 按用户统计
        *stats
            .user_counts
            .entry(sqllog.meta.username.clone())
            .or_insert(0) += 1;

        // 按EP统计
        *stats.ep_counts.entry(sqllog.meta.ep).or_insert(0) += 1;

        // 统计慢查询
        if let Some(indicators) = &sqllog.indicators {
            if indicators.execute_time > 10.0 {
                stats.slow_queries += 1;
            }
            stats.total_rows += indicators.row_count as u64;
        }
    })?;

    println!("📈 统计结果:");
    println!("  总记录数: {}", stats.total_records);
    println!("  慢查询数: {}", stats.slow_queries);
    println!("  总影响行数: {}", stats.total_rows);
    println!("\n  用户分布:");
    for (user, count) in &stats.user_counts {
        println!("    {}: {} 条", user, count);
    }
    println!("\n  EP 分布:");
    for (ep, count) in &stats.ep_counts {
        println!("    {}: {} 条", ep, count);
    }

    // 示例 3: 从文件流式处理（如果文件存在）
    println!("\n### 示例 3: 从文件流式处理");

    let log_path = "sqllogs/dmsql_OASIS_DB1_20251020_151030.log";
    match File::open(log_path) {
        Ok(file) => {
            let mut file_count = 0;
            let mut alice_queries = 0;

            for_each_sqllog(file, |sqllog| {
                file_count += 1;
                if sqllog.meta.username == "HBTCOMS_V3_PROD" {
                    alice_queries += 1;
                }

                // 只显示前 3 条
                if file_count <= 3 {
                    println!(
                        "  [{}] 用户: {}, SQL: {}",
                        file_count,
                        sqllog.meta.username,
                        sqllog.body.lines().next().unwrap_or("")
                    );
                }
            })?;

            println!("\n  文件中共 {} 条记录", file_count);
            println!("  其中用户 HBTCOMS_V3_PROD 的查询: {} 条", alice_queries);
        }
        Err(_) => {
            println!("  提示: 文件 {} 不存在，跳过此示例", log_path);
        }
    }

    // 示例 4: 过滤和处理特定记录
    println!("\n### 示例 4: 过滤特定条件的记录");

    let mut filtered_count = 0;

    for_each_sqllog_in_string(log_content, |sqllog| {
        // 只处理 EP[0] 的记录
        if sqllog.meta.ep == 0 {
            filtered_count += 1;
            println!(
                "  ✓ EP[0] 记录: 用户={}, 会话={}",
                sqllog.meta.username, sqllog.meta.sess_id
            );
        }
    })?;

    println!("\n  EP[0] 记录数: {}", filtered_count);

    Ok(())
}

// 统计数据结构
struct Statistics {
    total_records: usize,
    slow_queries: usize,
    total_rows: u64,
    user_counts: std::collections::HashMap<String, usize>,
    ep_counts: std::collections::HashMap<u8, usize>,
}

impl Statistics {
    fn new() -> Self {
        Self {
            total_records: 0,
            slow_queries: 0,
            total_rows: 0,
            user_counts: std::collections::HashMap::new(),
            ep_counts: std::collections::HashMap::new(),
        }
    }
}
