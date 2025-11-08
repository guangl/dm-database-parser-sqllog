/// 性能测试：测试解析大型 sqllog 文件的性能
///
/// 这个示例用于评估解析器在处理大文件时的性能表现，包括：
/// 1. 纯解析性能（不写入数据库）
/// 2. 内存使用情况（使用迭代器避免加载全部数据）
/// 3. 吞吐量统计
///
/// 运行方式：
/// ```bash
/// cargo run --release --example parse_performance -- <sqllog_file_path>
/// ```
use dm_database_parser_sqllog::iter_sqllogs_from_file;
use std::env;
use std::fs;
use std::time::Instant;

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

fn format_duration(millis: f64) -> String {
    if millis >= 60000.0 {
        let minutes = millis / 60000.0;
        format!("{:.2} 分钟", minutes)
    } else if millis >= 1000.0 {
        let seconds = millis / 1000.0;
        format!("{:.2} 秒", seconds)
    } else {
        format!("{:.2} 毫秒", millis)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 获取命令行参数
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <sqllog_file_path>", args[0]);
        eprintln!("示例: {} sqllogs/large.sqllog", args[0]);
        eprintln!("\n提示: 使用 --release 编译以获得最佳性能");
        std::process::exit(1);
    }

    let file_path = &args[1];

    // 获取文件大小
    let metadata = fs::metadata(file_path)?;
    let file_size = metadata.len();

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║          DM SQL Log 解析性能测试                        ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();
    println!("📁 文件路径: {}", file_path);
    println!("📊 文件大小: {}", format_size(file_size));
    println!();

    // 开始解析测试
    println!("⏳ 开始解析测试...");
    println!();

    let start = Instant::now();
    let mut total_records = 0u64;
    let mut error_count = 0u64;
    let mut last_report = Instant::now();
    let mut report_interval_records = 0u64;

    // 使用迭代器逐条解析，避免内存溢出
    let parser = iter_sqllogs_from_file(file_path)?;
    for (idx, result) in parser.enumerate() {
        match result {
            Ok(_sqllog) => {
                total_records += 1;
                report_interval_records += 1;

                // 每 10000 条记录报告一次进度
                if report_interval_records >= 10000 {
                    let elapsed = last_report.elapsed();
                    let speed = report_interval_records as f64 / elapsed.as_secs_f64();
                    let total_elapsed = start.elapsed().as_secs_f64();
                    let avg_speed = total_records as f64 / total_elapsed;

                    println!(
                        "  ⚡ 已解析: {} 条 | 瞬时速度: {:.0} 条/秒 | 平均速度: {:.0} 条/秒",
                        total_records, speed, avg_speed
                    );

                    report_interval_records = 0;
                    last_report = Instant::now();
                }
            }
            Err(e) => {
                error_count += 1;
                if error_count <= 10 {
                    eprintln!("  ❌ 第 {} 行解析失败: {}", idx + 1, e);
                } else if error_count == 11 {
                    eprintln!("  ⚠️  后续错误将不再显示...");
                }
            }
        }
    }

    let duration = start.elapsed();
    let duration_millis = duration.as_millis() as f64;
    let duration_secs = duration.as_secs_f64();

    println!();
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║                 性能测试结果                            ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();
    println!("📊 解析统计:");
    println!("  ✅ 成功解析: {} 条记录", total_records);
    println!("  ❌ 解析失败: {} 条记录", error_count);
    println!(
        "  📈 成功率: {:.2}%",
        (total_records as f64 / (total_records + error_count) as f64) * 100.0
    );
    println!();

    println!("⏱️  性能指标:");
    println!("  总耗时: {}", format_duration(duration_millis));
    println!(
        "  平均速度: {:.0} 条/秒",
        total_records as f64 / duration_secs
    );
    println!(
        "  平均每条耗时: {:.2} 微秒",
        (duration_millis * 1000.0) / total_records as f64
    );
    println!();

    if file_size > 0 {
        let throughput_mb = (file_size as f64 / duration_secs) / (1024.0 * 1024.0);
        println!("📦 吞吐量:");
        println!("  数据吞吐: {:.2} MB/秒", throughput_mb);
        println!("  文件大小: {}", format_size(file_size));
        println!();
    }

    println!("💡 性能评估:");
    let speed = total_records as f64 / duration_secs;
    if speed >= 100000.0 {
        println!("  🚀 优秀！解析速度超过 10万 条/秒");
    } else if speed >= 50000.0 {
        println!("  ✅ 良好！解析速度在 5-10万 条/秒");
    } else if speed >= 10000.0 {
        println!("  ⚡ 中等！解析速度在 1-5万 条/秒");
    } else {
        println!("  ⚠️  较慢，解析速度低于 1万 条/秒");
    }
    println!();

    // 估算处理更大文件所需时间
    if total_records > 0 {
        println!("📈 预估处理能力 (基于当前性能):");
        let records_per_sec = total_records as f64 / duration_secs;

        // 估算不同规模的文件处理时间
        let estimates = vec![
            (100_000, "10万条"),
            (1_000_000, "100万条"),
            (10_000_000, "1000万条"),
            (100_000_000, "1亿条"),
        ];

        for (records, label) in estimates {
            if records as u64 > total_records {
                let estimated_secs = records as f64 / records_per_sec;
                println!(
                    "  {} 记录: 约 {}",
                    label,
                    format_duration(estimated_secs * 1000.0)
                );
            }
        }
        println!();
    }

    Ok(())
}
