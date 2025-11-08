use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::{
    RecordParser, SqllogParser, parse_record, parse_records_from_string, parse_sqllogs_from_string,
};
use std::io::Cursor;

// 生成测试数据
fn generate_single_line_log() -> String {
    "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:myapp ip:::ffff:10.3.100.68) SELECT * FROM users WHERE id = 1".to_string()
}

fn generate_multi_line_log() -> String {
    r#"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:myapp ip:::ffff:10.3.100.68) SELECT u.id, u.name, u.email
FROM users u
JOIN orders o ON u.id = o.user_id
WHERE u.status = 'active'
  AND u.created_date > '2024-01-01'
ORDER BY u.id"#.to_string()
}

fn generate_log_with_indicators() -> String {
    "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:myapp ip:::ffff:10.3.100.68) SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.".to_string()
}

fn generate_multiple_records(count: usize) -> String {
    let mut logs = Vec::with_capacity(count);
    for i in 0..count {
        logs.push(format!(
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:USER_{} trxid:{} stmt:0x{:x} appname:app{}) SELECT * FROM table_{} WHERE id = {}",
            i % 60,
            i % 1000,
            0x178ebca0 + i,
            757455 + i,
            i,
            i,
            0x285eb060 + i,
            i % 10,
            i % 100,
            i
        ));
    }
    logs.join("\n")
}

fn generate_mixed_records(count: usize) -> String {
    let mut logs = Vec::with_capacity(count * 2);
    for i in 0..count {
        if i % 3 == 0 {
            // 多行记录
            logs.push(format!(
                "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:USER_{} trxid:{} stmt:0x{:x} appname:app) SELECT *\nFROM users\nWHERE id = {}",
                i % 60,
                i % 1000,
                0x178ebca0 + i,
                757455 + i,
                i,
                i,
                0x285eb060 + i,
                i
            ));
        } else {
            // 单行记录
            logs.push(format!(
                "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:USER_{} trxid:{} stmt:0x{:x} appname:app) SELECT {} FROM table",
                i % 60,
                i % 1000,
                0x178ebca0 + i,
                757455 + i,
                i,
                i,
                0x285eb060 + i,
                i
            ));
        }
    }
    logs.join("\n")
}

// Benchmark: parse_record (单行)
fn bench_parse_record_single_line(c: &mut Criterion) {
    let log = generate_single_line_log();
    let lines = vec![log.as_str()];

    c.bench_function("parse_record/single_line", |b| {
        b.iter(|| parse_record(black_box(&lines)))
    });
}

// Benchmark: parse_record (多行)
fn bench_parse_record_multi_line(c: &mut Criterion) {
    let log = generate_multi_line_log();
    let lines: Vec<&str> = log.lines().collect();

    c.bench_function("parse_record/multi_line", |b| {
        b.iter(|| parse_record(black_box(&lines)))
    });
}

// Benchmark: parse_record (带 indicators)
fn bench_parse_record_with_indicators(c: &mut Criterion) {
    let log = generate_log_with_indicators();
    let lines = vec![log.as_str()];

    c.bench_function("parse_record/with_indicators", |b| {
        b.iter(|| parse_record(black_box(&lines)))
    });
}

// Benchmark: RecordParser (不同数量的记录)
fn bench_record_parser_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("RecordParser/throughput");

    for count in [10, 100, 1000, 10000].iter() {
        let logs = generate_multiple_records(*count);
        let bytes = logs.len();

        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                let cursor = Cursor::new(logs.as_bytes());
                let parser = RecordParser::new(cursor);
                let records: Vec<_> = parser.collect();
                black_box(records)
            })
        });
    }

    group.finish();
}

// Benchmark: SqllogParser (不同数量的记录)
fn bench_sqllog_parser_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("SqllogParser/throughput");

    for count in [10, 100, 1000, 10000].iter() {
        let logs = generate_multiple_records(*count);
        let bytes = logs.len();

        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                let cursor = Cursor::new(logs.as_bytes());
                let parser = SqllogParser::new(cursor);
                let sqllogs: Vec<_> = parser.collect();
                black_box(sqllogs)
            })
        });
    }

    group.finish();
}

// Benchmark: parse_records_from_string
fn bench_parse_records_from_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_records_from_string");

    for count in [10, 100, 1000].iter() {
        let logs = generate_multiple_records(*count);

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| parse_records_from_string(black_box(&logs)))
        });
    }

    group.finish();
}

// Benchmark: parse_sqllogs_from_string
fn bench_parse_sqllogs_from_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_sqllogs_from_string");

    for count in [10, 100, 1000].iter() {
        let logs = generate_multiple_records(*count);

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| parse_sqllogs_from_string(black_box(&logs)))
        });
    }

    group.finish();
}

// Benchmark: 混合场景（单行 + 多行）
fn bench_mixed_records(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_records");

    for count in [100, 1000].iter() {
        let logs = generate_mixed_records(*count);

        group.bench_with_input(BenchmarkId::new("RecordParser", count), count, |b, _| {
            b.iter(|| {
                let cursor = Cursor::new(logs.as_bytes());
                let parser = RecordParser::new(cursor);
                let records: Vec<_> = parser.collect();
                black_box(records)
            })
        });

        group.bench_with_input(BenchmarkId::new("SqllogParser", count), count, |b, _| {
            b.iter(|| {
                let cursor = Cursor::new(logs.as_bytes());
                let parser = SqllogParser::new(cursor);
                let sqllogs: Vec<_> = parser.collect();
                black_box(sqllogs)
            })
        });
    }

    group.finish();
}

// Benchmark: Record.parse_to_sqllog
fn bench_record_to_sqllog(c: &mut Criterion) {
    let log = generate_multi_line_log();
    let records = parse_records_from_string(&log);
    let record = &records[0];

    c.bench_function("Record::parse_to_sqllog", |b| {
        b.iter(|| record.parse_to_sqllog())
    });
}

// Benchmark: 大文件模拟（10000 条记录）
fn bench_large_file_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_file");
    group.sample_size(10); // 减少采样次数，因为数据量大

    let count = 10000;
    let logs = generate_mixed_records(count);
    let bytes = logs.len();

    group.throughput(Throughput::Bytes(bytes as u64));

    group.bench_function("RecordParser_10k_records", |b| {
        b.iter(|| {
            let cursor = Cursor::new(logs.as_bytes());
            let parser = RecordParser::new(cursor);
            let count = parser.filter_map(|r| r.ok()).count();
            black_box(count)
        })
    });

    group.bench_function("SqllogParser_10k_records", |b| {
        b.iter(|| {
            let cursor = Cursor::new(logs.as_bytes());
            let parser = SqllogParser::new(cursor);
            let count = parser.filter_map(|r| r.ok()).count();
            black_box(count)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_record_single_line,
    bench_parse_record_multi_line,
    bench_parse_record_with_indicators,
    bench_record_parser_throughput,
    bench_sqllog_parser_throughput,
    bench_parse_records_from_string,
    bench_parse_sqllogs_from_string,
    bench_mixed_records,
    bench_record_to_sqllog,
    bench_large_file_simulation,
);

criterion_main!(benches);
