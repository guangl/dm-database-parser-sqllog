use dm_database_parser_sqllog::{LogParser, parser::parse_record};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn meta_closing_paren_without_space_then_body_on_next_line() {
    // meta 以 ')' 结尾，无空格，正文换至下一行，触发 parse_record_with_hint 的 memrchr 回退分支
    let content = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app)\nSELECT * FROM T\nEXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 7.\n";
    let rec = parse_record(content).expect("parse ok");
    let body = rec.body();
    assert!(body.trim_start().starts_with("SELECT * FROM T"));
    let ind = rec.parse_indicators().unwrap();
    assert_eq!(ind.execute_id, 7);
}

#[test]
fn appname_empty_then_take_next_token_as_appname_not_ip() {
    // 当 appname: 后为空且下个 token 不是 ip:，应将下个 token 作为 appname
    let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname: [SEL] ip:::ffff:10.0.0.1) X";
    let rec = parse_record(raw).unwrap();
    let meta = rec.parse_meta();
    assert_eq!(meta.appname, "[SEL]");
    assert_eq!(meta.client_ip, "::ffff:10.0.0.1");
}

#[test]
fn indicators_not_strictly_formatted_should_not_split_body() {
    // EXEC_ID 无空格：分割器不应识别切分点，整段作为正文
    let raw = b"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:app) SELECT 1; EXEC_ID:123";
    let rec = parse_record(raw).unwrap();
    assert!(rec.indicators_raw().is_none());
    assert!(rec.body().ends_with("EXEC_ID:123"));
}

#[test]
fn probable_record_start_line_and_iterator_singleline_detection() {
    // 第二条记录紧随其后，验证迭代器以单行方式切分
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:1 user:u trxid:1 stmt:1 appname:a) A\n";
    let r2 = "2025-11-17 16:09:41.124 (EP[0] sess:2 thrd:2 user:u trxid:2 stmt:2 appname:b) B EXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 2.\n";
    write!(file, "{}{}", r1, r2).unwrap();
    let parser = LogParser::from_path(file.path()).unwrap();
    let v: Vec<_> = parser.iter().collect();
    assert_eq!(v.len(), 2);
    let s2 = v[1].as_ref().unwrap();
    assert_eq!(s2.ts, "2025-11-17 16:09:41.124");
}
