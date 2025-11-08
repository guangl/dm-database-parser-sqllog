//! 简单的实时监控示例
//!
//! 演示最基本的实时监控用法
//!
//! 运行方式:
//! ```bash
//! cargo run --example simple_realtime --features realtime
//! ```

use dm_database_parser_sqllog::realtime::RealtimeSqllogParser;

fn main() {
    println!("开始监控 sqllog.txt...");
    println!("请在另一个终端向该文件追加内容，例如:");
    println!("  echo '2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1' >> sqllog.txt");
    println!();

    // 创建解析器
    let parser = RealtimeSqllogParser::new("sqllog.txt")
        .expect("无法创建解析器，请确保 sqllog.txt 文件存在");

    // 启动监控 (会一直运行，按 Ctrl+C 停止)
    parser
        .watch(|sqllog| {
            println!("[{}] {} - {}", 
                sqllog.ts, 
                sqllog.meta.username, 
                sqllog.body.lines().next().unwrap_or(""));
        })
        .expect("监控失败");
}
