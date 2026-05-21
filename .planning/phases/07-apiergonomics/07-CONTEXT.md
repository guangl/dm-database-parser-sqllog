# Phase 7: APIErgonomics - Context

**Gathered:** 2026-05-19
**Status:** Ready for planning

<domain>
## Phase Boundary

新增 `LogParserBuilder`（替代 `from_path`）、在 `LogIterator` 上添加过滤方法、在 `Sqllog` 上添加直接字段访问方法、定义 `FromSqllog` trait，使用户可以用流畅的链式 API 配置解析器并访问数据。

**依赖：** Phase 6（ErrorHandling）— `exec_time()` 等方法的错误返回需使用更新后的 `ParseError`。

**不在本 Phase 范围内：** 流式/增量解析 API（API-06 Future）、通用过滤适配器 API-05。

</domain>

<decisions>
## Implementation Decisions

### API-01: LogParserBuilder

- **D-01:** `LogParserBuilder` **完全替代** `LogParser::from_path`。移除 `from_path` 关联函数，统一入口为 `LogParserBuilder::new(path).threads(4).parallel_threshold(32 * 1024 * 1024).build()`。
- **D-02:** `LogParserBuilder::new(path)` 接受 `impl AsRef<Path>` 参数（与现有 `from_path` 一致）。
- **D-03:** Builder 支持的配置项：`threads(usize)`、`parallel_threshold(usize)`、编码提示（可选，命名由 planner 决定）。`build()` 返回 `Result<LogParser, ParseError>`。

### API-02: 过滤方法

- **D-04:** 在 `LogIterator` 的 `impl` 块中直接添加过滤方法，返回 `impl Iterator<Item = Result<Sqllog<'a>, ParseError>> + 'a`。
- **D-05:** 至少实现两个过滤方法：`filter_by_exec_time(min_ms: u64)` 和 `filter_by_sql_contains(pattern: &str)`。
- **D-06:** 不引入新类型（如 `FilteredIter`）和新 trait（如 `SqllogFilter`）——直接在 `LogIterator` impl 上添加方法最简单。

### API-03: 直接字段访问

- **D-07:** `Sqllog` 上新增 `exec_time()` 和 `row_count()` 方法，返回类型为 `Result<Option<u64>, ParseError>`。
  - `Ok(Some(v))` — 字段存在且解析成功
  - `Ok(None)` — 字段不存在（该记录无 indicators）
  - `Err(e)` — 字段存在但解析失败（含行号上下文来自 Phase 6）
- **D-08:** 这些方法内部复用 `parse_performance_metrics()` 逻辑（或 `parse_indicators()`），不重复实现解析。

### API-04: FromSqllog trait

- **D-09:** trait 定义为消费所有权版本：
  ```rust
  pub trait FromSqllog {
      fn from_sqllog(s: Sqllog<'_>) -> Self;
  }
  ```
  与标准库 `From` trait 风格一致。
- **D-10:** 用户可通过 `.map(MyType::from_sqllog)` 或 `.map(|s| MyType::from_sqllog(s))` 在迭代器链中使用。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 需求规格
- `.planning/REQUIREMENTS.md` — API-01、API-02、API-03、API-04 完整需求定义
- `.planning/ROADMAP.md` — Phase 7 Success Criteria（4 条验收标准）

### 现有代码（必读）
- `src/parser.rs` — `LogParser::from_path()`（将被 Builder 替代）、`LogIterator` 结构体定义、`par_iter()` 和 `iter()` 实现
- `src/sqllog.rs` — `Sqllog` 结构体、`parse_performance_metrics()`、`parse_indicators()` — 字段访问方法复用这些实现
- `src/lib.rs` — 当前公开 API 导出（`parse_record`、`LogParser`、`LogIterator` 等），Builder 引入后需更新
- `.planning/phases/06-errorhandling/06-CONTEXT.md` — ParseError 变体修改（line_number 字段），影响 exec_time() 错误返回

### 架构参考
- `.planning/ROADMAP.md` §Phase 7 — PAR_THRESHOLD = 32 MB 常量，Builder 的 `parallel_threshold` 默认值参照此值

</canonical_refs>

<code_context>
## Existing Code Insights

### 可复用资产
- `LogParser` 中的 `mmap` + `encoding` 字段 — Builder 最终构造的仍是同一个 `LogParser`，内部结构不变
- `FileEncodingHint` enum — 编码提示参数类型
- `parse_performance_metrics()` / `parse_indicators()` — `exec_time()` / `row_count()` 直接调用，不重复解析逻辑

### 已有模式
- `LogParser::from_path` 用 `impl AsRef<Path>` — Builder `new()` 保持相同签名风格
- `LogIterator::next()` 返回 `Option<Result<Sqllog, ParseError>>` — 过滤方法保持同样的 Item 类型
- `const PAR_THRESHOLD: usize = 32 * 1024 * 1024` 在 `par_iter()` 内部 — Builder 的 `parallel_threshold` 默认值用此常量

### 集成点
- `src/lib.rs` 的 `pub use` 列表需要新增 `LogParserBuilder`、`FromSqllog`
- 移除 `parse_record` 从公开导出（或保留向后兼容性，由 planner 决定）

### 破坏性变更说明
- `LogParser::from_path` 被移除 — 这是 breaking change（0.9.1 → 1.1.0，semver 允许）
- 现有使用 `from_path` 的测试文件需要同步更新

</code_context>

<specifics>
## Specific Ideas

- `LogParserBuilder::new("path/to/file.log").threads(4).build()` — 最简链式调用形式
- `parser.iter().filter_by_exec_time(100)` — 过滤执行时间超过 100ms 的记录
- `parser.iter().filter_by_sql_contains("SELECT")` — 过滤包含关键字的记录
- `sqllog.exec_time()` — 直接取执行时间，无需解构元组

</specifics>

<deferred>
## Deferred Ideas

- **API-05**（通用过滤适配器 `.filter_sqllog(|s| s.exec_time()? > 100)`）— Future requirements，更灵活但增加抽象层
- **API-06**（流式/增量解析）— 破坏零拷贝生命周期设计，Out of Scope

</deferred>

---

*Phase: 07-APIErgonomics*
*Context gathered: 2026-05-19*
