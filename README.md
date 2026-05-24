# dm-database-parser-sqllog

[![Crates.io](https://img.shields.io/crates/v/dm-database-parser-sqllog.svg)](https://crates.io/crates/dm-database-parser-sqllog)
[![Documentation](https://docs.rs/dm-database-parser-sqllog/badge.svg)](https://docs.rs/dm-database-parser-sqllog)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

一个高性能的达梦数据库 SQL 日志解析库。支持全字段链式 FilterBuilder、tokio 异步接口和顺序迭代，所有字段在解析时一次性填充，直接通过 `Sqllog` 的公开字段访问。

## 安装

```toml
[dependencies]
dm-database-parser-sqllog = "2.0.0"

# 若需要 async 支持
dm-database-parser-sqllog = { version = "2.0.0", features = ["async"] }
```

## 快速开始

### 基础解析

```rust,no_run
use dm_database_parser_sqllog::LogParserBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = LogParserBuilder::new("sqllog.txt").build()?;
    for result in parser.iter() {
        match result {
            Ok(sqllog) => println!("{} | {} | {}", sqllog.ts, sqllog.username, sqllog.sql),
            Err(e) => eprintln!("解析错误: {}", e),
        }
    }
    Ok(())
}
```

### FilterBuilder 过滤

```rust,no_run
use dm_database_parser_sqllog::{LogParserBuilder, FilterBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = FilterBuilder::new()
        .username_eq("SYSDBA")
        .exectime_gt(100.0)
        .sql_contains("SELECT")
        .build();

    let parser = LogParserBuilder::new("sqllog.txt").build()?;
    for record in parser.iter().apply_filter(filter) {
        let sqllog = record?;
        println!("{}ms - {}", sqllog.exectime, sqllog.sql);
    }
    Ok(())
}
```

### AsyncLogParser（需要 feature `async`）

```rust,no_run
use dm_database_parser_sqllog::{AsyncLogParser, FilterBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = FilterBuilder::new()
        .exectime_gt(50.0)
        .build();

    let records = AsyncLogParser::new("sqllog.txt")
        .with_filter(filter)
        .parse()
        .await?;

    println!("共 {} 条慢查询", records.len());
    Ok(())
}
```

### 批量导出

```rust,no_run
use dm_database_parser_sqllog::LogParserBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = LogParserBuilder::new("sqllog.txt").build()?;
    let records: Vec<_> = parser.iter().filter_map(|r| r.ok()).collect();
    for sqllog in &records {
        println!("{} | {} | {} | {}ms",
            sqllog.ts, sqllog.username, sqllog.sql, sqllog.exectime);
    }
    Ok(())
}
```

## 主要特点

- **FilterBuilder**：56 个链式谓词方法，覆盖 14 个字段，AND 语义组合
- **AsyncLogParser**：可选 `async` feature，基于 tokio 的异步解析接口
- **简洁直接**：所有字段在 `Sqllog` 上直接访问，无需调用方法解析
- **零 unsafe**：全 safe Rust 实现
- **GB18030 自动检测**：自动识别文件编码（UTF-8 或 GB18030）
- **错误处理**：`ParseError` 包含行号和原始数据，便于调试

## Sqllog 字段

```rust
pub struct Sqllog {
    pub ts: String,             // 时间戳
    pub tag: Option<String>,    // 标签 [SEL]/[ORA]
    pub ep: u8,                 // EP 编号
    pub sess_id: String,        // 会话 ID
    pub thrd_id: String,        // 线程 ID
    pub username: String,       // 用户名
    pub trxid: String,          // 事务 ID
    pub statement: String,      // 语句 ID
    pub appname: String,        // 应用名称
    pub client_ip: String,      // 客户端 IP
    pub sql: String,            // SQL 语句体
    pub exectime: f32,          // 执行时间 (ms)
    pub rowcount: u32,          // 影响行数
    pub exec_id: i64,           // 执行 ID
}
```

## API 概览

- `LogParserBuilder::new(path).encoding_hint(hint).build()` — 构建同步解析器
- `parser.iter()` — 顺序迭代，返回 `Result<Sqllog, ParseError>`
- `.skip_errors()` / `.filter_by_exec_time(ms)` / `.filter_by_sql_contains(pattern)` / `.apply_filter(filter)` — 链式过滤
- `FilterBuilder::new().<field_predicate>()....build()` — 构建组合过滤器（56 个谓词方法）
- `AsyncLogParser::new(path).with_filter(filter).parse().await` — 异步批量解析（`features = ["async"]`）
- `parse_record(&[u8])` — 独立解析函数

完整文档见 [docs.rs/dm-database-parser-sqllog](https://docs.rs/dm-database-parser-sqllog)。

## 许可证

MIT License — 详见 [LICENSE](LICENSE) 文件
