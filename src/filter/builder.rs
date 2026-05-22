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
///     .ts_contains("2024-06")
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

    // ── FILTER-01: ts ──

    /// 时间戳包含指定子串（FILTER-01）。
    pub fn ts_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.ts.contains(&pattern))
    }

    /// 时间戳等于指定值（FILTER-01）。
    pub fn ts_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.ts == value)
    }

    /// 时间戳以指定前缀开头（FILTER-01）。
    pub fn ts_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.ts.starts_with(&prefix))
    }

    /// 时间戳以指定后缀结尾（FILTER-01）。
    pub fn ts_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.ts.ends_with(&suffix))
    }

    // ── FILTER-02: tag ──

    /// tag 字段不为 None（FILTER-02）。
    pub fn tag_is_some(self) -> Self {
        self.add(|r| r.tag.is_some())
    }

    /// tag 字段为 None（FILTER-02）。
    pub fn tag_is_none(self) -> Self {
        self.add(|r| r.tag.is_none())
    }

    /// tag 字段等于指定值（FILTER-02）。
    pub fn tag_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.tag.as_deref() == Some(&value))
    }

    /// tag 字段包含指定子串（FILTER-02）。
    pub fn tag_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.tag.as_deref().is_some_and(|t| t.contains(&pattern)))
    }

    // ── FILTER-03: ep（u8 数值）──

    /// EP 等于指定值（FILTER-03）。
    pub fn ep_eq(self, value: u8) -> Self {
        self.add(move |r| r.ep == value)
    }

    /// EP 大于指定值（FILTER-03）。
    pub fn ep_gt(self, value: u8) -> Self {
        self.add(move |r| r.ep > value)
    }

    /// EP 小于指定值（FILTER-03）。
    pub fn ep_lt(self, value: u8) -> Self {
        self.add(move |r| r.ep < value)
    }

    /// EP 在闭区间 [min, max] 内（FILTER-03）。
    ///
    /// # Panics
    /// 当 `min > max` 时 panic。
    pub fn ep_between(self, min: u8, max: u8) -> Self {
        assert!(min <= max, "ep_between: min ({min}) must be <= max ({max})");
        self.add(move |r| r.ep >= min && r.ep <= max)
    }

    // ── FILTER-04: sess_id ──

    /// sess_id 包含指定子串（FILTER-04）。
    pub fn sess_id_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.sess_id.contains(&pattern))
    }

    /// sess_id 等于指定值（FILTER-04）。
    pub fn sess_id_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.sess_id == value)
    }

    /// sess_id 以指定前缀开头（FILTER-04）。
    pub fn sess_id_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.sess_id.starts_with(&prefix))
    }

    /// sess_id 以指定后缀结尾（FILTER-04）。
    pub fn sess_id_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.sess_id.ends_with(&suffix))
    }

    // ── FILTER-04: thrd_id ──

    /// thrd_id 包含指定子串（FILTER-04）。
    pub fn thrd_id_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.thrd_id.contains(&pattern))
    }

    /// thrd_id 等于指定值（FILTER-04）。
    pub fn thrd_id_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.thrd_id == value)
    }

    /// thrd_id 以指定前缀开头（FILTER-04）。
    pub fn thrd_id_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.thrd_id.starts_with(&prefix))
    }

    /// thrd_id 以指定后缀结尾（FILTER-04）。
    pub fn thrd_id_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.thrd_id.ends_with(&suffix))
    }

    // ── FILTER-04: username ──

    /// username 包含指定子串（FILTER-04）。
    pub fn username_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.username.contains(&pattern))
    }

    /// username 等于指定值（FILTER-04）。
    pub fn username_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.username == value)
    }

    /// username 以指定前缀开头（FILTER-04）。
    pub fn username_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.username.starts_with(&prefix))
    }

    /// username 以指定后缀结尾（FILTER-04）。
    pub fn username_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.username.ends_with(&suffix))
    }

    // ── FILTER-04: trxid ──

    /// trxid 包含指定子串（FILTER-04）。
    pub fn trxid_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.trxid.contains(&pattern))
    }

    /// trxid 等于指定值（FILTER-04）。
    pub fn trxid_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.trxid == value)
    }

    /// trxid 以指定前缀开头（FILTER-04）。
    pub fn trxid_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.trxid.starts_with(&prefix))
    }

    /// trxid 以指定后缀结尾（FILTER-04）。
    pub fn trxid_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.trxid.ends_with(&suffix))
    }

    // ── FILTER-04: statement ──

    /// statement 包含指定子串（FILTER-04）。
    pub fn statement_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.statement.contains(&pattern))
    }

    /// statement 等于指定值（FILTER-04）。
    pub fn statement_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.statement == value)
    }

    /// statement 以指定前缀开头（FILTER-04）。
    pub fn statement_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.statement.starts_with(&prefix))
    }

    /// statement 以指定后缀结尾（FILTER-04）。
    pub fn statement_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.statement.ends_with(&suffix))
    }

    // ── FILTER-04: appname ──

    /// appname 包含指定子串（FILTER-04）。
    pub fn appname_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.appname.contains(&pattern))
    }

    /// appname 等于指定值（FILTER-04）。
    pub fn appname_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.appname == value)
    }

    /// appname 以指定前缀开头（FILTER-04）。
    pub fn appname_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.appname.starts_with(&prefix))
    }

    /// appname 以指定后缀结尾（FILTER-04）。
    pub fn appname_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.appname.ends_with(&suffix))
    }

    // ── FILTER-04: client_ip ──

    /// client_ip 包含指定子串（FILTER-04）。
    pub fn client_ip_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.client_ip.contains(&pattern))
    }

    /// client_ip 等于指定值（FILTER-04）。
    pub fn client_ip_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.client_ip == value)
    }

    /// client_ip 以指定前缀开头（FILTER-04）。
    pub fn client_ip_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.client_ip.starts_with(&prefix))
    }

    /// client_ip 以指定后缀结尾（FILTER-04）。
    pub fn client_ip_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.client_ip.ends_with(&suffix))
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

    /// 影响行数在闭区间 [min, max] 内（FILTER-07）。
    ///
    /// # Panics
    /// 当 `min > max` 时 panic。
    pub fn rowcount_between(self, min: u32, max: u32) -> Self {
        assert!(min <= max, "rowcount_between: min ({min}) must be <= max ({max})");
        self.add(move |r| r.rowcount >= min && r.rowcount <= max)
    }

    // ── FILTER-08: exec_id（i64）──

    /// 执行 ID 等于指定值（FILTER-08）。
    pub fn exec_id_eq(self, value: i64) -> Self {
        self.add(move |r| r.exec_id == value)
    }

    /// 执行 ID 大于指定值（FILTER-08）。
    pub fn exec_id_gt(self, value: i64) -> Self {
        self.add(move |r| r.exec_id > value)
    }

    /// 执行 ID 小于指定值（FILTER-08）。
    pub fn exec_id_lt(self, value: i64) -> Self {
        self.add(move |r| r.exec_id < value)
    }

    /// 执行 ID 在闭区间 [min, max] 内（FILTER-08）。
    ///
    /// # Panics
    /// 当 `min > max` 时 panic。
    pub fn exec_id_between(self, min: i64, max: i64) -> Self {
        assert!(min <= max, "exec_id_between: min ({min}) must be <= max ({max})");
        self.add(move |r| r.exec_id >= min && r.exec_id <= max)
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
        }
    }

    // ── FILTER-01: ts ──

    #[test]
    fn test_ts_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_contains("2024-06").build().matches(&record));
        assert!(!FilterBuilder::new().ts_contains("2025-01").build().matches(&record));
    }

    #[test]
    fn test_ts_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_eq("2024-06-01 10:00:00.000").build().matches(&record));
        assert!(!FilterBuilder::new().ts_eq("2024-06-01 10:00:00.001").build().matches(&record));
    }

    #[test]
    fn test_ts_starts_with() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_starts_with("2024").build().matches(&record));
        assert!(!FilterBuilder::new().ts_starts_with("2025").build().matches(&record));
    }

    #[test]
    fn test_ts_ends_with() {
        let record = make_record();
        assert!(FilterBuilder::new().ts_ends_with(".000").build().matches(&record));
        assert!(!FilterBuilder::new().ts_ends_with(".999").build().matches(&record));
    }

    // ── FILTER-02: tag ──

    #[test]
    fn test_tag_is_some() {
        let record = make_record();
        assert!(FilterBuilder::new().tag_is_some().build().matches(&record));
        let mut no_tag = make_record();
        no_tag.tag = None;
        assert!(!FilterBuilder::new().tag_is_some().build().matches(&no_tag));
    }

    #[test]
    fn test_tag_is_none() {
        let record = make_record();
        assert!(!FilterBuilder::new().tag_is_none().build().matches(&record));
        let mut no_tag = make_record();
        no_tag.tag = None;
        assert!(FilterBuilder::new().tag_is_none().build().matches(&no_tag));
    }

    #[test]
    fn test_tag_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().tag_eq("SEL").build().matches(&record));
        assert!(!FilterBuilder::new().tag_eq("ORA").build().matches(&record));
    }

    #[test]
    fn test_tag_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().tag_contains("SE").build().matches(&record));
        assert!(!FilterBuilder::new().tag_contains("ORA").build().matches(&record));
    }

    // ── FILTER-03: ep ──

    #[test]
    fn test_ep_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().ep_eq(2).build().matches(&record));
        assert!(!FilterBuilder::new().ep_eq(3).build().matches(&record));
    }

    #[test]
    fn test_ep_gt() {
        let record = make_record();
        assert!(FilterBuilder::new().ep_gt(1).build().matches(&record));
        assert!(!FilterBuilder::new().ep_gt(2).build().matches(&record));
    }

    #[test]
    fn test_ep_lt() {
        let record = make_record();
        assert!(FilterBuilder::new().ep_lt(3).build().matches(&record));
        assert!(!FilterBuilder::new().ep_lt(2).build().matches(&record));
    }

    #[test]
    fn test_ep_between() {
        let record = make_record();
        assert!(FilterBuilder::new().ep_between(1, 3).build().matches(&record));
        assert!(!FilterBuilder::new().ep_between(3, 5).build().matches(&record));
    }

    // ── FILTER-04: 七个字符串元数据字段 ──

    #[test]
    fn test_sess_id_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().sess_id_contains("ABC").build().matches(&record));
        assert!(!FilterBuilder::new().sess_id_contains("XYZ").build().matches(&record));
    }

    #[test]
    fn test_thrd_id_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().thrd_id_contains("12").build().matches(&record));
        assert!(!FilterBuilder::new().thrd_id_contains("99").build().matches(&record));
    }

    #[test]
    fn test_username_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().username_contains("ali").build().matches(&record));
        assert!(!FilterBuilder::new().username_contains("bob").build().matches(&record));
    }

    #[test]
    fn test_trxid_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().trxid_contains("0").build().matches(&record));
        assert!(!FilterBuilder::new().trxid_contains("99").build().matches(&record));
    }

    #[test]
    fn test_statement_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().statement_contains("0x1").build().matches(&record));
        assert!(!FilterBuilder::new().statement_contains("0x2").build().matches(&record));
    }

    #[test]
    fn test_appname_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().appname_contains("app").build().matches(&record));
        assert!(!FilterBuilder::new().appname_contains("xyz").build().matches(&record));
    }

    #[test]
    fn test_client_ip_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().client_ip_contains("10.0").build().matches(&record));
        assert!(!FilterBuilder::new().client_ip_contains("192.168").build().matches(&record));
    }

    // ── FILTER-05: sql ──

    #[test]
    fn test_sql_contains() {
        let record = make_record();
        assert!(FilterBuilder::new().sql_contains("SELECT").build().matches(&record));
        assert!(!FilterBuilder::new().sql_contains("INSERT").build().matches(&record));
    }

    #[test]
    fn test_sql_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().sql_eq("SELECT * FROM users").build().matches(&record));
        assert!(!FilterBuilder::new().sql_eq("SELECT 1").build().matches(&record));
    }

    #[test]
    fn test_sql_starts_with() {
        let record = make_record();
        assert!(FilterBuilder::new().sql_starts_with("SELECT").build().matches(&record));
        assert!(!FilterBuilder::new().sql_starts_with("UPDATE").build().matches(&record));
    }

    #[test]
    fn test_sql_ends_with() {
        let record = make_record();
        assert!(FilterBuilder::new().sql_ends_with("users").build().matches(&record));
        assert!(!FilterBuilder::new().sql_ends_with("orders").build().matches(&record));
    }

    // ── FILTER-06: exectime ──

    #[test]
    fn test_exec_time_gt() {
        let record = make_record();
        assert!(FilterBuilder::new().exec_time_gt(100.0).build().matches(&record));
        assert!(!FilterBuilder::new().exec_time_gt(200.0).build().matches(&record));
    }

    #[test]
    fn test_exec_time_lt() {
        let record = make_record();
        assert!(FilterBuilder::new().exec_time_lt(200.0).build().matches(&record));
        assert!(!FilterBuilder::new().exec_time_lt(100.0).build().matches(&record));
    }

    #[test]
    fn test_exec_time_between() {
        let record = make_record();
        assert!(FilterBuilder::new().exec_time_between(100.0, 200.0).build().matches(&record));
        assert!(!FilterBuilder::new().exec_time_between(200.0, 300.0).build().matches(&record));
    }

    // ── FILTER-07: rowcount ──

    #[test]
    fn test_rowcount_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().rowcount_eq(10).build().matches(&record));
        assert!(!FilterBuilder::new().rowcount_eq(99).build().matches(&record));
    }

    #[test]
    fn test_rowcount_gt() {
        let record = make_record();
        assert!(FilterBuilder::new().rowcount_gt(5).build().matches(&record));
        assert!(!FilterBuilder::new().rowcount_gt(10).build().matches(&record));
    }

    #[test]
    fn test_rowcount_lt() {
        let record = make_record();
        assert!(FilterBuilder::new().rowcount_lt(20).build().matches(&record));
        assert!(!FilterBuilder::new().rowcount_lt(10).build().matches(&record));
    }

    #[test]
    fn test_rowcount_between() {
        let record = make_record();
        assert!(FilterBuilder::new().rowcount_between(5, 15).build().matches(&record));
        assert!(!FilterBuilder::new().rowcount_between(20, 50).build().matches(&record));
    }

    // ── FILTER-08: exec_id ──

    #[test]
    fn test_exec_id_eq() {
        let record = make_record();
        assert!(FilterBuilder::new().exec_id_eq(999).build().matches(&record));
        assert!(!FilterBuilder::new().exec_id_eq(0).build().matches(&record));
    }

    #[test]
    fn test_exec_id_gt() {
        let record = make_record();
        assert!(FilterBuilder::new().exec_id_gt(500).build().matches(&record));
        assert!(!FilterBuilder::new().exec_id_gt(999).build().matches(&record));
    }

    #[test]
    fn test_exec_id_lt() {
        let record = make_record();
        assert!(FilterBuilder::new().exec_id_lt(1000).build().matches(&record));
        assert!(!FilterBuilder::new().exec_id_lt(999).build().matches(&record));
    }

    #[test]
    fn test_exec_id_between() {
        let record = make_record();
        assert!(FilterBuilder::new().exec_id_between(500, 1000).build().matches(&record));
        assert!(!FilterBuilder::new().exec_id_between(1000, 2000).build().matches(&record));
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
            .ts_contains("2024")
            .exec_time_gt(100.0)
            .sql_contains("SELECT")
            .build();
        assert!(filter.matches(&record));

        // 其中一个不满足（exectime 150.0 < 200.0），整体 false
        let strict = FilterBuilder::new()
            .ts_contains("2024")
            .exec_time_gt(200.0)
            .build();
        assert!(!strict.matches(&record));
    }

    #[test]
    fn test_and_semantics_short_circuit() {
        let record = make_record();
        // 第一条件不满足（ts 不含 "2025"），短路返回 false
        let filter = FilterBuilder::new()
            .ts_contains("2025")
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
