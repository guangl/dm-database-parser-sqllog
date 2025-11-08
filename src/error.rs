//! 错误类型定义
//!
//! 定义了解析过程中可能出现的所有错误类型。

use thiserror::Error;

/// 解析错误类型
///
/// 包含了 SQL 日志解析过程中可能遇到的所有错误情况。
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseError {
    /// 通用的格式错误
    #[error("invalid format")]
    InvalidFormat,

    /// 文件未找到或无法访问
    #[error("file not found or inaccessible: {0}")]
    FileNotFound(String),

    /// 输入为空
    #[error("empty input: no lines provided")]
    EmptyInput,

    /// 无效的记录起始行
    #[error("invalid record start line: line does not match expected format")]
    InvalidRecordStartLine,

    /// 行长度不足
    #[error("line too short: expected at least 25 characters, got {0}")]
    LineTooShort(usize),

    /// Meta 部分缺少右括号
    #[error("missing closing parenthesis in meta section")]
    MissingClosingParen,

    /// Meta 字段数量不足
    #[error("insufficient meta fields: expected at least 7 fields, got {0}")]
    InsufficientMetaFields(usize),

    /// EP 字段格式错误
    #[error("invalid EP format: expected 'EP[number]', got '{0}'")]
    InvalidEpFormat(String),

    /// EP 数字解析失败
    #[error("failed to parse EP number: {0}")]
    EpParseError(String),

    /// 字段格式不匹配
    #[error("invalid field format: expected '{expected}', got '{actual}'")]
    InvalidFieldFormat {
        /// 期望的前缀
        expected: String,
        /// 实际的字段内容
        actual: String,
    },

    /// 整数解析失败
    #[error("failed to parse {field} as integer: {value}")]
    IntParseError {
        /// 字段名
        field: String,
        /// 字段值
        value: String,
    },

    /// Indicators 解析失败
    #[error("failed to parse indicators: {0}")]
    IndicatorsParseError(String),
}
