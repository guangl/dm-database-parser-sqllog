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
//! ### 从文件迭代处理 Sqllogs（推荐）
//!
//! ```rust,no_run
//! use dm_database_parser_sqllog::iter_records_from_file;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! for result in iter_records_from_file("sqllog.txt")? {
//!     match result {
//!         Ok(sqllog) => {
//!             println!("时间戳: {}", sqllog.ts);
//!             println!("用户: {}", sqllog.meta.username);
//!             println!("SQL: {}", sqllog.body);
//!         }
//!         Err(e) => eprintln!("解析错误: {}", e),
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### 从文件批量加载并解析为 Sqllog（自动并行处理）
//!
//! ```rust,no_run
//! use dm_database_parser_sqllog::parse_records_from_file;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // parse_records_from_file 现在直接返回 Vec<Sqllog>，内部自动使用并行处理
//! let (sqllogs, errors) = parse_records_from_file("sqllog.txt")?;
//! println!("成功解析 {} 条 SQL 日志", sqllogs.len());
//! println!("遇到 {} 个错误", errors.len());
//!
//! // 直接使用解析好的 Sqllog
//! for sqllog in sqllogs {
//!     println!("用户: {}, SQL: {}", sqllog.meta.username, sqllog.body);
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

pub mod error;
pub mod sqllog;

// 保留 parser 和 tools 模块作为公共模块，但不自动重导出所有内容
pub mod parser;
pub mod tools;

#[cfg(feature = "realtime")]
pub mod realtime;

// 核心类型
pub use error::ParseError;
pub use sqllog::Sqllog;

// 核心解析器类型
pub use parser::{Record, RecordParser};

// 文件解析 API
pub use parser::{iter_records_from_file, parse_records_from_file};

// 测试辅助 API - 仅供测试使用
#[doc(hidden)]
pub mod __test_helpers {
    pub use crate::parser::parse_functions::*;
}
