# dm-database-parser-sqllog

[![Crates.io](https://img.shields.io/crates/v/dm-database-parser-sqllog.svg)](https://crates.io/crates/dm-database-parser-sqllog)
[![Documentation](https://docs.rs/dm-database-parser-sqllog/badge.svg)](https://docs.rs/dm-database-parser-sqllog)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Coverage](https://img.shields.io/badge/coverage-%E2%89%A5%2090%25-brightgreen.svg)](docs/COVERAGE.md)

一个高性能的达梦数据库 sqllog 日志解析库，提供零分配或低分配的记录切分与解析功能。

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dm-database-parser-sqllog = "1.1.0"
```

## 快速开始

以下三个场景展示了库的核心用法。所有示例假设日志文件为 `sqllog.txt`。

### 基础解析

使用 `LogParserBuilder` 构建解析器，遍历所有 SQL 记录并打印 SQL 语句体。

```rust,no_run
use dm_database_parser_sqllog::LogParserBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = LogParserBuilder::new("sqllog.txt").build()?;
    for result in parser.iter() {
        match result {
            Ok(sqllog) => println!("SQL: {}", sqllog.body()),
            Err(e) => eprintln!("解析错误: {}", e),
        }
    }
    Ok(())
}
```

### 过滤慢查询

使用 `filter_by_exec_time(100)` 过滤执行时间 >= 100ms 的慢查询，通过 `exec_time()` 获取耗时。

```rust,no_run
use dm_database_parser_sqllog::LogParserBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = LogParserBuilder::new("sqllog.txt").build()?;
    for record in parser.iter().filter_by_exec_time(100) {
        let sqllog = record?;
        let exec_time = sqllog.exec_time()?.unwrap_or(0.0);
        println!("{}ms - {}", exec_time, sqllog.body());
    }
    Ok(())
}
```

### 批量导出

收集所有记录，提取元数据字段和 SQL 语句体，用于聚合分析或导出为 CSV。

```rust,no_run
use dm_database_parser_sqllog::LogParserBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = LogParserBuilder::new("sqllog.txt").build()?;
    let records: Vec<_> = parser.iter().filter_map(|r| r.ok()).collect();
    for sqllog in &records {
        let meta = sqllog.parse_meta();
        println!("{} | {} | {}", sqllog.ts, meta.username, sqllog.body());
    }
    Ok(())
}
```

## 主要特点

- **零拷贝 / 惰性解析**：使用 `Cow<'a, str>` 和惰性字段，避免不必要的堆分配
- **内存映射 I/O**：通过 mmap 处理大型日志文件，1 GB 文件 < 1 秒
- **极致性能**：单线程吞吐 **8.67 GiB/s**（5 MB 合成语料库，含 20% 多行记录，Apple M 系列芯片）
- **GB18030 自动检测**：自动识别文件编码（UTF-8 或 GB18030），无需手动指定
- **LogParserBuilder 链式 API**：通过 Builder 模式灵活配置解析器
- **过滤方法**：`filter_by_exec_time` 按执行时间过滤、`filter_by_sql_contains` 按 SQL 内容过滤
- **直接字段访问**：`exec_time()` / `row_count()` 无需解构元组即可取值
- **FromSqllog trait**：将 Sqllog 映射为自定义业务类型
- **Rayon 并行迭代**：`par_iter()` 多线程加速

## API 概览

- `LogParserBuilder` — 链式构建解析器，支持 `new(path).build()`
- `Sqllog` — 日志记录，提供 `body()`、`parse_meta()`、`parse_performance_metrics()`、`exec_time()`、`row_count()`
- `LogIterator` — 迭代器，支持 `filter_by_exec_time(ms)`、`filter_by_sql_contains(pattern)`
- `FromSqllog` trait — 自定义类型转换

完整 API 文档见 [docs.rs/dm-database-parser-sqllog](https://docs.rs/dm-database-parser-sqllog)。

## 许可证

MIT License — 详见 [LICENSE](LICENSE) 文件


## 相关链接

- [Crates.io](https://crates.io/crates/dm-database-parser-sqllog)
- [文档](https://docs.rs/dm-database-parser-sqllog)
- [GitHub](https://github.com/guangl/dm-database-parser-sqllog)
