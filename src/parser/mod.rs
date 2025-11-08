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
mod sqllog_parser;

#[cfg(test)]
mod tests;

// 重导出公共 API
pub use api::{
    for_each_sqllog,
    for_each_sqllog_from_file,
    for_each_sqllog_in_string,
    iter_records_from_file,
    iter_sqllogs_from_file,
    parse_records_from_file,
    parse_records_from_string,
    parse_sqllogs_from_file,
    parse_sqllogs_from_string,
    // 向后兼容的 deprecated 别名
    records_from_file,
    sqllogs_from_file,
};
pub use parse_functions::parse_record;
pub use record::Record;
pub use record_parser::RecordParser;
pub use sqllog_parser::SqllogParser;
