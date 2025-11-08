//! SQL 日志数据结构定义
//!
//! 定义了解析后的 SQL 日志记录的数据结构。

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// SQL 日志记录
///
/// 表示一条完整的 SQL 日志记录，包含时间戳、元数据、SQL 语句体和可选的性能指标。
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::parse_sqllogs_from_string;
///
/// let log = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"#;
/// let results = parse_sqllogs_from_string(log);
///
/// if let Ok(sqllog) = &results[0] {
///     assert_eq!(sqllog.ts, "2025-08-12 10:57:09.548");
///     assert_eq!(sqllog.meta.username, "alice");
///     assert_eq!(sqllog.body, "SELECT 1");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Sqllog {
    /// 时间戳，格式为 "YYYY-MM-DD HH:MM:SS.mmm"
    pub ts: String,

    /// 元数据部分，包含会话信息、用户信息等
    pub meta: MetaParts,

    /// SQL 语句体
    pub body: String,

    /// 可选的性能指标信息
    pub indicators: Option<IndicatorsParts>,
}

/// 元数据部分
///
/// 包含日志记录的所有元数据字段，如会话 ID、用户名等。
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MetaParts {
    /// EP（Execution Point）编号，范围 0-255
    pub ep: u8,

    /// 会话 ID
    pub sess_id: String,

    /// 线程 ID
    pub thrd_id: String,

    /// 用户名
    pub username: String,

    /// 事务 ID
    pub trxid: String,

    /// 语句 ID
    pub statement: String,

    /// 应用程序名称
    pub appname: String,

    /// 客户端 IP 地址（可选）
    pub client_ip: String,
}

/// 性能指标部分
///
/// 包含 SQL 执行的性能指标，如执行时间、影响行数等。
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::parse_sqllogs_from_string;
///
/// let log = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345."#;
/// let results = parse_sqllogs_from_string(log);
///
/// if let Ok(sqllog) = &results[0] {
///     if let Some(indicators) = &sqllog.indicators {
///         assert_eq!(indicators.execute_time, 10.5);
///         assert_eq!(indicators.row_count, 100);
///         assert_eq!(indicators.execute_id, 12345);
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct IndicatorsParts {
    /// 执行时间（毫秒）
    pub execute_time: f32,

    /// 影响的行数
    pub row_count: u32,

    /// 执行 ID
    pub execute_id: i64,
}

impl Sqllog {
    /// 判断是否有性能指标信息
    ///
    /// # 返回
    ///
    /// 如果存在性能指标返回 `true`，否则返回 `false`
    #[inline]
    pub fn has_indicators(&self) -> bool {
        self.indicators.is_some()
    }

    /// 获取执行时间（毫秒）
    ///
    /// # 返回
    ///
    /// 如果存在性能指标返回执行时间，否则返回 `None`
    #[inline]
    pub fn execute_time(&self) -> Option<f32> {
        self.indicators.map(|i| i.execute_time)
    }

    /// 获取影响行数
    ///
    /// # 返回
    ///
    /// 如果存在性能指标返回影响行数，否则返回 `None`
    #[inline]
    pub fn row_count(&self) -> Option<u32> {
        self.indicators.map(|i| i.row_count)
    }

    /// 获取执行 ID
    ///
    /// # 返回
    ///
    /// 如果存在性能指标返回执行 ID，否则返回 `None`
    #[inline]
    pub fn execute_id(&self) -> Option<i64> {
        self.indicators.map(|i| i.execute_id)
    }
}

impl MetaParts {
    /// 判断是否有客户端 IP 信息
    ///
    /// # 返回
    ///
    /// 如果存在客户端 IP 返回 `true`，否则返回 `false`
    #[inline]
    pub fn has_client_ip(&self) -> bool {
        !self.client_ip.is_empty()
    }
}
