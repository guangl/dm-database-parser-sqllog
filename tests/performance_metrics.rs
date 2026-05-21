use dm_database_parser_sqllog::parse_record;

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
    assert!((rec.exectime - 10.5).abs() < 1e-6);
    assert_eq!(rec.rowcount, 100);
    assert_eq!(rec.exec_id, 999);
    assert_eq!(rec.sql, "SELECT * FROM T ");
}

#[test]
fn performance_metrics_no_indicators() {
    let raw = build_record("SELECT 1;", "");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.exectime, 0.0);
    assert_eq!(rec.rowcount, 0);
    assert_eq!(rec.exec_id, 0);
    assert_eq!(rec.sql, "SELECT 1;");
}

#[test]
fn performance_metrics_ora_tag_strips_colon_space_prefix() {
    let raw = build_record(
        "[ORA] : SELECT 1 FROM DUAL ",
        "EXECTIME: 5.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 42.",
    );
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("ORA"));
    assert_eq!(rec.sql, "SELECT 1 FROM DUAL ");
    assert!((rec.exectime - 5.0).abs() < 1e-6);
    assert_eq!(rec.rowcount, 1);
    assert_eq!(rec.exec_id, 42);
}

#[test]
fn performance_metrics_ora_tag_no_prefix_unchanged() {
    let raw = build_record(
        "[ORA] SELECT 1 FROM DUAL ",
        "EXECTIME: 5.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 42.",
    );
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("ORA"));
    assert_eq!(rec.sql, "SELECT 1 FROM DUAL ");
}

#[test]
fn performance_metrics_non_ora_tag_keeps_prefix_intact() {
    let raw = build_record("[SEL] : SELECT 1 ", "EXEC_ID: 7.");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("SEL"));
    assert_eq!(rec.sql, ": SELECT 1 ");
}

#[test]
fn performance_metrics_no_tag_keeps_prefix_intact() {
    let raw = build_record(": SELECT 1 ", "EXEC_ID: 7.");
    let rec = parse_record(&raw).unwrap();
    assert!(rec.tag.is_none());
    assert_eq!(rec.sql, ": SELECT 1 ");
}

#[test]
fn performance_metrics_exectime_only() {
    let raw = build_record("DELETE FROM T; ", "EXECTIME: 3.5(ms)");
    let rec = parse_record(&raw).unwrap();
    assert!((rec.exectime - 3.5).abs() < 1e-6);
    assert_eq!(rec.rowcount, 0);
    assert_eq!(rec.exec_id, 0);
    assert_eq!(rec.sql, "DELETE FROM T; ");
}

#[test]
fn performance_metrics_rowcount_only() {
    let raw = build_record("UPDATE T SET A=1; ", "ROWCOUNT: 10(rows)");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.exectime, 0.0);
    assert_eq!(rec.rowcount, 10);
    assert_eq!(rec.exec_id, 0);
}

#[test]
fn performance_metrics_exec_id_only() {
    let raw = build_record("SELECT 1; ", "EXEC_ID: 42.");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.exectime, 0.0);
    assert_eq!(rec.rowcount, 0);
    assert_eq!(rec.exec_id, 42);
}

#[test]
fn performance_metrics_ora_tag_only_colon_space_sql_empty_after_strip() {
    let raw = build_record("[ORA] : ", "EXEC_ID: 1.");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("ORA"));
    assert_eq!(rec.sql, "");
}

#[test]
fn early_exit_no_dot_suffix() {
    let raw = build_record("SELECT * FROM users WHERE id = 1;", "");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.exectime, 0.0);
    assert_eq!(rec.rowcount, 0);
    assert_eq!(rec.exec_id, 0);
}

#[test]
fn dot_suffix_no_real_indicators_guarded() {
    let raw = build_record("SELECT url FROM t WHERE url = 'http://example.com'.", "");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.exec_id, 0);
    assert_eq!(rec.exectime, 0.0);
}

#[test]
fn dot_suffix_with_real_indicators() {
    let raw = build_record(
        "SELECT 1 FROM T ",
        "EXECTIME: 2.5(ms) ROWCOUNT: 5(rows) EXEC_ID: 77.",
    );
    let rec = parse_record(&raw).unwrap();
    assert!((rec.exectime - 2.5).abs() < 1e-6);
    assert_eq!(rec.rowcount, 5);
    assert_eq!(rec.exec_id, 77);
    assert_eq!(rec.sql, "SELECT 1 FROM T ");
}

#[test]
fn fake_keyword_in_body_plus_real_indicators() {
    let raw = build_record(
        "SELECT 'EXECTIME: fake' FROM T ",
        "EXECTIME: 1.0(ms) ROWCOUNT: 3(rows) EXEC_ID: 55.",
    );
    let rec = parse_record(&raw).unwrap();
    assert!((rec.exectime - 1.0).abs() < 1e-6);
    assert_eq!(rec.rowcount, 3);
    assert_eq!(rec.exec_id, 55);
    assert!(rec.sql.contains("EXECTIME: fake"));
}

#[test]
fn multiple_colons_in_body() {
    let raw = build_record(
        "SELECT 'http://example.com:8080/path' FROM T ",
        "EXECTIME: 3.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 99.",
    );
    let rec = parse_record(&raw).unwrap();
    assert!((rec.exectime - 3.0).abs() < 1e-6);
    assert_eq!(rec.rowcount, 1);
    assert_eq!(rec.exec_id, 99);
    assert!(rec.sql.contains("http://example.com:8080/path"));
}

#[test]
fn exec_id_only_split_correct() {
    let raw = build_record("INSERT INTO T VALUES (1); ", "EXEC_ID: 123.");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.exec_id, 123);
    assert_eq!(rec.exectime, 0.0);
    assert_eq!(rec.rowcount, 0);
    assert_eq!(rec.sql, "INSERT INTO T VALUES (1); ");
}
