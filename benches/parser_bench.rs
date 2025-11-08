use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use dm_database_parser_sqllog::{
    for_each_record, parse_all, parse_record, parse_records_with,
    split_by_ts_records_with_errors, RecordSplitter,
};

// 生成测试数据
fn generate_test_data(num_records: usize) -> String {
    let mut log_text = String::new();
    for i in 0..num_records {
        log_text.push_str(&format!(
            "2025-08-12 10:57:09.562 (EP[{}] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:688489653 stmt:0x7fb236077b70 appname: ip:::ffff:10.3.100.68) EXECTIME: {}ms ROWCOUNT: {} EXEC_ID: {}\n",
            i % 10,
            i * 10,
            i,
            i + 1000
        ));
        log_text.push_str(&format!("SELECT * FROM users WHERE id = {}\n", i));
    }
    log_text
}

// 生成单条记录
fn generate_single_record() -> String {
    "2025-08-12 10:57:09.562 (EP[0] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:688489653 stmt:0x7fb236077b70 appname: ip:::ffff:10.3.100.68) EXECTIME: 0ms ROWCOUNT: 1 EXEC_ID: 289655185\nSELECT * FROM users WHERE id = 1\n".to_string()
}

fn benchmark_record_splitting(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_splitting");
    
    for size in [10, 100, 1000, 10000].iter() {
        let log_text = generate_test_data(*size);
        group.bench_with_input(
            BenchmarkId::new("split_by_ts_records_with_errors", size),
            &log_text,
            |b, text| {
                b.iter(|| {
                    black_box(split_by_ts_records_with_errors(black_box(text)))
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_record_splitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_splitter");
    
    for size in [10, 100, 1000, 10000].iter() {
        let log_text = generate_test_data(*size);
        group.bench_with_input(
            BenchmarkId::new("RecordSplitter::new", size),
            &log_text,
            |b, text| {
                b.iter(|| {
                    black_box(RecordSplitter::new(black_box(text)))
                })
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("RecordSplitter iteration", size),
            &log_text,
            |b, text| {
                b.iter(|| {
                    let splitter = RecordSplitter::new(text);
                    let count = splitter.count();
                    black_box(count)
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_parse_record(c: &mut Criterion) {
    let record = generate_single_record();
    
    c.bench_function("parse_record", |b| {
        b.iter(|| {
            black_box(parse_record(black_box(&record)))
        })
    });
}

fn benchmark_parse_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_all");
    
    for size in [10, 100, 1000].iter() {
        let log_text = generate_test_data(*size);
        group.bench_with_input(
            BenchmarkId::new("parse_all", size),
            &log_text,
            |b, text| {
                b.iter(|| {
                    black_box(parse_all(black_box(text)))
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_for_each_record(c: &mut Criterion) {
    let mut group = c.benchmark_group("for_each_record");
    
    for size in [10, 100, 1000, 10000].iter() {
        let log_text = generate_test_data(*size);
        group.bench_with_input(
            BenchmarkId::new("for_each_record", size),
            &log_text,
            |b, text| {
                b.iter(|| {
                    let mut count = 0;
                    for_each_record(black_box(text), |_rec| {
                        count += 1;
                    });
                    black_box(count)
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_parse_records_with(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_records_with");
    
    for size in [10, 100, 1000].iter() {
        let log_text = generate_test_data(*size);
        group.bench_with_input(
            BenchmarkId::new("parse_records_with", size),
            &log_text,
            |b, text| {
                b.iter(|| {
                    let mut count = 0;
                    parse_records_with(black_box(text), |_parsed| {
                        count += 1;
                    });
                    black_box(count)
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_comparison(c: &mut Criterion) {
    let log_text = generate_test_data(1000);
    
    let mut group = c.benchmark_group("comparison_1000_records");
    
    group.bench_function("parse_all", |b| {
        b.iter(|| {
            black_box(parse_all(black_box(&log_text)))
        })
    });
    
    group.bench_function("for_each_record", |b| {
        b.iter(|| {
            let mut count = 0;
            for_each_record(black_box(&log_text), |_rec| {
                count += 1;
            });
            black_box(count)
        })
    });
    
    group.bench_function("parse_records_with", |b| {
        b.iter(|| {
            let mut count = 0;
            parse_records_with(black_box(&log_text), |_parsed| {
                count += 1;
            });
            black_box(count)
        })
    });
    
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(3));
    targets = 
        benchmark_record_splitting,
        benchmark_record_splitter,
        benchmark_parse_record,
        benchmark_parse_all,
        benchmark_for_each_record,
        benchmark_parse_records_with,
        benchmark_comparison
}

criterion_main!(benches);

