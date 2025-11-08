//! 流式处理示例
//!
//! 展示如何使用零分配的流式 API 处理大量日志

use dm_database_parser_sqllog::{for_each_record, parse_records_with};

fn main() {
    // 示例日志文本（可以是从文件读取的大量数据）
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:0 stmt:3 appname:MyApp) SELECT 3
"#;

    println!("=== 方法 1: 流式处理记录（零分配） ===");
    let mut count = 0;
    for_each_record(log_text, |rec| {
        count += 1;
        println!("记录 {}: {}", count, rec.lines().next().unwrap_or(""));
    });
    println!("总共处理了 {} 条记录", count);

    println!("\n=== 方法 2: 流式解析记录 ===");
    let mut parsed_count = 0;
    parse_records_with(log_text, |parsed| {
        parsed_count += 1;
        println!(
            "解析记录 {}: 用户={}, 事务ID={}, 执行时间={:?}ms",
            parsed_count,
            parsed.user,
            parsed.trxid,
            parsed.execute_time_ms
        );
    });
    println!("总共解析了 {} 条记录", parsed_count);
}

