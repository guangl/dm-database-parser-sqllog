//! filter_slow_queries -- 过滤执行时间 >= 100ms 的慢查询
//!
//! Reads a DM SQL log file and prints all queries whose execution time
//! is 100ms or more. Uses `filter_by_exec_time()` from the library API.
//!
//! Usage:
//!     cargo run --example filter_slow_queries -- <path-to-sqllog>

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse the file path from the first CLI argument
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Error: missing required file path argument");
        eprintln!("Usage: filter_slow_queries <path-to-sqllog>");
        std::process::exit(1);
    });

    // Build a parser using the builder pattern (Phase 7 API)
    let parser = dm_database_parser_sqllog::LogParserBuilder::new(&path).build()?;

    // Iterate over records, filtering to only those with exec_time >= 100ms
    // Each result is wrapped in Result; errors are propagated with `?`
    for result in parser.iter().filter_by_exec_time(100) {
        let record = result?;
        let exec_time_ms = record.exec_time()?.unwrap_or(0);
        println!(
            "{} | {}ms | {}",
            record.ts,
            exec_time_ms,
            record.body()
        );
    }

    Ok(())
}
