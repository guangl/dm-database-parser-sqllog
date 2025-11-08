use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::tools::{is_record_start_line, is_ts_millis_bytes};

/// Benchmark: is_ts_millis_bytes 函数
fn bench_is_ts_millis_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_ts_millis_bytes");

    // 有效的时间戳
    let valid_ts = b"2024-06-15 12:34:56.789";
    group.bench_function("valid_timestamp", |b| {
        b.iter(|| is_ts_millis_bytes(black_box(valid_ts)))
    });

    // 无效的时间戳 - 长度错误
    let invalid_length = b"2024-06-15 12:34:56";
    group.bench_function("invalid_length", |b| {
        b.iter(|| is_ts_millis_bytes(black_box(invalid_length)))
    });

    // 无效的时间戳 - 分隔符错误
    let invalid_separator = b"2024-06-15 12:34:56,789";
    group.bench_function("invalid_separator", |b| {
        b.iter(|| is_ts_millis_bytes(black_box(invalid_separator)))
    });

    // 无效的时间戳 - 包含非数字
    let invalid_digit = b"202a-06-15 12:34:56.789";
    group.bench_function("invalid_digit", |b| {
        b.iter(|| is_ts_millis_bytes(black_box(invalid_digit)))
    });

    // 批量测试不同的时间戳
    let test_cases = vec![
        ("edge_min", b"2000-01-01 00:00:00.000" as &[u8]),
        ("edge_max", b"2099-12-31 23:59:59.999" as &[u8]),
        ("leap_year", b"2024-02-29 12:34:56.789" as &[u8]),
        ("typical", b"2025-08-12 10:57:09.548" as &[u8]),
    ];

    for (name, ts) in test_cases {
        group.bench_with_input(BenchmarkId::new("various", name), &ts, |b, &ts| {
            b.iter(|| is_ts_millis_bytes(black_box(ts)))
        });
    }

    group.finish();
}

/// Benchmark: is_record_start_line 函数
fn bench_is_record_start_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_record_start_line");

    // 完整有效的记录行（带 IP）
    let valid_with_ip = "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:myapp ip:::ffff:10.3.100.68) [SEL] select 1 from dual EXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.";
    group.bench_function("valid_with_ip", |b| {
        b.iter(|| is_record_start_line(black_box(valid_with_ip)))
    });

    // 有效的记录行（不带 IP）
    let valid_without_ip = "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:myapp) [SEL] select * from users where id = 1";
    group.bench_function("valid_without_ip", |b| {
        b.iter(|| is_record_start_line(black_box(valid_without_ip)))
    });

    // 最小有效记录
    let minimal_valid = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
    group.bench_function("minimal_valid", |b| {
        b.iter(|| is_record_start_line(black_box(minimal_valid)))
    });

    // 无效 - 太短
    let too_short = "2025-08-12 10:57:09.548";
    group.bench_function("invalid_too_short", |b| {
        b.iter(|| is_record_start_line(black_box(too_short)))
    });

    // 无效 - 时间戳格式错误（快速失败）
    let invalid_timestamp = "2025-08-12 10:57:09,548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
    group.bench_function("invalid_timestamp", |b| {
        b.iter(|| is_record_start_line(black_box(invalid_timestamp)))
    });

    // 无效 - 缺少左括号
    let no_paren = "2025-08-12 10:57:09.548 EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
    group.bench_function("invalid_no_paren", |b| {
        b.iter(|| is_record_start_line(black_box(no_paren)))
    });

    // 无效 - 字段不足
    let insufficient_fields = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice) body";
    group.bench_function("invalid_insufficient_fields", |b| {
        b.iter(|| is_record_start_line(black_box(insufficient_fields)))
    });

    // 续行（非起始行）
    let continuation = "    SELECT * FROM users WHERE id = 1";
    group.bench_function("continuation_line", |b| {
        b.iter(|| is_record_start_line(black_box(continuation)))
    });

    // 复杂字段值
    let complex_values = "2025-08-12 10:57:09.548 (EP[123] sess:0xABCD1234 thrd:9999999 user:USER_WITH_UNDERSCORES trxid:12345678 stmt:0xFFFFFFFF appname:app-name-with-dashes ip:::ffff:10.20.30.40) SELECT * FROM table WHERE column IN (1,2,3,4,5)";
    group.bench_function("complex_values", |b| {
        b.iter(|| is_record_start_line(black_box(complex_values)))
    });

    group.finish();
}

/// Benchmark: 不同长度的记录行
fn bench_record_line_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_line_lengths");

    let base = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) ";

    let test_cases = vec![
        ("short", format!("{}{}", base, "SELECT 1")),
        (
            "medium",
            format!(
                "{}{}",
                base,
                "SELECT * FROM users WHERE id = 1 AND status = 'active' AND created_at > '2024-01-01'"
            ),
        ),
        (
            "long",
            format!(
                "{}{}",
                base,
                "SELECT u.id, u.name, u.email, o.order_id, o.total FROM users u JOIN orders o ON u.id = o.user_id WHERE u.status = 'active' AND o.created_at > '2024-01-01' AND o.total > 100 ORDER BY o.created_at DESC LIMIT 1000"
            ),
        ),
    ];

    for (name, line) in &test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), line, |b, line| {
            b.iter(|| is_record_start_line(black_box(line.as_str())))
        });
    }

    group.finish();
}

/// Benchmark: 批量处理混合行
fn bench_mixed_lines_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_lines_batch");

    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS trxid:0 stmt:0x285eb060 appname:app ip:::ffff:10.3.100.68) [SEL] select 1 from dual",
        "    EXECTIME: 0(ms) ROWCOUNT: 1(rows)",
        "2025-08-12 10:57:10.123 (EP[1] sess:0x178ebca1 thrd:757456 user:TESTUSER trxid:1 stmt:0x285eb061 appname:testapp) [INS] insert into table values (1)",
        "    EXECTIME: 5(ms) ROWCOUNT: 1(rows)",
        "2025-08-12 10:57:11.456 (EP[2] sess:0x178ebca2 thrd:757457 user:ADMIN trxid:2 stmt:0x285eb062 appname:admin) [UPD] update users set status = 'active'",
        "    WHERE id = 100",
        "    EXECTIME: 10(ms) ROWCOUNT: 1(rows)",
        "some random log line without timestamp",
        "2025-08-12 10:57:12.789 (EP[3] sess:0x178ebca3 thrd:757458 user:USER1 trxid:3 stmt:0x285eb063 appname:) [DEL] delete from temp",
        "    EXECTIME: 2(ms)",
    ];

    group.bench_function("process_10_mixed_lines", |b| {
        b.iter(|| {
            let mut count = 0;
            for line in &lines {
                if is_record_start_line(black_box(line)) {
                    count += 1;
                }
            }
            count
        })
    });

    group.finish();
}

/// Benchmark: 时间戳验证的早期退出性能
fn bench_early_exit_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("early_exit_performance");

    // 第一个检查就失败（长度）
    let fail_at_length = "";
    group.bench_function("fail_at_length_check", |b| {
        b.iter(|| is_record_start_line(black_box(fail_at_length)))
    });

    // 第二个检查失败（时间戳长度）
    let fail_at_timestamp_length = "2025-08-12 10:57:09.54 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
    group.bench_function("fail_at_timestamp_length", |b| {
        b.iter(|| is_record_start_line(black_box(fail_at_timestamp_length)))
    });

    // 第三个检查失败（时间戳格式）
    let fail_at_timestamp_format = "2025-08-12 10:57:09,548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
    group.bench_function("fail_at_timestamp_format", |b| {
        b.iter(|| is_record_start_line(black_box(fail_at_timestamp_format)))
    });

    // 第四个检查失败（括号）
    let fail_at_paren = "2025-08-12 10:57:09.548 EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
    group.bench_function("fail_at_paren_check", |b| {
        b.iter(|| is_record_start_line(black_box(fail_at_paren)))
    });

    // 最后检查失败（字段验证）
    let fail_at_fields =
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789) body";
    group.bench_function("fail_at_field_validation", |b| {
        b.iter(|| is_record_start_line(black_box(fail_at_fields)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_is_ts_millis_bytes,
    bench_is_record_start_line,
    bench_record_line_lengths,
    bench_mixed_lines_batch,
    bench_early_exit_performance,
);

criterion_main!(benches);
