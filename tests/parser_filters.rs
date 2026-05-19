use dm_database_parser_sqllog::LogParserBuilder;
use std::io::Write;
use tempfile::NamedTempFile;

// ── filter_by_exec_time tests ────────────────────────────────────────────

#[test]
#[cfg(not(miri))]
fn test_filter_by_exec_time_filters_low_exec_time() {
    let mut file = NamedTempFile::new().unwrap();
    // Record 1: EXECTIME 5ms (below 100ms threshold, should be filtered)
    let r1 = "2025-11-17 16:09:41.100 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1 EXECTIME: 5(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    // Record 2: EXECTIME 200ms (above 100ms threshold, should be kept)
    let r2 = "2025-11-17 16:09:41.200 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 2 EXECTIME: 200(ms) ROWCOUNT: 1(rows) EXEC_ID: 2.\n";
    // Record 3: No EXECTIME field (should be filtered automatically since exectime == 0.0)
    let r3 =
        "2025-11-17 16:09:41.300 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 3\n";
    write!(file, "{}{}{}", r1, r2, r3).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().filter_by_exec_time(100).collect();

    // Only r2 (200ms) should be retained
    assert_eq!(results.len(), 1);
    let record = results[0].as_ref().unwrap();
    assert!(record.body().contains("SELECT 2"));
}

#[test]
#[cfg(not(miri))]
fn test_filter_by_exec_time_keeps_high_exec_time() {
    let mut file = NamedTempFile::new().unwrap();
    // Both records are above 100ms threshold
    let r1 = "2025-11-17 16:09:41.100 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1 EXECTIME: 150(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    let r2 = "2025-11-17 16:09:41.200 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 2 EXECTIME: 300(ms) ROWCOUNT: 1(rows) EXEC_ID: 2.\n";
    write!(file, "{}{}", r1, r2).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().filter_by_exec_time(100).collect();

    assert_eq!(results.len(), 2);
}

#[test]
#[cfg(not(miri))]
fn test_filter_by_exec_time_empty_when_all_below() {
    let mut file = NamedTempFile::new().unwrap();
    // Both records are below 100ms threshold
    let r1 = "2025-11-17 16:09:41.100 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1 EXECTIME: 5(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    let r2 = "2025-11-17 16:09:41.200 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 2 EXECTIME: 50(ms) ROWCOUNT: 1(rows) EXEC_ID: 2.\n";
    write!(file, "{}{}", r1, r2).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().filter_by_exec_time(100).collect();

    assert_eq!(results.len(), 0);
}

// ── filter_by_sql_contains tests ────────────────────────────────────────────

#[test]
#[cfg(not(miri))]
fn test_filter_by_sql_contains_matches() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 =
        "2025-11-17 16:09:41.100 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1\n";
    let r2 = "2025-11-17 16:09:41.200 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) INSERT INTO t VALUES (1)\n";
    let r3 =
        "2025-11-17 16:09:41.300 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) select 1\n";
    write!(file, "{}{}{}", r1, r2, r3).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().filter_by_sql_contains("SELECT").collect();

    // Case-sensitive: r1 has "SELECT", r2 has "INSERT", r3 has "select"
    assert_eq!(results.len(), 1);
    let record = results[0].as_ref().unwrap();
    assert!(record.body().contains("SELECT"));
}

#[test]
#[cfg(not(miri))]
fn test_filter_by_sql_contains_empty_when_no_match() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.100 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) INSERT INTO t VALUES (1)\n";
    let r2 = "2025-11-17 16:09:41.200 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) UPDATE t SET x=2\n";
    write!(file, "{}{}", r1, r2).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().filter_by_sql_contains("SELECT").collect();

    assert_eq!(results.len(), 0);
}

#[test]
#[cfg(not(miri))]
fn test_filter_by_sql_contains_skips_parse_errors() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "this is not a valid log record\n";
    let r2 = "2025-11-17 16:09:41.200 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT * FROM users\n";
    write!(file, "{}{}", r1, r2).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().filter_by_sql_contains("SELECT").collect();

    assert_eq!(results.len(), 1);
    let record = results[0].as_ref().unwrap();
    assert!(record.body().contains("SELECT"));
}
