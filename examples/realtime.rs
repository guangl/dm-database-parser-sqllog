//! 实时解析示例
//!
//! 展示如何使用实时解析功能监控和处理持续增长的 sqllog 文件

use dm_database_parser_sqllog::realtime::{ParserConfig, RealtimeParser};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 实时 Sqllog 解析器示例 ===\n");

    // 创建临时测试文件
    let test_file = "test_sqllog_realtime.log";
    let test_path = PathBuf::from(test_file);

    // 初始化测试文件
    {
        let mut file = File::create(&test_path)?;
        writeln!(
            file,
            "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:100 stmt:1 appname:MyApp) SELECT * FROM users WHERE id = 1"
        )?;
        writeln!(
            file,
            "2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:101 stmt:2 appname:MyApp) INSERT INTO logs VALUES ('test')"
        )?;
        file.flush()?;
    }

    println!("✓ 创建测试文件: {}\n", test_file);

    // === 示例 1: 增量解析 ===
    println!("【示例 1】增量解析模式");
    println!("----------------------------------------");

    let config = ParserConfig {
        file_path: test_path.clone(),
        poll_interval: Duration::from_millis(500),
        buffer_size: 4096,
    };

    let mut parser = RealtimeParser::new(config)?;

    // 第一次解析
    println!("第一次解析（初始记录）:");
    let count1 = parser.parse_new_records(|parsed| {
        println!(
            "  → [{}] 用户: {:<10} 事务ID: {:<5} SQL: {}",
            parsed.ts,
            parsed.user,
            parsed.trxid,
            parsed.body.lines().next().unwrap_or("")
        );
    })?;
    println!("解析了 {} 条记录\n", count1);

    // 模拟文件追加新内容
    println!("模拟新日志写入...");
    thread::sleep(Duration::from_millis(500));
    {
        let mut file = OpenOptions::new().append(true).open(&test_path)?;
        writeln!(
            file,
            "2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:102 stmt:3 appname:MyApp) UPDATE products SET price = 99.99 WHERE id = 5"
        )?;
        writeln!(
            file,
            "2025-08-12 10:57:12.789 (EP[0] sess:4 thrd:4 user:admin trxid:103 stmt:4 appname:MyApp) DELETE FROM temp_data WHERE created < '2025-01-01'"
        )?;
        file.flush()?;
    }

    // 第二次解析（仅新增记录）
    println!("\n第二次解析（仅新增记录）:");
    let count2 = parser.parse_new_records(|parsed| {
        println!(
            "  → [{}] 用户: {:<10} 事务ID: {:<5} SQL: {}",
            parsed.ts,
            parsed.user,
            parsed.trxid,
            parsed.body.lines().next().unwrap_or("")
        );
    })?;
    println!("解析了 {} 条新记录\n", count2);

    println!("当前文件位置: {} 字节\n", parser.position());

    // === 示例 2: 从头完整解析 ===
    println!("【示例 2】从头完整解析");
    println!("----------------------------------------");

    let mut count_all = 0;
    let total = parser.parse_all(|parsed| {
        count_all += 1;
        println!(
            "  #{} 用户: {:<10} 应用: {:<10} SQL 类型: {}",
            count_all,
            parsed.user,
            parsed.appname,
            if parsed.body.trim().starts_with("SELECT") {
                "查询"
            } else if parsed.body.trim().starts_with("INSERT") {
                "插入"
            } else if parsed.body.trim().starts_with("UPDATE") {
                "更新"
            } else if parsed.body.trim().starts_with("DELETE") {
                "删除"
            } else {
                "其他"
            }
        );
    })?;
    println!("总共解析了 {} 条记录\n", total);

    // === 示例 3: 持续监听（演示 5 秒） ===
    println!("【示例 3】持续监听模式（演示 5 秒）");
    println!("----------------------------------------");
    println!("启动实时监听，每 500ms 检查一次新记录...");

    // 重置解析器以从当前文件末尾开始
    parser.reset();
    parser.seek_to(std::fs::metadata(&test_path)?.len());

    // 在后台线程中持续写入新记录
    let test_path_clone = test_path.clone();
    let writer_thread = thread::spawn(move || {
        for i in 5..=8 {
            thread::sleep(Duration::from_secs(1));
            if let Ok(mut file) = OpenOptions::new().append(true).open(&test_path_clone) {
                let _ = writeln!(
                    file,
                    "2025-08-12 10:57:{:02}.000 (EP[0] sess:{} thrd:{} user:monitor trxid:{} stmt:{} appname:Monitor) SELECT COUNT(*) FROM table_{}",
                    12 + i,
                    i,
                    i,
                    104 + i - 5,
                    i,
                    i
                );
                let _ = file.flush();
            }
        }
    });

    // 监听 5 秒
    let start = std::time::Instant::now();
    let mut watch_count = 0;
    while start.elapsed() < Duration::from_secs(5) {
        let count = parser.parse_new_records(|parsed| {
            watch_count += 1;
            println!(
                "  ★ 实时捕获 [{}] 用户: {} SQL: {}",
                parsed.ts,
                parsed.user,
                parsed.body.lines().next().unwrap_or("")
            );
        })?;

        if count == 0 {
            thread::sleep(Duration::from_millis(500));
        }
    }

    println!("\n监听期间捕获了 {} 条新记录", watch_count);

    // 等待写入线程结束
    writer_thread.join().unwrap();

    // 清理测试文件
    println!("\n清理测试文件...");
    std::fs::remove_file(&test_path)?;
    println!("✓ 完成");

    Ok(())
}
