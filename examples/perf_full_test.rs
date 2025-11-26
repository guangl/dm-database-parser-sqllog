use dm_database_parser_sqllog::parse_records_from_file;
use std::env;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <日志文件路径>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    println!("开始解析文件（自动使用并行处理）: {}", file_path);

    let start = Instant::now();
    let (sqllogs, errors) = parse_records_from_file(file_path);
    let elapsed = start.elapsed();

    println!("\n=== 性能报告（并行版本） ===");
    println!("成功解析: {} 条 SQL 日志", sqllogs.len());
    println!("解析错误: {} 个", errors.len());
    println!("总耗时: {:.3} 秒", elapsed.as_secs_f64());
    println!(
        "速度: {:.0} 条记录/秒",
        sqllogs.len() as f64 / elapsed.as_secs_f64()
    );
}
