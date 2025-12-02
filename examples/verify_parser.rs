use dm_database_parser_sqllog::LogParser;
use std::io::Write;
use tempfile::NamedTempFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a temporary file with sample log data
    let mut file = NamedTempFile::new()?;
    let log_content = r#"2025-11-17 16:09:41.123 (sess:123 thrd:456 user:SYSDBA trxid:789 stmt:0 appname:disql ip:::127.0.0.1) SELECT * FROM DUAL;
EXECTIME: 1.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 100.
2025-11-17 16:09:41.124 (sess:124 thrd:457 user:USER1 trxid:790 stmt:1 appname:manager ip:::192.168.1.1)
SELECT *
FROM USERS
WHERE ID = 1;
EXECTIME: 2.5(ms) ROWCOUNT: 5(rows) EXEC_ID: 101.
"#;

    write!(file, "{}", log_content)?;
    let path = file.path();

    println!("Created temporary log file at: {:?}", path);
    println!("Log content:\n---\n{}\n---", log_content);

    // 2. Parse the file
    println!("Parsing records...");
    let parser = LogParser::from_path(path)?;
    let records: Vec<_> = parser.iter().collect();

    // 3. Verify results
    println!("Found {} records", records.len());

    for (i, record) in records.into_iter().enumerate() {
        match record {
            Ok(sqllog) => {
                println!("\nRecord #{}:", i + 1);
                println!("  Timestamp: {}", sqllog.ts);
                println!("  Meta: {:?}", sqllog.parse_meta());
                println!("  Body: {:?}", sqllog.body());
                println!("  Indicators: {:?}", sqllog.parse_indicators());
            }
            Err(e) => {
                println!("\nRecord #{} Error: {}", i + 1, e);
            }
        }
    }

    Ok(())
}
