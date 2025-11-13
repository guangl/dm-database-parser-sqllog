use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dm_database_parser_sqllog::tools::is_record_start_line;

const VALID_LINES: [&str; 5] = [
    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1",
    "2025-08-12 10:57:09.548 (EP[1] sess:456 thrd:789 user:bob trxid:111 stmt:222 appname:test) SELECT 2",
    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:192.168.1.1) SELECT 1",
    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999) SELECT 1",
    "2025-08-12 10:57:09.548 (EP[255] sess:999999 thrd:888888 user:very_long_username trxid:777777 stmt:666666 appname:my_application) UPDATE users SET status='active'",
];

const INVALID_LINES: [&str; 10] = [
    "invalid line",
    "2025-08-12",
    "not a log line at all",
    "FROM users",
    "WHERE id > 0",
    "2025-08-12 10:57:09.548",
    "2025-08-12 10:57:09.548 (EP[0]",
    "2025-08-12 10:57:09.548 (EP[0] sess:123",
    "EP[0] sess:123 thrd:456 user:alice",
    "SELECT * FROM users",
];

const EDGE_CASE_LINES: [&str; 5] = [
    // 时间戳格式错误
    "2025-08-12 10:57:09.54 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999) SELECT 1",
    // 缺少右括号
    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app SELECT 1",
    // EP 格式错误
    "2025-08-12 10:57:09.548 (EPX sess:123 thrd:456 user:alice trxid:789 stmt:999) SELECT 1",
    // 字段不足
    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456) SELECT 1",
    // 字段顺序错误
    "2025-08-12 10:57:09.548 (EP[0] thrd:456 sess:123 user:alice trxid:789 stmt:999) SELECT 1",
];

/// Benchmark is_record_start_line 函数 - 有效行
fn bench_is_record_start_line_valid(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line_valid");

    for (i, line) in VALID_LINES.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("line", i), line, |b, l| {
            b.iter(|| is_record_start_line(black_box(l)))
        });
    }

    group.finish();
}

/// Benchmark is_record_start_line 函数 - 无效行
fn bench_is_record_start_line_invalid(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line_invalid");

    for (i, line) in INVALID_LINES.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("line", i), line, |b, l| {
            b.iter(|| is_record_start_line(black_box(l)))
        });
    }

    group.finish();
}

/// Benchmark is_record_start_line 函数 - 边界情况
fn bench_is_record_start_line_edge_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line_edge_cases");

    for (i, line) in EDGE_CASE_LINES.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("case", i), line, |b, l| {
            b.iter(|| is_record_start_line(black_box(l)))
        });
    }

    group.finish();
}

/// Benchmark is_record_start_line 函数 - 不同长度的行
fn bench_is_record_start_line_line_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line_lengths");

    let base = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) ";

    for sql_len in [10, 50, 100, 200, 500, 1000] {
        let sql = "A".repeat(sql_len);
        let line = format!("{}{}", base, sql);
        group.bench_with_input(
            BenchmarkId::new("sql_length", sql_len),
            &line,
            |b, l| {
                b.iter(|| is_record_start_line(black_box(l)))
            },
        );
    }

    group.finish();
}

/// Benchmark is_record_start_line 函数 - 混合场景
fn bench_is_record_start_line_mixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line_mixed");

    let mixed_lines: Vec<&str> = VALID_LINES
        .iter()
        .chain(INVALID_LINES.iter())
        .copied()
        .collect();

    group.bench_function("all_lines", |b| {
        b.iter(|| {
            let mut valid_count = 0;
            for line in &mixed_lines {
                if is_record_start_line(black_box(line)) {
                    valid_count += 1;
                }
            }
            valid_count
        })
    });

    group.finish();
}

/// Benchmark is_record_start_line 函数 - 早期退出性能
fn bench_is_record_start_line_early_exit(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line_early_exit");

    // 测试不同阶段失败的行
    let lines = [
        ("too_short", "short"),
        ("bad_timestamp_format", "2025-08-12 10:57:09.54X (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999) SELECT 1"),
        ("no_paren", "2025-08-12 10:57:09.548 EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 SELECT 1"),
        ("bad_ep_format", "2025-08-12 10:57:09.548 (EPX sess:123 thrd:456 user:alice trxid:789 stmt:999) SELECT 1"),
        ("missing_fields", "2025-08-12 10:57:09.548 (EP[0] sess:123) SELECT 1"),
    ];

    for (name, line) in lines.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), line, |b, l| {
            b.iter(|| is_record_start_line(black_box(l)))
        });
    }

    group.finish();
}

/// Benchmark is_record_start_line 函数 - 实际场景模拟
fn bench_is_record_start_line_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line_realistic");

    // 模拟实际日志文件中的行分布 (80% 有效, 20% 无效/继续行)
    let mut lines = Vec::new();
    for i in 0..100 {
        if i % 5 == 0 {
            // 20% 无效行
            lines.push(INVALID_LINES[i % INVALID_LINES.len()]);
        } else {
            // 80% 有效行
            lines.push(VALID_LINES[i % VALID_LINES.len()]);
        }
    }

    group.bench_function("realistic_distribution", |b| {
        b.iter(|| {
            let mut valid_count = 0;
            for line in &lines {
                if is_record_start_line(black_box(line)) {
                    valid_count += 1;
                }
            }
            valid_count
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_is_record_start_line_valid,
    bench_is_record_start_line_invalid,
    bench_is_record_start_line_edge_cases,
    bench_is_record_start_line_line_lengths,
    bench_is_record_start_line_mixed,
    bench_is_record_start_line_early_exit,
    bench_is_record_start_line_realistic
);
criterion_main!(benches);
