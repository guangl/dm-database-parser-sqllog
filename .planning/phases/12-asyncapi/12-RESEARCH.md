# Phase 12: AsyncAPI - Research

**Researched:** 2026-05-23
**Domain:** tokio async wrapper, optional feature flag, Rust async patterns
**Confidence:** HIGH

## Summary

Phase 12 在已完成的 Phase 10（重构）和 Phase 11（FilterBuilder）基础上添加异步 API 层。核心实现极为简洁：一个独立的 `AsyncLogParser` 结构体，通过 `tokio::task::spawn_blocking` 封装现有同步解析路径，返回 `Vec<Sqllog>`（所有字段均为 `String`，天然满足 `'static + Send`）。

关键设计约束已在 CONTEXT.md 中锁定：`AsyncLogParser` 独立于 `LogParserBuilder`，内部委托给 `LogParserBuilder` 实现，不重写解析逻辑。Filter 在 `spawn_blocking` 闭包内执行，减少跨线程传递的数据量。tokio 通过 `features = ["async"]` 可选引入，只拉取 `rt` 子特性。

当前代码库状态：`src/async_api/mod.rs` 是 Phase 10 创建的占位文件（单行注释），`lib.rs` 中 `async_api` 声明尚未添加（CONTEXT.md D-12 确认），`Cargo.toml` 尚无 tokio 依赖。所有前置条件均已满足，Phase 12 可直接实现。

**Primary recommendation:** 单 plan 完成全部实现（`AsyncLogParser` + `AsyncError` + Cargo.toml 更新 + lib.rs 声明 + 单元测试），无外部阻塞项。

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** 独立 `AsyncLogParser` struct，完整 builder 链：`new(path)` → `.encoding_hint(hint)` → `.with_filter(filter)` → `.parse().await`
- **D-02:** `parse()` 返回 `Result<Vec<Sqllog>, AsyncError>`；单条解析失败静默丢弃（`filter_map(Result::ok)`），仅 I/O 或 panic 级别错误通过 `AsyncError` 传播
- **D-03:** `pub enum AsyncError { Parse(ParseError), Panic(String) }` 定义在 `src/async_api/mod.rs`
- **D-04:** `impl From<ParseError> for AsyncError`，内部 `?` 可直接传播 `ParseError`
- **D-05:** `.with_filter(filter: Filter)` 方法链传入过滤条件，filter 在 `spawn_blocking` 内部执行后再返回 async 上下文
- **D-06:** 不传 filter 时等价于收集所有成功解析记录
- **D-07:** `Cargo.toml` 中 `tokio = { version = "1", features = ["rt"], optional = true }` + `[features] async = ["tokio/rt"]`
- **D-08:** dev-dependencies 中 `tokio = { version = "1", features = ["rt", "macros"] }`（不对库用户暴露）
- **D-09:** 所有实现放在 `src/async_api/mod.rs`，包括 `#[cfg(test)]` 单元测试块
- **D-10:** `lib.rs` 添加 `#[cfg(feature = "async")] pub mod async_api;` 和 `pub use async_api::{AsyncLogParser, AsyncError};`
- **D-11:** 继承 `encoding_hint` 能力，委托给 `LogParserBuilder::encoding_hint()`

### Claude's Discretion

- `AsyncError` 实现 `std::error::Error` trait（已决定实现）
- `spawn_blocking` 内 builder 初始化失败通过 `AsyncError::Parse(ParseError::IoError(...))` 传播
- 返回类型满足 `Send`（`Vec<Sqllog>` 全字段为 `String`）

### Deferred Ideas (OUT OF SCOPE)

- Stream API（STREAM-01/02）：`impl Stream<Item = Sqllog>` 逐条异步处理、背压支持
- Phase 13–15：不存在于当前 ROADMAP

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ASYNC-01 | 用户可在 `async fn` 中直接 `await` 解析整个日志文件，无需手写 `spawn_blocking` | `AsyncLogParser::parse().await` 封装实现，已确认 tokio::task::spawn_blocking API |
| ASYNC-02 | 内部使用 `tokio::task::spawn_blocking` 封装同步 mmap 解析，不破坏零拷贝内核路径 | spawn_blocking 要求闭包 `'static + Send`，Sqllog 全字段 String 满足，LogParserBuilder::build() 内部 `fs::read` 即同步路径 |
| ASYNC-03 | tokio 依赖通过 `features = ["async"]` 可选引入，不使用异步 API 的用户无需依赖 tokio | Cargo.toml optional = true + [features] 方案已验证，只需 rt 子特性 |
| ASYNC-04 | 异步 API 支持过滤条件传入（与 FilterBuilder 集成） | Filter 已满足 Send + Sync（Phase 11 确认），spawn_blocking 闭包内 apply_filter 可直接收集 Vec<Sqllog> |

</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 异步接口封装 | `src/async_api/mod.rs` | — | 独立模块，受 `#[cfg(feature = "async")]` 守卫 |
| 实际文件解析 | `src/parser/` (LogParserBuilder) | — | Phase 12 零修改复用，async 层仅委托 |
| 过滤逻辑 | `src/filter/adapter.rs` (apply_filter) | `src/async_api/mod.rs` (调用点) | spawn_blocking 闭包内调用已有适配器 |
| 公开类型导出 | `src/lib.rs` | — | `#[cfg(feature = "async")]` 守卫下添加 mod + pub use |
| tokio 运行时 | 调用方（用户侧） | tokio rt feature | 库本身只引入 rt，不创建全局运行时 |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.52.3 | async 运行时，提供 spawn_blocking + JoinHandle | Rust async 生态事实标准，688M+ crates.io 下载量，github.com/tokio-rs/tokio |

[VERIFIED: crates.io registry + cargo search]

### Existing (No Change)

| Library | Version | Purpose |
|---------|---------|---------|
| thiserror | 2.0.18 | AsyncError derive thiserror::Error（已在 Cargo.toml） |
| tempfile | 3.8 | 测试用临时文件（已在 dev-dependencies） |

[VERIFIED: Cargo.toml + cargo search]

### Supporting (dev-dependencies only)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio (macros feature) | 1.52.3 | `#[tokio::test]` 宏，仅测试 | `[dev-dependencies]` 中额外声明 |

[VERIFIED: docs.rs/tokio feature flags]

**Cargo.toml 变更（最小集）：**
```toml
[dependencies]
tokio = { version = "1", features = ["rt"], optional = true }

[features]
async = ["tokio/rt"]

[dev-dependencies]
tokio = { version = "1", features = ["rt", "macros"] }
```

**注意：** dev-dependencies 中的 tokio 声明不会影响库用户的依赖树，仅在 `cargo test` 时引入。

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| tokio | crates.io | ~6 年 | 688,790,136 | github.com/tokio-rs/tokio | N/A (slopcheck unavailable) | Approved — [VERIFIED: crates.io] |

**slopcheck unavailable** — tokio 是 Rust async 生态的核心 crate，下载量排名极高，官方仓库明确（tokio-rs 组织），无需额外人工验证。

**Packages removed due to slopcheck [SLOP] verdict:** none

**Packages flagged as suspicious [SUS]:** none

*slopcheck was unavailable at research time. tokio 的可信度基于独立来源（crates.io 官方 API + cargo search + 官方文档）交叉验证，风险极低。*

## Architecture Patterns

### System Architecture Diagram

```
用户 async fn
      |
      | AsyncLogParser::new(path).with_filter(f).parse().await
      v
AsyncLogParser::parse()
      |
      | tokio::task::spawn_blocking(move || { ... })
      v
[blocking thread pool]
      |
      | LogParserBuilder::new(path).encoding_hint(hint).build()?
      v
LogParser (同步解析，fs::read 读入内存)
      |
      | parser.iter()  →  LogIterator
      |
      | [if filter] iter.apply_filter(filter).filter_map(Result::ok).collect()
      | [no filter]  iter.filter_map(Result::ok).collect()
      v
Vec<Sqllog>  (owned, 'static)
      |
      | Ok::<_, ParseError>(records)
      v
JoinHandle<Result<Vec<Sqllog>, ParseError>>.await
      |
      | map JoinError::is_panic → AsyncError::Panic(msg)
      | map ParseError         → AsyncError::Parse(e)
      v
Result<Vec<Sqllog>, AsyncError>  →  用户
```

### Recommended Project Structure

```
src/
├── async_api/
│   └── mod.rs       # AsyncLogParser + AsyncError + #[cfg(test)]（Phase 12 填充）
├── filter/
│   ├── adapter.rs   # apply_filter / apply_filter_keep_errors（已实现）
│   ├── builder.rs   # Filter + FilterBuilder（已实现）
│   └── mod.rs       # 重导出（已实现）
├── parser/
│   ├── builder.rs   # LogParserBuilder（已实现）
│   ├── encoding.rs  # FileEncodingHint（已实现）
│   ├── iterator.rs  # LogIterator（已实现）
│   └── mod.rs       # LogParser（已实现）
├── error.rs         # ParseError（已实现）
├── lib.rs           # 重导出（Phase 12 添加 async_api 声明）
└── record.rs        # Sqllog（已实现）
```

### Pattern 1: spawn_blocking + Result 封装

**What:** 在异步函数中用 spawn_blocking 执行同步阻塞代码，处理 JoinError（panic）
**When to use:** 所有需要封装同步 IO 或 CPU 密集操作的 async 接口

```rust
// Source: docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
use tokio::task;

pub async fn parse(self) -> Result<Vec<Sqllog>, AsyncError> {
    let path = self.path;
    let encoding_hint = self.encoding_hint;
    let filter = self.filter;

    task::spawn_blocking(move || {
        let parser = LogParserBuilder::new(&path)
            .encoding_hint(encoding_hint)
            .build()?;
        let iter = parser.iter();
        let records: Vec<Sqllog> = if let Some(f) = filter {
            iter.apply_filter(f).filter_map(Result::ok).collect()
        } else {
            iter.filter_map(Result::ok).collect()
        };
        Ok::<_, ParseError>(records)
    })
    .await
    .map_err(|join_err| {
        // JoinError = 任务 panic（spawn_blocking 不会 cancel）
        let msg = if join_err.is_panic() {
            join_err
                .into_panic()
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    // &str panic message
                    // into_panic() consumes, must use try_into_panic pattern
                    None
                })
                .unwrap_or_else(|| "unknown panic".to_string())
        } else {
            join_err.to_string()
        };
        AsyncError::Panic(msg)
    })?
    .map_err(AsyncError::Parse)
}
```

**注意：** `join_err.into_panic()` 消费 JoinError，返回 `Box<dyn Any + Send + 'static>`，需要 `downcast_ref::<String>()` 再 `downcast_ref::<&str>()` 两步尝试。更简洁的方式是直接用 `join_err.to_string()`（JoinError 实现了 Display，输出格式为 "task <id> panicked with ..."）。

### Pattern 2: JoinError panic 消息提取（简化版）

```rust
// 推荐：直接用 Display，避免 Any downcast 复杂性
.map_err(|join_err| AsyncError::Panic(join_err.to_string()))
```

`JoinError` 的 `Display` 实现会包含 panic 信息的字符串表示，对调试足够，且代码最简洁。[VERIFIED: docs.rs/tokio/latest/tokio/task/struct.JoinError.html]

### Pattern 3: #[cfg(feature = "async")] 守卫

```rust
// src/lib.rs 中的声明方式
#[cfg(feature = "async")]
pub mod async_api;
#[cfg(feature = "async")]
pub use async_api::{AsyncLogParser, AsyncError};
```

```rust
// src/async_api/mod.rs 中的 import 方式
#[cfg(feature = "async")]  // 整个文件只在 feature 启用时编译
use tokio::task;
use crate::parser::builder::LogParserBuilder;
use crate::filter::builder::Filter;
use crate::record::Sqllog;
use crate::error::ParseError;
```

**注意：** `src/async_api/mod.rs` 文件本身不需要 `#[cfg(feature = "async")]` 在文件内部标注，因为 `lib.rs` 中的 `#[cfg(feature = "async")] pub mod async_api;` 已经在 feature 未启用时完全排除该模块的编译。

### Pattern 4: tokio::test 单元测试

```rust
// Source: docs.rs/tokio/latest/tokio/attr.test.html
// 需要 dev-dependencies 中的 tokio with features = ["rt", "macros"]
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_parse_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1").unwrap();
        
        let records = AsyncLogParser::new(file.path())
            .parse()
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].sql, "SELECT 1");
    }

    #[tokio::test]
    async fn test_parse_with_filter() {
        // ... 测试 with_filter() 路径
    }
}
```

**重要：** `#[tokio::test]` 只能在 `[dev-dependencies]` 中有 `tokio = { features = ["rt", "macros"] }` 时才能编译。测试文件本身需要用 `#[cfg(feature = "async")]` 包裹，或放在 `src/async_api/mod.rs` 内部 `#[cfg(test)]` 块（该模块已受 feature 守卫）。

### Anti-Patterns to Avoid

- **在 spawn_blocking 内使用借用引用：** 闭包必须是 `'static`，不能捕获 `&str` 或 `&Path`。路径和 filter 必须 move 进闭包（PathBuf 而非 &Path）。
- **在 async fn 内直接调用 LogParserBuilder::build()：** `fs::read` 是阻塞 IO，会阻塞 tokio 线程，必须在 `spawn_blocking` 内执行。
- **重复实现解析逻辑：** Phase 12 的 async API 必须复用 `LogParserBuilder`，不重写解析代码。

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 异步 IO 运行时 | 自定义 epoll/kqueue 封装 | tokio spawn_blocking | mmap/read 是同步内核调用，无法真正异步化 |
| panic 捕获 | `std::panic::catch_unwind` 包裹 | `spawn_blocking` 返回 `JoinError` | spawn_blocking 自动捕获 panic，用 JoinError 传递 |
| Send/Sync 验证 | 运行时检查 | 编译期：`Sqllog` 所有字段 `String` | Rust 类型系统自动推断 Send + Sync，无需手动标记 |
| 错误类型转换 | 手写 match 分支 | `impl From<ParseError> for AsyncError` + `?` | 标准 Rust 错误转换惯用法 |

**Key insight:** `spawn_blocking` 是 Rust 生态中封装同步阻塞操作为 async 的标准做法，不要尝试自己用 `thread::spawn` 或 channel 重新实现。

## Common Pitfalls

### Pitfall 1: spawn_blocking 要求闭包 'static

**What goes wrong:** 编译错误 "closure may outlive the current function / borrowed value does not live long enough"

**Why it happens:** `spawn_blocking` 的闭包签名是 `F: FnOnce() -> R + Send + 'static`，不能捕获任何借用。

**How to avoid:** 确保 `AsyncLogParser` 的 `parse(self)` 消费 self（不是 `&self`），所有字段通过 `move` 闭包传入（`PathBuf` 而非 `&Path`，`Option<Filter>` 而非 `Option<&Filter>`）。

**Warning signs:** 编译器报 lifetime 错误，提示 "closure may outlive"。

### Pitfall 2: dev-dependencies tokio 与 library dependency 声明冲突

**What goes wrong:** `#[tokio::test]` 在 feature `async` 未启用时报编译错误，因为 `tokio` 在 `[dependencies]` 中是 optional，但 `[dev-dependencies]` 中已有非 optional 声明。

**Why it happens:** Cargo 对同一 crate 在 dependencies 和 dev-dependencies 中的特性处理规则：dev-dependencies 的声明会合并到测试构建中，但 optional dependency 只有在 feature 启用时才加入。

**How to avoid:** 按 D-08 方案：`[dev-dependencies]` 中单独声明 `tokio = { version = "1", features = ["rt", "macros"] }`（不加 `optional = true`）。这样 `cargo test` 时 tokio 始终可用，`cargo build`（不带 test）时 tokio 只在 `features = ["async"]` 时引入。

**Verification:** `cargo build` 后 `cargo metadata --features ""` 确认 tokio 不在依赖树中；`cargo test --features async` 确认 tokio::test 可用。

### Pitfall 3: JoinError panic 消息提取复杂性

**What goes wrong:** `into_panic()` 返回 `Box<dyn Any + Send>`，downcast 逻辑繁琐，且 `into_panic()` 消费了 JoinError（无法重试 downcast）。

**Why it happens:** Rust panic payload 可以是任意类型（`panic!("str")` 是 `&str`，`panic!("{}", x)` 是 `String`）。

**How to avoid:** 直接用 `join_err.to_string()` 作为 `AsyncError::Panic` 的消息。JoinError 的 Display 实现包含 panic 信息，对调试足够。若需要精确类型，用 `try_into_panic()` + 两步 downcast。

### Pitfall 4: 测试需要 feature flag 激活

**What goes wrong:** `cargo test` 运行时 `#[tokio::test]` 测试被编译但 `tokio` 找不到（或 async_api 模块未编译）。

**Why it happens:** 整个 `src/async_api/mod.rs` 在 `feature = "async"` 未启用时不参与编译。

**How to avoid:** 运行 `cargo test --features async` 来跑 async 相关测试。在 CI/CLAUDE.md 中记录此命令。Coverage 命令：`cargo llvm-cov --workspace --all-features --fail-under-lines 90`（`--all-features` 会启用 `async` feature）。

### Pitfall 5: LogParserBuilder.encoding_hint() 期望 Option vs direct value

**What goes wrong:** `AsyncLogParser` 需要持有 `FileEncodingHint`，但 `LogParserBuilder::encoding_hint()` 接受具体值（非 `Option`），而 `AsyncLogParser` 内部可能需要默认值逻辑。

**Why it happens:** 从代码实测：`LogParserBuilder` 内部 `encoding_hint: Option<FileEncodingHint>`，`.encoding_hint(hint)` 设置为 `Some(hint)`；不调用时自动探测。

**How to avoid:** `AsyncLogParser` 持有 `encoding_hint: FileEncodingHint`（带默认值 `FileEncodingHint::Auto`），`parse()` 内 `LogParserBuilder::new(path).encoding_hint(self.encoding_hint).build()`。`FileEncodingHint::Auto` 表示自动探测，与不调用 `.encoding_hint()` 等效。

需确认 `FileEncodingHint` 有 `Auto` 变体（从 `parser/builder.rs` 代码可见 `Auto` 分支处理）。[VERIFIED: src/parser/builder.rs 代码检查]

## Code Examples

### AsyncLogParser 完整实现骨架

```rust
// src/async_api/mod.rs
// Source: CONTEXT.md D-01~D-11 + docs.rs/tokio spawn_blocking

use std::path::{Path, PathBuf};

use crate::error::ParseError;
use crate::filter::builder::Filter;
use crate::parser::builder::LogParserBuilder;
use crate::parser::encoding::FileEncodingHint;
use crate::record::Sqllog;

/// 异步日志解析器。
///
/// 通过 builder 链配置，调用 `.parse().await` 得到解析结果。
/// 内部使用 `tokio::task::spawn_blocking` 封装同步解析路径。
pub struct AsyncLogParser {
    path: PathBuf,
    encoding_hint: FileEncodingHint,
    filter: Option<Filter>,
}

impl AsyncLogParser {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            encoding_hint: FileEncodingHint::Auto,
            filter: None,
        }
    }

    pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
        self.encoding_hint = hint;
        self
    }

    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    pub async fn parse(self) -> Result<Vec<Sqllog>, AsyncError> {
        let path = self.path;
        let encoding_hint = self.encoding_hint;
        let filter = self.filter;

        tokio::task::spawn_blocking(move || {
            let parser = LogParserBuilder::new(&path)
                .encoding_hint(encoding_hint)
                .build()?;
            let iter = parser.iter();
            let records: Vec<Sqllog> = if let Some(f) = filter {
                iter.apply_filter(f).filter_map(Result::ok).collect()
            } else {
                iter.filter_map(Result::ok).collect()
            };
            Ok::<_, ParseError>(records)
        })
        .await
        .map_err(|join_err| AsyncError::Panic(join_err.to_string()))?
        .map_err(AsyncError::Parse)
    }
}

/// 异步解析错误类型。
#[derive(Debug, thiserror::Error)]
pub enum AsyncError {
    /// 解析阶段 I/O 或格式错误（文件找不到、读取失败等）。
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    /// spawn_blocking 内部发生 panic。
    #[error("blocking task panicked: {0}")]
    Panic(String),
}

impl From<ParseError> for AsyncError {
    fn from(err: ParseError) -> Self {
        AsyncError::Parse(err)
    }
}
```

### Cargo.toml 变更（最终态）

```toml
[dependencies]
# ... 已有依赖 ...
tokio = { version = "1", features = ["rt"], optional = true }

[features]
async = ["tokio/rt"]

[dev-dependencies]
# ... 已有 dev-dependencies ...
tokio = { version = "1", features = ["rt", "macros"] }
```

### lib.rs 新增行（Phase 12 唯一修改点）

```rust
// 在现有 pub(crate) mod 声明之后添加：
#[cfg(feature = "async")]
pub mod async_api;
#[cfg(feature = "async")]
pub use async_api::{AsyncLogParser, AsyncError};
```

### 测试结构（src/async_api/mod.rs 内部）

```rust
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

    // ASYNC-01: 基础 await 解析
    #[tokio::test]
    async fn test_parse_returns_records() {
        let file = write_test_file(
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1\n"
        );
        let records = AsyncLogParser::new(file.path()).parse().await.unwrap();
        assert_eq!(records.len(), 1);
    }

    // ASYNC-02: 底层使用同步路径（通过成功解析验证）
    #[tokio::test]
    async fn test_parse_file_not_found_returns_error() {
        let result = AsyncLogParser::new("/nonexistent/path.log").parse().await;
        assert!(matches!(result, Err(AsyncError::Parse(_))));
    }

    // ASYNC-03: feature flag 守卫（整个模块在 feature="async" 时编译）
    // 无需单独测试——能运行这些测试说明 feature 已启用

    // ASYNC-04: with_filter 集成
    #[tokio::test]
    async fn test_parse_with_filter() {
        let content = concat!(
            "2025-11-17 16:09:41.100 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1 EXECTIME: 200(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n",
            "2025-11-17 16:09:41.200 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 2 EXECTIME: 50(ms) ROWCOUNT: 1(rows) EXEC_ID: 2.\n",
        );
        let file = write_test_file(content);
        let filter = FilterBuilder::new().exec_time_gt(100.0).build();
        let records = AsyncLogParser::new(file.path())
            .with_filter(filter)
            .parse()
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].exectime > 100.0);
    }

    // encoding_hint 传递
    #[tokio::test]
    async fn test_encoding_hint_propagated() {
        use crate::parser::encoding::FileEncodingHint;
        let file = write_test_file(
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1\n"
        );
        let records = AsyncLogParser::new(file.path())
            .encoding_hint(FileEncodingHint::Utf8)
            .parse()
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
    }

    // AsyncError::Panic 变体（难以直接触发 spawn_blocking panic，使用属性测试覆盖接口）
    // 错误类型实现 std::error::Error
    #[test]
    fn test_async_error_is_error() {
        use std::error::Error;
        let err: &dyn Error = &AsyncError::Panic("test".to_string());
        assert!(err.to_string().contains("test"));
    }
}
```

### 用户侧使用示例（ROADMAP 验收标准）

```rust
// 与 CONTEXT.md specifics 中的示例对应
use dm_database_parser_sqllog::{AsyncLogParser, FilterBuilder};

async fn slow_queries(path: &str) -> Vec<Sqllog> {
    AsyncLogParser::new(path)
        .with_filter(FilterBuilder::new().exec_time_gt(100.0).build())
        .parse()
        .await
        .unwrap_or_default()
}
```

## Sqllog 'static 可行性分析

`Sqllog` 结构体定义（`src/record.rs`）：

```rust
pub struct Sqllog {
    pub ts: String,           // 'static
    pub tag: Option<String>,  // 'static
    pub ep: u8,               // 'static
    pub sess_id: String,      // ...
    pub thrd_id: String,
    pub username: String,
    pub trxid: String,
    pub statement: String,
    pub appname: String,
    pub client_ip: String,
    pub sql: String,
    pub exectime: f32,        // 'static
    pub rowcount: u32,        // 'static
    pub exec_id: i64,         // 'static
}
```

**结论：** 所有字段均为 `String`、`Option<String>` 或原始数值类型，自动满足：
- `'static`（无引用）
- `Send`（标准类型，无 `Rc` 等非 Send 类型）
- `Sync`（所有字段均 Sync）

[VERIFIED: src/record.rs 代码检查]

Phase 10 阶段原来的 `Cow<'a, str>` 零拷贝方案已在代码库中 **不存在**——实际代码直接使用 `String`（`String::from_utf8_lossy(...).into_owned()` 等），故 `'static` 转换问题不存在，`spawn_blocking` 闭包可直接 `collect::<Vec<Sqllog>>()`。

## FileEncodingHint 检查

```rust
// src/parser/encoding.rs（根据 builder.rs 引用路径）
// builder.rs 中: use crate::parser::encoding::FileEncodingHint;
// builder.rs 中有 FileEncodingHint::Utf8 和 Auto 分支
```

`FileEncodingHint` 有 `Auto`、`Utf8`、`Gb18030` 三个变体（从 `LogParserBuilder::build()` 的 match 分支可确认）。`AsyncLogParser` 默认使用 `Auto`，与不调用 `.encoding_hint()` 的 `LogParserBuilder` 行为一致。[VERIFIED: src/parser/builder.rs 代码检查]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 用户手写 `spawn_blocking` 样板 | `AsyncLogParser::parse().await` | Phase 12（本次） | 消除用户侧样板代码 |
| `Cow<'a, str>` 零拷贝字段 | `String`（owned）字段 | Phase 10 重构 | spawn_blocking 天然可行，无需转换 |
| 全量 tokio 依赖 | optional feature flag | Phase 12（本次） | 同步用户零依赖增加 |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `FileEncodingHint` 有 `Auto` 变体（从 builder.rs match 分支推断） | Code Examples | 若无 Auto 变体需调整 AsyncLogParser 默认值逻辑；低风险，可用 `None` + 条件调用替代 |

**注：** A1 可在实现前 `grep -r "FileEncodingHint" src/parser/encoding.rs` 确认。

## Open Questions

1. **`FileEncodingHint` 的确切变体列表**
   - What we know: `builder.rs` 中有 `Utf8`、`Auto`、`Gb18030` 分支（match 语句可见 `FileEncodingHint::Utf8`、`FileEncodingHint::Auto`、`FileEncodingHint::Gb18030`）
   - What's unclear: `encoding.rs` 文件未直接读取（但从 builder.rs 的 match 基本确认有三个变体）
   - Recommendation: 实现 Task 1 时先 `cargo check` 确认枚举变体，无实质不确定性

2. **`cargo test --features async` 与现有测试的兼容性**
   - What we know: `nyquist_validation: false`，无额外测试框架要求
   - What's unclear: 启用 async feature 后是否影响现有非 async 测试（理论上不影响，feature 是加法）
   - Recommendation: 在 Plan 验收时运行 `cargo test --features async`，再运行 `cargo test` 确认两者均通过

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo | 构建 + 测试 | ✓ | 内嵌 rust toolchain | — |
| cargo-llvm-cov | 覆盖率 ≥90% 验证 | ✓ | 0.8.5 | — |
| tempfile (dev-dep) | 测试临时文件 | ✓ | 3.8（Cargo.toml） | — |
| tokio | async 运行时 | ✗（未添加） | 1.52.3（crates.io 最新） | N/A — Phase 12 添加 |

**Missing dependencies with no fallback:** tokio（Cargo.toml 变更是 Plan 第一步，无阻塞）

**Missing dependencies with fallback:** None

## Validation Architecture

> nyquist_validation: false — 跳过此节

## Security Domain

Phase 12 不引入新的安全边界：
- 不处理用户身份验证或授权
- 不暴露网络接口
- 不引入新的文件读取路径（复用 `LogParserBuilder::build()` 中的 `fs::read`）
- `spawn_blocking` panic 捕获不会泄露敏感信息（AsyncError::Panic 只传递 panic 消息字符串）

唯一新攻击面：**panic 消息泄露**（若 panic 内容包含文件路径或敏感数据会暴露给调用方）。`JoinError::to_string()` 会包含 panic payload，属于正常调试信息，不视为安全问题。

## Sources

### Primary (HIGH confidence)
- `src/async_api/mod.rs` — 当前状态（Phase 10 创建的占位文件）
- `src/record.rs` — Sqllog 结构体字段定义，确认全为 String/owned 类型
- `src/parser/builder.rs` — LogParserBuilder API，`FileEncodingHint` 变体，`build()` 返回类型
- `src/parser/iterator.rs` — LogIterator::apply_filter 签名
- `src/filter/builder.rs` — Filter/FilterBuilder 定义，Predicate = Box<dyn Fn(&Sqllog) -> bool + Send + Sync>
- `src/lib.rs` — 当前重导出列表（Phase 12 添加 async_api）
- `Cargo.toml` — 当前依赖声明
- `.planning/phases/12-asyncapi/12-CONTEXT.md` — 所有锁定决策

### Secondary (MEDIUM confidence)
- `docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html` — spawn_blocking 签名和语义
- `docs.rs/tokio/latest/tokio/task/struct.JoinError.html` — JoinError 方法（is_panic, into_panic, to_string）
- `docs.rs/tokio/latest/tokio/attr.test.html` — #[tokio::test] 使用要求

### Tertiary (LOW confidence)
- `.planning/phases/11-filterbuilder/11-02-SUMMARY.md` — "Filter 满足 Send + Sync，可直接传入 spawn_blocking"（项目内部确认）

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — tokio 1.52.3 经 crates.io 官方 API 验证，下载量 688M+
- Architecture: HIGH — spawn_blocking 模式有官方文档支撑，Sqllog 字段类型已从代码库验证
- Pitfalls: HIGH — 均来自 Rust 语言/tokio 文档规则，非推测

**Research date:** 2026-05-23
**Valid until:** 2026-11-23（tokio API 稳定，半年内无重大变化风险）
