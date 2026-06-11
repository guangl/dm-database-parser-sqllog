use dm_database_parser_sqllog::LogParserBuilder;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
#[cfg(not(miri))]
fn probable_record_start_line_and_iterator_singleline_detection() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:1 user:u trxid:1 stmt:1 appname:a) A\n";
    let r2 = "2025-11-17 16:09:41.124 (EP[0] sess:2 thrd:2 user:u trxid:2 stmt:2 appname:b) B EXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 2.\n";
    write!(file, "{}{}", r1, r2).unwrap();
    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let v: Vec<_> = parser.iter().unwrap().collect();
    assert_eq!(v.len(), 2);
    let s2 = v[1].as_ref().unwrap();
    assert_eq!(s2.ts, "2025-11-17 16:09:41.124");
}
