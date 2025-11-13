use dm_database_parser_sqllog::iter_records_from_file;
use std::env;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <日志文件路径>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    println!("开始流式解析文件: {}", file_path);

    let start = Instant::now();
    let mut sqllog_count = 0;
    let mut error_count = 0;

    for result in iter_records_from_file(file_path).unwrap() {
        match result {
            Ok(_sqllog) => {
                sqllog_count += 1;
                // 每处理 100 万条打印一次进度
                if sqllog_count % 1_000_000 == 0 {
                    println!("已处理: {} 条...", sqllog_count);
                }
            }
            Err(_err) => {
                error_count += 1;
            }
        }
    }

    let elapsed = start.elapsed();

    println!("\n=== 流式解析性能报告 ===");
    println!("成功解析: {} 条 SQL 日志", sqllog_count);
    println!("解析错误: {} 个", error_count);
    println!("总耗时: {:.3} 秒", elapsed.as_secs_f64());
    println!(
        "速度: {:.0} 条记录/秒",
        sqllog_count as f64 / elapsed.as_secs_f64()
    );
}
