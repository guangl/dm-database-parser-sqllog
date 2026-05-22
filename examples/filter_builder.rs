//! filter_builder — FilterBuilder 链式构造组合过滤器演示
//!
//! 用法：`cargo run --example filter_builder <path-to-sqllog>`

use dm_database_parser_sqllog::{FilterBuilder, LogParserBuilder};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: filter_builder <path-to-sqllog>");
        std::process::exit(1);
    });

    // 多字段 AND 组合：exec_time > 1ms AND sql 含 "SELECT"
    let filter = FilterBuilder::new()
        .exec_time_gt(1.0)
        .sql_contains("SELECT")
        .build();

    let parser = LogParserBuilder::new(&path).build()?;
    let mut count = 0usize;

    for result in parser.iter().apply_filter(filter) {
        let record = result?;
        println!(
            "{} | ep={} | {}ms | {}",
            record.ts, record.ep, record.exectime, record.sql
        );
        count += 1;
    }

    println!("共匹配 {} 条记录", count);
    Ok(())
}
