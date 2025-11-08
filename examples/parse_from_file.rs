use dm_database_parser_sqllog::{parse_records_from_file, parse_sqllogs_from_file};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // 示例 1: 从文件解析为 Records（获取 records 和 errors）
    println!("=== 示例 1: 解析 Records ===");

    match parse_records_from_file("sqllogs/sqllog_sample.log") {
        Ok((records, errors)) => {
            println!("✓ 成功解析 {} 条记录", records.len());
            println!("✓ 遇到 {} 个 I/O 错误", errors.len());

            // 显示前 3 条记录
            for (i, record) in records.iter().take(3).enumerate() {
                println!("\n记录 {}:", i + 1);
                println!("  起始行: {}", record.start_line());
                println!("  总行数: {}", record.lines.len());
                if record.has_continuation_lines() {
                    println!("  包含继续行: 是");
                }
            }

            // 显示错误
            if !errors.is_empty() {
                println!("\n遇到的错误:");
                for (i, error) in errors.iter().enumerate() {
                    println!("  错误 {}: {}", i + 1, error);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ 文件打开失败: {}", e);
            return Err(e.into());
        }
    }

    println!("\n{}\n", "=".repeat(60));

    // 示例 2: 从文件解析为 Sqllogs（获取 sqllogs 和 parse errors）
    println!("=== 示例 2: 解析 Sqllogs ===");

    match parse_sqllogs_from_file("sqllogs/sqllog_sample.log") {
        Ok((sqllogs, errors)) => {
            println!("✓ 成功解析 {} 条 SQL 日志", sqllogs.len());
            println!("✓ 遇到 {} 个解析错误", errors.len());

            // 显示前 3 条 SQL 日志
            for (i, sqllog) in sqllogs.iter().take(3).enumerate() {
                println!("\nSQL 日志 {}:", i + 1);
                println!("  时间戳: {}", sqllog.ts);
                println!("  用户: {}", sqllog.meta.username);
                println!("  会话 ID: {}", sqllog.meta.sess_id);
                println!(
                    "  SQL: {}",
                    sqllog.body.chars().take(80).collect::<String>()
                );

                if let Some(time) = sqllog.execute_time() {
                    println!("  执行时间: {}ms", time);
                }
                if let Some(rows) = sqllog.row_count() {
                    println!("  影响行数: {}", rows);
                }
            }

            // 显示解析错误
            if !errors.is_empty() {
                println!("\n解析错误:");
                for (i, error) in errors.iter().enumerate() {
                    println!("  错误 {}: {}", i + 1, error);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ 文件打开失败: {}", e);
            return Err(e.into());
        }
    }

    println!("\n{}\n", "=".repeat(60));

    // 示例 3: 统计信息
    println!("=== 示例 3: 统计信息 ===");

    if let Ok((sqllogs, errors)) = parse_sqllogs_from_file("sqllogs/sqllog_sample.log") {
        println!("文件统计:");
        println!("  总记录数: {}", sqllogs.len());
        println!("  解析错误数: {}", errors.len());

        // 按用户统计
        let mut user_counts = std::collections::HashMap::new();
        for sqllog in &sqllogs {
            *user_counts.entry(&sqllog.meta.username).or_insert(0) += 1;
        }

        println!("\n按用户统计:");
        for (user, count) in user_counts.iter() {
            println!("  {}: {} 条", user, count);
        }

        // 统计有性能指标的记录
        let with_indicators = sqllogs.iter().filter(|s| s.has_indicators()).count();
        println!("\n性能指标:");
        println!("  包含性能指标的记录: {}", with_indicators);

        // 平均执行时间
        if with_indicators > 0 {
            let total_time: f32 = sqllogs.iter().filter_map(|s| s.execute_time()).sum();
            let avg_time = total_time / with_indicators as f32;
            println!("  平均执行时间: {:.2}ms", avg_time);
        }
    }

    Ok(())
}
