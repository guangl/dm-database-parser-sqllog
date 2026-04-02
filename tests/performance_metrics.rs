use dm_database_parser_sqllog::parser::parse_record;

fn build_record(tag_and_body: &str, tail: &str) -> Vec<u8> {
    let header =
        b"2025-11-17 16:09:41.123 (EP[1] sess:123 thrd:456 user:alice trxid:789 stmt:0x1 appname:bench) ";
    let mut v = Vec::new();
    v.extend_from_slice(header);
    v.extend_from_slice(tag_and_body.as_bytes());
    if !tail.is_empty() {
        v.extend_from_slice(tail.as_bytes());
    }
    v
}

#[test]
fn performance_metrics_full() {
    let raw = build_record(
        "SELECT * FROM T ",
        "EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 999.",
    );
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert!((pm.exectime - 10.5).abs() < 1e-6);
    assert_eq!(pm.rowcount, 100);
    assert_eq!(pm.exec_id, 999);
    assert_eq!(pm.sql, "SELECT * FROM T ");
}

#[test]
fn performance_metrics_no_indicators() {
    let raw = build_record("SELECT 1;", "");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.exectime, 0.0);
    assert_eq!(pm.rowcount, 0);
    assert_eq!(pm.exec_id, 0);
    assert_eq!(pm.sql, "SELECT 1;");
}

#[test]
fn performance_metrics_ora_tag_strips_colon_space_prefix() {
    // ORA tag: SQL 前端带 ": "，应当被去除
    let raw = build_record(
        "[ORA] : SELECT 1 FROM DUAL ",
        "EXECTIME: 5.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 42.",
    );
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("ORA"));
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.sql, "SELECT 1 FROM DUAL ");
    assert!((pm.exectime - 5.0).abs() < 1e-6);
    assert_eq!(pm.rowcount, 1);
    assert_eq!(pm.exec_id, 42);
}

#[test]
fn performance_metrics_ora_tag_no_prefix_unchanged() {
    // ORA tag 但 SQL 没有 ": " 前缀，保持不变
    let raw = build_record(
        "[ORA] SELECT 1 FROM DUAL ",
        "EXECTIME: 5.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 42.",
    );
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("ORA"));
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.sql, "SELECT 1 FROM DUAL ");
}

#[test]
fn performance_metrics_non_ora_tag_keeps_prefix_intact() {
    // 非 ORA tag（SEL）时，即使 SQL 开头有 ": " 也不去除
    let raw = build_record("[SEL] : SELECT 1 ", "EXEC_ID: 7.");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("SEL"));
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.sql, ": SELECT 1 ");
}

#[test]
fn performance_metrics_no_tag_keeps_prefix_intact() {
    // 无 tag 时，SQL 开头有 ": " 不去除
    let raw = build_record(": SELECT 1 ", "EXEC_ID: 7.");
    let rec = parse_record(&raw).unwrap();
    assert!(rec.tag.is_none());
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.sql, ": SELECT 1 ");
}

#[test]
fn performance_metrics_exectime_only() {
    let raw = build_record("DELETE FROM T; ", "EXECTIME: 3.5(ms)");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert!((pm.exectime - 3.5).abs() < 1e-6);
    assert_eq!(pm.rowcount, 0);
    assert_eq!(pm.exec_id, 0);
    assert_eq!(pm.sql, "DELETE FROM T; ");
}

#[test]
fn performance_metrics_rowcount_only() {
    let raw = build_record("UPDATE T SET A=1; ", "ROWCOUNT: 10(rows)");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.exectime, 0.0);
    assert_eq!(pm.rowcount, 10);
    assert_eq!(pm.exec_id, 0);
}

#[test]
fn performance_metrics_exec_id_only() {
    let raw = build_record("SELECT 1; ", "EXEC_ID: 42.");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.exectime, 0.0);
    assert_eq!(pm.rowcount, 0);
    assert_eq!(pm.exec_id, 42);
}

#[test]
fn performance_metrics_ora_tag_only_colon_space_sql_empty_after_strip() {
    // 极端情况：SQL 只有 ": "，去除后为空字符串
    let raw = build_record("[ORA] : ", "EXEC_ID: 1.");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("ORA"));
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.sql, "");
}
