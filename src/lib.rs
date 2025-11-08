//! # dm-database-parser-sqllog
//!
//! 一个高性能的达梦数据库 sqllog 日志解析库，提供零分配或低分配的记录切分与解析功能。
//!
//! ## 主要特点
//!
//! - **零分配解析**：基于时间戳的记录切分，使用流式 API 避免额外内存分配
//! - **高效模式匹配**：使用双数组 Aho-Corasick（daachorse）进行高效模式匹配
//! - **轻量级结构**：解析结果使用引用（`&str`），避免不必要的字符串复制
//! - **灵活的 API**：提供批量解析、流式解析等多种使用方式
//!
//! ## 快速开始
//!
//! ```rust
//! use dm_database_parser_sqllog::{split_by_ts_records_with_errors, parse_record, for_each_record};
//!
//! let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:joe trxid:0 stmt:1 appname:MyApp) SELECT 1"#;
//!
//! // 将文本拆分为记录并获取前导错误
//! let (records, errors) = split_by_ts_records_with_errors(log_text);
//! println!("records: {}, leading errors: {}", records.len(), errors.len());
//!
//! // 流式处理每条记录（零分配）
//! for_each_record(log_text, |rec| {
//!     let parsed = parse_record(rec);
//!     println!("ts={} body={}", parsed.ts, parsed.body);
//! });
//! ```
//!
//! ## 模块组织
//!
//! - [`parser`] - 核心解析功能，包括记录切分和解析
//! - [`error`] - 错误类型定义
//! - [`sqllog`] - Sqllog 结构体定义

pub mod error;
pub mod matcher;
pub mod parser;
pub mod realtime;
pub mod sqllog;
mod tools;

// Re-export commonly used types and functions
pub use error::ParseError;
pub use parser::{
    ParsedRecord, RecordSplitter, for_each_record, parse_all, parse_into, parse_record,
    parse_records_with, split_by_ts_records_with_errors, split_into,
};
pub use sqllog::Sqllog;
