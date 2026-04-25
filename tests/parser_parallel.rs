use dm_database_parser_sqllog::LogParser;
use rayon::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn make_record(ts_suffix: &str, body: &str) -> String {
    format!(
        "202{ts_suffix} (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:app) {body}\n"
    )
}

#[test]
#[cfg(not(miri))]
fn par_iter_yields_same_count_as_iter() {
    let mut file = NamedTempFile::new().unwrap();
    // Write 60 single-line records so rayon splits across multiple threads
    for i in 0..60u32 {
        let rec = make_record(
            &format!("5-01-01 10:{:02}:{:02}.000", i / 60, i % 60),
            &format!("SELECT {i}"),
        );
        file.write_all(rec.as_bytes()).unwrap();
    }
    file.flush().unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();

    let sequential_count = parser.iter().filter_map(|r| r.ok()).count();
    let parallel_count = parser.par_iter().filter_map(|r| r.ok()).count();

    assert_eq!(sequential_count, parallel_count);
    assert_eq!(sequential_count, 60);
}

#[test]
#[cfg(not(miri))]
fn par_iter_with_multiline_records() {
    let mut file = NamedTempFile::new().unwrap();
    // Mix single-line and multiline records — forces chunk boundaries inside multiline SQL
    for i in 0..40u32 {
        if i % 5 == 0 {
            // Multiline record with embedded newlines (no \n20 in body)
            let rec = format!(
                "2025-02-01 10:{:02}:{:02}.000 (EP[0] sess:{i} thrd:2 user:U trxid:3 stmt:4 appname:app) SELECT\n    col1,\n    col2\nFROM t\nWHERE id = {i} EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: {i}.\n",
                i / 60, i % 60
            );
            file.write_all(rec.as_bytes()).unwrap();
        } else {
            let rec = make_record(
                &format!("5-02-01 10:{:02}:{:02}.100", i / 60, i % 60),
                &format!("SELECT {i} EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: {i}."),
            );
            file.write_all(rec.as_bytes()).unwrap();
        }
    }
    file.flush().unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();

    let sequential_count = parser.iter().filter_map(|r| r.ok()).count();
    let parallel_count = parser.par_iter().filter_map(|r| r.ok()).count();

    assert_eq!(sequential_count, parallel_count);
    assert_eq!(sequential_count, 40);
}

#[test]
#[cfg(not(miri))]
fn par_iter_empty_file() {
    let file = NamedTempFile::new().unwrap();
    let parser = LogParser::from_path(file.path()).unwrap();

    let count = parser.par_iter().count();
    assert_eq!(count, 0);
}
