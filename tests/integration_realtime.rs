//! 实时解析器集成测试

use dm_database_parser_sqllog::realtime::{ParserConfig, RealtimeParser};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::NamedTempFile;

#[test]
fn test_realtime_parser_basic() -> Result<(), Box<dyn std::error::Error>> {
    // 创建临时文件
    let mut temp_file = NamedTempFile::new()?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1"
    )?;
    temp_file.flush()?;

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(100),
        buffer_size: 1024,
    };

    let mut parser = RealtimeParser::new(config)?;

    // 解析第一条记录
    let mut records = Vec::new();
    parser.parse_new_records(|parsed| {
        records.push((parsed.user.to_string(), parsed.trxid.to_string()));
    })?;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, "admin");
    assert_eq!(records[0].1, "0");

    Ok(())
}

#[test]
fn test_incremental_parsing() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;

    // 写入第一批记录
    writeln!(
        temp_file,
        "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:100 stmt:1 appname:MyApp) SELECT 1"
    )?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:101 stmt:2 appname:MyApp) SELECT 2"
    )?;
    temp_file.flush()?;

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(50),
        buffer_size: 2048,
    };

    let mut parser = RealtimeParser::new(config)?;

    // 第一次解析
    let mut count1 = 0;
    parser.parse_new_records(|_| count1 += 1)?;
    assert_eq!(count1, 2, "应该解析 2 条初始记录");

    let pos_after_first = parser.position();
    assert!(pos_after_first > 0, "文件位置应该前进");

    // 追加新记录
    writeln!(
        temp_file,
        "2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:102 stmt:3 appname:MyApp) SELECT 3"
    )?;
    temp_file.flush()?;

    // 第二次解析（增量）
    let mut count2 = 0;
    parser.parse_new_records(|parsed| {
        count2 += 1;
        assert_eq!(parsed.user, "test", "新记录的用户应该是 test");
        assert_eq!(parsed.trxid, "102", "新记录的事务ID应该是 102");
    })?;
    assert_eq!(count2, 1, "应该只解析 1 条新记录");

    let pos_after_second = parser.position();
    assert!(pos_after_second > pos_after_first, "文件位置应该继续前进");

    Ok(())
}

#[test]
fn test_parse_all() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;

    writeln!(
        temp_file,
        "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1"
    )?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2"
    )?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:11.456 (EP[0] sess:3 thrd:3 user:test trxid:0 stmt:3 appname:MyApp) SELECT 3"
    )?;
    temp_file.flush()?;

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(50),
        buffer_size: 4096,
    };

    let mut parser = RealtimeParser::new(config)?;

    let mut users = Vec::new();
    let total = parser.parse_all(|parsed| {
        users.push(parsed.user.to_string());
    })?;

    assert_eq!(total, 3, "应该解析 3 条记录");
    assert_eq!(users, vec!["admin", "guest", "test"]);

    Ok(())
}

#[test]
fn test_seek_and_reset() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;

    writeln!(
        temp_file,
        "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1"
    )?;
    writeln!(
        temp_file,
        "2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2"
    )?;
    temp_file.flush()?;

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(50),
        buffer_size: 1024,
    };

    let mut parser = RealtimeParser::new(config)?;

    // 解析所有记录
    let mut count = 0;
    parser.parse_new_records(|_| count += 1)?;
    assert_eq!(count, 2);

    let final_pos = parser.position();
    assert!(final_pos > 0);

    // 重置并重新解析
    parser.reset();
    assert_eq!(parser.position(), 0, "重置后位置应该为 0");

    count = 0;
    parser.parse_new_records(|_| count += 1)?;
    assert_eq!(count, 2, "重置后应该重新解析所有记录");

    // 测试 seek_to
    parser.seek_to(final_pos);
    assert_eq!(parser.position(), final_pos);

    count = 0;
    parser.parse_new_records(|_| count += 1)?;
    assert_eq!(count, 0, "seek 到末尾后不应该有新记录");

    Ok(())
}

#[test]
fn test_empty_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(50),
        buffer_size: 1024,
    };

    let mut parser = RealtimeParser::new(config)?;

    let count = parser.parse_new_records(|_| {})?;
    assert_eq!(count, 0, "空文件应该返回 0 条记录");

    Ok(())
}

#[test]
fn test_file_not_found() {
    let config = ParserConfig {
        file_path: PathBuf::from("nonexistent_file.log"),
        poll_interval: Duration::from_millis(50),
        buffer_size: 1024,
    };

    let result = RealtimeParser::new(config);
    assert!(result.is_err(), "不存在的文件应该返回错误");
}

#[test]
fn test_multiple_incremental_reads() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path().to_path_buf();

    // 初始写入
    {
        let mut file = OpenOptions::new().write(true).open(&path)?;
        writeln!(
            file,
            "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:user1 trxid:0 stmt:1 appname:MyApp) SELECT 1"
        )?;
        file.flush()?;
    }

    let config = ParserConfig {
        file_path: path.clone(),
        poll_interval: Duration::from_millis(50),
        buffer_size: 512,
    };

    let mut parser = RealtimeParser::new(config)?;

    // 多次增量读取
    for i in 2..=5 {
        let mut count = 0;
        parser.parse_new_records(|_| count += 1)?;
        assert!(count > 0 || i > 2, "第 {} 次读取", i);

        // 追加新记录
        let mut file = OpenOptions::new().append(true).open(&path)?;
        writeln!(
            file,
            "2025-08-12 10:57:{:02}.000 (EP[0] sess:{} thrd:{} user:user{} trxid:0 stmt:{} appname:MyApp) SELECT {}",
            9 + i,
            i,
            i,
            i,
            i,
            i
        )?;
        file.flush()?;
    }

    Ok(())
}

#[test]
fn test_large_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut temp_file = NamedTempFile::new()?;

    // 写入多条记录
    for i in 1..=100 {
        writeln!(
            temp_file,
            "2025-08-12 10:57:{:02}.{:03} (EP[0] sess:{} thrd:{} user:user{} trxid:{} stmt:{} appname:MyApp) SELECT {}",
            i / 10,
            (i % 10) * 100,
            i,
            i,
            i % 5,
            i,
            i,
            i
        )?;
    }
    temp_file.flush()?;

    let config = ParserConfig {
        file_path: temp_file.path().to_path_buf(),
        poll_interval: Duration::from_millis(50),
        buffer_size: 16384, // 16KB
    };

    let mut parser = RealtimeParser::new(config)?;

    let mut count = 0;
    parser.parse_new_records(|_| count += 1)?;
    assert_eq!(count, 100, "应该解析 100 条记录");

    Ok(())
}
