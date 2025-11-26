use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::iter_records_from_file;
use std::path::PathBuf;
use std::time::Duration;

fn bench_iter_records(c: &mut Criterion) {
    let paths = vec![
        PathBuf::from("sqllogs/dmsql_DSC0_20250812_092516.log"),
        PathBuf::from("sqllogs/dmsql_OASIS_DB1_20251020_151030.log"),
    ];
    let mut group = c.benchmark_group("iter_records_from_file");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    for path in paths.iter() {
        group.throughput(Throughput::Bytes(
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
        ));
        let p = path.clone();
        group.bench_function(BenchmarkId::from_parameter(path.to_str().unwrap()), |b| {
            b.iter(|| {
                let mut count = 0usize;
                for result in iter_records_from_file(&p) {
                    match result {
                        Ok(_sqllog) => count += 1,
                        Err(_e) => {}
                    }
                }
                black_box(count);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_iter_records);
criterion_main!(benches);
