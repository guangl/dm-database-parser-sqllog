//! 基本使用示例
//!
//! 展示如何使用 dm-database-parser-sqllog 库解析日志文件

use dm_database_parser_sqllog::{parse_record, split_by_ts_records_with_errors};

fn main() {
    // 示例日志文本
    let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:688489653 stmt:0x7fb236077b70 appname: ip:::ffff:10.3.100.68) EXECTIME: 0ms ROWCOUNT: 1 EXEC_ID: 289655185
SELECT * FROM users WHERE id = 1
2025-08-12 10:57:09.562 (EP[0] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:0 stmt:NULL appname:) TRX: START
BEGIN TRANSACTION
"#;

    // 方法 1: 拆分并解析所有记录
    println!("=== 方法 1: 拆分并解析所有记录 ===");
    let (records, errors) = split_by_ts_records_with_errors(log_text);
    
    println!("找到 {} 条记录，{} 条前导错误", records.len(), errors.len());
    
    for (i, record) in records.iter().enumerate() {
        let parsed = parse_record(record);
        println!("\n记录 {}:", i + 1);
        println!("  时间戳: {}", parsed.ts);
        println!("  用户: {}", parsed.user);
        println!("  事务ID: {}", parsed.trxid);
        println!("  执行时间: {:?}ms", parsed.execute_time_ms);
        println!("  行数: {:?}", parsed.row_count);
        println!("  执行ID: {:?}", parsed.execute_id);
        println!("  主体:");
        for (line_num, line) in parsed.body.lines().enumerate() {
            println!("    {}: {}", line_num + 1, line);
        }
    }
    
    if !errors.is_empty() {
        println!("\n前导错误行:");
        for error in errors {
            println!("  - {}", error);
        }
    }
}

