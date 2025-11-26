use dm_database_parser_sqllog::*;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

fn create_temp_file_with_content(content: &str) -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.log");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    (temp_dir, file_path)
}

const VALID_SINGLE_RECORD: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";

const VALID_MULTIPLE_RECORDS: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n\
    2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2\n\
    2025-08-12 12:30:15.123 (EP[2] sess:789 thrd:123 user:charlie trxid:222 stmt:333 appname:demo) INSERT INTO logs";

const VALID_MULTILINE_RECORD: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\n\
FROM users\n\
WHERE id > 0";

const MIXED_VALID_INVALID: &str = "invalid line\n\
another invalid\n\
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n\
2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2";

#[test]
fn test_iter_records_from_file_success() {
    let (_temp_dir, file_path) = create_temp_file_with_content(VALID_SINGLE_RECORD);
    let sqllogs: Vec<_> = iter_records_from_file(&file_path).collect();
    assert_eq!(sqllogs.len(), 1);
    assert!(sqllogs[0].is_ok());
}

#[test]
fn test_iter_records_from_file_multiple() {
    let (_temp_dir, file_path) = create_temp_file_with_content(VALID_MULTIPLE_RECORDS);
    let sqllogs: Vec<_> = iter_records_from_file(&file_path).collect();
    assert_eq!(sqllogs.len(), 3);
    assert!(sqllogs.iter().all(|r| r.is_ok()));
}

#[test]
fn test_iter_records_from_file_multiline() {
    let (_temp_dir, file_path) = create_temp_file_with_content(VALID_MULTILINE_RECORD);
    let sqllogs: Vec<_> = iter_records_from_file(&file_path).collect();
    assert_eq!(sqllogs.len(), 1);

    let sqllog = sqllogs[0].as_ref().unwrap();
    assert!(sqllog.body.contains("FROM users"));
    assert!(sqllog.body.contains("WHERE id > 0"));
}

#[test]
fn test_iter_records_from_file_skip_invalid() {
    let (_temp_dir, file_path) = create_temp_file_with_content(MIXED_VALID_INVALID);
    let sqllogs: Vec<_> = iter_records_from_file(&file_path).collect();
    assert_eq!(sqllogs.len(), 2);
}

#[test]
fn test_iter_records_from_file_nonexistent() {
    let sqllogs: Vec<_> = iter_records_from_file("nonexistent.log").collect();
    assert_eq!(sqllogs.len(), 1);
    assert!(sqllogs[0].is_err());
}

#[test]
fn test_parse_records_from_file_success() {
    let (_temp_dir, file_path) = create_temp_file_with_content(VALID_SINGLE_RECORD);
    let (sqllogs, errors) = parse_records_from_file(&file_path);
    assert_eq!(sqllogs.len(), 1);
    assert_eq!(errors.len(), 0);
    assert_eq!(sqllogs[0].meta.username, "alice");
}

#[test]
fn test_parse_records_from_file_multiple() {
    let (_temp_dir, file_path) = create_temp_file_with_content(VALID_MULTIPLE_RECORDS);
    let (sqllogs, errors) = parse_records_from_file(&file_path);
    assert_eq!(sqllogs.len(), 3);
    assert_eq!(errors.len(), 0);
    assert_eq!(sqllogs[0].meta.username, "alice");
    assert_eq!(sqllogs[1].meta.username, "bob");
    assert_eq!(sqllogs[2].meta.username, "charlie");
}

#[test]
fn test_parse_records_from_file_multiline() {
    let (_temp_dir, file_path) = create_temp_file_with_content(VALID_MULTILINE_RECORD);
    let (sqllogs, errors) = parse_records_from_file(&file_path);
    assert_eq!(sqllogs.len(), 1);
    assert_eq!(errors.len(), 0);
    assert!(sqllogs[0].body.contains("FROM users"));
    assert!(sqllogs[0].body.contains("WHERE id > 0"));
}

#[test]
fn test_parse_records_from_file_skip_invalid() {
    let (_temp_dir, file_path) = create_temp_file_with_content(MIXED_VALID_INVALID);
    let (sqllogs, _errors) = parse_records_from_file(&file_path);
    assert_eq!(sqllogs.len(), 2);
}

#[test]
fn test_parse_records_from_file_nonexistent() {
    let (_sqllogs, errors) = parse_records_from_file("nonexistent.log");
    assert_eq!(errors.len(), 1);
    match &errors[0] {
        ParseError::FileNotFound { path: _ } => (),
        _ => panic!("expected FileNotFound"),
    }
}

#[test]
fn test_parse_records_from_file_empty() {
    let (_temp_dir, file_path) = create_temp_file_with_content("");
    let (sqllogs, errors) = parse_records_from_file(&file_path);
    assert_eq!(sqllogs.len(), 0);
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_iter_records_from_file_empty() {
    let (_temp_dir, file_path) = create_temp_file_with_content("");
    let sqllogs: Vec<_> = iter_records_from_file(&file_path).collect();
    assert_eq!(sqllogs.len(), 0);
}

#[test]
fn test_parse_records_from_file_all_invalid() {
    let (_temp_dir, file_path) = create_temp_file_with_content("invalid\nlines\nonly");
    let (sqllogs, _errors) = parse_records_from_file(&file_path);
    assert_eq!(sqllogs.len(), 0);
}

#[test]
fn test_iter_records_from_file_all_invalid() {
    let (_temp_dir, file_path) = create_temp_file_with_content("invalid\nlines\nonly");
    let sqllogs: Vec<_> = iter_records_from_file(&file_path).collect();
    assert_eq!(sqllogs.len(), 0);
}
