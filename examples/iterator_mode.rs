use dm_database_parser_sqllog::{iter_records_from_file, iter_sqllogs_from_file};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== 迭代器模式示例 - 适合大文件 ===\n");

    // 示例 1: 使用 records 迭代器（内存友好）
    println!("示例 1: 使用 Records 迭代器");
    println!("{}", "-".repeat(60));

    let parser = iter_records_from_file("sqllogs/dmsql_OASIS_DB1_20251020_151030.log")?;

    let mut record_count = 0;
    let mut error_count = 0;
    let mut multi_line_count = 0;

    for result in parser {
        match result {
            Ok(record) => {
                record_count += 1;
                if record.has_continuation_lines() {
                    multi_line_count += 1;
                }

                // 只显示前 5 条
                if record_count <= 5 {
                    println!("\n记录 {}:", record_count);
                    println!(
                        "  起始行: {}",
                        record.start_line().chars().take(100).collect::<String>()
                    );
                    println!("  总行数: {}", record.lines.len());
                }
            }
            Err(err) => {
                error_count += 1;
                if error_count <= 3 {
                    eprintln!("I/O 错误 {}: {}", error_count, err);
                }
            }
        }
    }

    println!("\n统计:");
    println!("  总记录数: {}", record_count);
    println!("  多行记录: {}", multi_line_count);
    println!("  I/O 错误: {}", error_count);

    println!("\n{}\n", "=".repeat(60));

    // 示例 2: 使用 sqllogs 迭代器进行流式处理
    println!("示例 2: 使用 Sqllogs 迭代器（流式处理）");
    println!("{}", "-".repeat(60));

    let parser = iter_sqllogs_from_file("sqllogs/dmsql_OASIS_DB1_20251020_151030.log")?;

    let mut success_count = 0;
    let mut parse_error_count = 0;
    let mut with_indicators = 0;
    let mut total_exec_time = 0.0_f32;
    let mut user_stats = std::collections::HashMap::new();

    for result in parser {
        match result {
            Ok(sqllog) => {
                success_count += 1;

                // 统计用户
                *user_stats.entry(sqllog.meta.username.clone()).or_insert(0) += 1;

                // 统计性能指标
                if let Some(time) = sqllog.execute_time() {
                    with_indicators += 1;
                    total_exec_time += time;
                }

                // 显示前 3 条
                if success_count <= 3 {
                    println!("\nSQL 日志 {}:", success_count);
                    println!("  时间: {}", sqllog.ts);
                    println!("  用户: {}", sqllog.meta.username);
                    println!("  会话: {}", sqllog.meta.sess_id);
                    println!(
                        "  SQL: {}",
                        sqllog.body.chars().take(80).collect::<String>()
                    );
                    if let Some(time) = sqllog.execute_time() {
                        println!("  执行时间: {}ms", time);
                    }
                }
            }
            Err(err) => {
                parse_error_count += 1;
                if parse_error_count <= 3 {
                    eprintln!("解析错误 {}: {}", parse_error_count, err);
                }
            }
        }

        // 演示流式处理：每处理 1000 条打印进度
        if (success_count + parse_error_count) % 1000 == 0 {
            println!(
                "\n进度: 已处理 {} 条记录...",
                success_count + parse_error_count
            );
        }
    }

    println!("\n统计:");
    println!("  成功解析: {} 条", success_count);
    println!("  解析错误: {} 个", parse_error_count);
    println!("  包含性能指标: {} 条", with_indicators);

    if with_indicators > 0 {
        println!(
            "  平均执行时间: {:.2}ms",
            total_exec_time / with_indicators as f32
        );
    }

    println!("\n用户统计:");
    for (user, count) in user_stats.iter() {
        println!("  {}: {} 条", user, count);
    }

    println!("\n{}\n", "=".repeat(60));

    // 示例 3: 使用迭代器进行过滤和转换
    println!("示例 3: 过滤慢查询（执行时间 > 100ms）");
    println!("{}", "-".repeat(60));

    let parser = iter_sqllogs_from_file("sqllogs/dmsql_OASIS_DB1_20251020_151030.log")?;

    let slow_queries: Vec<_> = parser
        .filter_map(|result| result.ok())
        .filter(|sqllog| sqllog.execute_time().map_or(false, |time| time > 100.0))
        .take(5) // 只取前 5 条
        .collect();

    println!("\n找到 {} 条慢查询（仅显示前 5 条）:\n", slow_queries.len());

    for (i, sqllog) in slow_queries.iter().enumerate() {
        println!("慢查询 {}:", i + 1);
        println!("  时间: {}", sqllog.ts);
        println!("  用户: {}", sqllog.meta.username);
        println!("  执行时间: {:.2}ms", sqllog.execute_time().unwrap());
        println!(
            "  SQL: {}",
            sqllog.body.chars().take(100).collect::<String>()
        );
        println!();
    }

    println!("{}\n", "=".repeat(60));

    // 示例 4: 展示内存优势
    println!("示例 4: 内存优势演示");
    println!("{}", "-".repeat(60));
    println!("\n使用迭代器模式的优势:");
    println!("✓ 不会一次性加载所有数据到内存");
    println!("✓ 可以处理任意大小的文件");
    println!("✓ 支持流式处理和实时分析");
    println!("✓ 可以使用 filter、map、take 等迭代器组合器");
    println!("✓ 遇到错误可以选择继续或中断");

    println!("\n推荐使用场景:");
    println!("• 大文件（> 100MB）");
    println!("• 实时日志分析");
    println!("• 需要早停（找到目标后停止）");
    println!("• 内存受限环境");

    Ok(())
}
