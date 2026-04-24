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

// ── HOT-01 早退逻辑测试 ────────────────────────────────────────────────────────

/// 末尾为纯 SQL 文本（无 '.' 结尾）的记录应被早退，body_len == content_raw 全长
#[test]
fn hot01_early_exit_no_dot_suffix() {
    // SQL 语句末尾为 ';'，不含指标，应早退（无截断）
    let raw = build_record("SELECT * FROM users WHERE id = 1;", "");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    // 无指标时，sql == 全部 content_raw
    assert_eq!(pm.exectime, 0.0);
    assert_eq!(pm.rowcount, 0);
    assert_eq!(pm.exec_id, 0);
    // body_len() 应等于 content_raw 长度（早退路径，无截断）
    assert_eq!(rec.body_len(), rec.content_raw.len());
}

/// 末尾为 '\n' 的无指标记录应被早退
#[test]
fn hot01_early_exit_newline_suffix() {
    let header =
        b"2025-11-17 16:09:41.123 (EP[1] sess:123 thrd:456 user:alice trxid:789 stmt:0x1 appname:bench) ";
    let mut raw = Vec::new();
    raw.extend_from_slice(header);
    raw.extend_from_slice(b"SELECT 1;\n");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.exec_id, 0);
    assert_eq!(rec.body_len(), rec.content_raw.len());
}

/// 末尾为 '.' 但无真实指标的记录，CORR-03 守卫最终返回全文（不截断）
#[test]
fn hot01_dot_suffix_no_real_indicators_guarded() {
    // SQL 以 '.' 结尾（URL 或语句结尾），不含指标关键字
    let raw = build_record("SELECT url FROM t WHERE url = 'http://example.com'.", "");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    // CORR-03 守卫应拦截假阳性，返回全文
    assert_eq!(pm.exec_id, 0);
    assert_eq!(pm.exectime, 0.0);
}

/// 末尾为 '.' 且含真实指标的记录，应正常分割，指标被正确解析
#[test]
fn hot01_dot_suffix_with_real_indicators() {
    let raw = build_record(
        "SELECT 1 FROM T ",
        "EXECTIME: 2.5(ms) ROWCOUNT: 5(rows) EXEC_ID: 77.",
    );
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert!((pm.exectime - 2.5).abs() < 1e-6);
    assert_eq!(pm.rowcount, 5);
    assert_eq!(pm.exec_id, 77);
    assert_eq!(pm.sql, "SELECT 1 FROM T ");
}

// ── HOT-02 单次反向扫描测试 ────────────────────────────────────────────────────

/// SQL body 内含假关键字 'EXECTIME:' + 真实指标，split 应返回真实指标起始
#[test]
fn hot02_fake_keyword_in_body_plus_real_indicators() {
    // SQL body 中包含 "EXECTIME:" 字样（假关键字），真实指标在末尾
    let raw = build_record(
        "SELECT 'EXECTIME: fake' FROM T ",
        "EXECTIME: 1.0(ms) ROWCOUNT: 3(rows) EXEC_ID: 55.",
    );
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    // 真实指标应被正确解析
    assert!((pm.exectime - 1.0).abs() < 1e-6);
    assert_eq!(pm.rowcount, 3);
    assert_eq!(pm.exec_id, 55);
    // SQL body 应包含假关键字（说明 split 点在真实指标处）
    assert!(pm.sql.contains("EXECTIME: fake"));
}

/// SQL body 含多个 ':' 字符（如 URL），不影响 split 结果
#[test]
fn hot02_multiple_colons_in_body() {
    let raw = build_record(
        "SELECT 'http://example.com:8080/path' FROM T ",
        "EXECTIME: 3.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 99.",
    );
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert!((pm.exectime - 3.0).abs() < 1e-6);
    assert_eq!(pm.rowcount, 1);
    assert_eq!(pm.exec_id, 99);
    assert!(pm.sql.contains("http://example.com:8080/path"));
}

/// 仅有 EXEC_ID 无 EXECTIME/ROWCOUNT 的记录，split 正确
#[test]
fn hot02_exec_id_only_split_correct() {
    let raw = build_record("INSERT INTO T VALUES (1); ", "EXEC_ID: 123.");
    let rec = parse_record(&raw).unwrap();
    let pm = rec.parse_performance_metrics();
    assert_eq!(pm.exec_id, 123);
    assert_eq!(pm.exectime, 0.0);
    assert_eq!(pm.rowcount, 0);
    assert_eq!(pm.sql, "INSERT INTO T VALUES (1); ");
}
