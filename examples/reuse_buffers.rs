//! 重用缓冲区示例
//!
//! 展示如何在循环中重用缓冲区以避免重复分配

use dm_database_parser_sqllog::{parse_into, split_into};

fn main() {
    // 模拟处理多个日志文件
    let log_files = vec![
        r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
"#,
        r#"2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
"#,
        r#"2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:0 stmt:3 appname:MyApp) SELECT 3
"#,
    ];

    // 重用这些缓冲区，避免每次处理都分配新的 Vec
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut parsed_records = Vec::new();

    println!("=== 处理多个日志文件（重用缓冲区） ===");
    
    for (i, log_text) in log_files.iter().enumerate() {
        // 清空并重用缓冲区
        split_into(log_text, &mut records, &mut errors);
        
        println!("\n文件 {}: 找到 {} 条记录，{} 条错误", i + 1, records.len(), errors.len());
        
        // 解析记录，重用 parsed_records 缓冲区
        parse_into(log_text, &mut parsed_records);
        
        for parsed in &parsed_records {
            println!(
                "  用户: {}, 事务ID: {}, 执行时间: {:?}ms",
                parsed.user, parsed.trxid, parsed.execute_time_ms
            );
        }
    }
    
    println!("\n缓冲区在整个处理过程中被重用，避免了重复分配");
}

