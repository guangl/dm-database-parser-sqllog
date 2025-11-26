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
    #[error("invalid format | raw: {raw}")]
    InvalidFormat {
        /// 原始输入数据
        raw: String,
    },

    /// 文件未找到或无法访问
    #[error("file not found or inaccessible: {path}")]
    FileNotFound {
        /// 文件路径
        path: String,
    },

    /// 输入为空
    #[error("empty input: no lines provided")]
    EmptyInput,

    /// 无效的记录起始行
    #[error("invalid record start line: line does not match expected format | raw: {raw}")]
    InvalidRecordStartLine {
        /// 原始行内容
        raw: String,
    },

    /// 行长度不足
    #[error("line too short: expected at least 25 characters, got {length} | raw: {raw}")]
    LineTooShort {
        /// 实际长度
        length: usize,
        /// 原始行内容
        raw: String,
    },

    /// Meta 部分缺少右括号
    #[error("missing closing parenthesis in meta section | raw: {raw}")]
    MissingClosingParen {
        /// 原始行内容
        raw: String,
    },

    /// Meta 字段数量不足
    #[error("insufficient meta fields: expected at least 5 fields, got {count} | raw: {raw}")]
    InsufficientMetaFields {
        /// 实际字段数量
        count: usize,
        /// 原始 meta 内容
        raw: String,
    },

    /// EP 字段格式错误
    #[error("invalid EP format: expected 'EP[number]', got '{value}' | raw: {raw}")]
    InvalidEpFormat {
        /// EP 字段值
        value: String,
        /// 原始 meta 内容
        raw: String,
    },

    /// EP 数字解析失败
    #[error("failed to parse EP number: {value} | raw: {raw}")]
    EpParseError {
        /// EP 数字值
        value: String,
        /// 原始 meta 内容
        raw: String,
    },

    /// 字段格式不匹配
    #[error("invalid field format: expected '{expected}', got '{actual}' | raw: {raw}")]
    InvalidFieldFormat {
        /// 期望的前缀
        expected: String,
        /// 实际的字段内容
        actual: String,
        /// 原始 meta 内容
        raw: String,
    },

    /// 整数解析失败
    #[error("failed to parse {field} as integer: {value} | raw: {raw}")]
    IntParseError {
        /// 字段名
        field: String,
        /// 字段值
        value: String,
        /// 原始内容
        raw: String,
    },

    /// Indicators 解析失败
    #[error("failed to parse indicators: {reason} | raw: {raw}")]
    IndicatorsParseError {
        /// 失败原因
        reason: String,
        /// 原始 body 内容
        raw: String,
    },

    /// IO 操作错误
    #[error("IO error: {0}")]
    IoError(String),
}
