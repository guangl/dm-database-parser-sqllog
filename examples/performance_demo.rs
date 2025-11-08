use dm_database_parser_sqllog::{RecordParser, SqllogParser};
use std::io::Cursor;
use std::time::Instant;

fn main() {
    println!("=== Parser 性能快速测试 ===\n");

    // 生成测试数据
    let record_count = 10000;
    println!("生成 {} 条测试记录...", record_count);

    let mut logs = Vec::with_capacity(record_count);
    for i in 0..record_count {
        if i % 3 == 0 {
            // 多行记录
            logs.push(format!(
                "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:USER_{} trxid:{} stmt:0x{:x} appname:app ip:::ffff:192.168.1.{}) SELECT u.id, u.name\nFROM users u\nWHERE u.id = {} EXECTIME: {}(ms) ROWCOUNT: {}(rows) EXEC_ID: {}.",
                i % 60,
                i % 1000,
                0x178ebca0 + i,
                757455 + i,
                i,
                i,
                0x285eb060 + i,
                i % 256,
                i,
                i % 100,
                i % 10,
                i * 100
            ));
        } else {
            // 单行记录
            logs.push(format!(
                "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:0x{:x} thrd:{} user:USER_{} trxid:{} stmt:0x{:x} appname:app) SELECT {} FROM table",
                i % 60,
                i % 1000,
                0x178ebca0 + i,
                757455 + i,
                i,
                i,
                0x285eb060 + i,
                i
            ));
        }
    }

    let log_content = logs.join("\n");
    let total_bytes = log_content.len();
    println!("总大小: {:.2} MB\n", total_bytes as f64 / 1024.0 / 1024.0);

    // 测试 1: RecordParser
    println!("【测试 1】RecordParser - 仅分组记录");
    let start = Instant::now();
    let cursor = Cursor::new(log_content.as_bytes());
    let parser = RecordParser::new(cursor);
    let records: Vec<_> = parser.filter_map(|r| r.ok()).collect();
    let duration = start.elapsed();

    println!("  记录数: {}", records.len());
    println!("  耗时: {:.2} ms", duration.as_secs_f64() * 1000.0);
    println!(
        "  吞吐量: {:.2} MiB/s",
        (total_bytes as f64 / 1024.0 / 1024.0) / duration.as_secs_f64()
    );
    println!(
        "  每条记录: {:.2} µs\n",
        duration.as_secs_f64() * 1_000_000.0 / records.len() as f64
    );

    // 测试 2: SqllogParser
    println!("【测试 2】SqllogParser - 完整解析");
    let start = Instant::now();
    let cursor = Cursor::new(log_content.as_bytes());
    let parser = SqllogParser::new(cursor);
    let sqllogs: Vec<_> = parser.filter_map(|r| r.ok()).collect();
    let duration = start.elapsed();

    println!("  记录数: {}", sqllogs.len());
    println!("  耗时: {:.2} ms", duration.as_secs_f64() * 1000.0);
    println!(
        "  吞吐量: {:.2} MiB/s",
        (total_bytes as f64 / 1024.0 / 1024.0) / duration.as_secs_f64()
    );
    println!(
        "  每条记录: {:.2} µs\n",
        duration.as_secs_f64() * 1_000_000.0 / sqllogs.len() as f64
    );

    // 测试 3: RecordParser + 选择性解析
    println!("【测试 3】RecordParser + 条件解析 (只解析多行记录)");
    let start = Instant::now();
    let cursor = Cursor::new(log_content.as_bytes());
    let parser = RecordParser::new(cursor);
    let mut parsed_count = 0;
    let mut total_records = 0;

    for result in parser {
        if let Ok(record) = result {
            total_records += 1;
            // 只解析多行记录
            if record.has_continuation_lines() {
                if let Ok(_sqllog) = record.parse_to_sqllog() {
                    parsed_count += 1;
                }
            }
        }
    }
    let duration = start.elapsed();

    println!("  总记录数: {}", total_records);
    println!("  解析记录数: {}", parsed_count);
    println!("  耗时: {:.2} ms", duration.as_secs_f64() * 1000.0);
    println!(
        "  吞吐量: {:.2} MiB/s",
        (total_bytes as f64 / 1024.0 / 1024.0) / duration.as_secs_f64()
    );
    println!(
        "  每条记录: {:.2} µs\n",
        duration.as_secs_f64() * 1_000_000.0 / total_records as f64
    );

    // 统计信息
    println!("=== 统计信息 ===");
    let sample_sqllog = &sqllogs[0];
    println!("  样本记录:");
    println!("    时间戳: {}", sample_sqllog.ts);
    println!("    用户: {}", sample_sqllog.meta.username);
    println!(
        "    SQL: {}",
        sample_sqllog.body.lines().next().unwrap_or("")
    );

    let with_indicators = sqllogs.iter().filter(|s| s.indicators.is_some()).count();
    println!(
        "\n  带 Indicators 的记录: {}/{}",
        with_indicators,
        sqllogs.len()
    );

    let multi_line = records
        .iter()
        .filter(|r| r.has_continuation_lines())
        .count();
    println!("  多行记录: {}/{}", multi_line, records.len());

    println!("\n=== 性能对比 ===");
    println!("  RecordParser:     最快，适合只需要分组的场景");
    println!("  SqllogParser:     完整解析，适合需要结构化数据的场景");
    println!("  条件解析:         折中方案，按需解析减少开销");

    // 推算大文件性能
    println!("\n=== 大文件性能推算 (基于当前测试) ===");
    let mb_per_sec_record = (total_bytes as f64 / 1024.0 / 1024.0)
        / (duration.as_secs_f64() * sqllogs.len() as f64 / total_records as f64);

    for file_size_gb in [1, 10, 100] {
        let file_size_mb = file_size_gb * 1024;
        let estimated_time = file_size_mb as f64 / mb_per_sec_record;
        println!(
            "  {} GB 文件: 约 {:.1} 秒 (RecordParser)",
            file_size_gb, estimated_time
        );
    }
}
