//! Parser 模块 - 解析 DM 数据库 SQL 日志
//!
//! 此模块提供了完整的日志解析功能,包括:
//! - Record 结构和解析
//! - 流式读取和解析
//! - 便捷 API 函数

mod api;
mod constants;
pub(crate) mod parse_functions;
pub mod record;
pub mod record_parser;

pub use api::{iter_records_from_file, parse_records_from_file};
pub use record::Record;
pub use record_parser::RecordParser;

// 测试辅助模块 - 仅在测试时导出内部函数
#[cfg(test)]
pub mod test_helpers {
    pub use super::parse_functions::*;
}
