//! # DM Database Parser - SQL Log
//!
//! 一个高性能的达梦数据库 SQL 日志解析器，支持批量解析和流式处理。
//!
//! ## 功能特性
//!
//! - **高性能解析**: 使用零拷贝和预编译正则表达式优化性能
//! - **灵活的 API**: 支持批量解析和流式处理两种模式
//! - **完整的类型安全**: 使用强类型结构表示日志数据
//! - **详细的错误信息**: 提供清晰的错误类型和消息
//!
//! ## 快速开始
//!
//! ### 批量解析
//!
//! ```rust
//! use dm_database_parser_sqllog::parse_sqllogs_from_string;
//!
//! let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users"#;
//! let results = parse_sqllogs_from_string(log_content);
//!
//! for result in results {
//!     if let Ok(sqllog) = result {
//!         println!("时间戳: {}", sqllog.ts);
//!         println!("用户: {}", sqllog.meta.username);
//!         println!("SQL: {}", sqllog.body);
//!     }
//! }
//! ```
//!
//! ### 流式处理
//!
//! ```rust
//! use dm_database_parser_sqllog::for_each_sqllog_in_string;
//!
//! let log_content = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"#;
//!
//! let _result = for_each_sqllog_in_string(log_content, |sqllog| {
//!     println!("处理记录: EP={}, 会话={}", sqllog.meta.ep, sqllog.meta.sess_id);
//! });
//! ```
//!
//! ### 从文件流式读取
//!
//! ```rust,no_run
//! use dm_database_parser_sqllog::for_each_sqllog;
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! let file = File::open("sqllog.txt").unwrap();
//! let reader = BufReader::new(file);
//!
//! let _result = for_each_sqllog(reader, |sqllog| {
//!     // 处理每条日志记录
//!     println!("SQL: {}", sqllog.body);
//! });
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

pub mod error;
pub mod parser;
pub mod sqllog;
pub mod tools;

pub use error::ParseError;
pub use parser::{
    Record,
    RecordParser,
    SqllogParser,
    for_each_sqllog,
    for_each_sqllog_from_file,
    for_each_sqllog_in_string,
    iter_records_from_file,
    iter_sqllogs_from_file,
    parse_record,
    parse_records_from_file,
    parse_records_from_string,
    parse_sqllogs_from_file,
    parse_sqllogs_from_string,
    // 向后兼容的 deprecated 别名
    records_from_file,
    sqllogs_from_file,
};
pub use sqllog::Sqllog;
