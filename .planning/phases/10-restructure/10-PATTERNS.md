# Phase 10: Restructure - Pattern Map

**Mapped:** 2026-05-22
**Files analyzed:** 12 (4 source + 5 tests + 3 new skeleton files)
**Analogs found:** 10 / 12

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/parser/mod.rs` | module-entry | request-response | `src/parser.rs` lines 1-41, 208-521 | exact (split source) |
| `src/parser/builder.rs` | builder | request-response | `src/parser.rs` lines 43-87 | exact (extracted) |
| `src/parser/iterator.rs` | iterator | streaming | `src/parser.rs` lines 101-203 | exact (extracted) |
| `src/parser/encoding.rs` | enum/config | — | `src/parser.rs` lines 22-31 | exact (extracted) |
| `src/record.rs` | model | transform | `src/sqllog.rs` | exact (rename) |
| `src/filter/mod.rs` | module-entry | — | `src/lib.rs` (module declaration pattern) | role-match |
| `src/filter/adapter.rs` | utility | transform | `src/parser.rs` lines 116-138 (filter methods) | role-match |
| `src/async_api/mod.rs` | placeholder | — | — | no analog (empty file) |
| `src/lib.rs` | re-export | — | `src/lib.rs` (current) | exact (update) |
| `tests/parser_coverage.rs` | test (partial) | — | `src/parser.rs` lines 441-521 (internal tests) | role-match |
| `tests/performance_metrics.rs` | test (migrate) | — | `src/parser.rs` lines 441-521 (internal tests) | role-match |
| `tests/parser_errors.rs` | test (partial) | — | `src/parser.rs` lines 441-521 | role-match |

---

## Pattern Assignments

### `src/parser/mod.rs` (module-entry, keeps LogParser + parse_record_with_hint)

**Analog:** `src/parser.rs`

**Module declarations and re-exports pattern** — place at top of file:
```rust
// src/parser/mod.rs 顶部
pub mod builder;
pub mod iterator;
pub(crate) mod encoding;

pub use builder::LogParserBuilder;
pub use iterator::LogIterator;
pub use encoding::FileEncodingHint;
```

**Imports pattern** (from `src/parser.rs` lines 1-13):
```rust
use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::record::Sqllog;   // 注意：sqllog → record
use encoding::all::GB18030;
use encoding::{DecoderTrap, Encoding};
```

**Static finders pattern** (from `src/parser.rs` lines 16-19):
```rust
static FINDER_CLOSE_META: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b") "));
static FINDER_RECORD_START: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b"\n20"));
```

**parse_record pub(crate) wrapper** (from `src/parser.rs` lines 208-210, visibility降级):
```rust
// D-03: pub → pub(crate)，从 lib.rs 重导出列表中移除
pub(crate) fn parse_record(record_bytes: &[u8]) -> Result<Sqllog, ParseError> {
    parse_record_with_hint(record_bytes, FileEncodingHint::Auto, 0)
}
```

**parse_record_with_hint core pattern** (from `src/parser.rs` lines 213-410):
- 函数签名 `fn parse_record_with_hint(record_bytes: &[u8], encoding_hint: FileEncodingHint, line_number: u64) -> Result<Sqllog, ParseError>`
- 保持私有（无 `pub`）
- 调用 `crate::record::parse_meta_from_bytes`、`crate::record::find_indicators_split`、`crate::record::parse_indicators_from_bytes`（路径从 `sqllog::` 改为 `crate::record::`）

**LogParser struct + impl** (from `src/parser.rs` lines 37-99, impl LogParser only):
```rust
pub struct LogParser {
    data: Vec<u8>,
    encoding: FileEncodingHint,
}

impl LogParser {
    pub fn iter(&self) -> LogIterator<'_> {
        LogIterator {
            data: &self.data,
            pos: 0,
            encoding: self.encoding,
            line_number: 1,
        }
    }
}
```

**Internal test block pattern** (from `src/parser.rs` lines 441-521):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // 迁入自 tests/performance_metrics.rs（全部 16 个测试）
    // 迁入自 tests/parser_coverage.rs 中 parse_record 相关测试（3 个）
    // 迁入自 tests/edge_cases.rs 中 parse_record 相关测试（3 个）
    // 迁入自 tests/sqllog_additional.rs（全部测试）
    // 迁入自 tests/parser_errors.rs 的 test_parse_record_timestamp_validation
}
```

---

### `src/parser/builder.rs` (builder, request-response)

**Analog:** `src/parser.rs` lines 43-87

**Complete extract** — 直接从 `src/parser.rs` 剪切以下内容，只需更新 imports：

**Imports pattern**:
```rust
use std::fs;
use std::path::{Path, PathBuf};
use std::str;

use crate::error::ParseError;
use crate::parser::encoding::FileEncodingHint;
use crate::parser::LogParser;
```

**Core builder pattern** (from `src/parser.rs` lines 43-87, unchanged logic):
```rust
pub struct LogParserBuilder {
    path: PathBuf,
    encoding_hint: Option<FileEncodingHint>,
}

impl LogParserBuilder {
    pub fn new<P: AsRef<Path>>(path: P) -> Self { ... }
    pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self { ... }
    pub fn build(self) -> Result<LogParser, ParseError> { ... }
}
```

关键点：`build()` 内的 `LogParser { data, encoding }` 构造需要 `LogParser` 类型可见——因为 `builder.rs` 和 `mod.rs` 同属 `parser` 模块，直接用 `super::LogParser` 或在 mod.rs 中通过 `pub(super)` 暴露字段即可。

---

### `src/parser/iterator.rs` (iterator, streaming)

**Analog:** `src/parser.rs` lines 101-203, 412-437

**Imports pattern**:
```rust
use memchr::memmem::Finder;
use memchr::memchr;
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::filter::adapter;           // D-07: 委托调用
use crate::parser::encoding::FileEncodingHint;
use crate::record::Sqllog;
```

**LogIterator struct pattern** (from `src/parser.rs` lines 101-107):
```rust
pub struct LogIterator<'a> {
    data: &'a [u8],
    pos: usize,
    encoding: FileEncodingHint,
    line_number: u64,
}
```

**Filter delegation pattern** (D-07, replaces direct inline filter at lines 116-138):
```rust
impl<'a> LogIterator<'a> {
    pub fn skip_errors(self) -> impl Iterator<Item = Sqllog> + 'a {
        self.filter_map(Result::ok)
    }

    // D-07: 保持 pub（examples/ 使用此方法，Pitfall 5 决议）
    pub fn filter_by_exec_time(
        self,
        min_ms: u64,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::filter_by_exec_time(self, min_ms)
    }

    // D-07: 保持 pub（同理）
    pub fn filter_by_sql_contains(
        self,
        pattern: &str,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::filter_by_sql_contains(self, pattern)
    }
}
```

**Iterator impl pattern** (from `src/parser.rs` lines 140-203): 逻辑完全保留，调用
`super::parse_record_with_hint(...)` 或通过 `crate::parser::parse_record_with_hint`。

**Private helper constants** (from `src/parser.rs` lines 412-429):
```rust
// 时间戳 SIMD 验证常量和函数迁入此文件（就近原则 D-02）
const LO_MASK: u64 = 0xFF0000FF0000FFFF;
const LO_EXPECTED: u64 = 0x2D00002D00003032;
const HI_MASK: u64 = 0x0000FF0000FF0000;
const HI_EXPECTED: u64 = 0x00003A0000200000;

#[inline(always)]
fn is_timestamp_start(bytes: &[u8]) -> bool { ... }

#[cold]
fn make_invalid_format_error(raw_bytes: &[u8], line_number: u64) -> ParseError { ... }
```

---

### `src/parser/encoding.rs` (enum/config)

**Analog:** `src/parser.rs` lines 22-31

**Complete extract** — 仅需保留枚举定义，添加 mod-level use：
```rust
// src/parser/encoding.rs（无需额外 imports）

/// 文件编码提示，用于指示日志文件的字符编码。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FileEncodingHint {
    #[default]
    Auto,
    Utf8,
    Gb18030,
}
```

此文件无 imports，无私有函数，直接复制枚举定义即可。

---

### `src/record.rs` (model, transform)

**Analog:** `src/sqllog.rs` (直接重命名)

**操作：** `git mv src/sqllog.rs src/record.rs`，内容不变。

**验证路径更新清单**（搜索所有 `crate::sqllog` 和 `use crate::sqllog` 并改为 `crate::record`）：
- `src/parser.rs` lines 10-11: `use crate::sqllog` → `use crate::record`
- `src/parser.rs` lines 279-392: `sqllog::parse_meta_from_bytes` → `crate::record::parse_meta_from_bytes`

文件本身内容无需修改，Pitfall 6 的路径更新由拆分后的各文件处理。

---

### `src/filter/mod.rs` (module-entry)

**Analog:** `src/lib.rs` (module declaration pattern)

**Pattern**（极简，无 imports）:
```rust
// src/filter/mod.rs
pub(crate) mod adapter;
```

仅声明子模块，无其他内容（D-05）。

---

### `src/filter/adapter.rs` (utility, transform)

**Analog:** `src/parser.rs` lines 116-138 (inline filter methods)

**Imports pattern**:
```rust
use crate::error::ParseError;
use crate::record::Sqllog;
```

**Core generic iterator adapter pattern** (D-06，从 RESEARCH.md Pattern 3 提取，与 lines 116-138 逻辑等价):
```rust
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

注意：函数可见性为 `pub(crate)`，不对外暴露（D-08）。`filter/adapter.rs` 不引用任何 `parser::` 模块类型（防止 Pitfall 3 循环依赖）。

---

### `src/async_api/mod.rs` (placeholder)

**No analog** — 空文件，仅包含注释：
```rust
// Phase 12 will populate this module with async API implementation.
```

注意：`src/lib.rs` Phase 10 **不添加** `mod async_api;` 声明（D-12）。

---

### `src/lib.rs` (re-export, update)

**Analog:** `src/lib.rs` (current, lines 82-90)

**Current state** (lines 82-90):
```rust
pub(crate) mod error;
pub(crate) mod parser;
pub(crate) mod sqllog;        // ← 需改为 record

pub use error::ParseError;
pub use parser::{
    FileEncodingHint, LogIterator, LogParser, LogParserBuilder, parse_record,  // ← 移除 parse_record
};
pub use sqllog::Sqllog;       // ← 需改为 record::Sqllog
```

**Target state** (D-13/D-14/D-15):
```rust
pub(crate) mod error;
pub(crate) mod parser;
pub(crate) mod filter;        // ← 新增
pub(crate) mod record;        // ← sqllog → record

pub use error::ParseError;
pub use parser::{
    FileEncodingHint, LogIterator, LogParser, LogParserBuilder,
    // parse_record 不再重导出（D-14）
};
pub use record::Sqllog;       // ← 路径更新
```

rustdoc 示例（lines 1-80）保留不变。示例 2 中 `filter_by_exec_time` 调用仍有效，因 `LogIterator::filter_by_exec_time` 保持 `pub`。

---

## Tests Migration Map

### 迁入 `src/parser/mod.rs #[cfg(test)]` 的测试

| 来源文件 | 测试函数 | 迁移原因 |
|----------|---------|---------|
| `tests/performance_metrics.rs` | 全部 16 个测试（`performance_metrics_full` 等） | 全部调用 `parse_record` |
| `tests/parser_coverage.rs` | `parse_record_single_line_no_newline`、`parse_record_no_meta_open_paren`、`parse_record_no_meta_close_paren` | 调用 `parse_record` |
| `tests/edge_cases.rs` | `meta_closing_paren_without_space_then_body_on_next_line`、`appname_empty_then_take_next_token_as_appname_not_ip`、`indicators_not_strictly_formatted_should_not_split_body` | 调用 `parse_record` |
| `tests/sqllog_additional.rs` | 全部（`body_without_indicators` 等，含 GB18030 测试） | 全部调用 `parse_record`；GB18030 测试用 `encoding` dev-dep，在 `#[cfg(test)]` 内可用 |
| `tests/parser_errors.rs` | `test_parse_record_timestamp_validation` | 调用 `parse_record` |

### 保留在 `tests/` 的测试（无需修改）

| 文件 | 保留原因 |
|------|---------|
| `tests/parser_coverage.rs` — `iterator_skips_leading_blank_line`、`crlf_in_multiline_first_line` | 仅用 `LogParserBuilder` + `LogParser::iter()` |
| `tests/edge_cases.rs` — `probable_record_start_line_and_iterator_singleline_detection` | 仅用 `LogParserBuilder` |
| `tests/parser_errors.rs` — 其余 8 个测试 | 仅用 `LogParserBuilder`、`ParseError` 公开 API |
| `tests/integration_test.rs` | 仅用公开 API |
| `tests/parser_filters.rs` | 仅用公开 API |
| `tests/parser_iterator.rs` | 仅用公开 API |

---

## Shared Patterns

### pub(crate) 可见性模式
**Source:** `src/sqllog.rs` lines 59, 147, 227（已建立）
**Apply to:** `parse_record`（降级）、`filter_by_exec_time` / `filter_by_sql_contains`（降级，仅 `adapter.rs` 中的函数版本）
```rust
// 降级前（pub）：
pub fn parse_record(...)
// 降级后（pub(crate)）：
pub(crate) fn parse_record(...)
```

### 跨模块 crate:: 路径规则
**Apply to:** 所有 `parser/` 子文件
```rust
// 任意 parser/ 子文件中引用其他模块
use crate::record::Sqllog;
use crate::error::ParseError;
use crate::filter::adapter;           // 仅 iterator.rs 使用
use crate::parser::encoding::FileEncodingHint;  // builder.rs 使用
```

### #[cfg(test)] 内部测试模式
**Source:** `src/parser.rs` lines 441-521（已建立）
**Apply to:** `src/parser/mod.rs` 的测试迁移块
```rust
#[cfg(test)]
mod tests {
    use super::*;   // 访问 parse_record（pub(crate)）和私有函数
    // 迁入的测试用 parse_record(&raw) 语法不变，无需修改测试逻辑
}
```

### mod.rs 子模块声明 + pub use 提升模式
**Source:** `src/lib.rs` lines 82-90（已建立）
**Apply to:** `src/parser/mod.rs`（为 lib.rs 提升 `FileEncodingHint`）
```rust
// parser/mod.rs 必须包含此行，否则 lib.rs 的 pub use parser::FileEncodingHint 失败
pub use encoding::FileEncodingHint;
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `src/async_api/mod.rs` | placeholder | — | 纯空占位文件，Phase 12 才填充；无任何类似空模块可参照 |

---

## Key Pitfalls (for planner reference)

1. **Pitfall 3 — 循环依赖**：`filter/adapter.rs` 只能用泛型 `I: Iterator<...>`，禁止引用 `LogIterator`
2. **Pitfall 5 — examples 可见性**：`LogIterator::filter_by_exec_time` 保持 `pub`（`examples/filter_slow_queries.rs` line 14 直接调用）；`filter/adapter.rs` 中的函数版本为 `pub(crate)`
3. **Pitfall 6 — sqllog 路径残留**：拆分时全局替换 `crate::sqllog` → `crate::record`，`use crate::sqllog` → `use crate::record`
4. **迁移顺序**：先迁移测试到 `#[cfg(test)]`，再将 `parse_record` 降为 `pub(crate)`，否则 `tests/` 中编译失败

---

## Metadata

**Analog search scope:** `src/`, `tests/`, `examples/`
**Files scanned:** `src/parser.rs` (521L), `src/sqllog.rs` (284L), `src/lib.rs` (91L), `src/error.rs` (37L), `tests/parser_coverage.rs`, `tests/performance_metrics.rs`, `tests/edge_cases.rs`, `tests/sqllog_additional.rs`, `tests/parser_errors.rs`, `examples/filter_slow_queries.rs`
**Pattern extraction date:** 2026-05-22
