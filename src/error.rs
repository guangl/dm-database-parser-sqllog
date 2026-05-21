//! 错误类型定义
//!
//! 定义了解析过程中可能出现的所有错误类型。
//!
//! # 用法说明
//!
//! ParseError 仅作为库 API 的错误返回类型，普通用户无需手动构造。

use thiserror::Error;

/// 解析错误类型
///
/// 包含了 SQL 日志解析过程中可能遇到的所有错误情况。
/// 所有错误都包含原始输入数据以便于调试。
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseError {
    /// 通用的格式错误
    #[error("invalid format at line {line_number} | raw: {raw}")]
    InvalidFormat {
        /// 原始输入数据
        raw: String,
        /// 文件行号
        line_number: u64,
    },

    /// 文件未找到或无法访问
    #[error("file not found or inaccessible: {path}")]
    FileNotFound {
        /// 文件路径
        path: String,
    },

    /// IO 操作错误
    #[error("IO error: {0}")]
    IoError(String),
}
