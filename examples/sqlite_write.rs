//! SQLite 写入示例
//!
//! 展示如何将解析的 sqllog 记录写入 SQLite 数据库

use dm_database_parser_sqllog::{for_each_record, parse_record};
use rusqlite::{Connection, Result, params};
use std::time::Instant;

fn main() -> Result<()> {
    println!("=== SQLite 写入示例 ===\n");

    // 示例日志数据
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:100 stmt:1 appname:MyApp ip:192.168.1.10) SELECT * FROM users WHERE id = 1
EXECTIME: 15ms ROWCOUNT: 1 EXEC_ID: 1001
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:101 stmt:2 appname:MyApp ip:192.168.1.20) INSERT INTO logs (message) VALUES ('test log')
EXECTIME: 5ms ROWCOUNT: 1 EXEC_ID: 1002
2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:102 stmt:3 appname:MyApp ip:192.168.1.30) UPDATE products SET price = 99.99 WHERE id = 5
EXECTIME: 8ms ROWCOUNT: 1 EXEC_ID: 1003
2025-08-12 10:57:12.789 (EP[0] sess:4 thrd:4 user:admin trxid:103 stmt:4 appname:MyApp ip:192.168.1.10) DELETE FROM temp_data WHERE created < '2025-01-01'
EXECTIME: 120ms ROWCOUNT: 450 EXEC_ID: 1004
"#;

    // 创建内存数据库（也可以使用文件：Connection::open("sqllog.db")?）
    // let mut conn = Connection::open_in_memory()?;
    let mut conn = Connection::open("sqllog.db")?; // 使用文件数据库

    // 创建表
    println!("创建数据库表...");
    conn.execute(
        "CREATE TABLE sqllog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT NOT NULL,
            ep TEXT,
            sess TEXT,
            thrd TEXT,
            username TEXT,
            trxid TEXT,
            stmt TEXT,
            appname TEXT,
            ip TEXT,
            body TEXT,
            execute_time_ms INTEGER,
            row_count INTEGER,
            execute_id INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    println!("✓ 表创建成功\n");

    // === 方法 1: 逐条插入（慢，不推荐） ===
    println!("【方法 1】逐条插入（无事务）");
    println!("----------------------------------------");
    let start = Instant::now();
    let mut count = 0;

    for_each_record(log_text, |rec| {
        let parsed = parse_record(rec);
        conn.execute(
            "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                parsed.ts,
                parsed.ep,
                parsed.sess,
                parsed.thrd,
                parsed.user,
                parsed.trxid,
                parsed.stmt,
                parsed.appname,
                parsed.ip,
                parsed.body,
                parsed.execute_time_ms.map(|v| v as i64),
                parsed.row_count.map(|v| v as i64),
                parsed.execute_id.map(|v| v as i64),
            ],
        )
        .unwrap();
        count += 1;
    });

    println!("插入了 {} 条记录", count);
    println!("耗时: {:?}\n", start.elapsed());

    // 清空表以便演示下一种方法
    conn.execute("DELETE FROM sqllog", [])?;

    // === 方法 2: 使用事务批量插入（推荐） ===
    println!("【方法 2】使用事务批量插入（推荐）");
    println!("----------------------------------------");
    let start = Instant::now();
    count = 0;

    // 开始事务
    let tx = conn.transaction()?;
    {
        for_each_record(log_text, |rec| {
            let parsed = parse_record(rec);
            tx.execute(
                "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    parsed.ts,
                    parsed.ep,
                    parsed.sess,
                    parsed.thrd,
                    parsed.user,
                    parsed.trxid,
                    parsed.stmt,
                    parsed.appname,
                    parsed.ip,
                    parsed.body,
                    parsed.execute_time_ms.map(|v| v as i64),
                    parsed.row_count.map(|v| v as i64),
                    parsed.execute_id.map(|v| v as i64),
                ],
            )
            .unwrap();
            count += 1;
        });
    }
    tx.commit()?;

    println!("插入了 {} 条记录", count);
    println!("耗时: {:?}\n", start.elapsed());

    // === 方法 3: 准备语句 + 事务（最优性能） ===
    conn.execute("DELETE FROM sqllog", [])?;

    println!("【方法 3】准备语句 + 事务（最优）");
    println!("----------------------------------------");
    let start = Instant::now();
    count = 0;

    let tx = conn.transaction()?;
    let mut stmt = tx.prepare(
        "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?;

    for_each_record(log_text, |rec| {
        let parsed = parse_record(rec);
        stmt.execute(params![
            parsed.ts,
            parsed.ep,
            parsed.sess,
            parsed.thrd,
            parsed.user,
            parsed.trxid,
            parsed.stmt,
            parsed.appname,
            parsed.ip,
            parsed.body,
            parsed.execute_time_ms.map(|v| v as i64),
            parsed.row_count.map(|v| v as i64),
            parsed.execute_id.map(|v| v as i64),
        ])
        .unwrap();
        count += 1;
    });

    drop(stmt);
    tx.commit()?;

    println!("插入了 {} 条记录", count);
    println!("耗时: {:?}\n", start.elapsed());

    // === 查询数据 ===
    println!("【查询结果】");
    println!("----------------------------------------");

    // 统计每个用户的操作次数
    let mut stmt = conn.prepare(
        "SELECT username, COUNT(*) as count, AVG(execute_time_ms) as avg_time
         FROM sqllog
         GROUP BY username
         ORDER BY count DESC",
    )?;

    let user_stats = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<f64>>(2)?,
        ))
    })?;

    println!("\n用户操作统计:");
    for stat in user_stats {
        let (username, count, avg_time) = stat?;
        println!(
            "  用户: {:10} 操作次数: {:3}  平均执行时间: {:.2}ms",
            username,
            count,
            avg_time.unwrap_or(0.0)
        );
    }

    // 查询慢查询（执行时间 > 10ms）
    println!("\n慢查询记录（执行时间 > 10ms）:");
    let mut stmt = conn.prepare(
        "SELECT ts, username, body, execute_time_ms
         FROM sqllog
         WHERE execute_time_ms > 10
         ORDER BY execute_time_ms DESC",
    )?;

    let slow_queries = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    for query in slow_queries {
        let (ts, username, body, exec_time) = query?;
        println!("  [{}] 用户: {} ({}ms)", ts, username, exec_time);
        println!("    SQL: {}", body.lines().next().unwrap_or(""));
    }

    // 统计总记录数
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM sqllog", [], |row| row.get(0))?;
    println!("\n数据库总记录数: {}", total);

    println!("\n✓ 完成");

    Ok(())
}
