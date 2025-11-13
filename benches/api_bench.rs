use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::{iter_records_from_file, parse_records_from_file};

/// 测试 iter_records_from_file 函数（流式解析，返回 Sqllog 迭代器）
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
                    if let Ok(iter) = iter_records_from_file(black_box(path)) {
                        for result in iter {
                            if result.is_ok() {
                                count += 1;
                            }
                        }
                    }
                    count
                });
            },
        );
    }

    group.finish();
}

/// 测试 parse_records_from_file 函数（批量解析，返回 Vec<Sqllog>）
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
                    if let Ok((sqllogs, _errors)) = parse_records_from_file(black_box(path)) {
                        sqllogs.len()
                    } else {
                        0
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
                    if result.is_ok() {
                        count += 1;
                    }
                }
            }
            count
        });
    });

    group.bench_function("parse_records_from_file", |b| {
        b.iter(|| {
            if let Ok((sqllogs, _errors)) = parse_records_from_file(black_box(test_file)) {
                sqllogs.len()
            } else {
                0
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_iter_records_from_file,
    bench_parse_records_from_file,
    bench_api_comparison
);
criterion_main!(benches);
