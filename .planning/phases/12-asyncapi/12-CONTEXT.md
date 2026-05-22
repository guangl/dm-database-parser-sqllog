# Phase 12: AsyncAPI - Context

**Gathered:** 2026-05-22
**Status:** Ready for planning

<domain>
## Phase Boundary

在 `src/async_api/` 中实现 `AsyncLogParser` 结构体，让用户可在 tokio async 运行时中直接 `.parse().await` 解析日志文件。tokio 通过 `features = ["async"]` 可选引入，不使用异步 API 的用户无需拉入 tokio 依赖树。

此阶段实现 ASYNC-01~ASYNC-04 四个需求，不扩展其他功能。

</domain>

<decisions>
## Implementation Decisions

### API 结构体设计

- **D-01:** 独立 `AsyncLogParser` 结构体（不扩展现有 `LogParserBuilder`）。对外暴露完整 builder 链：
  ```rust
  pub struct AsyncLogParser {
      path: PathBuf,
      encoding_hint: FileEncodingHint,
      filter: Option<Filter>,
  }

  impl AsyncLogParser {
      pub fn new(path: impl AsRef<Path>) -> Self
      pub fn encoding_hint(self, hint: FileEncodingHint) -> Self
      pub fn with_filter(self, filter: Filter) -> Self
      pub async fn parse(self) -> Result<Vec<Sqllog>, AsyncError>
  }
  ```
- **D-02:** `parse()` 返回 `Result<Vec<Sqllog>, AsyncError>`；解析失败的单条记录静默丢弃（与 `skip_errors()` 语义一致），只有 I/O 或 panic 级别的错误才通过 `AsyncError` 传播给调用者。

### 错误类型

- **D-03:** 新增 `pub enum AsyncError` 于 `src/async_api/mod.rs`：
  ```rust
  pub enum AsyncError {
      Parse(ParseError),
      Panic(String),
  }
  impl From<ParseError> for AsyncError { ... }
  ```
  `Parse` 变体用于 I/O / 文件读取错误（封装 `ParseError::IoError`），`Panic` 变体用于 `spawn_blocking` 内部 panic。
- **D-04:** 实现 `From<ParseError> for AsyncError`，让内部 `?` 运算符可直接传播 `ParseError`。

### Filter 整合（ASYNC-04）

- **D-05:** 通过 `.with_filter(filter: Filter)` 方法链传入过滤条件，filter 在 `spawn_blocking` 内部执行（过滤完成后再返回 async 上下文），减少传出数据量：
  ```rust
  spawn_blocking(move || {
      let parser = LogParserBuilder::new(path).build()?;
      let iter = parser.iter();
      let records: Vec<Sqllog> = if let Some(f) = filter {
          iter.apply_filter(f).filter_map(Result::ok).collect()
      } else {
          iter.filter_map(Result::ok).collect()
      };
      Ok::<_, ParseError>(records)
  }).await
  ```
- **D-06:** 不传入 filter 时（未调用 `.with_filter()`），`parse()` 等价于收集所有成功解析的记录。

### tokio Feature Flag

- **D-07:** `Cargo.toml` 中 tokio 仅在 `async` feature 启用时引入，且只引入最小依赖集：
  ```toml
  [dependencies]
  tokio = { version = "1", features = ["rt"], optional = true }

  [features]
  async = ["tokio/rt"]
  ```
- **D-08:** 测试用 dev-dependencies 额外加 `macros` feature（用于 `#[tokio::test]`），不暴露给库用户：
  ```toml
  [dev-dependencies]
  tokio = { version = "1", features = ["rt", "macros"] }
  ```

### 模块结构

- **D-09:** 所有 async API 实现放在 `src/async_api/mod.rs`，包括 `AsyncLogParser`、`AsyncError` 定义及 `#[cfg(test)]` 单元测试块。
- **D-10:** `lib.rs` 在 `#[cfg(feature = "async")]` 守卫下添加：
  ```rust
  #[cfg(feature = "async")]
  pub mod async_api;
  #[cfg(feature = "async")]
  pub use async_api::{AsyncLogParser, AsyncError};
  ```

### 编码支持

- **D-11:** `AsyncLogParser` 通过 `.encoding_hint(hint: FileEncodingHint)` 方法继承编码 hint 能力，内部委托给 `LogParserBuilder::encoding_hint()`，与同步 API 特性对齐。

### Claude's Discretion

- `AsyncError` 是否实现 `std::error::Error` trait：Claude 决定实现（crate 公开错误类型的标准做法）。
- `spawn_blocking` 内 `LogParserBuilder` 初始化失败（如文件不存在）：通过 `AsyncError::Parse(ParseError::IoError(...))` 传播。
- 返回类型是否实现 `Send`：`Vec<Sqllog>` 已满足（`Sqllog` 全用 `String`，是 `Send + Sync`）。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` §Async — ASYNC-01~ASYNC-04 需求定义
- `.planning/ROADMAP.md` §Phase 12 — Success Criteria（5 条验收标准）

### Prior Decisions
- `.planning/phases/10-restructure/10-CONTEXT.md` — D-11 / D-12 / D-15：`async_api/` 骨架已创建，`lib.rs` 中 `async_api` 声明留给 Phase 12 添加
- `.planning/phases/11-filterbuilder/11-01-SUMMARY.md` — Filter / FilterBuilder API 最终形态
- `.planning/phases/11-filterbuilder/11-02-SUMMARY.md` — `apply_filter` / `apply_filter_keep_errors` 适配器签名

### Existing Implementation
- `src/async_api/mod.rs` — 当前空占位模块（Phase 10 创建）
- `src/filter/builder.rs` — `Filter` / `FilterBuilder` 定义（`with_filter` 参数类型来源）
- `src/parser/builder.rs` — `LogParserBuilder` 构造器（`encoding_hint` 委托目标）
- `src/parser/iterator.rs` — `LogIterator::apply_filter` 签名（spawn_blocking 内使用）
- `src/lib.rs` — 当前重导出列表（Phase 12 要在此添加 `async_api` 声明）

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `LogParserBuilder::new(path).encoding_hint(hint).build()` — `AsyncLogParser::parse()` 内部直接复用，无需重写解析逻辑
- `LogIterator::apply_filter(filter)` — spawn_blocking 闭包内使用，已支持 `Send + Sync` 谓词
- `FileEncodingHint` — 已公开导出，`AsyncLogParser::encoding_hint()` 参数类型直接使用

### Established Patterns
- 错误类型扩展：`ParseError` 已有 `IoError(std::io::Error)` 变体，`AsyncError::Parse(ParseError)` 可自然嵌套
- `pub(crate) mod` 模式：filter、parser 等内部模块用 `pub(crate)`；`async_api` 因为需要从 lib.rs `pub use` 导出，模块本身应 `pub`（但受 `#[cfg(feature="async")]` 守卫控制）
- 测试结构：async 单元测试放 `src/async_api/mod.rs` 内 `#[cfg(test)]`，与 `filter/builder.rs` 的做法一致

### Integration Points
- `lib.rs`：Phase 12 唯一需要修改的 lib.rs 变更是添加 `#[cfg(feature = "async")]` 守卫下的 `mod async_api` 声明和 `pub use` 导出
- `Cargo.toml`：新增 `[features] async = ["tokio/rt"]` 和 `tokio` optional dependency

</code_context>

<specifics>
## Specific Ideas

- 用户示例（讨论中明确）：
  ```rust
  let records = AsyncLogParser::new("sqllog.txt")
      .with_filter(FilterBuilder::new().exec_time_gt(100.0).build())
      .parse()
      .await?;
  ```
- 成功标准 1 中的 `parse_file_async(path).await` 语法在本阶段通过 `AsyncLogParser::new(path).parse().await` 满足（builder 模式，等价含义）。

</specifics>

<deferred>
## Deferred Ideas

- **Stream API**（STREAM-01/02）：`impl Stream<Item = Sqllog>` 逐条异步处理、背压支持 — 已在 REQUIREMENTS.md 标注为 Future Requirements，本阶段不实现。
- **Phase 13–15**：用户以 `12,13,14,15 --chain` 调用，但 ROADMAP.md 中仅定义了 Phase 10–12（v2.0 里程碑）。Phase 13–15 不存在，本次讨论仅处理 Phase 12。

</deferred>

---

*Phase: 12-AsyncAPI*
*Context gathered: 2026-05-22*
