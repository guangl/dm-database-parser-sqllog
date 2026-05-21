//! batch_export -- 将 SQL 日志导出为 CSV 格式到 stdout

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Error: missing required file path argument");
        eprintln!("Usage: batch_export <path-to-sqllog>");
        std::process::exit(1);
    });

    let parser = dm_database_parser_sqllog::LogParserBuilder::new(&path).build()?;

    println!("timestamp,username,sql,exec_time_ms");

    for result in parser.iter() {
        let record = result?;

        let ts = &record.ts;
        let username = &record.username;
        let sql = &record.sql;
        let exec_time_ms = record.exectime as u64;

        println!(
            "{},{},{},{}",
            csv_quote(ts),
            csv_quote(username),
            csv_quote(sql),
            exec_time_ms,
        );
    }

    Ok(())
}

fn csv_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
