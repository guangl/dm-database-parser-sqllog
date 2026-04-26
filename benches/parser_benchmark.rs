use criterion::{Criterion, criterion_group, criterion_main};
use dm_database_parser_sqllog::LogParser;
use rayon::iter::ParallelIterator;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::NamedTempFile;

fn generate_synthetic_log(target_bytes: usize) -> NamedTempFile {
    let mut tmp = NamedTempFile::new().expect("tmpfile");
    let record = b"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:BENCHMARK trxid:0 stmt:0x285eb060 appname:bench) [SEL] SELECT id, name, value FROM benchmark_table WHERE id = 12345 EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.\n";
    let mut written = 0;
    while written < target_bytes {
        tmp.write_all(record).expect("write");
        written += record.len();
    }
    tmp.flush().expect("flush");
    tmp
}

fn generate_synthetic_log_multiline(target_bytes: usize) -> NamedTempFile {
    let mut tmp = NamedTempFile::new().expect("tmpfile");
    let single_line_record = b"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:BENCHMARK trxid:0 stmt:0x285eb060 appname:bench) [SEL] SELECT id, name, value FROM benchmark_table WHERE id = 12345 EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.\n";
    let multi_line_record = b"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:BENCHMARK trxid:0 stmt:0x285eb060 appname:bench) [SEL] SELECT\n    t1.id,\n    t2.name\nFROM benchmark_table t1\nJOIN other_table t2 ON t1.id = t2.id\nWHERE t1.id = 12345 EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.\n";
    let mut written = 0;
    let mut record_index: usize = 0;
    while written < target_bytes {
        let record = if record_index.is_multiple_of(5) {
            multi_line_record.as_ref()
        } else {
            single_line_record.as_ref()
        };
        tmp.write_all(record).expect("write");
        written += record.len();
        record_index += 1;
    }
    tmp.flush().expect("flush");
    tmp
}

fn benchmark_parser(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let real_path = root.join("sqllogs").join("dmsql_DSC0_20250812_092516.log");

    let mut group = c.benchmark_group("parser_group");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(60));
    group.warm_up_time(Duration::from_secs(5));

    if real_path.exists() {
        group.bench_function("parse_sqllog_file_full", |b| {
            b.iter(|| {
                let parser = LogParser::from_path(&real_path).unwrap();
                let count = parser.iter().count();
                criterion::black_box(count);
            })
        });
    } else {
        eprintln!("Note: real log file not found, using synthetic 5 MB data");
    }

    // Synthetic 5 MB benchmark — always runs
    let tmp = generate_synthetic_log(5 * 1024 * 1024);
    let tmp_path = tmp.path().to_path_buf();
    group.throughput(criterion::Throughput::Bytes(5 * 1024 * 1024));
    group.bench_function("parse_sqllog_file_5mb", |b| {
        b.iter(|| {
            let parser = LogParser::from_path(&tmp_path).unwrap();
            let count = parser.iter().count();
            criterion::black_box(count);
        })
    });

    // MEAS-01: records/sec 变体（与 parse_sqllog_file_5mb 测相同代码）
    let record_count_single = {
        let parser = LogParser::from_path(&tmp_path).unwrap();
        parser.iter().filter_map(|r| r.ok()).count() as u64
    };
    group.throughput(criterion::Throughput::Elements(record_count_single));
    group.bench_function("parse_sqllog_file_5mb_rps", |b| {
        b.iter(|| {
            let parser = LogParser::from_path(&tmp_path).unwrap();
            criterion::black_box(parser.iter().count())
        })
    });

    // MEAS-03: 多行语料库（20% 多行 SQL，D-04）
    let tmp_multiline = generate_synthetic_log_multiline(5 * 1024 * 1024);
    let tmp_multiline_path = tmp_multiline.path().to_path_buf();

    group.throughput(criterion::Throughput::Bytes(5 * 1024 * 1024));
    group.bench_function("parse_sqllog_multiline_5mb", |b| {
        b.iter(|| {
            let parser = LogParser::from_path(&tmp_multiline_path).unwrap();
            criterion::black_box(parser.iter().count())
        })
    });

    // MEAS-01: 多行 records/sec 变体
    let record_count_multi = {
        let parser = LogParser::from_path(&tmp_multiline_path).unwrap();
        parser.iter().filter_map(|r| r.ok()).count() as u64
    };
    group.throughput(criterion::Throughput::Elements(record_count_multi));
    group.bench_function("parse_sqllog_multiline_5mb_rps", |b| {
        b.iter(|| {
            let parser = LogParser::from_path(&tmp_multiline_path).unwrap();
            criterion::black_box(parser.iter().count())
        })
    });

    // MEAS-02: parse_performance_metrics() 变体（复用多行语料库，反映真实热路径）
    group.throughput(criterion::Throughput::Bytes(5 * 1024 * 1024));
    group.bench_function("parse_sqllog_metrics_5mb", |b| {
        b.iter(|| {
            let parser = LogParser::from_path(&tmp_multiline_path).unwrap();
            let count = parser
                .iter()
                .filter_map(|r| r.ok())
                .inspect(|s| {
                    criterion::black_box(s.parse_performance_metrics());
                })
                .count();
            criterion::black_box(count)
        })
    });

    // PAR-02: 64 MB 单线程基准（speedup 对比基线）
    // parser 在计时循环外创建，隔离纯迭代性能（排除 mmap 创建 / page fault 干扰）
    let tmp_64mb = generate_synthetic_log(64 * 1024 * 1024);
    let tmp_64mb_path = tmp_64mb.path().to_path_buf();
    let parser_64mb = LogParser::from_path(&tmp_64mb_path).unwrap();

    group.throughput(criterion::Throughput::Bytes(64 * 1024 * 1024));
    group.bench_function("parse_sqllog_file_64mb_seq", |b| {
        b.iter(|| {
            let count = parser_64mb.iter().count();
            criterion::black_box(count);
        })
    });

    // PAR-02: 64 MB 并行基准（目标：≥1.6x seq 在 ≥2 核机器）
    group.throughput(criterion::Throughput::Bytes(64 * 1024 * 1024));
    group.bench_function("parse_sqllog_file_64mb_par", |b| {
        b.iter(|| {
            let count = parser_64mb.par_iter().count();
            criterion::black_box(count);
        })
    });

    group.finish();
    drop(tmp);
    drop(tmp_multiline);
    drop(tmp_64mb);
}

criterion_group!(benches, benchmark_parser);
criterion_main!(benches);
