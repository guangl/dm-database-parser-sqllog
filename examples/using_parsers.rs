use dm_database_parser_sqllog::{RecordParser, SqllogParser};
use std::fs::File;

fn main() {
    println!("=== 使用 RecordParser 和 SqllogParser 的示例 ===\n");

    // 示例 1: 使用 RecordParser 先获取 Record，然后按需解析
    println!("### 示例 1: RecordParser -> Record -> Sqllog");
    println!("适用场景：需要先筛选 Record，再解析成 Sqllog\n");

    let sample_log = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
WHERE status = 'active'
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE products
SET price = price * 1.1"#;

    let cursor = std::io::Cursor::new(sample_log.as_bytes());
    let record_parser = RecordParser::new(cursor);

    for result in record_parser {
        match result {
            Ok(record) => {
                println!("📝 发现记录，包含 {} 行", record.lines.len());

                // 只解析多行记录
                if record.has_continuation_lines() {
                    match record.parse_to_sqllog() {
                        Ok(sqllog) => {
                            println!("  用户: {}", sqllog.meta.username);
                            println!("  SQL: {}", sqllog.body);
                        }
                        Err(e) => eprintln!("  解析错误: {}", e),
                    }
                } else {
                    println!("  (跳过单行记录)");
                }
                println!();
            }
            Err(e) => eprintln!("读取错误: {}", e),
        }
    }

    // 示例 2: 直接使用 SqllogParser 流式解析
    println!("\n### 示例 2: SqllogParser 直接解析");
    println!("适用场景：直接将所有 Record 解析为 Sqllog\n");

    let cursor = std::io::Cursor::new(sample_log.as_bytes());
    let sqllog_parser = SqllogParser::new(cursor);

    for (i, result) in sqllog_parser.enumerate() {
        match result {
            Ok(sqllog) => {
                println!("记录 #{}", i + 1);
                println!("  时间: {}", sqllog.ts);
                println!("  用户: {}", sqllog.meta.username);
                println!("  线程: {}", sqllog.meta.thrd_id);
                println!("  SQL: {}", sqllog.body);

                if let Some(indicators) = sqllog.indicators {
                    println!("  执行时间: {} ms", indicators.execute_time);
                    println!("  影响行数: {}", indicators.row_count);
                }
                println!();
            }
            Err(e) => eprintln!("解析错误: {}", e),
        }
    }

    // 示例 3: 从文件流式解析（大文件推荐）
    println!("\n### 示例 3: 从文件流式解析（内存高效）");
    println!("适用场景：处理大型日志文件\n");

    let log_path = "sqllogs/sample.log";
    if let Ok(file) = File::open(log_path) {
        let sqllog_parser = SqllogParser::new(file);
        let mut count = 0;
        let mut slow_queries = 0;

        for result in sqllog_parser {
            match result {
                Ok(sqllog) => {
                    count += 1;

                    // 统计慢查询
                    if let Some(indicators) = sqllog.indicators {
                        if indicators.execute_time > 50.0 {
                            slow_queries += 1;
                            println!(
                                "⚠️  慢查询 ({} ms): {}",
                                indicators.execute_time,
                                sqllog.body.lines().next().unwrap_or("")
                            );
                        }
                    }
                }
                Err(e) => eprintln!("解析错误: {}", e),
            }
        }

        println!("\n总计: {} 条记录", count);
        println!("慢查询: {} 条", slow_queries);
    } else {
        println!("未找到文件: {}", log_path);
        println!("提示: 可以创建一个示例日志文件来测试");
    }

    // 示例 4: 条件过滤和统计
    println!("\n### 示例 4: 条件过滤和统计");
    println!("适用场景：分析特定用户的查询模式\n");

    let cursor = std::io::Cursor::new(sample_log.as_bytes());
    let sqllog_parser = SqllogParser::new(cursor);

    let alice_sqls: Vec<_> = sqllog_parser
        .filter_map(|result| result.ok())
        .filter(|sqllog| sqllog.meta.username == "alice")
        .collect();

    println!("用户 alice 的查询数: {}", alice_sqls.len());
    for (i, sqllog) in alice_sqls.iter().enumerate() {
        println!("  [{}] {}", i + 1, sqllog.body.lines().next().unwrap_or(""));
    }

    println!("\n=== API 对比 ===\n");
    println!("RecordParser:");
    println!("  - 返回: Iterator<Item = Result<Record, io::Error>>");
    println!("  - 用途: 按行分组，得到原始行数据");
    println!("  - 优势: 灵活，可以先筛选再解析");
    println!();
    println!("SqllogParser:");
    println!("  - 返回: Iterator<Item = Result<Sqllog, ParseError>>");
    println!("  - 用途: 直接解析为结构化数据");
    println!("  - 优势: 简洁，适合直接处理所有记录");
    println!();
    println!("Record.parse_to_sqllog():");
    println!("  - 输入: &Record");
    println!("  - 返回: Result<Sqllog, ParseError>");
    println!("  - 用途: 将单个 Record 转换为 Sqllog");
    println!("  - 优势: 配合 RecordParser 使用，按需解析");
}
