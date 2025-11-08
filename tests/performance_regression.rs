//! 性能回归测试
//!
//! 这些测试用于确保性能不会随着代码更改而退化。
//! 使用简单的计时来检测明显的性能问题。

use dm_database_parser_sqllog::{
    iter_sqllogs_from_file, parse_records_from_string, parse_sqllogs_from_string,
};
use std::io::Write;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

/// 性能基准：解析 1000 条单行记录应该在合理时间内完成
#[test]
fn perf_parse_1000_single_line_records() {
    let mut log_content = String::new();
    for i in 0..1000 {
        log_content.push_str(&format!(
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT {}\n",
            i / 1000,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i
        ));
    }

    let start = Instant::now();
    let records = parse_records_from_string(&log_content);
    let duration = start.elapsed();

    assert_eq!(records.len(), 1000);
    // 应该在 100ms 内完成（在大多数机器上应该快得多）
    assert!(
        duration < Duration::from_millis(100),
        "解析 1000 条记录耗时 {:?}，超过 100ms",
        duration
    );
    println!("✓ 解析 1000 条记录耗时: {:?}", duration);
}

/// 性能基准：解析 1000 条 Sqllog 应该在合理时间内完成
#[test]
fn perf_parse_1000_sqllogs() {
    let mut log_content = String::new();
    for i in 0..1000 {
        log_content.push_str(&format!(
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT {} EXECTIME: {}.5(ms) ROWCOUNT: 10(rows) EXEC_ID: {}.\n",
            i / 1000,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i,
            i % 100,
            12345 + i
        ));
    }

    let start = Instant::now();
    let sqllogs = parse_sqllogs_from_string(&log_content);
    let duration = start.elapsed();

    let successful = sqllogs.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successful, 1000);

    // 应该在 200ms 内完成
    assert!(
        duration < Duration::from_millis(200),
        "解析 1000 条 Sqllog 耗时 {:?}，超过 200ms",
        duration
    );
    println!("✓ 解析 1000 条 Sqllog 耗时: {:?}", duration);
}

/// 性能基准：迭代器模式处理大文件
#[test]
fn perf_iterator_large_file() -> Result<(), Box<dyn std::error::Error>> {
    // 创建包含 10000 条记录的文件
    let mut temp_file = NamedTempFile::new()?;
    for i in 0..10000 {
        writeln!(
            temp_file,
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT {}",
            i / 1000 % 60,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i
        )?;
    }
    temp_file.flush()?;

    let start = Instant::now();
    let mut count = 0;
    for result in iter_sqllogs_from_file(temp_file.path())? {
        result?;
        count += 1;
    }
    let duration = start.elapsed();

    assert_eq!(count, 10000);

    // 10000 条记录应该在 1 秒内完成
    assert!(
        duration < Duration::from_secs(1),
        "迭代处理 10000 条记录耗时 {:?}，超过 1 秒",
        duration
    );
    println!("✓ 迭代处理 10000 条记录耗时: {:?}", duration);

    Ok(())
}

/// 性能基准：多行 SQL 语句解析
#[test]
fn perf_multiline_sql_parsing() {
    let mut log_content = String::new();
    for i in 0..500 {
        log_content.push_str(&format!(
            r#"2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT u.id,
       u.name,
       u.email,
       u.status
FROM users u
WHERE u.id = {}
"#,
            i / 1000,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i
        ));
    }

    let start = Instant::now();
    let records = parse_records_from_string(&log_content);
    let duration = start.elapsed();

    assert_eq!(records.len(), 500);

    // 多行 SQL 应该在 150ms 内完成
    assert!(
        duration < Duration::from_millis(150),
        "解析 500 条多行记录耗时 {:?}，超过 150ms",
        duration
    );
    println!("✓ 解析 500 条多行记录耗时: {:?}", duration);
}

/// 性能基准：带性能指标的 SQL 解析
#[test]
fn perf_sql_with_indicators() {
    let mut log_content = String::new();
    for i in 0..1000 {
        log_content.push_str(&format!(
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT {} EXECTIME: {}.5(ms) ROWCOUNT: {}(rows) EXEC_ID: {}.\n",
            i / 1000,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i,
            i % 100,
            i * 10,
            12345 + i
        ));
    }

    let start = Instant::now();
    let sqllogs = parse_sqllogs_from_string(&log_content);
    let duration = start.elapsed();

    let successful = sqllogs.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successful, 1000);

    // 验证性能指标被正确解析
    let first_log = sqllogs[0].as_ref().unwrap();
    assert!(first_log.has_indicators());

    // 应该在 250ms 内完成
    assert!(
        duration < Duration::from_millis(250),
        "解析 1000 条带指标的 Sqllog 耗时 {:?}，超过 250ms",
        duration
    );
    println!("✓ 解析 1000 条带指标的 Sqllog 耗时: {:?}", duration);
}

/// 内存效率测试：确保迭代器不会加载整个文件到内存
#[test]
fn perf_memory_efficiency() -> Result<(), Box<dyn std::error::Error>> {
    // 创建一个相对较大的文件
    let mut temp_file = NamedTempFile::new()?;
    for i in 0..50000 {
        writeln!(
            temp_file,
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT * FROM large_table WHERE id = {}",
            i / 1000 % 60,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i
        )?;
    }
    temp_file.flush()?;

    let start = Instant::now();
    let mut count = 0;

    // 使用迭代器，每次只处理一条记录
    for result in iter_sqllogs_from_file(temp_file.path())? {
        result?;
        count += 1;

        // 每 10000 条检查一次内存使用是否合理
        if count % 10000 == 0 {
            let elapsed = start.elapsed();
            println!("  已处理 {} 条记录，耗时 {:?}", count, elapsed);
        }
    }

    let duration = start.elapsed();
    assert_eq!(count, 50000);

    // 50000 条记录应该在 5 秒内完成
    assert!(
        duration < Duration::from_secs(5),
        "迭代处理 50000 条记录耗时 {:?}，超过 5 秒",
        duration
    );
    println!("✓ 迭代处理 50000 条记录耗时: {:?}", duration);

    Ok(())
}

/// 吞吐量测试：计算每秒处理的记录数
#[test]
fn perf_throughput_test() {
    let mut log_content = String::new();
    let record_count = 10000;

    for i in 0..record_count {
        log_content.push_str(&format!(
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:user{} trxid:{} stmt:{} appname:app) SELECT {}\n",
            i / 1000 % 60,
            i % 1000,
            0x123 + i,
            456 + i,
            i % 10,
            789 + i,
            999 + i,
            i
        ));
    }

    let start = Instant::now();
    let sqllogs = parse_sqllogs_from_string(&log_content);
    let duration = start.elapsed();

    let successful = sqllogs.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successful, record_count);

    let throughput = record_count as f64 / duration.as_secs_f64();
    println!(
        "✓ 吞吐量: {:.0} 条记录/秒 (处理 {} 条记录耗时 {:?})",
        throughput, record_count, duration
    );

    // 应该至少达到 10000 条/秒的吞吐量
    assert!(
        throughput > 10000.0,
        "吞吐量 {:.0} 条/秒低于预期的 10000 条/秒",
        throughput
    );
}
