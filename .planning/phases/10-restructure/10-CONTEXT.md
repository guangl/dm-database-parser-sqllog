# Phase 10: Restructure - Context

**Gathered:** 2026-05-22
**Status:** Ready for planning

<domain>
## Phase Boundary

将现有 4 个平铺 Rust 文件（`src/parser.rs` / `src/sqllog.rs` / `src/error.rs` / `src/lib.rs`）重组为功能分层子模块，库用户侧公开导入路径不变，所有现有测试继续通过。

这是 v2.0 的首个阶段 — 纯结构重组，不引入新的公开 API 功能。`filter/` 和 `async_api/` 子模块在本阶段创建骨架，Phase 11 / 12 再填充实现。

</domain>

<decisions>
## Implementation Decisions

### parser/ 内部结构

- **D-01:** `src/parser.rs` 拆分为 `src/parser/` 子目录，包含 4 个文件：
  - `src/parser/mod.rs` — `LogParser` + `parse_record_with_hint`（私有）
  - `src/parser/builder.rs` — `LogParserBuilder`
  - `src/parser/iterator.rs` — `LogIterator` + `is_timestamp_start`（私有）+ `make_invalid_format_error`（私有）
  - `src/parser/encoding.rs` — `FileEncodingHint`（公开）
- **D-02:** 私有辅助函数就近原则 — 与使用它们的主类共居同一文件，不单独抽 utils/helpers 文件
- **D-03:** `parse_record()` 降为 `pub(crate)`，从 `lib.rs` 重导出列表中移除（v2.0 breaking change）
- **D-04:** 所有直接调用 `parse_record()` 的集成测试从 `tests/` 迁移到 `src/parser/` 内部的 `#[cfg(test)]` 模块：
  - `tests/parser_coverage.rs` → `src/parser/mod.rs` 内 `#[cfg(test)]` 块
  - `tests/performance_metrics.rs` → `src/parser/mod.rs` 内 `#[cfg(test)]` 块（或 iterator.rs）
  - `tests/edge_cases.rs` → 相关测试迁入 parser/ 内部测试
  - `tests/sqllog_additional.rs` → 相关测试迁入 parser/ 或 record 内部测试
  
  注意：仅使用公开 API 的测试用例保留在 `tests/`，只有直接依赖 `parse_record` 的需要迁移

### filter/ 骨架

- **D-05:** 创建 `src/filter/` 含 2 个文件：`mod.rs`（模块入口）+ `adapter.rs`（迭代器适配器实现）
- **D-06:** 将 `filter_by_exec_time` 和 `filter_by_sql_contains` 的逻辑迁入 `src/filter/adapter.rs`，改为泛型迭代器签名：
  ```rust
  pub(crate) fn filter_by_exec_time<I>(iter: I, min_ms: u64) -> impl Iterator<Item = Result<Sqllog, ParseError>>
  where I: Iterator<Item = Result<Sqllog, ParseError>>
  
  pub(crate) fn filter_by_sql_contains<I>(iter: I, pattern: &str) -> impl Iterator<Item = Result<Sqllog, ParseError>>
  where I: Iterator<Item = Result<Sqllog, ParseError>>
  ```
- **D-07:** 这两个方法降为 `pub(crate)`（v2.0 breaking change）。`parser/iterator.rs` 中的 `LogIterator` 保留同名方法，但内部委托调用 `filter::adapter::filter_by_*`，方法可见性改为 `pub(crate)`
- **D-08:** `lib.rs` 不重导出 `filter_by_exec_time` / `filter_by_sql_contains`（Phase 11 的 `FilterBuilder` 将提供公开的过滤 API）

### sqllog → record 重命名

- **D-09:** `src/sqllog.rs` 重命名为 `src/record.rs`（实现 REFACTOR-04）
- **D-10:** `parse_meta_from_bytes`、`parse_indicators_from_bytes`、`find_indicators_split` 等 `pub(crate)` 辅助函数保留在 `src/record.rs` 中（不移动）

### async_api/ 骨架

- **D-11:** 创建 `src/async_api/mod.rs` 作为纯空占位模块（只有模块声明，无任何内容）
- **D-12:** Phase 10 不在 `lib.rs` 中添加 `async_api` 模块声明。Phase 12 才添加（带 `#[cfg(feature = "async")]` 守卫）

### lib.rs 重导出

- **D-13:** `lib.rs` 继续重导出所有现有公开类型：`LogParser`、`LogParserBuilder`、`LogIterator`、`FileEncodingHint`、`Sqllog`、`ParseError`
- **D-14:** 移除 `parse_record` 的重导出（D-03 决定）
- **D-15:** 模块声明改为：`pub(crate) mod parser; pub(crate) mod filter; pub(crate) mod record; pub(crate) mod error;`（`async_api` 在 Phase 10 不添加）

### 测试策略

- **D-16:** 所有使用公开 API 的集成测试（通过 `LogParserBuilder`、`LogParser`、`Sqllog` 等）保留在 `tests/` 目录，无需迁移
- **D-17:** `cargo test` 全量通过、`cargo clippy -- -D warnings` 零警告、覆盖率 ≥90% 是 Phase 10 的完成条件

### Claude's Discretion

- `parser/mod.rs` vs `parser/core.rs` 的命名：使用标准的 `mod.rs`，不引入 `core.rs`
- 拆分后各子文件的具体代码行数：由实际代码分配决定，无硬性限制

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements & Roadmap

- `.planning/ROADMAP.md` §Phase 10 — Phase 10 目标、成功标准、依赖关系
- `.planning/REQUIREMENTS.md` §Refactor — REFACTOR-01 至 REFACTOR-07 完整需求定义
- `.planning/PROJECT.md` §Key Decisions — v1.1 以来的架构决策记录

### 当前代码结构（必读）

- `src/parser.rs` — 521 行，包含所有将被拆分的源代码
- `src/sqllog.rs` — 284 行，Sqllog 结构体和 pub(crate) 辅助函数
- `src/lib.rs` — 当前重导出结构（重组后需更新）
- `src/error.rs` — ParseError（本阶段不变）

### 测试文件（迁移相关）

- `tests/parser_coverage.rs` — 直接使用 `parse_record`，需迁移到内部测试
- `tests/performance_metrics.rs` — 直接使用 `parse_record`，需迁移到内部测试
- `tests/edge_cases.rs` — 部分使用 `parse_record`，相关用例需迁移
- `tests/sqllog_additional.rs` — 部分使用 `parse_record`，相关用例需迁移

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `src/parser.rs::LogIterator` — 将拆分到 `parser/iterator.rs`；现有过滤方法逻辑迁入 `filter/adapter.rs`
- `src/parser.rs::LogParserBuilder` — 整体迁入 `parser/builder.rs`，无需修改逻辑
- `src/sqllog.rs::Sqllog` — 整体迁入 `record.rs`，文件重命名即可

### Established Patterns

- `pub(crate)` 用于内部辅助函数（已建立，Phase 10 扩大使用范围到 `parse_record` 和过滤方法）
- `lib.rs` 仅包含 rustdoc + re-export（保持不变）
- `mod.rs` 作为子模块入口（新建 `parser/mod.rs`、`filter/mod.rs`、`async_api/mod.rs`）

### Integration Points

- `lib.rs` 的 `pub use` 列表是用户侧 API 的唯一出口 — 重组后需逐一验证
- `tests/` 中通过 `use dm_database_parser_sqllog::*` 使用的集成测试 — 不应受结构变动影响
- `examples/` 中的 3 个示例 — 需确认 `cargo test --doc` 通过（REFACTOR-07）
- `Cargo.toml` — 无需改动（`async` feature flag 在 Phase 12 才添加）

</code_context>

<specifics>
## Specific Ideas

- `parse_record` 的测试用例是覆盖率的重要来源（performance_metrics.rs 有 ~16 个测试）。迁入内部测试模块时需确保覆盖率不降至 90% 以下
- 泛型迭代器签名（`where I: Iterator<...>`）让 Phase 11 的 `FilterBuilder` 可以复用 `filter/adapter.rs` 的逻辑，不需要重复实现

</specifics>

<deferred>
## Deferred Ideas

- `par_iter()` / Rayon 并行迭代器 — 当前代码库中不存在，不在 Phase 10 范围内
- 完整 `FilterBuilder` 公开 API — Phase 11 实现
- `tokio` feature flag 和 async API — Phase 12 实现
- `async_api/mod.rs` 的实际内容 — Phase 12 填充

</deferred>

---

*Phase: 10-Restructure*
*Context gathered: 2026-05-22*
