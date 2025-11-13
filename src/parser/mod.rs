//! Parser 模块 - 解析 DM 数据库 SQL 日志
//!
//! 此模块提供了完整的日志解析功能，包括：
//! - Record 结构和解析
//! - 流式读取和解析
//! - 便捷 API 函数

mod api;
mod constants;
mod parse_functions;
mod record;
mod record_parser;

// 重导出公共 API
pub use api::{iter_records_from_file, parse_records_from_file};
pub use record::Record;
pub use record_parser::RecordParser;
