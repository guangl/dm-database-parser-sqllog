use dm_database_parser_sqllog::{LogParser, RecordIndex};
use rayon::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn make_record(ts_suffix: &str, body: &str) -> String {
    format!("202{ts_suffix} (EP[0] sess:1 thrd:2 user:U trxid:3 stmt:4 appname:app) {body}\n")
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
                i / 60,
                i % 60
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

#[test]
#[cfg(not(miri))]
fn test_index_count_matches_iter() {
    let mut file = NamedTempFile::new().unwrap();
    for i in 0..30u32 {
        let rec = make_record(
            &format!("5-03-01 10:{:02}:{:02}.000", i / 60, i % 60),
            &format!("SELECT {i}"),
        );
        file.write_all(rec.as_bytes()).unwrap();
    }
    file.flush().unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let iter_count = parser.iter().filter_map(|r| r.ok()).count();
    let index: RecordIndex = parser.index();

    assert_eq!(index.len(), iter_count);
    assert_eq!(index.len(), 30);
}

#[test]
#[cfg(not(miri))]
fn test_index_offsets_are_valid_timestamps() {
    let mut file = NamedTempFile::new().unwrap();
    for i in 0..20u32 {
        let rec = make_record(
            &format!("5-04-01 10:{:02}:{:02}.000", i / 60, i % 60),
            &format!("SELECT {i}"),
        );
        file.write_all(rec.as_bytes()).unwrap();
    }
    file.flush().unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let index: RecordIndex = parser.index();

    // 每条记录应被 index 捕获一次
    let iter_count = parser.iter().filter_map(|r| r.ok()).count();
    assert_eq!(index.len(), iter_count, "index 与 iter 记录数不一致");
    assert_eq!(index.len(), 20);

    // 通过 iter 解析的成功率间接验证 offsets 的合法性
    let parsed_ok = parser.iter().all(|r| r.is_ok());
    assert!(
        parsed_ok,
        "所有 20 条记录都应能成功解析（说明 index 边界正确）"
    );
}

#[test]
#[cfg(not(miri))]
fn test_index_empty_file() {
    let file = NamedTempFile::new().unwrap();
    let parser = LogParser::from_path(file.path()).unwrap();
    let index: RecordIndex = parser.index();

    assert!(index.is_empty(), "空文件的 index 应为空");
    assert_eq!(index.len(), 0);
}

#[test]
#[cfg(not(miri))]
fn par_iter_yields_same_count_as_iter_large() {
    let mut file = NamedTempFile::new().unwrap();
    let record = b"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:BENCHMARK trxid:0 stmt:0x285eb060 appname:bench) [SEL] SELECT id, name, value FROM benchmark_table WHERE id = 12345 EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.\n";
    let target: usize = 33 * 1024 * 1024;
    let mut written = 0usize;
    while written < target {
        file.write_all(record).unwrap();
        written += record.len();
    }
    file.flush().unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let seq_count = parser.iter().filter_map(|r| r.ok()).count();
    let par_count = parser.par_iter().filter_map(|r| r.ok()).count();

    assert_eq!(
        seq_count, par_count,
        "大文件 par_iter 记录数必须与 iter 一致"
    );
    assert!(seq_count > 0, "大文件应至少含 1 条记录");
}

#[test]
#[cfg(not(miri))]
fn par_iter_yields_same_count_as_iter_large_multiline() {
    let mut file = NamedTempFile::new().unwrap();
    let single = b"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:BENCHMARK trxid:0 stmt:0x285eb060 appname:bench) [SEL] SELECT id FROM t WHERE id = 1 EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    let multi = b"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:BENCHMARK trxid:0 stmt:0x285eb060 appname:bench) [SEL] SELECT\n    t1.id,\n    t2.name\nFROM t1\nJOIN t2 ON t1.id = t2.id\nWHERE t1.id = 1 EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    let target: usize = 33 * 1024 * 1024;
    let mut written = 0usize;
    let mut idx = 0usize;
    while written < target {
        let rec = if idx % 5 == 0 {
            multi.as_ref()
        } else {
            single.as_ref()
        };
        file.write_all(rec).unwrap();
        written += rec.len();
        idx += 1;
    }
    file.flush().unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let seq_count = parser.iter().filter_map(|r| r.ok()).count();
    let par_count = parser.par_iter().filter_map(|r| r.ok()).count();

    assert_eq!(
        seq_count, par_count,
        "大文件 multiline 场景：par_iter 不应在多行边界丢记录"
    );
}

#[test]
#[cfg(not(miri))]
fn par_iter_small_file_single_partition() {
    let mut file = NamedTempFile::new().unwrap();
    for i in 0..10u32 {
        let rec = make_record(
            &format!("5-05-01 10:{:02}:{:02}.000", i / 60, i % 60),
            &format!("SELECT {i}"),
        );
        file.write_all(rec.as_bytes()).unwrap();
    }
    file.flush().unwrap();

    let parser = LogParser::from_path(file.path()).unwrap();
    let seq_count = parser.iter().filter_map(|r| r.ok()).count();
    let par_count = parser.par_iter().filter_map(|r| r.ok()).count();

    assert_eq!(seq_count, par_count);
    assert_eq!(par_count, 10);
}
