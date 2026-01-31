use dm_database_parser_sqllog::parser::parse_record;

fn build_record(line1_body: &str, tail: &str) -> Vec<u8> {
    let header = b"2025-11-17 16:09:41.123 (EP[1] sess:123 thrd:456 user:alice trxid:789 stmt:0x1 appname:bench) ";
    let mut v = Vec::new();
    v.extend_from_slice(header);
    v.extend_from_slice(line1_body.as_bytes());
    if !tail.is_empty() {
        v.extend_from_slice(tail.as_bytes());
    }
    v
}

#[test]
fn body_without_indicators() {
    let raw = build_record("SELECT 1;", "");
    let rec = parse_record(&raw).expect("parse ok");
    assert_eq!(rec.body(), "SELECT 1;");
    assert!(rec.parse_indicators().is_none());
}

#[test]
fn indicators_exec_id_only() {
    let raw = build_record("SELECT 1; ", "EXEC_ID: 42.");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.body(), "SELECT 1; ");
    let ind = rec.parse_indicators().unwrap();
    assert_eq!(ind.execute_id, 42);
}

#[test]
fn indicators_rowcount_only() {
    let raw = build_record("UPDATE T SET A=1; ", "ROWCOUNT: 10(rows)");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.body(), "UPDATE T SET A=1; ");
    let ind = rec.parse_indicators().unwrap();
    assert_eq!(ind.row_count, 10);
}

#[test]
fn indicators_exectime_only() {
    let raw = build_record("DELETE FROM T; ", "EXECTIME: 3.5(ms)");
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.body(), "DELETE FROM T; ");
    let ind = rec.parse_indicators().unwrap();
    assert!((ind.execute_time - 3.5).abs() < 1e-6);
}

#[test]
fn indicators_permutation_all() {
    let tail = "ROWCOUNT: 5(rows) EXECTIME: 12.25(ms) EXEC_ID: 999.";
    let raw = build_record("SELECT * FROM T ", tail);
    let rec = parse_record(&raw).unwrap();
    assert_eq!(rec.body(), "SELECT * FROM T ");
    let ind = rec.parse_indicators().unwrap();
    assert_eq!(ind.row_count, 5);
    assert!((ind.execute_time - 12.25).abs() < 1e-6);
    assert_eq!(ind.execute_id, 999);
}

#[test]
fn meta_parsing_basic() {
    let raw = b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:SYSDBA trxid:0 stmt:0x2 appname:cli) SELECT";
    let rec = parse_record(raw).unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.ep, 2);
    assert_eq!(meta.sess_id, "0xABC");
    assert_eq!(meta.thrd_id, "777");
    assert_eq!(meta.username, "SYSDBA");
    assert_eq!(meta.trxid, "0");
    assert_eq!(meta.statement, "0x2");
    assert_eq!(meta.appname, "cli");
    // client_ip 可能未填，此处不做断言
}

#[test]
fn meta_parsing_empty_appname() {
    let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:) X";
    let rec = parse_record(raw).unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.appname, "");
}

#[test]
fn appname_empty_followed_by_ip_colon_single_should_keep_appname_empty() {
    // appname: 后跟 token 为 ip:1.2.3.4（单冒号），应识别为 ip 字段而非 appname 的值
    let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname: ip:10.1.1.1) X";
    let rec = parse_record(raw).unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.appname, "");
    assert_eq!(meta.client_ip, "10.1.1.1");
}

#[test]
fn appname_empty_followed_by_ip_triple_colon_should_keep_appname_empty() {
    // appname: 后跟 token 为 ip:::ffff:10.3.100.68（三冒号形式），应识别为 ip 字段而非 appname 的值
    let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname: ip:::ffff:10.3.100.68) X";
    let rec = parse_record(raw).unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.appname, "");
    assert_eq!(meta.client_ip, "::ffff:10.3.100.68");
}

#[test]
fn meta_parsing_gb18030_username() {
    use encoding::all::GB18030;
    use encoding::{EncoderTrap, Encoding};

    let username = "用户";
    let user_bytes = GB18030
        .encode(username, EncoderTrap::Strict)
        .expect("encode");

    let mut raw: Vec<u8> = b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:".to_vec();
    raw.extend_from_slice(&user_bytes);
    raw.extend_from_slice(b" trxid:0 stmt:0x2 appname:cli) SELECT");

    let rec = parse_record(&raw).unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.username, username);
}

#[test]
fn tag_extraction_and_body_trim() {
    let raw = b"2025-11-17 16:09:41.123 (EP[1] sess:123 thrd:456 user:u trxid:3 stmt:4 appname:bench) [SEL] SELECT 1; EXEC_ID: 42.";
    let rec = parse_record(raw).unwrap();
    assert_eq!(rec.tag.as_deref(), Some("SEL"));
    assert_eq!(rec.body(), "SELECT 1; ");
}

#[test]
fn file_encoding_detection_gb18030() {
    use dm_database_parser_sqllog::parser::LogParser;
    use encoding::all::GB18030;
    use encoding::{EncoderTrap, Encoding};
    use std::io::Write;
    use tempfile::NamedTempFile;

    let username = "用户";
    let user_bytes = GB18030
        .encode(username, EncoderTrap::Strict)
        .expect("encode");

    let mut line: Vec<u8> = b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:".to_vec();
    line.extend_from_slice(&user_bytes);
    line.extend_from_slice(b" trxid:0 stmt:0x2 appname:cli) SELECT\n");

    let mut tmp = NamedTempFile::new().expect("tmp");
    tmp.write_all(&line).expect("write");
    tmp.as_file().sync_all().expect("sync");

    let parser = LogParser::from_path(tmp.path()).expect("open");
    let rec = parser.iter().next().unwrap().unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.username, username);
}

#[test]
fn file_encoding_detection_utf8() {
    use dm_database_parser_sqllog::parser::LogParser;
    use std::io::Write;
    use tempfile::NamedTempFile;

    let username = "用户";
    let user_bytes = username.as_bytes();

    let mut line: Vec<u8> = b"2025-11-17 16:09:41.123 (EP[2] sess:0xABC thrd:777 user:".to_vec();
    line.extend_from_slice(&user_bytes);
    line.extend_from_slice(b" trxid:0 stmt:0x2 appname:cli) SELECT\n");

    let mut tmp = NamedTempFile::new().expect("tmp");
    tmp.write_all(&line).expect("write");
    tmp.as_file().sync_all().expect("sync");

    let parser = LogParser::from_path(tmp.path()).expect("open");
    let rec = parser.iter().next().unwrap().unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.username, username);
}
