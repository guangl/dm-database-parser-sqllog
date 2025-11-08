//! 性能测试和压力测试
//!
//! 这个文件包含各种性能测试，用于验证库在不同场景下的性能表现

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dm_database_parser_sqllog::{parse_all, parse_record, RecordSplitter};

// 生成大型日志文件（模拟真实场景）
fn generate_large_log(num_records: usize) -> String {
    let mut log_text = String::with_capacity(num_records * 200);
    
    for i in 0..num_records {
        // 模拟不同的记录类型
        let record_type = i % 4;
        match record_type {
            0 => {
                // SELECT 查询
                log_text.push_str(&format!(
                    "2025-08-12 10:57:09.562 (EP[{}] sess:0x7fb24f392a30 thrd:757794 user:USER_{} trxid:{} stmt:0x7fb236077b70 appname:MyApp) EXECTIME: {}ms ROWCOUNT: {} EXEC_ID: {}\n",
                    i % 10,
                    i % 100,
                    i * 100,
                    i * 5,
                    i % 1000,
                    i + 10000
                ));
                log_text.push_str(&format!("SELECT * FROM table_{} WHERE id = {}\n", i % 10, i));
            }
            1 => {
                // INSERT 操作
                log_text.push_str(&format!(
                    "2025-08-12 10:57:09.562 (EP[{}] sess:0x7fb24f392a30 thrd:757794 user:USER_{} trxid:{} stmt:0x7fb236077b70 appname:MyApp) EXECTIME: {}ms ROWCOUNT: {} EXEC_ID: {}\n",
                    i % 10,
                    i % 100,
                    i * 100,
                    i * 3,
                    i % 500,
                    i + 10000
                ));
                log_text.push_str(&format!("INSERT INTO table_{} VALUES ({}, 'value_{}')\n", i % 10, i, i));
            }
            2 => {
                // UPDATE 操作
                log_text.push_str(&format!(
                    "2025-08-12 10:57:09.562 (EP[{}] sess:0x7fb24f392a30 thrd:757794 user:USER_{} trxid:{} stmt:0x7fb236077b70 appname:MyApp) EXECTIME: {}ms ROWCOUNT: {} EXEC_ID: {}\n",
                    i % 10,
                    i % 100,
                    i * 100,
                    i * 2,
                    i % 200,
                    i + 10000
                ));
                log_text.push_str(&format!("UPDATE table_{} SET value = 'updated_{}' WHERE id = {}\n", i % 10, i, i));
            }
            _ => {
                // DELETE 操作
                log_text.push_str(&format!(
                    "2025-08-12 10:57:09.562 (EP[{}] sess:0x7fb24f392a30 thrd:757794 user:USER_{} trxid:{} stmt:0x7fb236077b70 appname:MyApp) EXECTIME: {}ms ROWCOUNT: {} EXEC_ID: {}\n",
                    i % 10,
                    i % 100,
                    i * 100,
                    i * 4,
                    i % 100,
                    i + 10000
                ));
                log_text.push_str(&format!("DELETE FROM table_{} WHERE id = {}\n", i % 10, i));
            }
        }
    }
    
    log_text
}

fn benchmark_large_file_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_file_parsing");
    
    for size in [1000, 10000, 100000].iter() {
        let log_text = generate_large_log(*size);
        let log_text_clone = log_text.clone();
        
        group.bench_with_input(
            criterion::BenchmarkId::new("parse_all", size),
            &log_text,
            |b, text| {
                b.iter(|| {
                    black_box(parse_all(black_box(text)))
                })
            },
        );
        
        group.bench_with_input(
            criterion::BenchmarkId::new("RecordSplitter count", size),
            &log_text_clone,
            |b, text| {
                b.iter(|| {
                    let splitter = RecordSplitter::new(text);
                    black_box(splitter.count())
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_memory_efficiency(c: &mut Criterion) {
    let log_text = generate_large_log(10000);
    
    c.bench_function("memory_efficiency_parse_all", |b| {
        b.iter(|| {
            let records = parse_all(&log_text);
            // 验证所有记录都是引用，没有复制
            let total_size: usize = records.iter().map(|r| r.body.len()).sum();
            black_box(total_size)
        })
    });
    
    c.bench_function("memory_efficiency_splitter", |b| {
        b.iter(|| {
            let splitter = RecordSplitter::new(&log_text);
            let mut total_size = 0;
            for rec in splitter {
                total_size += rec.len();
            }
            black_box(total_size)
        })
    });
}

fn benchmark_parse_record_variations(c: &mut Criterion) {
    let records = vec![
        "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1",
        "2025-08-12 10:57:09.562 (EP[12345] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:688489653 stmt:0x7fb236077b70 appname: ip:::ffff:10.3.100.68) EXECTIME: 0ms ROWCOUNT: 1 EXEC_ID: 289655185\nSELECT * FROM users",
        "2025-08-12 10:57:09.562 (EP[0] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:0 stmt:NULL appname:) TRX: START\nBEGIN TRANSACTION",
    ];
    
    let mut group = c.benchmark_group("parse_record_variations");
    
    for (i, record) in records.iter().enumerate() {
        group.bench_function(format!("record_type_{}", i), |b| {
            b.iter(|| {
                black_box(parse_record(black_box(record)))
            })
        });
    }
    
    group.finish();
}

criterion_group! {
    name = performance;
    config = Criterion::default()
        .sample_size(50)
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(5));
    targets = 
        benchmark_large_file_parsing,
        benchmark_memory_efficiency,
        benchmark_parse_record_variations
}

criterion_main!(performance);

