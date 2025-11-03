use dm_database_parser_sqllog::matcher::Matcher;
use std::fs;

#[test]
fn integration_patterns_and_matcher() {
    // 从 fixtures 加载模式
    let txt = fs::read_to_string("tests/fixtures/patterns.txt").expect("read patterns");
    let pats: Vec<&str> = txt.lines().collect();
    let m = Matcher::from_patterns(&pats);

    let meta = "EP[0] foobar sess:abc baz thrd:1 qux user:joe trxid:123 stmt:0x1 zz appname:my";
    let fp = m.find_first_positions(meta.as_bytes());
    // 期望所有模式都存在
    assert_eq!(fp.len(), pats.len());
    for (i, p) in fp.iter().enumerate() {
        assert!(p.is_some(), "pattern {} missing", pats[i]);
    }

    // 确保顺序：位置严格递增
    let mut prev = None;
    for p in fp.iter() {
        let cur = p.unwrap();
        if let Some(prev_pos) = prev {
            assert!(cur > prev_pos, "positions not strictly increasing");
        }
        prev = Some(cur);
    }
}
