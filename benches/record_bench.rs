use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::parser::Record;

const SINGLE_LINE: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";

const VALID_LINE: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users";

const CONTINUATION_LINES: [&str; 5] = [
    "FROM table1",
    "JOIN table2 ON table1.id = table2.id",
    "WHERE table1.status = 'active'",
    "AND table2.created_at > '2025-01-01'",
    "ORDER BY table1.id DESC",
];

/// Benchmark Record::new 函数
fn bench_record_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_new");

    group.bench_function("short_line", |b| {
        b.iter(|| Record::new(black_box(SINGLE_LINE.to_string())))
    });

    group.bench_function("long_line", |b| {
        let long_line = format!(
            "{} WHERE col1 = 'value1' AND col2 = 'value2' AND col3 = 'value3' AND col4 = 'value4'",
            VALID_LINE
        );
        b.iter(|| Record::new(black_box(long_line.clone())))
    });

    group.finish();
}

/// Benchmark Record::add_line 函数
fn bench_record_add_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_add_line");

    // 单次添加
    group.bench_function("single_add", |b| {
        b.iter_batched(
            || Record::new(VALID_LINE.to_string()),
            |mut record| {
                record.add_line(black_box("FROM users".to_string()));
                record
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // 批量添加不同数量的行
    for line_count in [5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("multiple_adds", line_count),
            &line_count,
            |b, &count| {
                b.iter_batched(
                    || Record::new(VALID_LINE.to_string()),
                    |mut record| {
                        for i in 0..count {
                            record.add_line(black_box(CONTINUATION_LINES[i % 5].to_string()));
                        }
                        record
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

/// Benchmark Record::full_content 函数
fn bench_record_full_content(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_full_content");

    // 不同行数的记录
    for line_count in [1, 5, 10, 20] {
        let mut record = Record::new(VALID_LINE.to_string());
        for i in 1..line_count {
            record.add_line(CONTINUATION_LINES[i % 5].to_string());
        }

        group.bench_with_input(
            BenchmarkId::new("line_count", line_count),
            &record,
            |b, rec| b.iter(|| rec.full_content()),
        );
    }

    group.finish();
}

/// Benchmark Record::parse_to_sqllog 函数
fn bench_record_parse_to_sqllog(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_parse_to_sqllog");

    // 单行记录
    group.bench_function("single_line", |b| {
        let record = Record::new(SINGLE_LINE.to_string());
        b.iter(|| record.parse_to_sqllog())
    });

    // 多行记录（不同行数）
    for line_count in [2, 5, 10, 20] {
        let mut record = Record::new(VALID_LINE.to_string());
        for i in 1..line_count {
            record.add_line(CONTINUATION_LINES[i % 5].to_string());
        }

        group.bench_with_input(
            BenchmarkId::new("multiline", line_count),
            &record,
            |b, rec| b.iter(|| rec.parse_to_sqllog()),
        );
    }

    // 带指标的记录
    group.bench_function("with_indicators", |b| {
        let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";
        let record = Record::new(line.to_string());
        b.iter(|| record.parse_to_sqllog())
    });

    group.finish();
}

/// Benchmark Record::clone 函数
fn bench_record_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_clone");

    // 不同行数的记录
    for line_count in [1, 5, 10, 20] {
        let mut record = Record::new(VALID_LINE.to_string());
        for i in 1..line_count {
            record.add_line(CONTINUATION_LINES[i % 5].to_string());
        }

        group.bench_with_input(
            BenchmarkId::new("line_count", line_count),
            &record,
            |b, rec| b.iter(|| rec.clone()),
        );
    }

    group.finish();
}

/// Benchmark Record 完整工作流
fn bench_record_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_workflow");

    group.bench_function("create_add_parse", |b| {
        b.iter(|| {
            let mut record = Record::new(black_box(VALID_LINE.to_string()));
            record.add_line(black_box("FROM users".to_string()));
            record.add_line(black_box("WHERE id > 0".to_string()));
            record.parse_to_sqllog()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_record_new,
    bench_record_add_line,
    bench_record_full_content,
    bench_record_parse_to_sqllog,
    bench_record_clone,
    bench_record_workflow
);
criterion_main!(benches);
