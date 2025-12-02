use criterion::{Criterion, criterion_group, criterion_main};
use dm_database_parser_sqllog::iter_records_from_file;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

fn prepare_benchmark_file() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut source_path = root.clone();
    source_path.push("sqllogs");
    source_path.push("dmsql_OA01_20251117_160941.log");

    let mut target_path = root.clone();
    target_path.push("sqllogs");
    target_path.push("benchmark_sample_small.log");

    if target_path.exists() {
        return target_path;
    }

    if !source_path.exists() {
        eprintln!(
            "Source file not found: {:?}. Generating synthetic data.",
            source_path
        );

        // Generate synthetic data
        let sample_record = r#"2025-11-17 16:09:41.123 (sess:123 thrd:456 user:SYSDBA trxid:789 stmt:0 appname:disql ip:::127.0.0.1) SELECT * FROM DUAL; EXECTIME: 1.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 100.
"#;
        let target_size = 5 * 1024 * 1024; // 5MB
        let mut file = File::create(&target_path).expect("Failed to create target file");
        let mut current_size = 0;
        while current_size < target_size {
            file.write_all(sample_record.as_bytes())
                .expect("Failed to write to target file");
            current_size += sample_record.len();
        }
        return target_path;
    }

    println!("Creating small benchmark sample file (5MB)...");
    let mut file = File::open(&source_path).expect("Failed to open source file");
    // 5MB sample
    let mut buffer = vec![0; 5 * 1024 * 1024];
    let n = file.read(&mut buffer).expect("Failed to read source file");

    // Find the last newline to ensure we don't cut a line in the middle
    let mut valid_len = n;
    while valid_len > 0 && buffer[valid_len - 1] != b'\n' {
        valid_len -= 1;
    }

    if valid_len == 0 {
        valid_len = n;
    }

    let mut target = File::create(&target_path).expect("Failed to create target file");
    target
        .write_all(&buffer[..valid_len])
        .expect("Failed to write target file");

    target_path
}

fn benchmark_parser(c: &mut Criterion) {
    let path = prepare_benchmark_file();

    // Ensure the file exists
    if !path.exists() {
        eprintln!("Warning: Benchmark file not found at {:?}", path);
        return;
    }

    let mut group = c.benchmark_group("parser_group");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));
    group.warm_up_time(Duration::from_secs(1));

    group.bench_function("parse_sqllog_file_5mb", |b| {
        b.iter(|| {
            let count = iter_records_from_file(&path).count();
            criterion::black_box(count);
        })
    });
    group.finish();
}

criterion_group!(benches, benchmark_parser);
criterion_main!(benches);
