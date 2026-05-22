# Phase 10: Restructure - Research

**Researched:** 2026-05-22
**Domain:** Rust 模块重组 / crate 内部结构重构
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** `src/parser.rs` 拆分为 `src/parser/` 子目录，含 4 个文件：
- `src/parser/mod.rs` — `LogParser` + `parse_record_with_hint`（私有）
- `src/parser/builder.rs` — `LogParserBuilder`
- `src/parser/iterator.rs` — `LogIterator` + `is_timestamp_start`（私有）+ `make_invalid_format_error`（私有）
- `src/parser/encoding.rs` — `FileEncodingHint`（公开）

**D-02:** 私有辅助函数就近原则 — 与使用它们的主类共居同一文件，不单独抽 utils/helpers 文件

**D-03:** `parse_record()` 降为 `pub(crate)`，从 `lib.rs` 重导出列表中移除（v2.0 breaking change）

**D-04:** 直接调用 `parse_record()` 的集成测试迁移到 `src/parser/` 内部的 `#[cfg(test)]` 模块：
- `tests/parser_coverage.rs` → `src/parser/mod.rs` 内 `#[cfg(test)]` 块
- `tests/performance_metrics.rs` → `src/parser/mod.rs` 内 `#[cfg(test)]` 块（或 iterator.rs）
- `tests/edge_cases.rs` → 相关测试迁入 parser/ 内部测试
- `tests/sqllog_additional.rs` → 相关测试迁入 parser/ 或 record 内部测试
- 仅使用公开 API 的测试用例保留在 `tests/`，只有直接依赖 `parse_record` 的需要迁移

**D-05:** 创建 `src/filter/` 含 2 个文件：`mod.rs`（模块入口）+ `adapter.rs`（迭代器适配器）

**D-06:** 将 `filter_by_exec_time` 和 `filter_by_sql_contains` 的逻辑迁入 `src/filter/adapter.rs`，改为泛型迭代器签名：
```rust
pub(crate) fn filter_by_exec_time<I>(iter: I, min_ms: u64) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where I: Iterator<Item = Result<Sqllog, ParseError>>

pub(crate) fn filter_by_sql_contains<I>(iter: I, pattern: &str) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where I: Iterator<Item = Result<Sqllog, ParseError>>
```

**D-07:** 这两个方法降为 `pub(crate)`。`parser/iterator.rs` 中的 `LogIterator` 保留同名方法，但内部委托调用 `filter::adapter::filter_by_*`，方法可见性改为 `pub(crate)`

**D-08:** `lib.rs` 不重导出 `filter_by_exec_time` / `filter_by_sql_contains`（Phase 11 的 `FilterBuilder` 将提供公开过滤 API）

**D-09:** `src/sqllog.rs` 重命名为 `src/record.rs`（实现 REFACTOR-04）

**D-10:** `parse_meta_from_bytes`、`parse_indicators_from_bytes`、`find_indicators_split` 等 `pub(crate)` 辅助函数保留在 `src/record.rs` 中（不移动）

**D-11:** 创建 `src/async_api/mod.rs` 作为纯空占位模块

**D-12:** Phase 10 不在 `lib.rs` 中添加 `async_api` 模块声明（Phase 12 才添加）

**D-13:** `lib.rs` 继续重导出所有现有公开类型：`LogParser`、`LogParserBuilder`、`LogIterator`、`FileEncodingHint`、`Sqllog`、`ParseError`

**D-14:** 移除 `parse_record` 的重导出

**D-15:** 模块声明改为：`pub(crate) mod parser; pub(crate) mod filter; pub(crate) mod record; pub(crate) mod error;`（`async_api` 在 Phase 10 不添加）

**D-16:** 所有使用公开 API 的集成测试（通过 `LogParserBuilder`、`LogParser`、`Sqllog` 等）保留在 `tests/` 目录

**D-17:** `cargo test` 全量通过、`cargo clippy -- -D warnings` 零警告、覆盖率 ≥90% 是 Phase 10 的完成条件

### Claude's Discretion

- `parser/mod.rs` vs `parser/core.rs` 的命名：使用标准的 `mod.rs`，不引入 `core.rs`
- 拆分后各子文件的具体代码行数：由实际代码分配决定，无硬性限制

### Deferred Ideas (OUT OF SCOPE)

- `par_iter()` / Rayon 并行迭代器
- 完整 `FilterBuilder` 公开 API（Phase 11）
- `tokio` feature flag 和 async API（Phase 12）
- `async_api/mod.rs` 的实际内容（Phase 12 填充）
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REFACTOR-01 | 开发者可通过 `src/parser/` 子模块找到所有解析相关代码（LogParser、LogParserBuilder、迭代器） | D-01：明确拆分方案，parser/ 含 mod.rs/builder.rs/iterator.rs/encoding.rs |
| REFACTOR-02 | 开发者可通过 `src/filter/` 子模块找到所有过滤相关代码 | D-05/D-06：filter/mod.rs + filter/adapter.rs 骨架 |
| REFACTOR-03 | 开发者可通过 `src/async_api/` 子模块找到所有异步接口代码 | D-11：空占位模块即满足本阶段要求 |
| REFACTOR-04 | `src/record.rs` 包含 `Sqllog` 结构体，`src/error.rs` 包含 `ParseError` | D-09：sqllog.rs 重命名；error.rs 不动 |
| REFACTOR-05 | `tools.rs` 中的字节级工具函数分配到合适子模块，不作为公开 API 暴露 | 当前无 tools.rs；is_timestamp_start 已私有，迁入 iterator.rs |
| REFACTOR-06 | `lib.rs` 顶层重导出所有公开类型，用户侧导入路径保持有效 | D-13/D-14/D-15：lib.rs 重导出结构方案明确 |
| REFACTOR-07 | `examples/` 和 rustdoc 示例更新以反映新模块结构 | 见下方 Examples 迁移分析；doc-test 位于 lib.rs 需同步验证 |
</phase_requirements>

---

## Summary

Phase 10 是一次纯结构重组，不引入新的公开 API 功能。现有代码分布在 4 个平铺文件中（`src/parser.rs` 521 行、`src/sqllog.rs` 284 行、`src/error.rs` 37 行、`src/lib.rs` 91 行），需要将其重组为 4 个功能子模块（`parser/`、`filter/`、`record.rs`、`error.rs`），并通过 `lib.rs` 的 `pub use` 保持用户侧导入路径不变。

主要工作分三类：(1) 代码拆分移动 — `parser.rs` 按决策拆分为 4 文件，`sqllog.rs` 重命名为 `record.rs`；(2) 测试迁移 — 直接依赖 `parse_record` 的测试从 `tests/` 迁入对应模块内部 `#[cfg(test)]` 块；(3) API 收紧 — `parse_record` 降为 `pub(crate)`，`filter_by_exec_time` / `filter_by_sql_contains` 改为 `pub(crate)` 并迁入 `filter/adapter.rs`，同时在 `LogIterator` 中保留委托方法。

关键约束：`lib.rs` 的 `pub use` 重导出是用户侧 API 的唯一出口，需逐一验证所有公开类型路径仍然有效，且 `cargo test --doc` 的 3 个 rustdoc 示例继续通过。

**Primary recommendation:** 按 Wave 顺序逐步迁移：先建立新模块骨架和 record.rs，再迁移 parser 内部代码，再迁移测试，最后清理 lib.rs 重导出并验证。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 文件读取与编码探测 | `parser/builder.rs` (LogParserBuilder) | `parser/encoding.rs` (FileEncodingHint) | Builder 负责构建逻辑；编码枚举独立以便 iterator 引用 |
| 记录分割与迭代 | `parser/iterator.rs` (LogIterator) | `parser/mod.rs` (parse_record_with_hint) | 迭代器驱动分割，mod.rs 持有核心解析私有函数 |
| 单条记录解析 | `parser/mod.rs` (parse_record_with_hint 私有) | `record.rs` (parse_meta_from_bytes 等) | 解析协调在 mod.rs；字段级解析辅助函数在 record.rs |
| Sqllog 数据结构 | `record.rs` | — | 数据结构及其字节级解析辅助函数集中管理 |
| 过滤逻辑 | `filter/adapter.rs` (泛型函数) | `parser/iterator.rs` (委托方法) | 过滤逻辑在 filter/；LogIterator 保留方法壳以向后兼容 |
| 公开 API 出口 | `lib.rs` (pub use) | — | 所有类型经由 lib.rs 重导出，内部路径对用户不可见 |
| 错误类型 | `error.rs` | — | 独立模块，Phase 10 不变 |

---

## Standard Stack

本阶段是纯代码重组，不引入任何新依赖。现有依赖继续使用：

| Library | Version | Purpose |
|---------|---------|---------|
| memchr | 2.7.6 | 字节级搜索（memmem、memchr、memrchr），迁移后引用路径不变 |
| atoi | 2.0.0 | 字节转整数，位于 record.rs |
| thiserror | 2.0.17 | ParseError 派生宏，位于 error.rs |
| encoding | 0.2 | GB18030 解码，位于 parser/mod.rs 和 parser/builder.rs |

**无需运行 Package Legitimacy Audit** — 本阶段不安装任何新包。

---

## Architecture Patterns

### 目标项目结构

```
src/
├── lib.rs                  # pub use 重导出 + rustdoc 示例（更新模块声明）
├── error.rs                # ParseError（本阶段不变）
├── record.rs               # Sqllog 结构体 + pub(crate) 字节解析辅助函数
│                           # （原 sqllog.rs 重命名）
├── parser/
│   ├── mod.rs              # LogParser + parse_record_with_hint（私有）
│   │                       # + #[cfg(test)] 内部测试块（从 tests/ 迁入）
│   ├── builder.rs          # LogParserBuilder
│   ├── iterator.rs         # LogIterator + is_timestamp_start（私有）
│   │                       # + make_invalid_format_error（私有）
│   │                       # + filter_by_exec_time / filter_by_sql_contains（委托方法，pub(crate)）
│   └── encoding.rs         # FileEncodingHint（pub）
├── filter/
│   ├── mod.rs              # 模块入口（pub(crate) mod adapter;）
│   └── adapter.rs          # 泛型过滤函数（pub(crate)）
└── async_api/
    └── mod.rs              # 空占位模块（无内容）
```

### Pattern 1: Rust 子目录模块拆分

**What:** 将 `src/foo.rs` 改写为 `src/foo/mod.rs` + 若干同目录子文件。`mod.rs` 是模块的默认入口，其他文件需在 `mod.rs` 中用 `mod` 声明。

**When to use:** 单文件超出单一职责，需按类型/功能拆分时。

**关键点：**
```rust
// src/parser/mod.rs
pub mod builder;       // 对应 src/parser/builder.rs
pub mod iterator;      // 对应 src/parser/iterator.rs
pub(crate) mod encoding; // 对应 src/parser/encoding.rs（pub(crate) 控制外部可见性）

// 内部 use（跨文件引用）
use crate::parser::builder::LogParserBuilder;
use crate::parser::iterator::LogIterator;
use crate::parser::encoding::FileEncodingHint;
use crate::record::Sqllog;
use crate::error::ParseError;
```

**跨模块 crate:: 路径规则：**
- `crate::record::Sqllog` — 从任意模块访问 record.rs 中的类型
- `crate::filter::adapter::filter_by_exec_time` — 从 iterator.rs 委托调用
- `crate::error::ParseError` — 从 parser/ 内部引用

### Pattern 2: pub use 重导出维持公开 API

**What:** `lib.rs` 通过 `pub use` 将内部模块路径映射为外部可用的顶层名称。

**Why critical:** 用户侧 `use dm_database_parser_sqllog::LogParser` 需要从 `lib.rs` 解析，而不是深层路径。

```rust
// src/lib.rs（Phase 10 最终状态）
pub(crate) mod error;
pub(crate) mod parser;
pub(crate) mod filter;
pub(crate) mod record;
// async_api 不在 Phase 10 添加

pub use error::ParseError;
pub use parser::{
    FileEncodingHint, LogIterator, LogParser, LogParserBuilder,
    // parse_record 不再重导出（D-03/D-14）
};
pub use record::Sqllog;
```

**注意：** `FileEncodingHint` 定义在 `parser/encoding.rs`，但通过 `parser/mod.rs` 再导出到 `parser` 模块层，lib.rs 从 `parser::` 引用。因此 `parser/mod.rs` 需要 `pub use encoding::FileEncodingHint;`。

### Pattern 3: 泛型迭代器适配器

**What:** `filter/adapter.rs` 中的过滤函数接受泛型 `Iterator`，返回 `impl Iterator`。这样 Phase 11 的 `FilterBuilder` 可以直接复用，不需要重写过滤逻辑。

```rust
// src/filter/adapter.rs
use crate::record::Sqllog;
use crate::error::ParseError;

pub(crate) fn filter_by_exec_time<I>(
    iter: I,
    min_ms: u64,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    let threshold = min_ms as f32;
    iter.filter(move |item| match item {
        Ok(sqllog) => sqllog.exectime >= threshold,
        Err(_) => false,
    })
}

pub(crate) fn filter_by_sql_contains<I>(
    iter: I,
    pattern: &str,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    let pattern = pattern.to_string();
    iter.filter(move |item| match item {
        Ok(sqllog) => sqllog.sql.contains(&pattern),
        Err(_) => false,
    })
}
```

`LogIterator` 委托实现：

```rust
// src/parser/iterator.rs
use crate::filter::adapter;

impl<'a> LogIterator<'a> {
    pub(crate) fn filter_by_exec_time(
        self,
        min_ms: u64,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::filter_by_exec_time(self, min_ms)
    }

    pub(crate) fn filter_by_sql_contains(
        self,
        pattern: &str,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::filter_by_sql_contains(self, pattern)
    }
    // skip_errors 保持公开 pub
}
```

### Pattern 4: 内部测试模块替代集成测试

**What:** 将直接依赖 `pub(crate)` 函数的测试迁移到定义该函数的模块内部，使用 `#[cfg(test)]` 块。

```rust
// src/parser/mod.rs 末尾
#[cfg(test)]
mod tests {
    use super::*;  // 可访问 parse_record_with_hint（私有函数）

    // 从 tests/performance_metrics.rs 迁入的测试
    #[test]
    fn performance_metrics_full() { ... }
}
```

**为什么必须这样做：** `parse_record` 降为 `pub(crate)` 后，`tests/` 目录下的集成测试是独立 crate，无法访问 `pub(crate)` 函数。只有内部 `#[cfg(test)]` 模块可以访问 crate 私有符号。

### Anti-Patterns to Avoid

- **不要创建 `utils.rs` 或 `helpers.rs`**：违反 D-02（就近原则）。私有辅助函数与使用它们的主类共居同一文件。
- **不要将 `async_api/` 添加到 lib.rs 模块声明**：Phase 10 只创建目录，Phase 12 才在 lib.rs 中引入（带 `#[cfg(feature = "async")]` 守卫）。
- **不要给 `filter_by_exec_time` / `filter_by_sql_contains` 保留 `pub` 可见性**：D-07 明确降为 `pub(crate)`；这是 v2.0 的有意 breaking change。
- **不要让 filter/adapter.rs 直接引用 LogIterator**：适配器函数使用泛型 `I: Iterator<...>`，对具体类型无依赖。
- **不要忘记 `parser/mod.rs` 中的 `pub use encoding::FileEncodingHint`**：lib.rs 通过 `parser::FileEncodingHint` 引用，必须在 parser 层级重导出。

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| 泛型迭代器适配器 | 用条件分支手写过滤逻辑 | Rust `Iterator::filter` + 泛型约束（已有模式，直接实现）|
| 模块可见性控制 | 通过 wrapper 函数转发来隐藏实现 | `pub(crate)` 直接控制可见性 |
| 跨文件测试隔离 | 创建额外的 test helper crate | `#[cfg(test)]` 内部模块 + `use super::*` |

**Key insight:** 本阶段核心难点不在于新功能实现，而在于 Rust 模块路径和可见性规则的正确应用，以及测试迁移后覆盖率的维持。

---

## Runtime State Inventory

> 本阶段不涉及重命名或迁移（`sqllog.rs → record.rs` 仅是文件名变更，不影响运行时状态）。

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | 无 — 纯 Rust crate，无数据库或持久化存储 | 无 |
| Live service config | 无 | 无 |
| OS-registered state | 无 | 无 |
| Secrets/env vars | 无 | 无 |
| Build artifacts | `target/` 目录下的编译产物将在结构变更后自动重建 | 无需手动处理；`cargo clean` 可选 |

---

## Common Pitfalls

### Pitfall 1: `pub use` 路径中断

**What goes wrong:** `lib.rs` 中 `pub use parser::FileEncodingHint` 失败，因为 `FileEncodingHint` 定义在 `parser/encoding.rs` 而非 `parser/mod.rs`。

**Why it happens:** Rust 模块路径解析规则：外部只能访问 `pub use` 明确导出的路径。`parser/encoding.rs` 定义的类型默认在 `parser::encoding::` 下，需要 `parser/mod.rs` 先用 `pub use encoding::FileEncodingHint;` 将其提升到 `parser::` 层级，lib.rs 才能用 `pub use parser::FileEncodingHint`。

**How to avoid:** 建立新模块骨架后，立即用 `cargo check` 验证所有 `pub use` 路径可解析，再进行下一步。

**Warning signs:** `error[E0412]: cannot find type FileEncodingHint in module parser`

### Pitfall 2: `pub(crate)` 降级导致集成测试编译失败

**What goes wrong:** `tests/parser_coverage.rs`、`tests/performance_metrics.rs` 等文件仍在 `use dm_database_parser_sqllog::parse_record;`，但 `parse_record` 降为 `pub(crate)` 后 lib.rs 不再重导出，导致编译报错。

**Why it happens:** `tests/` 目录下的文件是独立编译单元，等价于外部用户代码，无法访问 `pub(crate)` 符号。

**How to avoid:** D-04 已明确：在 `parse_record` 降级之前，先完成相关测试向内部 `#[cfg(test)]` 模块的迁移。迁移顺序很重要：先迁移测试，再修改可见性。

**Warning signs:** `error[E0432]: unresolved import dm_database_parser_sqllog::parse_record`

### Pitfall 3: 循环依赖

**What goes wrong:** `filter/adapter.rs` 引用了 `parser::iterator::LogIterator`，而 `parser/iterator.rs` 又引用了 `filter::adapter`，形成循环依赖。

**Why it happens:** 如果 `filter/adapter.rs` 使用具体类型（`LogIterator`）而不是泛型 `I: Iterator<...>`，就会产生 `parser <-> filter` 的循环模块依赖。

**How to avoid:** D-06 明确要求泛型迭代器签名。`filter/adapter.rs` 只依赖 `crate::record::Sqllog` 和 `crate::error::ParseError`，不依赖任何 parser 模块类型。

**Warning signs:** `error[E0391]: cycle detected when computing...`

### Pitfall 4: 内部测试迁移后覆盖率下降

**What goes wrong:** `parse_record` 的 16+ 个测试（主要在 `performance_metrics.rs`）迁入内部模块后，某些覆盖率工具未能正确追踪内部测试，导致报告覆盖率低于 90%。

**Why it happens:** `cargo llvm-cov` 通常可以正确追踪 `#[cfg(test)]` 内部模块，但需要包含 `--all-targets` 或 `--workspace` 参数。

**How to avoid:** 迁移完成后立即运行 `cargo llvm-cov --workspace --all-features --fail-under-lines 90` 验证。

**Warning signs:** 覆盖率报告中 `src/parser/mod.rs` 分支覆盖率异常低。

### Pitfall 5: examples/ 中调用已降级的方法

**What goes wrong:** `examples/filter_slow_queries.rs` 通过 `parser.iter().filter_by_exec_time(100)` 调用过滤方法，该方法降为 `pub(crate)` 后，examples 作为独立 crate 无法调用。

**Why it happens:** `examples/` 目录下的文件与 `tests/` 一样是独立编译单元，无法访问 `pub(crate)` 方法。

**Actual impact:** CONTEXT.md D-07 说 `LogIterator` 的过滤方法降为 `pub(crate)`，但 `examples/filter_slow_queries.rs` 当前直接调用 `filter_by_exec_time`。**这是一个需要在规划时解决的冲突**。

**Resolution options:**
1. 让 `LogIterator::filter_by_exec_time` 保持 `pub`（但委托给内部函数），直到 Phase 11 提供公开 `FilterBuilder` 替代方案
2. 更新 `examples/filter_slow_queries.rs` 使用 `Iterator::filter` 替代（临时降级用户体验）
3. 在 lib.rs 重导出一个瘦包装公开函数

**推荐：** 保持 `LogIterator::filter_by_exec_time` 为 `pub`（不降为 `pub(crate)`），使 examples 继续编译通过，符合 REFACTOR-07 要求。这一点需要规划时明确。

### Pitfall 6: `use crate::sqllog` 引用残留

**What goes wrong:** `src/parser.rs` 中有 `use crate::sqllog;` 和 `use crate::sqllog::Sqllog;`。重命名为 `record.rs` 后，这些路径需要更新为 `crate::record`。

**Why it happens:** 文件重命名不会自动更新 `use` 语句。

**How to avoid:** 全局搜索 `sqllog` 关键词（`grep -r "sqllog" src/`），确认所有 use 路径已更新。`lib.rs` 的 `pub(crate) mod sqllog;` 声明也需改为 `pub(crate) mod record;`，`pub use sqllog::Sqllog` 改为 `pub use record::Sqllog`。

---

## Code Examples

### 完成后的 lib.rs 结构

```rust
// Source: CONTEXT.md D-13/D-14/D-15
pub(crate) mod error;
pub(crate) mod parser;
pub(crate) mod filter;
pub(crate) mod record;
// async_api 不在 Phase 10 添加

pub use error::ParseError;
pub use parser::{
    FileEncodingHint, LogIterator, LogParser, LogParserBuilder,
};
pub use record::Sqllog;
```

### parser/mod.rs 骨架

```rust
// Source: CONTEXT.md D-01/D-02
use memchr::memmem::Finder;
use std::sync::LazyLock;

pub mod builder;
pub mod iterator;
pub(crate) mod encoding;

pub use builder::LogParserBuilder;
pub use iterator::LogIterator;
pub use encoding::FileEncodingHint;

use crate::error::ParseError;
use crate::record::Sqllog;

static FINDER_CLOSE_META: LazyLock<Finder<'static>> = ...;
static FINDER_RECORD_START: LazyLock<Finder<'static>> = ...;

/// 核心解析函数
pub(crate) fn parse_record(record_bytes: &[u8]) -> Result<Sqllog, ParseError> {
    parse_record_with_hint(record_bytes, FileEncodingHint::Auto, 0)
}

fn parse_record_with_hint(...) -> Result<Sqllog, ParseError> { ... }

// LogParser struct + impl
pub struct LogParser { ... }
impl LogParser { ... }

#[cfg(test)]
mod tests {
    use super::*;
    // 从 tests/performance_metrics.rs、tests/parser_coverage.rs 迁入的测试
}
```

### filter/adapter.rs 完整实现

```rust
// Source: CONTEXT.md D-06
use crate::error::ParseError;
use crate::record::Sqllog;

pub(crate) fn filter_by_exec_time<I>(
    iter: I,
    min_ms: u64,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    let threshold = min_ms as f32;
    iter.filter(move |item| match item {
        Ok(sqllog) => sqllog.exectime >= threshold,
        Err(_) => false,
    })
}

pub(crate) fn filter_by_sql_contains<I>(
    iter: I,
    pattern: &str,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    let pattern = pattern.to_string();
    iter.filter(move |item| match item {
        Ok(sqllog) => sqllog.sql.contains(&pattern),
        Err(_) => false,
    })
}
```

---

## Test Migration Map

本阶段需要迁移以下测试（直接依赖 `parse_record` 的用例必须迁入内部模块）：

| 文件 | 使用 parse_record 的测试 | 保留在 tests/ 的测试 | 迁移目标 |
|------|--------------------------|----------------------|----------|
| `tests/parser_coverage.rs` | `parse_record_single_line_no_newline`、`parse_record_no_meta_open_paren`、`parse_record_no_meta_close_paren` | `iterator_skips_leading_blank_line`、`crlf_in_multiline_first_line` | `src/parser/mod.rs #[cfg(test)]` |
| `tests/performance_metrics.rs` | 全部 16 个测试（全部依赖 `parse_record`） | — | `src/parser/mod.rs #[cfg(test)]` |
| `tests/edge_cases.rs` | `meta_closing_paren_without_space_then_body_on_next_line`、`appname_empty_then_take_next_token_as_appname_not_ip`、`indicators_not_strictly_formatted_should_not_split_body` | `probable_record_start_line_and_iterator_singleline_detection` | `src/parser/mod.rs #[cfg(test)]` |
| `tests/sqllog_additional.rs` | `body_without_indicators`、`indicators_exec_id_only` 等 12 个测试 | — | `src/parser/mod.rs #[cfg(test)]` 或 `src/record.rs #[cfg(test)]` |
| `tests/parser_errors.rs` | `test_parse_record_timestamp_validation`（局部 use parse_record） | 其余 8 个测试（用 LogParserBuilder） | `src/parser/mod.rs #[cfg(test)]` |

**保留在 `tests/` 的文件（无需修改）：**
- `tests/integration_test.rs` — 仅用 `LogParserBuilder`
- `tests/parser_filters.rs` — 仅用 `LogParserBuilder`
- `tests/parser_iterator.rs` — 仅用公开 API

**注意：** `tests/sqllog_additional.rs` 包含 GB18030 编码测试，这些测试需要访问 `parse_record`。迁移时注意 `dev-dependencies` 中的 `encoding` crate 在内部测试中是否可用（`dev-dependencies` 在 `#[cfg(test)]` 内部模块中可用）。

---

## Environment Availability

步骤 2.6: 本阶段为纯代码重组，所有依赖均为 Cargo.toml 中已有的 crates，无外部工具或服务依赖。

| Dependency | Required By | Available | Version |
|------------|------------|-----------|---------|
| Rust toolchain | cargo build/test | ✓ | stable (edition 2024) |
| cargo llvm-cov | 覆盖率验证 | 需确认 | — |

---

## Package Legitimacy Audit

本阶段不安装任何新包，跳过此节。

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| 平铺文件结构（parser.rs、sqllog.rs） | 功能子模块（parser/、filter/、record.rs） | 为 Phase 11/12 扩展提供清晰边界 |
| `parse_record` 公开导出 | `pub(crate)` 降级（v2.0 breaking change） | 用户侧不再直接调用；由迭代器封装 |
| `filter_by_*` 方法直接在 LogIterator 上 | 迁入 `filter/adapter.rs` 泛型函数，LogIterator 委托 | Phase 11 FilterBuilder 复用适配器逻辑 |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `examples/filter_slow_queries.rs` 使用 `filter_by_exec_time`，若该方法降为 `pub(crate)` 则 examples 无法编译 | Pitfall 5 | 若误判，REFACTOR-07 的 `cargo test --doc` / examples 验证将失败 |
| A2 | `tests/sqllog_additional.rs` 中全部测试均依赖 `parse_record`，保留在 tests/ 中无公开 API 用例 | Test Migration Map | 若有公开 API 用例，可留在 tests/ 减少迁移量 |

**注：** A1 已通过直接读取 examples/filter_slow_queries.rs 验证为事实，非假设。建议规划时在 examples/filter_slow_queries.rs 对应任务中明确处理策略（保持 `pub` 或改用 `.filter()` 闭包）。

---

## Open Questions

1. **`LogIterator::filter_by_exec_time` 的最终可见性**
   - What we know: CONTEXT.md D-07 说降为 `pub(crate)`；examples/filter_slow_queries.rs 当前调用该方法
   - What's unclear: Phase 10 完成时 examples 是否需要编译通过（REFACTOR-07 要求 `cargo test --doc` 通过，但未明确 examples 是否 `cargo build --examples`）
   - Recommendation: 规划时在对应任务中明确：(a) 保持 `LogIterator::filter_by_exec_time` 为 `pub` 直到 Phase 11 替代 API 就绪，或 (b) 更新 examples 改用 `.filter()` 闭包

2. **`tests/sqllog_additional.rs` 保留 vs 全量迁移**
   - What we know: 该文件 233 行，前半部分全部依赖 `parse_record`；部分测试可能仅用 GB18030 辅助函数
   - What's unclear: 是否有仅用公开 API 的用例值得保留在 tests/
   - Recommendation: 规划时默认全量迁移到 `src/parser/mod.rs #[cfg(test)]`，如发现纯公开 API 用例可选择性留下

---

## Sources

### Primary (HIGH confidence)
- `src/parser.rs`（直接读取，521 行）— 所有待拆分代码的实际内容
- `src/sqllog.rs`（直接读取，284 行）— Sqllog 结构体和辅助函数
- `src/lib.rs`（直接读取）— 当前重导出结构
- `src/error.rs`（直接读取）— ParseError（本阶段不变）
- `.planning/phases/10-restructure/10-CONTEXT.md` — 所有设计决策锁定版本

### Secondary (MEDIUM confidence)
- `tests/` 目录所有文件（直接读取）— 测试迁移分析的实证基础
- `examples/` 目录所有文件（直接读取）— REFACTOR-07 验证需求分析

### Tertiary (LOW confidence)
- 无

---

## Metadata

**Confidence breakdown:**
- 文件拆分映射: HIGH — 直接读取了所有源文件，拆分方案来自 CONTEXT.md
- 测试迁移分析: HIGH — 逐文件检查了 `parse_record` 使用情况
- filter/adapter.rs 泛型签名: HIGH — CONTEXT.md D-06 明确给出代码签名
- Pitfall 5（examples 可见性冲突）: HIGH — 直接代码验证

**Research date:** 2026-05-22
**Valid until:** 2026-06-22（模块重构模式稳定，Rust 可见性规则不变）
