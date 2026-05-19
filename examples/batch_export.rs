//! batch_export -- 将 SQL 日志导出为 CSV 格式到 stdout
//!
//! Reads a DM SQL log file and exports all records as CSV to stdout.
//! Each row contains the timestamp, user name, SQL body, and execution time.
//! Fields containing commas or double-quotes are properly escaped.
//!
//! Usage:
//!     cargo run --example batch_export -- <path-to-sqllog>

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse the file path from the first CLI argument
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Error: missing required file path argument");
        eprintln!("Usage: batch_export <path-to-sqllog>");
        std::process::exit(1);
    });

    // Build a parser using the builder pattern
    let parser = dm_database_parser_sqllog::LogParserBuilder::new(&path).build()?;

    // Print CSV header
    println!("timestamp,username,sql,exec_time_ms");

    // Iterate over all records, streaming output line by line
    for result in parser.iter() {
        let record = result?;

        // Extract fields for CSV export (use references to avoid partial moves)
        let ts = &record.ts;
        let username = record.parse_meta().username;
        let sql = record.body();
        let exec_time_ms = record.exec_time()?.unwrap_or(0);

        // Quote fields and escape embedded double-quotes
        // Deref coercion converts &Cow<str> to &str automatically
        println!(
            "{},{},{},{}",
            csv_quote(ts),
            csv_quote(&username),
            csv_quote(&sql),
            exec_time_ms,
        );
    }

    Ok(())
}

/// Wrap a string in double-quotes, escaping any embedded quotes as "".
fn csv_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
