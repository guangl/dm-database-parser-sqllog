use std::error::Error;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};

/// 解析错误类型
///
/// 说明：该枚举表示在将日志字段从字符串解析为数字或其他期望格式时可能发生的错误。
/// 我们把常见的错误包装成特定变体，便于上层调用者进行匹配、报告或恢复处理。
/// - `MissingFields(usize)`：当解析时发现字段数量不足（例如缺少期望的元数据项）时使用；携带期望字段数用于诊断；
/// - `Int(ParseIntError)`：整数解析失败的包装（保留原始 ParseIntError 以便追踪来源）；
/// - `Float(ParseFloatError)`：浮点数解析失败的包装；
/// - `InvalidFormat`：通用格式错误，用于无法归类或发现奇怪格式时的占位错误。
#[derive(Debug)]
pub enum ParseError {
    MissingFields(usize),
    Int(ParseIntError),
    Float(ParseFloatError),
    InvalidFormat,
}

impl fmt::Display for ParseError {
    /// 为 ParseError 提供可读的错误信息，方便在日志或用户界面中直接打印。
    ///
    /// 目的：实现 Display 可以让错误在使用 `eprintln!`、`format!` 或 `println!` 时显示更友好的文本。
    /// 同时，Display 不丢弃底层错误的信息；`Error::source()` 会指向原始的解析错误（若有），以便链式错误追踪。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingFields(n) => write!(f, "missing fields: expected {} fields", n),
            ParseError::Int(e) => write!(f, "int parse error: {}", e),
            ParseError::Float(e) => write!(f, "float parse error: {}", e),
            ParseError::InvalidFormat => write!(f, "invalid format"),
        }
    }
}

impl Error for ParseError {
    /// 返回底层错误以支持错误链（`source`），这对于调试和日志记录很有用。
    /// 例如，当 `ParseIntError` 发生时，`source()` 会返回原始的 `ParseIntError`，调用方可以通过它获取更多上下文。
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::Int(e) => Some(e),
            ParseError::Float(e) => Some(e),
            _ => None,
        }
    }
}

/// 为方便使用 `?` 操作符，将标准库的解析错误转换为 `ParseError`。
///
/// 解释：实现 `From<ParseIntError>` / `From<ParseFloatError>` 能让在解析整数/浮点数时直接使用 `?`，
/// 并自动将底层错误转换为本 crate 的统一错误类型，简化上层错误传播逻辑。
impl From<ParseIntError> for ParseError {
    fn from(e: ParseIntError) -> Self {
        ParseError::Int(e)
    }
}

impl From<ParseFloatError> for ParseError {
    fn from(e: ParseFloatError) -> Self {
        ParseError::Float(e)
    }
}