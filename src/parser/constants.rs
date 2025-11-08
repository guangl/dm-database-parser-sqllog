//! 解析器使用的常量定义
//!
//! 定义了解析过程中使用的所有常量，包括长度常量、字段前缀、指标模式等。

use once_cell::sync::Lazy;

// 长度相关常量

/// 时间戳字符串的固定长度（"YYYY-MM-DD HH:MM:SS.mmm"）
pub const TIMESTAMP_LENGTH: usize = 23;

/// 记录起始行的最小长度
pub const MIN_RECORD_LENGTH: usize = 25;

/// Meta 部分的起始索引（时间戳后 + 空格 + 左括号）
pub const META_START_INDEX: usize = 25;

/// Body 部分相对于右括号的偏移量（") " 两个字符）
pub const BODY_OFFSET: usize = 2;

// 使用 Lazy 静态初始化 indicator 模式集合，避免重复创建

/// Indicator 关键字模式数组（用于查找 indicator 在 body 中的位置）
pub static INDICATOR_PATTERNS: Lazy<[&'static str; 3]> =
    Lazy::new(|| ["EXECTIME:", "ROWCOUNT:", "EXEC_ID:"]);

// Meta 字段前缀常量

/// 会话 ID 字段前缀
pub static SESS_PREFIX: &str = "sess:";

/// 线程 ID 字段前缀
pub static THRD_PREFIX: &str = "thrd:";

/// 用户名字段前缀
pub static USER_PREFIX: &str = "user:";

/// 事务 ID 字段前缀
pub static TRXID_PREFIX: &str = "trxid:";

/// 语句 ID 字段前缀
pub static STMT_PREFIX: &str = "stmt:";

/// 应用名称字段前缀
pub static APPNAME_PREFIX: &str = "appname:";

/// IP 地址字段前缀
pub static IP_PREFIX: &str = "ip:::ffff:";

// Indicator 相关的静态常量

/// 执行时间字段前缀
pub static EXECTIME_PREFIX: &str = "EXECTIME: ";

/// 执行时间字段后缀
pub static EXECTIME_SUFFIX: &str = "(ms)";

/// 行数字段前缀
pub static ROWCOUNT_PREFIX: &str = "ROWCOUNT: ";

/// 行数字段后缀
pub static ROWCOUNT_SUFFIX: &str = "(rows)";

/// 执行 ID 字段前缀
pub static EXEC_ID_PREFIX: &str = "EXEC_ID: ";

/// 执行 ID 字段后缀
pub static EXEC_ID_SUFFIX: &str = ".";
