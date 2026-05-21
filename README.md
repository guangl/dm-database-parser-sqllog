# dm-database-parser-sqllog

[![Crates.io](https://img.shields.io/crates/v/dm-database-parser-sqllog.svg)](https://crates.io/crates/dm-database-parser-sqllog)
[![Documentation](https://docs.rs/dm-database-parser-sqllog/badge.svg)](https://docs.rs/dm-database-parser-sqllog)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

一个简洁的达梦数据库 SQL 日志解析库。所有字段在解析时一次性填充，直接通过 `Sqllog` 结构体的公开字段访问。

## 安装

```toml
[dependencies]
dm-database-parser-sqllog = "1.1.0"
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

### 过滤慢查询

```rust,no_run
use dm_database_parser_sqllog::LogParserBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = LogParserBuilder::new("sqllog.txt").build()?;
    for record in parser.iter().filter_by_exec_time(100) {
        let sqllog = record?;
        println!("{}ms - {}", sqllog.exectime, sqllog.sql);
    }
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

- **简洁直接**：所有字段在 `Sqllog` 上直接访问，无需调用方法解析
- **零 unsafe**：全 safe Rust 实现
- **GB18030 自动检测**：自动识别文件编码（UTF-8 或 GB18030）
- **过滤方法**：`filter_by_exec_time` / `filter_by_sql_contains` 链式过滤
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

- `LogParserBuilder::new(path).encoding_hint(hint).build()` — 构建解析器
- `parser.iter()` — 顺序迭代，返回 `Result<Sqllog, ParseError>`
- `.skip_errors()` / `.filter_by_exec_time(ms)` / `.filter_by_sql_contains(pattern)` — 链式过滤
- `parse_record(&[u8])` — 独立解析函数

完整文档见 [docs.rs/dm-database-parser-sqllog](https://docs.rs/dm-database-parser-sqllog)。

## 许可证

MIT License — 详见 [LICENSE](LICENSE) 文件
