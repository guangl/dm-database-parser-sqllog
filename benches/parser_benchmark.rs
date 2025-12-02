use criterion::{Criterion, criterion_group, criterion_main};
use dm_database_parser_sqllog::LogParser;
use std::path::PathBuf;
use std::time::Duration;

fn benchmark_parser(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut path = root.clone();
    path.push("sqllogs");
    path.push("dmsql_OA01_20251117_160941.log");

    // Ensure the file exists
    if !path.exists() {
        eprintln!("Warning: Benchmark file not found at {:?}", path);
        return;
    }

    let mut group = c.benchmark_group("parser_group");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(60));
    group.warm_up_time(Duration::from_secs(5));

    group.bench_function("parse_sqllog_file_full", |b| {
        b.iter(|| {
            let parser = LogParser::from_path(&path).unwrap();
            let count = parser.iter().count();
            criterion::black_box(count);
        })
    });
    group.finish();
}

criterion_group!(benches, benchmark_parser);
criterion_main!(benches);
