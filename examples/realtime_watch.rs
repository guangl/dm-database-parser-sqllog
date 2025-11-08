//! 实时监控 SQL 日志文件示例
//!
//! 演示如何使用 RealtimeSqllogParser 实时监控和解析 SQL 日志文件
//!
//! 运行方式:
//! ```bash
//! cargo run --example realtime_watch --features realtime
//! ```

#[cfg(feature = "realtime")]
use dm_database_parser_sqllog::realtime::RealtimeSqllogParser;
use std::env;
use std::time::Duration;

fn main() {
    // 从命令行参数获取文件路径
    let args: Vec<String> = env::args().collect();

    let file_path = if args.len() > 1 {
        &args[1]
    } else {
        println!(
            "用法: cargo run --example realtime_watch --features realtime <文件路径> [监控秒数]"
        );
        println!("示例: cargo run --example realtime_watch --features realtime sqllog.txt 60");
        println!("\n使用默认值: sqllog.txt");
        "sqllog.txt"
    };

    let duration_secs = if args.len() > 2 {
        args[2].parse::<u64>().unwrap_or(60)
    } else {
        60
    };

    println!("╔════════════════════════════════════════════════════╗");
    println!("║       实时 SQL 日志监控器                          ║");
    println!("╚════════════════════════════════════════════════════╝");
    println!();
    println!("📁 监控文件: {}", file_path);
    println!("⏱️  监控时长: {} 秒", duration_secs);
    println!("🔍 开始监控...");
    println!();

    #[cfg(feature = "realtime")]
    // 创建解析器 - 从当前位置开始（默认从文件末尾）
    let parser = match RealtimeSqllogParser::new(file_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ 无法创建解析器: {}", e);
            eprintln!("\n提示: 请确保文件 '{}' 存在", file_path);
            return;
        }
    };

    // 如果想从文件开头解析所有内容，可以使用:
    // let parser = parser.from_beginning().unwrap();

    let mut count = 0;

    // 启动监控
    #[cfg(feature = "realtime")]
    let result = parser.watch_for(Duration::from_secs(duration_secs), |sqllog| {
        count += 1;
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📝 记录 #{}", count);
        println!("🕐 时间戳:  {}", sqllog.ts);
        println!("👤 用户:    {}", sqllog.meta.username);
        println!("🔢 EP:      {}", sqllog.meta.ep);
        println!("🔑 会话ID:  {}", sqllog.meta.sess_id);
        println!("🧵 线程ID:  {}", sqllog.meta.thrd_id);
        println!("📦 事务ID:  {}", sqllog.meta.trxid);
        println!("📋 语句ID:  {}", sqllog.meta.stmt_id);
        println!("📱 应用名:  {}", sqllog.meta.appname);

        if let Some(ref ip) = sqllog.meta.client_ip {
            println!("🌐 客户端IP: {}", ip);
        }

        println!("\n💾 SQL 语句:");
        println!("{}", sqllog.body);

        if let Some(ref indicators) = sqllog.indicators {
            println!("\n📊 性能指标:");
            println!("  ⏱️  执行时间: {} ms", indicators.exectime);
            println!("  📊 影响行数: {}", indicators.rowcount);
            println!("  🔢 执行ID:   {}", indicators.exec_id);
        }
        println!();
    });

    #[cfg(feature = "realtime")]
    match result {
        Ok(_) => {
            println!("\n✅ 监控完成");
            println!("📊 共处理 {} 条日志记录", count);
        }
        Err(e) => {
            eprintln!("\n❌ 监控出错: {}", e);
        }
    }
}
