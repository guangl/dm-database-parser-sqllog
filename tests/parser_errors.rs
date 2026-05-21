use dm_database_parser_sqllog::{LogParserBuilder, ParseError};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
#[cfg(not(miri))]
fn iterator_yields_error_for_invalid_first_line_then_ok() {
    let mut file = NamedTempFile::new().unwrap();
    let bad = "this is not a record\n";
    let good = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) X\n";
    write!(file, "{}{}", bad, good).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter();
    let r1 = it.next().unwrap();
    assert!(r1.is_err());
    let r2 = it.next().unwrap();
    assert!(r2.is_ok());
}

#[test]
#[cfg(not(miri))]
fn iterator_skips_empty_record_slice_between_valid_records() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) A\n";
    let r2 = "2025-11-17 16:09:41.124 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) B\n";
    write!(file, "{}\n{}", r1, r2).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let v: Vec<_> = parser.iter().collect();
    assert_eq!(v.len(), 2);
    assert!(v[0].as_ref().unwrap().sql.contains("A"));
    assert!(v[1].as_ref().unwrap().sql.contains("B"));
}

#[test]
#[cfg(not(miri))]
fn test_skip_errors_filters_invalid_records() {
    let mut file = NamedTempFile::new().unwrap();
    let valid_a =
        "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) VALID_A\n";
    let invalid = "this is not a record\n";
    let valid_b =
        "2025-11-17 16:09:41.124 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) VALID_B\n";
    write!(file, "{}{}{}", valid_a, invalid, valid_b).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let v: Vec<_> = parser.iter().skip_errors().collect();

    assert_eq!(v.len(), 2);
    assert!(v[0].sql.contains("VALID_A"));
    assert!(v[1].sql.contains("VALID_B"));
}

#[test]
#[cfg(not(miri))]
fn test_error_contains_correct_line_number() {
    let mut file = NamedTempFile::new().unwrap();
    let good = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) OK\n";
    let bad = "2025-11-17 16:09:41.124 BAD WITHOUT META\n";
    let trailing_good =
        "2025-11-17 16:09:41.125 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) OK2\n";
    write!(file, "{}{}{}", good, bad, trailing_good).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter();

    let r1 = it.next().unwrap();
    assert!(r1.is_ok());

    let err = it.next().unwrap().unwrap_err();
    match err {
        ParseError::InvalidFormat { line_number, .. } => {
            assert_eq!(line_number, 2);
        }
        _ => panic!("Expected InvalidFormat"),
    }
}

#[test]
#[cfg(not(miri))]
fn test_line_number_after_multiple_valid_records() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.121 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) A\n";
    let r2 = "2025-11-17 16:09:41.122 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) B\n";
    let r3 = "2025-11-17 16:09:41.123 (EP[0] sess:3 thrd:4 user:u trxid:5 stmt:6 appname:c) C\n";
    let bad = "2025-11-17 16:09:41.124 BAD WITHOUT META\n";
    let trailing_good =
        "2025-11-17 16:09:41.125 (EP[0] sess:4 thrd:5 user:u trxid:6 stmt:7 appname:d) D\n";
    write!(file, "{}{}{}{}{}", r1, r2, r3, bad, trailing_good).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter();

    assert!(it.next().unwrap().is_ok());
    assert!(it.next().unwrap().is_ok());
    assert!(it.next().unwrap().is_ok());

    let err = it.next().unwrap().unwrap_err();
    match err {
        ParseError::InvalidFormat { line_number, .. } => {
            assert_eq!(line_number, 4);
        }
        _ => panic!("Expected InvalidFormat"),
    }
}

#[test]
fn test_parse_error_impl_std_error() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<ParseError>();
}

#[test]
fn test_error_display_contains_line_number() {
    let err = ParseError::InvalidFormat {
        raw: "test".to_string(),
        line_number: 42,
    };
    let msg = err.to_string();
    assert!(msg.contains("42"));
    assert!(msg.contains("line"));

    let err = ParseError::FileNotFound {
        path: "missing.log".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("missing.log"));

    let err = ParseError::IoError("permission denied".to_string());
    let msg = err.to_string();
    assert!(msg.contains("permission denied"));
}

#[test]
#[cfg(not(miri))]
fn test_line_number_with_multiline_record() {
    let mut file = NamedTempFile::new().unwrap();
    let content = concat!(
        "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT\n",
        "  col1\n",
        "  FROM t\n",
        "2025-11-17 16:09:41.124 BAD WITHOUT META\n",
    );
    file.write_all(content.as_bytes()).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter();

    let r1 = it.next().unwrap();
    assert!(r1.is_ok());

    let err = it.next().unwrap().unwrap_err();
    match err {
        ParseError::InvalidFormat { line_number, .. } => {
            assert_eq!(line_number, 4);
        }
        _ => panic!("Expected InvalidFormat, got {err:?}"),
    }
}

#[test]
fn test_parse_record_timestamp_validation() {
    use dm_database_parser_sqllog::parse_record;

    let valid = b"2025-11-17 16:09:41.123 (EP[0]) SELECT";
    let result = parse_record(valid);
    assert!(result.is_ok());

    let bad_ts_no_meta = b"2025-11-17 16:09:41.123 INVALID NO META";
    let result = parse_record(bad_ts_no_meta);
    assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));

    let short = b"2025-11-17 16:0";
    let result = parse_record(short);
    assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));
}
