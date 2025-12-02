use smartstring::alias::String as SmartString;

/// SQL 日志记录
///
/// 表示一条完整的 SQL 日志记录，包含时间戳、元数据、SQL 语句体和可选的性能指标。
///

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sqllog {
    /// 时间戳，格式为 "YYYY-MM-DD HH:MM:SS.mmm"
    pub ts: SmartString,

    /// 元数据部分，包含会话信息、用户信息等
    pub meta: MetaParts,

    /// SQL 语句体
    pub body: String, // Body might be large, keep as String or use SmartString? SmartString falls back to heap.

    /// 可选的性能指标信息
    pub indicators: Option<IndicatorsParts>,
}

/// 元数据部分
///
/// 包含日志记录的所有元数据字段，如会话 ID、用户名等。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetaParts {
    /// EP（Execution Point）编号，范围 0-255
    pub ep: u8,

    /// 会话 ID
    pub sess_id: SmartString,

    /// 线程 ID
    pub thrd_id: SmartString,

    /// 用户名
    pub username: SmartString,

    /// 事务 ID
    pub trxid: SmartString,

    /// 语句 ID
    pub statement: SmartString,

    /// 应用程序名称
    pub appname: SmartString,

    /// 客户端 IP 地址（可选）
    pub client_ip: SmartString,
}

/// 性能指标部分
///
/// 包含 SQL 执行的性能指标，如执行时间、影响行数等。
///

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IndicatorsParts {
    /// 执行时间（毫秒）
    pub execute_time: f32,

    /// 影响的行数
    pub row_count: u32,

    /// 执行 ID
    pub execute_id: i64,
}
