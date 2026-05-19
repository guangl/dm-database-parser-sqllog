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
    // Should parse exactly two records
    assert_eq!(v.len(), 2);
    assert!(v[0].as_ref().unwrap().body().contains("A"));
    assert!(v[1].as_ref().unwrap().body().contains("B"));
}

/// 验证 skip_errors() 能过滤掉无效记录，只保留成功解析的 Sqllog。
#[test]
#[cfg(not(miri))]
fn test_skip_errors_filters_invalid_records() {
    let mut file = NamedTempFile::new().unwrap();
    let valid_a = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) VALID_A\n";
    let invalid = "this is not a record\n";
    let valid_b = "2025-11-17 16:09:41.124 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) VALID_B\n";
    write!(file, "{}{}{}", valid_a, invalid, valid_b).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let v: Vec<_> = parser.iter().skip_errors().collect();

    assert_eq!(v.len(), 2);
    assert!(v[0].body().contains("VALID_A"));
    assert!(v[1].body().contains("VALID_B"));
}

/// 验证当文件中同时包含有效和无效记录时，错误信息中包含正确的行号。
#[test]
#[cfg(not(miri))]
fn test_error_contains_correct_line_number() {
    let mut file = NamedTempFile::new().unwrap();
    let good = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) OK\n";
    // A line with valid timestamp prefix but missing meta section (no "(...)")
    // triggers a parse error and is NOT absorbed as multiline body because
    // the trailing good record provides a clear record boundary.
    let bad = "2025-11-17 16:09:41.124 BAD WITHOUT META\n";
    let trailing_good = "2025-11-17 16:09:41.125 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) OK2\n";
    write!(file, "{}{}{}", good, bad, trailing_good).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter();

    // 第 1 条有效
    let r1 = it.next().unwrap();
    assert!(r1.is_ok());

    // 第 2 条无效，验证行号
    let err = it.next().unwrap().unwrap_err();
    match err {
        ParseError::InvalidFormat { line_number, .. } => {
            assert_eq!(line_number, 2);
        }
        _ => panic!("Expected InvalidFormat"),
    }
}

/// 验证在多条有效记录后出现的错误仍持有正确的行号。
#[test]
#[cfg(not(miri))]
fn test_line_number_after_multiple_valid_records() {
    let mut file = NamedTempFile::new().unwrap();
    let r1 = "2025-11-17 16:09:41.121 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) A\n";
    let r2 = "2025-11-17 16:09:41.122 (EP[0] sess:2 thrd:3 user:u trxid:4 stmt:5 appname:b) B\n";
    let r3 = "2025-11-17 16:09:41.123 (EP[0] sess:3 thrd:4 user:u trxid:5 stmt:6 appname:c) C\n";
    // Line with timestamp prefix but missing meta section triggers a parse error.
    let bad = "2025-11-17 16:09:41.124 BAD WITHOUT META\n";
    let trailing_good = "2025-11-17 16:09:41.125 (EP[0] sess:4 thrd:5 user:u trxid:6 stmt:7 appname:d) D\n";
    write!(file, "{}{}{}{}{}", r1, r2, r3, bad, trailing_good).unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let mut it = parser.iter();

    // 前 3 条有效
    assert!(it.next().unwrap().is_ok());
    assert!(it.next().unwrap().is_ok());
    assert!(it.next().unwrap().is_ok());

    // 第 4 条无效，验证行号
    let err = it.next().unwrap().unwrap_err();
    match err {
        ParseError::InvalidFormat { line_number, .. } => {
            assert_eq!(line_number, 4);
        }
        _ => panic!("Expected InvalidFormat"),
    }
}

/// 编译期验证 ParseError 实现了 std::error::Error trait。
#[test]
fn test_parse_error_impl_std_error() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<ParseError>();
}

/// 验证所有 ParseError 变体的 Display 格式正确。
#[test]
fn test_error_display_contains_line_number() {
    // InvalidFormat — 应包含行号
    let err = ParseError::InvalidFormat {
        raw: "test".to_string(),
        line_number: 42,
    };
    let msg = err.to_string();
    assert!(msg.contains("42"), "InvalidFormat display should contain line number 42, got: {msg}");
    assert!(msg.contains("line"), "InvalidFormat display should contain 'line', got: {msg}");

    // FileNotFound — 不包含行号，但应包含路径
    let err = ParseError::FileNotFound {
        path: "missing.log".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("missing.log"), "FileNotFound display should contain path, got: {msg}");

    // IoError — 应包含错误描述
    let err = ParseError::IoError("permission denied".to_string());
    let msg = err.to_string();
    assert!(msg.contains("permission denied"), "IoError display should contain message, got: {msg}");
}

/// 验证多行记录之后的行号计数是否正确。
#[test]
#[cfg(not(miri))]
fn test_line_number_with_multiline_record() {
    let mut file = NamedTempFile::new().unwrap();
    // Multiline record spanning 3 lines, followed by invalid record
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
    assert!(r1.is_ok(), "first record should parse OK");

    let err = it.next().unwrap().unwrap_err();
    match err {
        ParseError::InvalidFormat { line_number, .. } => {
            // Multiline record spans lines 1-3; bad record starts at line 4.
            assert_eq!(line_number, 4, "expected line_number 4, got {line_number}");
        }
        _ => panic!("Expected InvalidFormat, got {err:?}"),
    }
}

/// 通过公共 parse_record API 间接验证 is_timestamp_start 的行为。
#[test]
fn test_parse_record_timestamp_validation() {
    use dm_database_parser_sqllog::parse_record;

    // Valid record with correct timestamp — should parse OK.
    let valid = b"2025-11-17 16:09:41.123 (EP[0]) SELECT";
    let result = parse_record(valid);
    assert!(result.is_ok(), "valid record should parse OK");

    // Record where first 23 bytes look like a timestamp but meta is missing —
    // timestamp validation passes, but parsing fails on meta extraction.
    let bad_ts_no_meta = b"2025-11-17 16:09:41.123 INVALID NO META";
    let result = parse_record(bad_ts_no_meta);
    assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));

    // Record shorter than 23 bytes — caught by length check before timestamp parsing.
    let short = b"2025-11-17 16:0";
    let result = parse_record(short);
    assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));

    // Record with ASCII byte at position 2 that breaks "20" prefix —
    // is_timestamp_start says false (not a timestamp), but parse_record
    // still reads it as a record and fails on meta.
    let wrong_ts = b"1025-11-17 16:09:41.123 (EP[0]) X";
    let result = parse_record(wrong_ts);
    assert!(result.is_ok(), "record with non-standard prefix should still parse if meta is present");
}
