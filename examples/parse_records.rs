use dm_database_parser_sqllog::parser::{RecordParser, parse_records_from_string};
use std::fs::File;

fn main() {
    // 示例 1: 从字符串解析
    println!("=== 示例 1: 从字符串解析记录 ===\n");
    let sample_log = r#"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname: ip:::ffff:10.3.100.68) SELECT *
FROM users
WHERE status = 'active'
  AND created_date > '2024-01-01'
2025-08-12 10:57:10.123 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb061 appname:) UPDATE products
SET price = price * 1.1
WHERE category = 'electronics'
2025-08-12 10:57:11.456 (EP[0] sess:0x178ebca1 thrd:757456 user:ADMIN trxid:1 stmt:0x285eb062 appname:admin-tool ip:::ffff:192.168.1.1) DELETE FROM temp_table"#;

    let records = parse_records_from_string(sample_log);

    println!("解析到 {} 条记录\n", records.len());

    for (i, record) in records.iter().enumerate() {
        println!("记录 #{}", i + 1);
        println!("  起始行: {}", record.start_line());
        println!("  总行数: {}", record.lines.len());
        println!("  有继续行: {}", record.has_continuation_lines());

        if record.has_continuation_lines() {
            println!("  继续行:");
            for (j, line) in record.all_lines().iter().skip(1).enumerate() {
                println!("    [{}] {}", j + 1, line);
            }
        }

        println!("  完整内容:");
        for line in &record.lines {
            println!("    {}", line);
        }
        println!();
    }

    // 示例 2: 从文件读取（如果存在示例文件）
    println!("\n=== 示例 2: 从文件流式解析 ===\n");

    let test_log_path = "sqllogs/sample.log";
    if let Ok(file) = File::open(test_log_path) {
        let parser = RecordParser::new(file);
        let mut count = 0;

        for result in parser {
            match result {
                Ok(record) => {
                    count += 1;
                    println!("记录 #{}: {} 行", count, record.lines.len());
                    if count >= 5 {
                        println!("（只显示前 5 条...）");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("读取错误: {}", e);
                    break;
                }
            }
        }

        if count == 0 {
            println!("文件中没有找到有效记录");
        }
    } else {
        println!("提示: 没有找到 {} 文件", test_log_path);
        println!("你可以创建一个示例日志文件来测试流式解析功能");
    }

    // 示例 3: 展示如何处理大文件
    println!("\n=== 示例 3: 处理大文件的推荐方式 ===\n");
    println!("对于大文件，使用 RecordParser 迭代器可以避免一次性加载所有内容到内存：");
    println!();
    println!("```rust");
    println!("use std::fs::File;");
    println!("use dm_database_parser_sqllog::parser::RecordParser;");
    println!();
    println!("let file = File::open(\"large.log\")?;");
    println!("let parser = RecordParser::new(file);");
    println!();
    println!("for result in parser {{");
    println!("    match result {{");
    println!("        Ok(record) => {{");
    println!("            // 处理每条记录");
    println!("            process_record(&record);");
    println!("        }}");
    println!("        Err(e) => eprintln!(\"错误: {{}}\", e),");
    println!("    }}");
    println!("}}");
    println!("```");
}
