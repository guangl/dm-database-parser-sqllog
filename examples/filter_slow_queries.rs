//! filter_slow_queries -- 过滤执行时间 >= 100ms 的慢查询

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Error: missing required file path argument");
        eprintln!("Usage: filter_slow_queries <path-to-sqllog>");
        std::process::exit(1);
    });

    let parser = dm_database_parser_sqllog::LogParserBuilder::new(&path).build()?;

    for result in parser.iter().unwrap().filter_by_exec_time(100.0) {
        let record = result?;
        println!("{} | {}ms | {}", record.ts, record.exectime, record.sql);
    }

    Ok(())
}
