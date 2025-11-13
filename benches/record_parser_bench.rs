use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dm_database_parser_sqllog::parser::RecordParser;

const SINGLE_RECORD: &str =
    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";

const MULTILINE_RECORD: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\n\
FROM users\n\
WHERE id > 0";

const MULTIPLE_RECORDS: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n\
2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2\n\
2025-08-12 12:30:15.123 (EP[2] sess:789 thrd:123 user:charlie trxid:222 stmt:333 appname:demo) INSERT INTO logs";

const MIXED_VALID_INVALID: &str = "invalid line\n\
another invalid\n\
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n\
not a valid line\n\
2025-08-12 11:00:00.000 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2";

/// 生成测试数据
fn generate_test_data(record_count: usize) -> String {
    let mut data = String::with_capacity(record_count * 150);
    for i in 0..record_count {
        data.push_str(&format!(
            "2025-08-12 10:57:09.{:03} (EP[{}] sess:{} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT * FROM table{}\n",
            i % 1000,
            i % 256,
            i,
            i + 1000,
            i % 100,
            i + 2000,
            i + 3000,
            i % 50
        ));
    }
    data
}

/// Benchmark RecordParser::new 函数
fn bench_record_parser_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_parser_new");

    group.bench_function("small_input", |b| {
        b.iter(|| RecordParser::new(black_box(SINGLE_RECORD.as_bytes())))
    });

    for size in [1_000, 10_000, 100_000] {
        let data = generate_test_data(size / 150);
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("size", size), &data, |b, d| {
            b.iter(|| RecordParser::new(black_box(d.as_bytes())))
        });
    }

    group.finish();
}

/// Benchmark RecordParser 迭代
fn bench_record_parser_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_parser_iteration");

    group.bench_function("single_record", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(SINGLE_RECORD.as_bytes()));
            parser.count()
        })
    });

    group.bench_function("multiline_record", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(MULTILINE_RECORD.as_bytes()));
            parser.count()
        })
    });

    group.bench_function("multiple_records", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(MULTIPLE_RECORDS.as_bytes()));
            parser.count()
        })
    });

    group.bench_function("mixed_valid_invalid", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(MIXED_VALID_INVALID.as_bytes()));
            parser.count()
        })
    });

    group.finish();
}

/// Benchmark RecordParser 解析不同数量的记录
fn bench_record_parser_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_parser_scaling");

    for record_count in [10, 100, 1_000, 10_000] {
        let data = generate_test_data(record_count);
        group.throughput(Throughput::Elements(record_count as u64));
        group.bench_with_input(
            BenchmarkId::new("records", record_count),
            &data,
            |b, d| {
                b.iter(|| {
                    let parser = RecordParser::new(black_box(d.as_bytes()));
                    parser.count()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark RecordParser 完整解析流程（包括 parse_to_sqllog）
fn bench_record_parser_full_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_parser_full_parse");

    for record_count in [10, 100, 1_000] {
        let data = generate_test_data(record_count);
        group.throughput(Throughput::Elements(record_count as u64));
        group.bench_with_input(
            BenchmarkId::new("records", record_count),
            &data,
            |b, d| {
                b.iter(|| {
                    let parser = RecordParser::new(black_box(d.as_bytes()));
                    let mut success_count = 0;
                    for record_result in parser {
                        if let Ok(record) = record_result {
                            if record.parse_to_sqllog().is_ok() {
                                success_count += 1;
                            }
                        }
                    }
                    success_count
                })
            },
        );
    }

    group.finish();
}

/// Benchmark RecordParser 处理不同行结束符
fn bench_record_parser_line_endings(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_parser_line_endings");

    let unix_data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\nFROM users\nWHERE id > 0";
    let windows_data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\r\nFROM users\r\nWHERE id > 0";
    let mixed_data = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *\r\nFROM users\nWHERE id > 0\r\n";

    group.bench_function("unix_lf", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(unix_data.as_bytes()));
            parser.count()
        })
    });

    group.bench_function("windows_crlf", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(windows_data.as_bytes()));
            parser.count()
        })
    });

    group.bench_function("mixed", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(mixed_data.as_bytes()));
            parser.count()
        })
    });

    group.finish();
}

/// Benchmark RecordParser collect vs iterator
fn bench_record_parser_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_parser_collection");

    let data = generate_test_data(1_000);

    group.bench_function("collect_all", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(data.as_bytes()));
            let _records: Vec<_> = parser.collect();
        })
    });

    group.bench_function("iterate_only", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(data.as_bytes()));
            parser.count()
        })
    });

    group.bench_function("iterate_filter", |b| {
        b.iter(|| {
            let parser = RecordParser::new(black_box(data.as_bytes()));
            parser.filter(|r| r.is_ok()).count()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_record_parser_new,
    bench_record_parser_iteration,
    bench_record_parser_scaling,
    bench_record_parser_full_parse,
    bench_record_parser_line_endings,
    bench_record_parser_collection
);
criterion_main!(benches);
