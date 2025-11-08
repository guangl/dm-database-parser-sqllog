/// 示例：解析 sqllog 文件并写入 SQLite 数据库
///
/// 这个示例演示如何：
/// 1. 使用迭代器模式读取 sqllog 文件
/// 2. 将解析后的数据写入 SQLite 数据库
/// 3. 处理大文件时避免内存溢出
///
/// 运行方式：
/// ```bash
/// cargo run --example sqlite_import -- <sqllog_file_path>
/// ```
use dm_database_parser_sqllog::iter_sqllogs_from_file;
use rusqlite::{Connection, Result, params};
use std::env;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 获取命令行参数
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <sqllog_file_path>", args[0]);
        eprintln!("示例: {} sqllogs/example.sqllog", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    println!("📁 读取文件: {}", file_path);

    // 创建 SQLite 数据库
    let conn = Connection::open("sqllogs.db")?;
    println!("✅ 创建数据库: sqllogs.db");

    // 创建表结构
    create_tables(&conn)?;
    println!("✅ 创建表结构");

    // 开始解析和导入
    let start = Instant::now();
    let (success_count, error_count) = import_sqllogs(&conn, file_path)?;
    let duration = start.elapsed();

    // 输出统计信息
    println!("\n📊 导入统计:");
    println!("  ✅ 成功: {} 条", success_count);
    println!("  ❌ 失败: {} 条", error_count);
    println!("  ⏱️  耗时: {:.2?}", duration);
    println!(
        "  🚀 速度: {:.0} 条/秒",
        success_count as f64 / duration.as_secs_f64()
    );

    // 查询示例
    println!("\n📋 数据库查询示例:");
    query_examples(&conn)?;

    Ok(())
}

/// 创建数据库表结构
fn create_tables(conn: &Connection) -> Result<()> {
    // 主表：SQL 日志记录
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sqllogs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp       TEXT NOT NULL,
            ep              INTEGER NOT NULL,
            session_id      TEXT NOT NULL,
            thread_id       TEXT NOT NULL,
            username        TEXT NOT NULL,
            transaction_id  TEXT NOT NULL,
            statement_id    TEXT NOT NULL,
            appname         TEXT NOT NULL,
            client_ip       TEXT,
            sql_body        TEXT NOT NULL,
            execute_time    REAL,
            row_count       INTEGER,
            exec_id         TEXT,
            created_at      DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // 创建索引以加速查询
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timestamp ON sqllogs(timestamp)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_username ON sqllogs(username)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_execute_time ON sqllogs(execute_time)",
        [],
    )?;

    Ok(())
}

/// 导入 sqllog 数据到数据库
fn import_sqllogs(
    conn: &Connection,
    file_path: &str,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let mut success_count = 0;
    let mut error_count = 0;

    // 使用事务批量插入，提升性能
    let tx = conn.unchecked_transaction()?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO sqllogs (
                timestamp, ep, session_id, thread_id, username,
                transaction_id, statement_id, appname, client_ip,
                sql_body, execute_time, row_count, exec_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;

        // 使用迭代器逐条处理，避免内存溢出
        for (index, result) in iter_sqllogs_from_file(file_path)?.enumerate() {
            match result {
                Ok(sqllog) => {
                    // 插入数据库
                    stmt.execute(params![
                        sqllog.ts,
                        sqllog.meta.ep,
                        sqllog.meta.sess_id,
                        sqllog.meta.thrd_id,
                        sqllog.meta.username,
                        sqllog.meta.trxid,
                        sqllog.meta.statement,
                        sqllog.meta.appname,
                        if sqllog.meta.client_ip.is_empty() {
                            None::<String>
                        } else {
                            Some(sqllog.meta.client_ip.clone())
                        },
                        sqllog.body,
                        sqllog.execute_time(),
                        sqllog.row_count(),
                        sqllog.indicators.as_ref().map(|i| i.execute_id.to_string()),
                    ])?;

                    success_count += 1;

                    // 每 1000 条显示进度
                    if (index + 1) % 1000 == 0 {
                        print!("\r⏳ 已处理: {} 条", index + 1);
                        use std::io::{self, Write};
                        io::stdout().flush().unwrap();
                    }
                }
                Err(e) => {
                    error_count += 1;
                    eprintln!("\n❌ 解析错误 (第 {} 行): {}", index + 1, e);
                }
            }
        }
    }

    // 提交事务
    tx.commit()?;
    println!("\r✅ 提交事务完成                    ");

    Ok((success_count, error_count))
}

/// 查询示例
fn query_examples(conn: &Connection) -> Result<()> {
    // 1. 统计总记录数
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM sqllogs", [], |row| row.get(0))?;
    println!("  📝 总记录数: {}", total);

    // 2. 统计每个用户的查询数
    println!("\n  👥 用户查询统计 (Top 5):");
    let mut stmt = conn.prepare(
        "SELECT username, COUNT(*) as cnt
         FROM sqllogs
         GROUP BY username
         ORDER BY cnt DESC
         LIMIT 5",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for (i, row) in rows.enumerate() {
        let (username, count) = row?;
        println!("     {}. {}: {} 条", i + 1, username, count);
    }

    // 3. 查找慢查询 (执行时间 > 100ms)
    println!("\n  🐌 慢查询 (执行时间 > 100ms, Top 5):");
    let mut stmt = conn.prepare(
        "SELECT username, execute_time, SUBSTR(sql_body, 1, 50) as sql_preview
         FROM sqllogs
         WHERE execute_time > 100.0
         ORDER BY execute_time DESC
         LIMIT 5",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for (i, row) in rows.enumerate() {
        let (username, exec_time, sql) = row?;
        println!(
            "     {}. {}: {:.2}ms - {}...",
            i + 1,
            username,
            exec_time,
            sql
        );
    }

    // 4. 统计平均执行时间
    let avg_time: f64 = conn.query_row(
        "SELECT AVG(execute_time) FROM sqllogs WHERE execute_time IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    println!("\n  ⏱️  平均执行时间: {:.2}ms", avg_time);

    // 5. 统计 SQL 类型分布
    println!("\n  📊 SQL 类型分布:");
    let mut stmt = conn.prepare(
        "SELECT
            CASE
                WHEN sql_body LIKE 'SELECT%' THEN 'SELECT'
                WHEN sql_body LIKE 'INSERT%' THEN 'INSERT'
                WHEN sql_body LIKE 'UPDATE%' THEN 'UPDATE'
                WHEN sql_body LIKE 'DELETE%' THEN 'DELETE'
                ELSE 'OTHER'
            END as sql_type,
            COUNT(*) as cnt
         FROM sqllogs
         GROUP BY sql_type
         ORDER BY cnt DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for row in rows {
        let (sql_type, count) = row?;
        println!("     {}: {} 条", sql_type, count);
    }

    Ok(())
}
