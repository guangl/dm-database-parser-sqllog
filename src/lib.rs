//! # DM Database Parser - SQL Log
//!
//! 一个高性能的达梦数据库 SQL 日志解析器，支持流式处理。
//!
//! ## 功能特性
//!
//! - **高性能解析**: 使用内存映射和预编译模式优化性能
//! - **简洁的 API**: 所有字段在解析时一次性填充，直接访问
//! - **完整的类型安全**: 使用强类型结构表示日志数据
//! - **详细的错误信息**: 提供清晰的错误类型和消息
//!
//! ## 快速开始
//!
//! 以下示例展示了最常见的三种使用场景。所有示例假设日志文件为 `sqllog.txt`。
//!
//! ### 示例 1：基础迭代
//!
//! 使用 `LogParserBuilder` 构建解析器，遍历所有 SQL 记录并打印时间戳和 SQL 语句体。
//!
//! ```rust,no_run
//! use dm_database_parser_sqllog::LogParserBuilder;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parser = LogParserBuilder::new("sqllog.txt").build()?;
//! for result in parser.iter() {
//!     let record = result?;
//!     println!("时间戳: {}", record.ts);
//!     println!("SQL: {}", record.sql);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### 示例 2：过滤慢查询
//!
//! 使用 `filter_by_exec_time(100)` 过滤出执行时间 >= 100ms 的慢查询。
//!
//! ```rust,no_run
//! use dm_database_parser_sqllog::LogParserBuilder;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parser = LogParserBuilder::new("sqllog.txt").build()?;
//! for record in parser.iter().filter_by_exec_time(100) {
//!     let sqllog = record?;
//!     println!("{}ms - {}", sqllog.exectime, sqllog.sql);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### 示例 3：批量导出
//!
//! 收集所有记录，提取元数据字段和 SQL 语句体。
//!
//! ```rust,no_run
//! use dm_database_parser_sqllog::LogParserBuilder;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parser = LogParserBuilder::new("sqllog.txt").build()?;
//! let records: Vec<_> = parser.iter().filter_map(|r| r.ok()).collect();
//! for sqllog in &records {
//!     println!("{} | {} | {}", sqllog.ts, sqllog.username, sqllog.sql);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## 日志格式
//!
//! 支持的日志格式示例：
//!
//! ```text
//! 2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:myapp) SELECT * FROM table
//! ```
//!
//! 可选的性能指标：
//!
//! ```text
//! SELECT * FROM table EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.
//! ```

pub(crate) mod error;
pub(crate) mod parser;
pub(crate) mod sqllog;

pub use error::ParseError;
pub use parser::{
    FileEncodingHint, LogIterator, LogParser, LogParserBuilder, parse_record,
};
pub use sqllog::Sqllog;
