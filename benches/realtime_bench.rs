use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::realtime::{ParserConfig, RealtimeParser};
use std::fs::File;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

/// 生成测试日志数据
fn generate_test_log(num_records: usize) -> String {
    let mut log = String::new();
    for i in 0..num_records {
        log.push_str(&format!(
            "2025-08-12 10:{:02}:{:02}.{:03} (EP[0] sess:{} thrd:{} user:user{} trxid:{} stmt:{} appname:TestApp) SELECT * FROM table_{} WHERE id = {}\n",
            (i / 600) % 60,
            (i / 10) % 60,
            (i % 10) * 100,
            i % 100,
            i % 50,
            i % 10,
            i,
            i,
            i % 20,
            i
        ));
    }
    log
}

/// 基准测试：初始化解析器
fn bench_parser_creation(c: &mut Criterion) {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{}", generate_test_log(10)).unwrap();
    temp_file.flush().unwrap();

    c.bench_function("realtime_parser_new", |b| {
        b.iter(|| {
            let config = ParserConfig {
                file_path: temp_file.path().to_path_buf(),
                poll_interval: Duration::from_secs(1),
                buffer_size: 8192,
            };
            black_box(RealtimeParser::new(config).unwrap())
        });
    });
}

/// 基准测试：增量解析不同数量的记录
fn bench_incremental_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_parsing");

    for size in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "{}", generate_test_log(*size)).unwrap();
        temp_file.flush().unwrap();

        let config = ParserConfig {
            file_path: temp_file.path().to_path_buf(),
            poll_interval: Duration::from_millis(100),
            buffer_size: 8192,
        };

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut parser = RealtimeParser::new(config.clone()).unwrap();
                let mut count = 0;
                parser
                    .parse_new_records(|_| {
                        count += 1;
                    })
                    .unwrap();
                black_box(count)
            });
        });
    }
    group.finish();
}

/// 基准测试：多次增量读取
fn bench_multiple_incremental_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_incremental_reads");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    // 初始写入
    {
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", generate_test_log(100)).unwrap();
        file.flush().unwrap();
    }

    let config = ParserConfig {
        file_path: path.clone(),
        poll_interval: Duration::from_millis(50),
        buffer_size: 8192,
    };

    group.bench_function("5_incremental_reads", |b| {
        b.iter(|| {
            let mut parser = RealtimeParser::new(config.clone()).unwrap();

            // 第一次读取
            parser.parse_new_records(|_| {}).unwrap();

            // 追加并读取 4 次
            for i in 0..4 {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap();
                writeln!(file, "{}", generate_test_log(20)).unwrap();
                file.flush().unwrap();

                let count = parser.parse_new_records(|_| {}).unwrap();
                black_box(count);
            }
        });
    });

    group.finish();
}

/// 基准测试：不同缓冲区大小的性能
fn bench_buffer_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_sizes");

    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{}", generate_test_log(1000)).unwrap();
    temp_file.flush().unwrap();

    for buffer_size in [1024, 4096, 8192, 16384, 32768].iter() {
        let config = ParserConfig {
            file_path: temp_file.path().to_path_buf(),
            poll_interval: Duration::from_millis(100),
            buffer_size: *buffer_size,
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(buffer_size),
            buffer_size,
            |b, _| {
                b.iter(|| {
                    let mut parser = RealtimeParser::new(config.clone()).unwrap();
                    let mut count = 0;
                    parser
                        .parse_new_records(|_| {
                            count += 1;
                        })
                        .unwrap();
                    black_box(count)
                });
            },
        );
    }
    group.finish();
}

/// 基准测试：parse_all vs 增量解析
fn bench_parse_all_vs_incremental(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_all_vs_incremental");

    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{}", generate_test_log(1000)).unwrap();
    temp_file.flush().unwrap();

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(100),
        buffer_size: 8192,
    };

    group.bench_function("parse_all", |b| {
        b.iter(|| {
            let mut parser = RealtimeParser::new(config.clone()).unwrap();
            let count = parser.parse_all(|_| {}).unwrap();
            black_box(count)
        });
    });

    group.bench_function("parse_new_records", |b| {
        b.iter(|| {
            let mut parser = RealtimeParser::new(config.clone()).unwrap();
            let count = parser.parse_new_records(|_| {}).unwrap();
            black_box(count)
        });
    });

    group.finish();
}

/// 基准测试：位置跟踪开销
fn bench_position_tracking(c: &mut Criterion) {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{}", generate_test_log(100)).unwrap();
    temp_file.flush().unwrap();

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(100),
        buffer_size: 8192,
    };

    c.bench_function("position_tracking_overhead", |b| {
        b.iter(|| {
            let mut parser = RealtimeParser::new(config.clone()).unwrap();
            parser.parse_new_records(|_| {}).unwrap();
            let pos1 = parser.position();
            parser.seek_to(0);
            parser.parse_new_records(|_| {}).unwrap();
            let pos2 = parser.position();
            black_box((pos1, pos2))
        });
    });
}

criterion_group!(
    benches,
    bench_parser_creation,
    bench_incremental_parsing,
    bench_multiple_incremental_reads,
    bench_buffer_sizes,
    bench_parse_all_vs_incremental,
    bench_position_tracking,
);

criterion_main!(benches);
