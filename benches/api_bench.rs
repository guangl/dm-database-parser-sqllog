use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::{iter_records_from_file, parse_records_from_file};

/// 测试 iter_records_from_file 函数
fn bench_iter_records_from_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_records_from_file");
    group.sample_size(10); // 减少样本数量以加快测试速度

    let test_files = [
        "sqllogs/dmsql_DSC0_20250812_092516.log",
        "sqllogs/dmsql_OASIS_DB1_20251020_151030.log",
    ];
    for file_path in test_files.iter() {
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);

        group.bench_with_input(
            BenchmarkId::from_parameter(file_name),
            file_path,
            |b, path| {
                b.iter(|| {
                    let mut count = 0;
                    let mut parsed_count = 0;
                    if let Ok(iter) = iter_records_from_file(black_box(path)) {
                        for result in iter {
                            if let Ok(record) = result {
                                count += 1;
                                // 测试进一步解析为 Sqllog
                                if record.parse_to_sqllog().is_ok() {
                                    parsed_count += 1;
                                }
                            }
                        }
                    }
                    (count, parsed_count)
                });
            },
        );
    }

    group.finish();
}

/// 测试 parse_records_from_file 函数
fn bench_parse_records_from_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_records_from_file");
    group.sample_size(10); // 减少样本数量以加快测试速度

    let test_files = [
        "sqllogs/dmsql_DSC0_20250812_092516.log",
        "sqllogs/dmsql_OASIS_DB1_20251020_151030.log",
    ];
    for file_path in test_files.iter() {
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);

        group.bench_with_input(
            BenchmarkId::from_parameter(file_name),
            file_path,
            |b, path| {
                b.iter(|| {
                    if let Ok((records, _errors)) = parse_records_from_file(black_box(path)) {
                        // 测试进一步解析为 Sqllog
                        let parsed: Vec<_> = records
                            .iter()
                            .filter_map(|r| r.parse_to_sqllog().ok())
                            .collect();
                        (records.len(), parsed.len())
                    } else {
                        (0, 0)
                    }
                });
            },
        );
    }

    group.finish();
}

/// 比较两种 API 的性能
fn bench_api_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_comparison");
    group.sample_size(10); // 减少样本数量以加快测试速度

    let test_file = "sqllogs/dmsql_DSC0_20250812_092516.log";
    group.bench_function("iter_records_from_file", |b| {
        b.iter(|| {
            let mut count = 0;
            if let Ok(iter) = iter_records_from_file(black_box(test_file)) {
                for result in iter {
                    if let Ok(record) = result {
                        if record.parse_to_sqllog().is_ok() {
                            count += 1;
                        }
                    }
                }
            }
            count
        });
    });

    group.bench_function("parse_records_from_file", |b| {
        b.iter(|| {
            if let Ok((records, _errors)) = parse_records_from_file(black_box(test_file)) {
                records
                    .iter()
                    .filter(|r| r.parse_to_sqllog().is_ok())
                    .count()
            } else {
                0
            }
        });
    });

    group.finish();
}

/// 测试 RecordParser（流式解析器）
fn bench_record_parser(c: &mut Criterion) {
    use dm_database_parser_sqllog::RecordParser;
    use std::fs::File;
    use std::io::BufReader;

    let mut group = c.benchmark_group("record_parser");
    group.sample_size(10); // 减少样本数量以加快测试速度

    let test_files = [
        "sqllogs/dmsql_DSC0_20250812_092516.log",
        "sqllogs/dmsql_OASIS_DB1_20251020_151030.log",
    ];
    for file_path in test_files.iter() {
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);

        group.bench_with_input(
            BenchmarkId::from_parameter(file_name),
            file_path,
            |b, path| {
                b.iter(|| {
                    if let Ok(file) = File::open(black_box(path)) {
                        let reader = BufReader::new(file);
                        let parser = RecordParser::new(reader);
                        let mut count = 0;
                        let mut parsed_count = 0;

                        for result in parser {
                            if let Ok(record) = result {
                                count += 1;
                                if record.parse_to_sqllog().is_ok() {
                                    parsed_count += 1;
                                }
                            }
                        }
                        (count, parsed_count)
                    } else {
                        (0, 0)
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_iter_records_from_file,
    bench_parse_records_from_file,
    bench_api_comparison,
    bench_record_parser
);
criterion_main!(benches);
