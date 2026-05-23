# Phase 12: AsyncAPI - Pattern Map

**Mapped:** 2026-05-23
**Files analyzed:** 3
**Analogs found:** 3 / 3

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/async_api/mod.rs` | service (async wrapper) | request-response (spawn_blocking) | `src/parser/builder.rs` | role-match (同为 builder 模式委托层) |
| `Cargo.toml` | config | — | `Cargo.toml` (current) | exact (直接修改) |
| `src/lib.rs` | config (re-export) | — | `src/lib.rs` (current) | exact (直接修改) |

---

## Pattern Assignments

### `src/async_api/mod.rs` (async wrapper service, request-response)

**Analog:** `src/parser/builder.rs` (builder 结构 + 委托模式) + `src/filter/builder.rs` (测试中的 `#[cfg(test)] mod tests` 结构)

**关键约束（来自代码库实测）：**

1. `LogIterator<'a>` 有生命周期参数，绑定到 `LogParser` 的 `data: &'a [u8]`。在 `spawn_blocking` 闭包内，`LogParser` 和 `LogIterator` 均在闭包内就地创建和消费，不跨线程边界，因此 `'static` 约束不是问题。
2. `FileEncodingHint` 实现了 `#[derive(Copy, Clone)]`（见 `src/parser/encoding.rs` 第 2 行），可直接 `move` 进闭包，无需 `Clone` 调用。
3. `filter::adapter::apply_filter` 是 `pub(crate)` 函数，不能从 `async_api` 模块直接调用——必须通过 `LogIterator::apply_filter()` 公开方法（见 `src/parser/iterator.rs` 第 68-73 行）。
4. `ParseError` 没有实现 `std::error::Error`（仅用 `thiserror::Error` derive），`AsyncError` 需要单独为 `std::error::Error` derive（`thiserror::Error` 自动实现它）。

**Imports pattern**（参考 `src/parser/builder.rs` 第 1-7 行 + `src/parser/iterator.rs` 第 1-9 行）：

```rust
use std::path::{Path, PathBuf};

use crate::error::ParseError;
use crate::filter::builder::Filter;
use crate::parser::builder::LogParserBuilder;
use crate::parser::encoding::FileEncodingHint;
use crate::record::Sqllog;
```

**Builder struct pattern**（参考 `src/parser/builder.rs` 第 10-28 行）：

```rust
// LogParserBuilder 的字段持有方式：
pub struct LogParserBuilder {
    path: PathBuf,               // 持有 PathBuf 而非 &Path
    encoding_hint: Option<FileEncodingHint>,
}
impl LogParserBuilder {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),  // 立即转换为 PathBuf
            encoding_hint: None,
        }
    }
    pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
        self.encoding_hint = Some(hint);
        self
    }
}
```

`AsyncLogParser` 直接复制此模式，但 `encoding_hint` 字段类型改为 `FileEncodingHint`（非 `Option`），默认值 `FileEncodingHint::Auto`（`encoding.rs` 第 4 行有 `#[default]` 标注）。

**`spawn_blocking` 核心 pattern**（`LogParser` + `LogIterator` 的正确使用方式）：

```rust
// 关键：LogParser 必须在闭包内创建，因为 iter() 返回的 LogIterator<'_>
// 生命周期绑定到 LogParser，整个解析生命周期在闭包内完成
tokio::task::spawn_blocking(move || {
    let parser = LogParserBuilder::new(&path)
        .encoding_hint(encoding_hint)   // FileEncodingHint 是 Copy
        .build()?;                       // 失败返回 ParseError::IoError
    let iter = parser.iter();            // LogIterator<'_> 生命周期绑定到 parser
    let records: Vec<Sqllog> = if let Some(filter) = filter {
        iter.apply_filter(filter)        // 通过 LogIterator 公开方法调用
            .filter_map(Result::ok)
            .collect()
    } else {
        iter.filter_map(Result::ok).collect()
    };
    Ok::<_, ParseError>(records)
})
.await
.map_err(|join_err| AsyncError::Panic(join_err.to_string()))?
.map_err(AsyncError::Parse)
```

**Error type pattern**（参考 `src/error.rs` 第 9-36 行，`thiserror` 用法）：

```rust
// error.rs 中 ParseError 的 thiserror 写法：
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseError {
    #[error("IO error: {0}")]
    IoError(String),
}

// AsyncError 跟随同样惯用法，但不需要 Clone/PartialEq（跨线程语义）：
#[derive(Debug, thiserror::Error)]
pub enum AsyncError {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),    // #[from] 自动生成 From<ParseError> impl

    #[error("blocking task panicked: {0}")]
    Panic(String),
}
// 注意：#[from] 已生成 From<ParseError>，手写 impl From 会导致冲突
// CONTEXT.md D-04 要求 From<ParseError> 实现，通过 #[from] 属性完成，不需要额外 impl 块
```

**Test structure pattern**（参考 `src/filter/builder.rs` 第 451-469 行）：

```rust
// filter/builder.rs 中测试块的写法（async_api 测试跟随相同结构）：
#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Sqllog;

    // 辅助函数写在测试块内
    fn make_record() -> Sqllog { ... }

    #[test]
    fn test_xxx() { ... }
}

// async_api/mod.rs 的测试块扩展：
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use crate::filter::builder::FilterBuilder;

    fn write_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[tokio::test]   // 需要 dev-dependencies tokio with features = ["rt", "macros"]
    async fn test_parse_returns_records() { ... }
}
```

**tempfile 使用 pattern**（参考 `src/parser/mod.rs` 第 254-270 行）：

```rust
// 项目已有的 NamedTempFile 用法：
let mut tmp = NamedTempFile::new().expect("tmp");
write!(tmp, "...log content...").unwrap();
tmp.as_file().sync_all().unwrap();
let parser = LogParserBuilder::new(tmp.path()).build().expect("build");
```

---

### `Cargo.toml` (config)

**Analog:** `Cargo.toml` 当前文件（直接修改）

**当前 `[dependencies]` 结构**（第 39-44 行）：

```toml
[dependencies]
atoi = "2.0.0"
memchr = "2.7.6"
thiserror = "2.0.17"
encoding = "0.2"
```

**当前 `[dev-dependencies]` 结构**（第 46-48 行）：

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports", "plotters"] }
tempfile = "3.8"
```

**需要添加的内容（精确位置）：**

在 `[dependencies]` 块末尾追加：
```toml
tokio = { version = "1", features = ["rt"], optional = true }
```

在 `[dev-dependencies]` 块末尾追加：
```toml
tokio = { version = "1", features = ["rt", "macros"] }
```

在 `[dev-dependencies]` 之后（或 `[[bench]]` 之前）新增节：
```toml
[features]
async = ["tokio/rt"]
```

**注意：** dev-dependencies 中的 tokio 声明不加 `optional = true`，确保 `cargo test` 时始终可用 `#[tokio::test]`，而不依赖 feature flag 状态。

---

### `src/lib.rs` (config, re-export)

**Analog:** `src/lib.rs` 当前文件（直接修改）

**当前模块声明结构**（第 82-90 行）：

```rust
pub(crate) mod error;
pub(crate) mod filter;
pub(crate) mod parser;
pub(crate) mod record;

pub use error::ParseError;
pub use filter::{Filter, FilterBuilder};
pub use parser::{FileEncodingHint, LogIterator, LogParser, LogParserBuilder};
pub use record::Sqllog;
```

**需要添加的内容（追加到文件末尾，第 91 行起）：**

```rust
#[cfg(feature = "async")]
pub mod async_api;
#[cfg(feature = "async")]
pub use async_api::{AsyncError, AsyncLogParser};
```

**注意：** 现有模块用 `pub(crate) mod`，而 `async_api` 需要用 `pub mod`（因为 `pub use` 要求模块本身可见），但受 `#[cfg(feature = "async")]` 守卫控制，不影响默认构建。

---

## Shared Patterns

### Builder 模式（消费 self 的方法链）

**Source:** `src/parser/builder.rs` 第 23-28 行 + `src/filter/builder.rs` 第 61-68 行

```rust
// 两个现有 builder 都用消费 self 的链式方法：
pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
    self.encoding_hint = Some(hint);
    self
}
// AsyncLogParser 完全跟随此惯用法
```

**Apply to:** `AsyncLogParser::encoding_hint()` 和 `AsyncLogParser::with_filter()`

### `thiserror::Error` derive 模式

**Source:** `src/error.rs` 第 15-36 行

```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseError {
    #[error("invalid format at line {line_number} | raw: {raw}")]
    InvalidFormat { raw: String, line_number: u64 },
    #[error("file not found or inaccessible: {path}")]
    FileNotFound { path: String },
    #[error("IO error: {0}")]
    IoError(String),
}
```

**Apply to:** `AsyncError` 跟随相同 derive 风格，但使用 `#[from]` 属性（`thiserror` 功能）自动生成 `From<ParseError>`

### `#[cfg(not(miri))]` 测试守卫

**Source:** `src/parser/mod.rs` 第 249 行、第 584 行、第 653 行

```rust
#[cfg(not(miri))]
#[test]
fn test_builder_encoding_hint_utf8() { ... }
```

**Apply to:** 所有 `AsyncLogParser` 测试中使用 `NamedTempFile` 的 `#[tokio::test]` 均应加 `#[cfg(not(miri))]`，与现有文件 I/O 测试的惯例一致

### `apply_filter` 调用路径

**Source:** `src/parser/iterator.rs` 第 68-73 行

```rust
// LogIterator 的公开方法（pub，可从 async_api 访问）：
pub fn apply_filter(
    self,
    filter: Filter,
) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
    adapter::apply_filter(self, filter)  // 实际实现在 pub(crate) adapter 中
}
```

**Apply to:** `AsyncLogParser::parse()` 的 `spawn_blocking` 闭包内调用 `iter.apply_filter(filter)` 而不是 `adapter::apply_filter(iter, filter)`（后者是 `pub(crate)`，从 `async_api` 无法访问）

---

## No Analog Found

无。所有三个文件都有明确的现有代码模式可参考。

---

## Critical Notes for Planner

1. **`#[from]` 与手写 `impl From` 不能共存：** CONTEXT.md D-04 要求实现 `From<ParseError>`，通过 `#[from]` 属性在 `AsyncError::Parse` 变体上实现，不需要也不能再写手动 `impl From<ParseError> for AsyncError { ... }` 块（编译错误：duplicate impl）。

2. **`LogIterator` 生命周期在闭包内自洽：** `LogParser::iter()` 返回 `LogIterator<'_>`，其生命周期绑定到 `parser` 局部变量。因为 `parser`、`iter`、`records` 全在 `spawn_blocking` 闭包内创建和使用，`Vec<Sqllog>` 收集完成后 `iter` 和 `parser` 就被 drop，不存在生命周期逃逸问题。

3. **`FileEncodingHint` 默认值：** `encoding.rs` 第 4 行有 `#[default]` 标注在 `Auto` 变体上，可以用 `FileEncodingHint::default()` 或直接 `FileEncodingHint::Auto` 作为 `AsyncLogParser` 的默认编码，两者等价。

4. **测试文件内容格式：** 从 `src/parser/mod.rs` 测试可见，标准测试记录格式为：`"2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1"`，末尾不需要换行符（单条记录）或需要 `\n` 分隔多条记录。

---

## Metadata

**Analog search scope:** `src/parser/`, `src/filter/`, `src/error.rs`, `src/lib.rs`, `Cargo.toml`
**Files scanned:** 8
**Pattern extraction date:** 2026-05-23
