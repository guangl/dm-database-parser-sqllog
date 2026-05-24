//! FilterBuilder 端到端集成测试（FILTER-09 AND 语义 + FILTER-10 LogIterator 集成）

use dm_database_parser_sqllog::{Filter, FilterBuilder, LogParserBuilder};
use std::io::Write;
use tempfile::NamedTempFile;

// ── 辅助函数 ──

fn record_line(ts: &str, ep: u8, user: &str, sql: &str, exectime: u64) -> String {
    format!(
        "{} (EP[{}] sess:1 thrd:2 user:{} trxid:0 stmt:1 appname:app) {} EXECTIME: {}(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n",
        ts, ep, user, sql, exectime
    )
}

fn record_line_no_metrics(ts: &str, ep: u8, user: &str, sql: &str) -> String {
    format!(
        "{} (EP[{}] sess:1 thrd:2 user:{} trxid:0 stmt:1 appname:app) {}\n",
        ts, ep, user, sql
    )
}

// ── 测试用例 ──

/// 单条件过滤：exec_time_gt(100.0) 只保留 200ms 那条记录
#[test]
#[cfg(not(miri))]
fn test_apply_filter_single_condition_exec_time() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = record_line("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1", 5);
    let r2 = record_line("2025-11-17 16:09:41.200", 0, "alice", "SELECT 2", 200);
    let r3 = record_line("2025-11-17 16:09:41.300", 0, "alice", "SELECT 3", 50);
    write!(file, "{}{}{}", r1, r2, r3).unwrap();

    let filter = FilterBuilder::new().exec_time_gt(100.0).build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter(filter).collect();

    assert_eq!(results.len(), 1);
    assert!(results[0].as_ref().unwrap().sql.contains("SELECT 2"));
}

/// 多条件 AND 语义：username_eq("alice") AND sql_contains("SELECT") 同时生效
#[test]
#[cfg(not(miri))]
fn test_apply_filter_multiple_conditions_and_semantics() {
    let mut file = NamedTempFile::new().unwrap();
    // alice + SELECT -> 通过
    let r1 = record_line_no_metrics("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1");
    // bob + SELECT -> username 不匹配，过滤掉
    let r2 = record_line_no_metrics("2025-11-17 16:09:41.200", 0, "bob", "SELECT 2");
    // alice + INSERT -> sql 不匹配，过滤掉
    let r3 = record_line_no_metrics(
        "2025-11-17 16:09:41.300",
        0,
        "alice",
        "INSERT INTO t VALUES(1)",
    );
    write!(file, "{}{}{}", r1, r2, r3).unwrap();

    let filter = FilterBuilder::new()
        .username_eq("alice")
        .sql_contains("SELECT")
        .build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter(filter).collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].as_ref().unwrap().username, "alice");
    assert!(results[0].as_ref().unwrap().sql.contains("SELECT"));
}

/// 空 filter 匹配所有记录（无谓词 = 全通过）
#[test]
#[cfg(not(miri))]
fn test_apply_filter_empty_filter_matches_all() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = record_line_no_metrics("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1");
    let r2 = record_line_no_metrics("2025-11-17 16:09:41.200", 0, "bob", "UPDATE t SET x=2");
    write!(file, "{}{}", r1, r2).unwrap();

    let filter = FilterBuilder::new().build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter(filter).collect();

    assert_eq!(results.len(), 2);
}

/// 无匹配：条件不满足时返回空集合
#[test]
#[cfg(not(miri))]
fn test_apply_filter_no_match_returns_empty() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = record_line_no_metrics("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1");
    let r2 = record_line_no_metrics("2025-11-17 16:09:41.200", 0, "alice", "SELECT 2");
    write!(file, "{}{}", r1, r2).unwrap();

    // username_eq("nonexistent") 不会匹配任何记录
    let filter = FilterBuilder::new().username_eq("nonexistent").build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter(filter).collect();

    assert_eq!(results.len(), 0);
}

/// apply_filter 丢弃解析错误（Err 被丢弃）
#[test]
#[cfg(not(miri))]
fn test_apply_filter_drops_parse_errors() {
    let mut file = NamedTempFile::new().unwrap();
    // 无效行，会导致解析错误
    let invalid = "this is not a valid log record\n";
    let valid = record_line_no_metrics("2025-11-17 16:09:41.200", 0, "alice", "SELECT 1");
    write!(file, "{}{}", invalid, valid).unwrap();

    // 空 filter 保留所有有效记录，Err 被丢弃
    let filter = FilterBuilder::new().build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter(filter).collect();

    // 只有 1 条有效记录，错误被丢弃
    assert_eq!(results.len(), 1);
    assert!(results[0].is_ok());
}

/// apply_filter_keep_errors 透传解析错误（Err 保留在结果中）
#[test]
#[cfg(not(miri))]
fn test_apply_filter_keep_errors_propagates_parse_errors() {
    use dm_database_parser_sqllog::ParseError;

    let mut file = NamedTempFile::new().unwrap();
    // 无效行，会导致解析错误
    let invalid = "this is not a valid log record\n";
    let valid = record_line_no_metrics("2025-11-17 16:09:41.200", 0, "alice", "SELECT 1");
    write!(file, "{}{}", invalid, valid).unwrap();

    let filter = FilterBuilder::new().build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter_keep_errors(filter).collect();

    // 1 个 Err + 1 个 Ok（通过 filter 的有效记录）
    assert_eq!(results.len(), 2);
    let has_err = results.iter().any(|r| r.is_err());
    let has_ok = results.iter().any(|r| r.is_ok());
    assert!(has_err, "期望至少一个 Err");
    assert!(has_ok, "期望至少一个 Ok");

    // 验证 Err 的类型为 ParseError::InvalidFormat
    let err_result = results.iter().find(|r| r.is_err()).unwrap();
    assert!(matches!(err_result, Err(ParseError::InvalidFormat { .. })));
}

/// ts_starts_with 前缀过滤：按时间戳前缀区分记录
#[test]
#[cfg(not(miri))]
fn test_apply_filter_ts_starts_with() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = record_line_no_metrics("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1");
    let r2 = record_line_no_metrics("2025-11-18 09:00:00.000", 0, "alice", "SELECT 2");
    write!(file, "{}{}", r1, r2).unwrap();

    let filter = FilterBuilder::new()
        .ts_starts_with("2025-11-17 16:09:41")
        .build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter(filter).collect();

    assert_eq!(results.len(), 1);
    assert!(
        results[0]
            .as_ref()
            .unwrap()
            .ts
            .starts_with("2025-11-17 16:09:41")
    );
}

/// ep_between 范围过滤：闭区间 [min, max]
#[test]
#[cfg(not(miri))]
fn test_apply_filter_ep_between() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = record_line_no_metrics("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1");
    let r2 = record_line_no_metrics("2025-11-17 16:09:41.200", 2, "alice", "SELECT 2");
    let r3 = record_line_no_metrics("2025-11-17 16:09:41.300", 5, "alice", "SELECT 3");
    write!(file, "{}{}{}", r1, r2, r3).unwrap();

    // 只保留 ep 在 [1, 3] 内的记录（ep=2 满足，ep=0 和 ep=5 不满足）
    let filter = FilterBuilder::new().ep_between(1, 3).build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter(filter).collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].as_ref().unwrap().ep, 2);
}

/// Filter 满足 Send + Sync：跨线程传递验证（Phase 12 async 集成铺路）
#[test]
#[cfg(not(miri))]
fn test_filter_send_sync_across_threads() {
    use dm_database_parser_sqllog::Sqllog;

    let filter = FilterBuilder::new()
        .exec_time_gt(100.0)
        .sql_contains("SELECT")
        .build();

    // 构造一条内存记录用于测试
    let record = Sqllog {
        ts: "2025-11-17 16:09:41.100".to_string(),
        tag: None,
        ep: 0,
        sess_id: "1".to_string(),
        thrd_id: "2".to_string(),
        username: "alice".to_string(),
        trxid: "0".to_string(),
        statement: "1".to_string(),
        appname: "app".to_string(),
        client_ip: String::new(),
        sql: "SELECT * FROM users".to_string(),
        exectime: 150.0,
        rowcount: 1,
        exec_id: 1,
    };

    // Filter 跨线程传递（Send），线程内调用 matches（Sync 不需要但这证明 Send）
    let handle = std::thread::spawn(move || filter.matches(&record));
    let result = handle.join().unwrap();
    assert!(result, "exectime=150 > 100 且 sql 含 SELECT，应该匹配");
}

/// apply_filter 与 filter_map(Result::ok) 组合使用（链式可用性验证）
#[test]
#[cfg(not(miri))]
fn test_apply_filter_with_skip_errors_pattern() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = record_line("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1", 200);
    let r2 = record_line("2025-11-17 16:09:41.200", 0, "alice", "SELECT 2", 50);
    write!(file, "{}{}", r1, r2).unwrap();

    let filter = FilterBuilder::new().exec_time_gt(100.0).build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();

    // 链式：apply_filter + filter_map(Result::ok) 产生 Vec<Sqllog>
    let records: Vec<dm_database_parser_sqllog::Sqllog> = parser
        .iter()
        .apply_filter(filter)
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(records.len(), 1);
    assert!(records[0].sql.contains("SELECT 1"));
}

/// apply_filter_keep_errors 与过滤条件组合：满足 filter 的 Ok + 所有 Err 都保留
#[test]
#[cfg(not(miri))]
fn test_apply_filter_keep_errors_with_condition() {
    let mut file = NamedTempFile::new().unwrap();
    let invalid = "this is not a valid log record\n";
    // ep=0，被 ep_gt(1) 过滤掉
    let r1 = record_line_no_metrics("2025-11-17 16:09:41.100", 0, "alice", "SELECT 1");
    // ep=2，通过 ep_gt(1)
    let r2 = record_line_no_metrics("2025-11-17 16:09:41.200", 2, "alice", "SELECT 2");
    write!(file, "{}{}{}", invalid, r1, r2).unwrap();

    let filter = FilterBuilder::new().ep_gt(1).build();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = parser.iter().apply_filter_keep_errors(filter).collect();

    // Err（1 条）+ Ok 满足条件（1 条 ep=2），ep=0 的 Ok 被 filter 丢弃
    assert_eq!(results.len(), 2);
    let err_count = results.iter().filter(|r| r.is_err()).count();
    let ok_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(err_count, 1);
    assert_eq!(ok_count, 1);
    assert_eq!(
        results
            .iter()
            .find(|r| r.is_ok())
            .unwrap()
            .as_ref()
            .unwrap()
            .ep,
        2
    );
}

/// Filter::matches 直接调用验证（单元行为）
#[test]
#[cfg(not(miri))]
fn test_filter_matches_directly() {
    use dm_database_parser_sqllog::Sqllog;

    let record = Sqllog {
        ts: "2025-11-17 16:09:41.100".to_string(),
        tag: None,
        ep: 3,
        sess_id: "1".to_string(),
        thrd_id: "2".to_string(),
        username: "bob".to_string(),
        trxid: "0".to_string(),
        statement: "1".to_string(),
        appname: "app".to_string(),
        client_ip: String::new(),
        sql: "UPDATE t SET x=1".to_string(),
        exectime: 500.0,
        rowcount: 10,
        exec_id: 42,
    };

    // 多条件 AND，全部满足
    let filter: Filter = FilterBuilder::new()
        .ep_between(1, 5)
        .exec_time_gt(100.0)
        .username_eq("bob")
        .build();
    assert!(filter.matches(&record));

    // 有一条件不满足（username 不匹配）
    let strict_filter = FilterBuilder::new()
        .username_eq("alice")
        .exec_time_gt(100.0)
        .build();
    assert!(!strict_filter.matches(&record));
}
