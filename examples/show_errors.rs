/// 显示解析失败的记录
use dm_database_parser_sqllog::iter_sqllogs_from_file;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <sqllog_file_path>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];

    println!("分析文件: {}", file_path);
    println!();

    let mut success_count = 0;
    let mut error_count = 0;
    let mut error_samples: Vec<(usize, String)> = Vec::new();

    for (idx, result) in iter_sqllogs_from_file(file_path)?.enumerate() {
        match result {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_count += 1;
                if error_samples.len() < 20 {
                    error_samples.push((idx + 1, e.to_string()));
                }
            }
        }
    }

    println!("成功: {} 条", success_count);
    println!("失败: {} 条", error_count);
    println!(
        "成功率: {:.2}%",
        (success_count as f64 / (success_count + error_count) as f64) * 100.0
    );
    println!();

    if !error_samples.is_empty() {
        println!("失败样本 (前 20 个):");
        for (idx, error) in error_samples {
            println!("  [{}] {}", idx, error);
        }
    }

    Ok(())
}
