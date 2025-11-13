use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::__test_helpers::*;

const VALID_SINGLE_LINE: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";

const VALID_WITH_INDICATORS: &str = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";

const MULTILINE_RECORD: [&str; 3] = [
    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *",
    "FROM users",
    "WHERE id > 0",
];

const META_STR: &str = "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app";
const META_WITH_IP: &str =
    "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:192.168.1.1";

const BODY_WITH_INDICATORS: &str =
    "SELECT * FROM users EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";

/// Benchmark parse_record 函数
fn bench_parse_record(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_record");

    // 单行记录
    group.bench_function("single_line", |b| {
        b.iter(|| parse_record(black_box(&[VALID_SINGLE_LINE])))
    });

    // 多行记录
    group.bench_function("multiline", |b| {
        b.iter(|| parse_record(black_box(&MULTILINE_RECORD)))
    });

    // 带指标的记录
    group.bench_function("with_indicators", |b| {
        b.iter(|| parse_record(black_box(&[VALID_WITH_INDICATORS])))
    });

    group.finish();
}

/// Benchmark parse_meta 函数
fn bench_parse_meta(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_meta");

    group.bench_function("basic", |b| b.iter(|| parse_meta(black_box(META_STR))));

    group.bench_function("with_client_ip", |b| {
        b.iter(|| parse_meta(black_box(META_WITH_IP)))
    });

    // 不同 EP 值
    for ep in [0, 1, 128, 255] {
        let meta = format!(
            "EP[{}] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app",
            ep
        );
        group.bench_with_input(BenchmarkId::new("ep_value", ep), &meta, |b, meta_str| {
            b.iter(|| parse_meta(black_box(meta_str)))
        });
    }

    group.finish();
}

/// Benchmark parse_indicators 函数
fn bench_parse_indicators(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_indicators");

    group.bench_function("all_fields", |b| {
        b.iter(|| parse_indicators(black_box(BODY_WITH_INDICATORS)))
    });

    // 不同的指标值范围
    let bodies = [
        (
            "small_values",
            "SELECT 1 EXECTIME: 1.5(ms) ROWCOUNT: 10(rows) EXEC_ID: 100.",
        ),
        (
            "medium_values",
            "SELECT * FROM t EXECTIME: 123.45(ms) ROWCOUNT: 5000(rows) EXEC_ID: 99999.",
        ),
        (
            "large_values",
            "UPDATE big_table SET x=1 EXECTIME: 9999.99(ms) ROWCOUNT: 1000000(rows) EXEC_ID: 999999999.",
        ),
    ];

    for (name, body) in bodies.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), body, |b, body_str| {
            b.iter(|| parse_indicators(black_box(body_str)))
        });
    }

    group.finish();
}

/// Benchmark build_body 函数
fn bench_build_body(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_body");

    let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *";
    let body_start = 92;

    group.bench_function("single_line", |b| {
        let continuation_lines: Vec<&str> = vec![];
        b.iter(|| {
            build_body(
                black_box(first_line),
                black_box(body_start),
                black_box(&continuation_lines),
            )
        })
    });

    // 不同数量的继续行
    for line_count in [1, 5, 10, 20] {
        let continuation_lines: Vec<&str> = (0..line_count)
            .map(|i| match i % 3 {
                0 => "FROM table_name",
                1 => "WHERE condition = true",
                _ => "AND another_condition = false",
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("multiline", line_count),
            &continuation_lines,
            |b, lines| {
                b.iter(|| {
                    build_body(
                        black_box(first_line),
                        black_box(body_start),
                        black_box(lines),
                    )
                })
            },
        );
    }

    group.finish();
}

/// Benchmark extract_sql_body 函数
fn bench_extract_sql_body(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_sql_body");

    let bodies = [
        (
            "with_exectime",
            "SELECT * FROM users EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.",
        ),
        (
            "with_rowcount_only",
            "INSERT INTO logs VALUES (1) ROWCOUNT: 5(rows) EXEC_ID: 111.",
        ),
        ("with_exec_id_only", "DELETE FROM temp EXEC_ID: 999."),
        ("no_indicators", "SELECT * FROM users WHERE id > 0"),
    ];

    for (name, body) in bodies.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), body, |b, body_str| {
            b.iter(|| extract_sql_body(black_box(body_str)))
        });
    }

    group.finish();
}

/// Benchmark extract_indicator 函数
fn bench_extract_indicator(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_indicator");

    let body = "SELECT * FROM users EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";

    group.bench_function("exectime", |b| {
        b.iter(|| extract_indicator(black_box(body), black_box("EXECTIME:"), black_box("(ms)")))
    });

    group.bench_function("rowcount", |b| {
        b.iter(|| extract_indicator(black_box(body), black_box("ROWCOUNT:"), black_box("(rows)")))
    });

    group.bench_function("exec_id", |b| {
        b.iter(|| extract_indicator(black_box(body), black_box("EXEC_ID:"), black_box(".")))
    });

    group.finish();
}

/// Benchmark parse_ep_field 函数
fn bench_parse_ep_field(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_ep_field");

    let ep_fields = [
        ("ep_0", "EP[0]"),
        ("ep_1", "EP[1]"),
        ("ep_128", "EP[128]"),
        ("ep_255", "EP[255]"),
    ];

    for (name, ep_str) in ep_fields.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(name), ep_str, |b, ep| {
            b.iter(|| parse_ep_field(black_box(ep), black_box("raw")))
        });
    }

    group.finish();
}

/// Benchmark extract_field_value 函数
fn bench_extract_field_value(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_field_value");

    let fields = [
        ("sess", "sess:123456", "sess:"),
        ("thrd", "thrd:789012", "thrd:"),
        ("user", "user:alice_long_username", "user:"),
        ("trxid", "trxid:999999999", "trxid:"),
        ("stmt", "stmt:888888888", "stmt:"),
        ("appname", "appname:my_application_name", "appname:"),
    ];

    for (name, field, prefix) in fields.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(field, prefix),
            |b, (f, p)| {
                b.iter(|| extract_field_value(black_box(f), black_box(p), black_box("raw")))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_record,
    bench_parse_meta,
    bench_parse_indicators,
    bench_build_body,
    bench_extract_sql_body,
    bench_extract_indicator,
    bench_parse_ep_field,
    bench_extract_field_value
);
criterion_main!(benches);
