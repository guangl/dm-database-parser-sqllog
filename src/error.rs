use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

/// 解析错误类型
///
/// 说明：该枚举表示在将日志字段从字符串解析为数字或其他期望格式时可能发生的错误。
/// 我们把常见的错误包装成特定变体，便于上层调用者进行匹配、报告或恢复处理。
/// - `MissingFields(usize)`：当解析时发现字段数量不足（例如缺少期望的元数据项）时使用；携带期望字段数用于诊断；
/// - `Int(ParseIntError)`：整数解析失败的包装（保留原始 ParseIntError 以便追踪来源）；
/// - `Float(ParseFloatError)`：浮点数解析失败的包装；
/// - `InvalidFormat`：通用格式错误，用于无法归类或发现奇怪格式时的占位错误；
/// - `FileNotFound(String)`：文件不存在或无法访问时的错误。
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("missing fields: expected {0} fields")]
    MissingFields(usize),

    #[error("int parse error: {0}")]
    Int(#[source] ParseIntError),

    #[error("float parse error: {0}")]
    Float(#[source] ParseFloatError),

    #[error("invalid format")]
    InvalidFormat,

    #[error("file not found or inaccessible: {0}")]
    FileNotFound(String),
}
