use dm_database_parser_sqllog::LogParser;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn iterator_yields_error_for_invalid_first_line_then_ok() {
    let mut file = NamedTempFile::new().unwrap();
    let bad = "this is not a record\n";
    let good = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) X\n";
    write!(file, "{}{}", bad, good).unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let mut it = parser.iter();
    let r1 = it.next().unwrap();
    assert!(r1.is_err());
    let r2 = it.next().unwrap();
    assert!(r2.is_ok());
}

#[test]
fn iterator_skips_empty_record_slice_between_valid_records() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) A\n";
    let r2 = "2025-11-17 16:09:41.124 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) B\n";
    write!(file, "{}\n{}", r1, r2).unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let v: Vec<_> = parser.iter().collect();
    // Should parse exactly two records
    assert_eq!(v.len(), 2);
    assert!(v[0].as_ref().unwrap().body().contains("A"));
    assert!(v[1].as_ref().unwrap().body().contains("B"));
}
