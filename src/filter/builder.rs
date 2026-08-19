//! 链式过滤器构建器。
//!
//! 通过 [`FilterBuilder`] 链式添加谓词，调用 [`FilterBuilder::build`] 生成 [`Filter`]。
//! 所有条件以 AND 语义组合，单次迭代无中间分配，与 `LogParser::iter()` 无缝集成。

use crate::record::Sqllog;

/// 装箱的谓词函数类型，满足 Send + Sync 以支持跨线程传递（Phase 12 async 准备）。
type Predicate = Box<dyn Fn(&Sqllog) -> bool + Send + Sync>;

/// 组合过滤器，持有所有谓词的 AND 组合。
///
/// 通过 [`FilterBuilder`] 构建，不能直接实例化。
/// 对记录调用 [`matches`](Filter::matches) 时，所有谓词均须通过（AND 短路求值）。
pub struct Filter {
    predicates: Vec<Predicate>,
}

impl Filter {
    /// 对给定记录运行所有谓词，全部通过返回 `true`（AND 语义，短路求值）。
    #[inline]
    pub fn matches(&self, record: &Sqllog) -> bool {
        self.predicates.iter().all(|pred| pred(record))
    }
}

/// 链式构建组合过滤器。
///
/// 所有条件以 AND 语义组合，调用 [`build`](FilterBuilder::build) 生成 [`Filter`]。
///
/// # 示例
///
/// ```rust,no_run
/// use dm_database_parser_sqllog::FilterBuilder;
///
/// let filter = FilterBuilder::new()
///     .exec_time_gt(100.0)
///     .sql_contains("SELECT")
///     .build();
/// ```
pub struct FilterBuilder {
    predicates: Vec<Predicate>,
}

impl FilterBuilder {
    /// 创建一个空的 `FilterBuilder`（无任何谓词）。
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
        }
    }

    /// 消费 builder，产出可复用的 [`Filter`]。
    pub fn build(self) -> Filter {
        Filter {
            predicates: self.predicates,
        }
    }

    /// 私有辅助：装箱并添加谓词，返回 self 以支持链式调用。
    fn add<F>(mut self, pred: F) -> Self
    where
        F: Fn(&Sqllog) -> bool + Send + Sync + 'static,
    {
        self.predicates.push(Box::new(pred));
        self
    }

    // ── FILTER-01: ts 范围（字符串字典序等价于时间顺序）──

    /// 时间戳晚于指定值（严格大于，FILTER-01）。
    pub fn ts_gt(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.ts.as_str() > value.as_str())
    }

    /// 时间戳晚于或等于指定值（FILTER-01）。
    pub fn ts_gte(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.ts.as_str() >= value.as_str())
    }

    /// 时间戳早于指定值（严格小于，FILTER-01）。
    pub fn ts_lt(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.ts.as_str() < value.as_str())
    }

    /// 时间戳早于或等于指定值（FILTER-01）。
    pub fn ts_lte(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.ts.as_str() <= value.as_str())
    }

    /// 时间戳在闭区间 [start, end] 内（FILTER-01）。
    ///
    /// # Panics
    /// 当 `start > end` 时 panic。
    pub fn ts_between(self, start: impl Into<String>, end: impl Into<String>) -> Self {
        let start = start.into();
        let end = end.into();
        assert!(start <= end, "ts_between: start ({start}) must be <= end ({end})");
        self.add(move |r| r.ts.as_str() >= start.as_str() && r.ts.as_str() <= end.as_str())
    }

    // ── FILTER-02: tag ──

    /// tag 字段等于指定值（FILTER-02）。
    pub fn tag_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.tag.as_deref() == Some(&value))
    }

    // ── FILTER-03: ep（u8 数值）──

    /// EP 等于指定值（FILTER-03）。
    pub fn ep_eq(self, value: u8) -> Self {
        self.add(move |r| r.ep == value)
    }

    // ── FILTER-04: sess_id ──

    /// sess_id 等于指定值（FILTER-04）。
    pub fn sess_id_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.sess_id == value)
    }

    // ── FILTER-04: thrd_id ──

    /// thrd_id 等于指定值（FILTER-04）。
    pub fn thrd_id_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.thrd_id == value)
    }

    // ── FILTER-04: username ──

    /// username 等于指定值（FILTER-04）。
    pub fn username_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.username == value)
    }

    // ── FILTER-04: trxid ──

    /// trxid 等于指定值（FILTER-04）。
    pub fn trxid_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.trxid == value)
    }

    // ── FILTER-04: statement ──

    /// statement 等于指定值（FILTER-04）。
    pub fn statement_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.statement == value)
    }

    // ── FILTER-04: appname ──

    /// appname 等于指定值（FILTER-04）。
    pub fn appname_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.appname == value)
    }

    // ── FILTER-04: client_ip ──

    /// client_ip 等于指定值（FILTER-04）。
    pub fn client_ip_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.client_ip == value)
    }

    // ── FILTER-05: sql ──

    /// SQL 语句包含指定子串（FILTER-05）。
    pub fn sql_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.sql.contains(&pattern))
    }

    /// SQL 语句等于指定值（FILTER-05）。
    pub fn sql_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.sql == value)
    }

    /// SQL 语句以指定前缀开头（FILTER-05）。
    pub fn sql_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.sql.starts_with(&prefix))
    }

    /// SQL 语句以指定后缀结尾（FILTER-05）。
    pub fn sql_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.sql.ends_with(&suffix))
    }

    // ── FILTER-06: exectime（f32，不提供 eq）──

    /// 执行时间大于 min_ms 毫秒（严格不等，FILTER-06）。
    ///
    /// 参数单位为毫秒；f32 精度约 7 位有效数字，不提供 `exec_time_eq` 方法以避免误用。
    pub fn exec_time_gt(self, min_ms: f32) -> Self {
        self.add(move |r| r.exectime > min_ms)
    }

    /// 执行时间大于等于 min_ms 毫秒（含边界，FILTER-06）。
    ///
    /// 与 [`LogIterator::filter_by_exec_time`] 语义一致（`>=`）。
    /// 参数单位为毫秒；f32 精度约 7 位有效数字。
    pub fn exec_time_gte(self, min_ms: f32) -> Self {
        self.add(move |r| r.exectime >= min_ms)
    }

    /// 执行时间小于 max_ms 毫秒（FILTER-06）。
    ///
    /// 参数单位为毫秒；f32 精度约 7 位有效数字，不提供 `exec_time_eq` 方法以避免误用。
    pub fn exec_time_lt(self, max_ms: f32) -> Self {
        self.add(move |r| r.exectime < max_ms)
    }

    /// 执行时间在闭区间 [min_ms, max_ms] 毫秒内（FILTER-06）。
    ///
    /// 参数单位为毫秒；f32 精度约 7 位有效数字，不提供 `exec_time_eq` 方法以避免误用。
    ///
    /// # Panics
    /// 当 `min_ms > max_ms` 时 panic。
    pub fn exec_time_between(self, min_ms: f32, max_ms: f32) -> Self {
        assert!(
            min_ms <= max_ms,
            "exec_time_between: min_ms ({min_ms}) must be <= max_ms ({max_ms})"
        );
        self.add(move |r| r.exectime >= min_ms && r.exectime <= max_ms)
    }

    // ── FILTER-07: rowcount（u32）──

    /// 影响行数等于指定值（FILTER-07）。
    pub fn rowcount_eq(self, value: u32) -> Self {
        self.add(move |r| r.rowcount == value)
    }

    /// 影响行数大于指定值（FILTER-07）。
    pub fn rowcount_gt(self, value: u32) -> Self {
        self.add(move |r| r.rowcount > value)
    }

    /// 影响行数小于指定值（FILTER-07）。
    pub fn rowcount_lt(self, value: u32) -> Self {
        self.add(move |r| r.rowcount < value)
    }

    // ── FILTER-08: exec_id（i64）──

    /// 执行 ID 等于指定值（FILTER-08）。
    pub fn exec_id_eq(self, value: i64) -> Self {
        self.add(move |r| r.exec_id == value)
    }
}

impl Default for FilterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Sqllog;

    fn make_record() -> Sqllog {
        Sqllog {
            ts: "2024-06-01 10:00:00.000".to_string(),
            tag: Some("SEL".to_string()),
            ep: 2,
            sess_id: "0xABC".to_string(),
            thrd_id: "123".to_string(),
            username: "alice".to_string(),
            trxid: "0".to_string(),
            statement: "0x1".to_string(),
            appname: "myapp".to_string(),
            client_ip: "10.0.0.1".to_string(),
            sql: "SELECT * FROM users".to_string(),
            exectime: 150.0,
            rowcount: 10,
            exec_id: 999,
            lock_event: None,
        }
    }

    // ── FILTER-01: ts 范围 ──

    #[test]
    fn test_ts_gt() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_gt("2024-05-31").build().matches(&record));
        assert!(!FilterBuilder::new().ts_gt("2024-06-01 10:00:00.000").build().matches(&record));
    }

    #[test]
    fn test_ts_gte() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_gte("2024-06-01 10:00:00.000").build().matches(&record));
        assert!(!FilterBuilder::new().ts_gte("2024-06-01 10:00:00.001").build().matches(&record));
    }

    #[test]
    fn test_ts_lt() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_lt("2024-06-02").build().matches(&record));
        assert!(!FilterBuilder::new().ts_lt("2024-06-01 10:00:00.000").build().matches(&record));
    }

    #[test]
    fn test_ts_lte() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_lte("2024-06-01 10:00:00.000").build().matches(&record));
        assert!(!FilterBuilder::new().ts_lte("2024-06-01 09:59:59.999").build().matches(&record));
    }

    #[test]
    fn test_ts_between() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .ts_between("2024-06-01 10:00:00.000", "2024-06-01 11:00:00.000")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .ts_between("2024-06-01 10:00:00.001", "2024-06-01 11:00:00.000")
                .build()
                .matches(&record)
        );
    }

    #[test]
    #[should_panic(expected = "ts_between: start")]
    fn test_ts_between_panics_on_invalid_range() {
        FilterBuilder::new().ts_between("2024-06-02", "2024-06-01").build();
    }

    // ── FILTER-02: tag ──

    #[test]
    fn test_tag_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().tag_eq("SEL").build().matches(&record));
        assert!(!FilterBuilder::new().tag_eq("ORA").build().matches(&record));
    }

    // ── FILTER-03: ep ──

    #[test]
    fn test_ep_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().ep_eq(2).build().matches(&record));
        assert!(!FilterBuilder::new().ep_eq(3).build().matches(&record));
    }

    // ── FILTER-04: 七个字符串元数据字段 ──

    #[test]
    fn test_sess_id_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .sess_id_eq("0xABC")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .sess_id_eq("0xXYZ")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_thrd_id_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .thrd_id_eq("123")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .thrd_id_eq("999")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_username_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .username_eq("alice")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .username_eq("bob")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_trxid_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().trxid_eq("0").build().matches(&record));
        assert!(
            !FilterBuilder::new()
                .trxid_eq("99")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_statement_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .statement_eq("0x1")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .statement_eq("0x2")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_appname_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .appname_eq("myapp")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .appname_eq("other")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_client_ip_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .client_ip_eq("10.0.0.1")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .client_ip_eq("192.168.0.1")
                .build()
                .matches(&record)
        );
    }

    // ── FILTER-05: sql ──

    #[test]
    fn test_sql_contains() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .sql_contains("SELECT")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .sql_contains("INSERT")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_sql_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .sql_eq("SELECT * FROM users")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .sql_eq("SELECT 1")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_sql_starts_with() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .sql_starts_with("SELECT")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .sql_starts_with("UPDATE")
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_sql_ends_with() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .sql_ends_with("users")
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .sql_ends_with("orders")
                .build()
                .matches(&record)
        );
    }

    // ── FILTER-06: exectime ──

    #[test]
    fn test_exec_time_gt() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .exec_time_gt(100.0)
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .exec_time_gt(200.0)
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_exec_time_lt() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .exec_time_lt(200.0)
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .exec_time_lt(100.0)
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_exec_time_between() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .exec_time_between(100.0, 200.0)
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .exec_time_between(200.0, 300.0)
                .build()
                .matches(&record)
        );
    }

    // ── FILTER-07: rowcount ──

    #[test]
    fn test_rowcount_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .rowcount_eq(10)
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .rowcount_eq(99)
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_rowcount_gt() {
        let record = make_record();
        assert!(FilterBuilder::new().rowcount_gt(5).build().matches(&record));
        assert!(
            !FilterBuilder::new()
                .rowcount_gt(10)
                .build()
                .matches(&record)
        );
    }

    #[test]
    fn test_rowcount_lt() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .rowcount_lt(20)
                .build()
                .matches(&record)
        );
        assert!(
            !FilterBuilder::new()
                .rowcount_lt(10)
                .build()
                .matches(&record)
        );
    }

    // ── FILTER-08: exec_id ──

    #[test]
    fn test_exec_id_eq() {
        let record = make_record();
        assert!(
            FilterBuilder::new()
                .exec_id_eq(999)
                .build()
                .matches(&record)
        );
        assert!(!FilterBuilder::new().exec_id_eq(0).build().matches(&record));
    }

    // ── FILTER-09: AND 语义 ──

    #[test]
    fn test_empty_filter_matches_all() {
        let record = make_record();
        assert!(FilterBuilder::new().build().matches(&record));
    }

    #[test]
    fn test_multiple_predicates_all_must_pass() {
        let record = make_record();
        let filter = FilterBuilder::new()
            .exec_time_gt(100.0)
            .sql_contains("SELECT")
            .username_eq("alice")
            .build();
        assert!(filter.matches(&record));

        // 其中一个不满足（exectime 150.0 < 200.0），整体 false
        let strict = FilterBuilder::new()
            .username_eq("alice")
            .exec_time_gt(200.0)
            .build();
        assert!(!strict.matches(&record));
    }

    #[test]
    fn test_and_semantics_short_circuit() {
        let record = make_record();
        // 第一条件不满足（username 不匹配），短路返回 false
        let filter = FilterBuilder::new()
            .username_eq("bob")
            .exec_time_gt(100.0)
            .sql_contains("SELECT")
            .build();
        assert!(!filter.matches(&record));
    }

    #[test]
    fn test_default_is_same_as_new() {
        let record = make_record();
        assert!(FilterBuilder::default().build().matches(&record));
    }
}
