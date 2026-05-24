# Phase 11: FilterBuilder - Pattern Map

**Mapped:** 2026-05-22
**Files analyzed:** 4 (builder.rs, predicate.rs, mod.rs update, lib.rs update)
**Analogs found:** 4 / 4

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/filter/builder.rs` | builder | request-response | `src/parser/builder.rs` | exact |
| `src/filter/predicate.rs` | utility / enum | transform | `src/parser/encoding.rs` | role-match |
| `src/filter/mod.rs` | module root | — | `src/parser/mod.rs` | exact |
| `src/lib.rs` | crate root | — | 自身现有结构 | self |

---

## Pattern Assignments

### `src/filter/builder.rs` (builder, request-response)

**Analog:** `src/parser/builder.rs`

**结构体声明模式** (builder.rs lines 10-13):
```rust
pub struct LogParserBuilder {
    path: PathBuf,
    encoding_hint: Option<FileEncodingHint>,
}
```
FilterBuilder 应照此声明一个同名 pub struct，每个过滤条件对应一个 `Option<T>` 字段：
```rust
// 建议骨架 — src/filter/builder.rs
use crate::error::ParseError;
use crate::record::Sqllog;

pub struct FilterBuilder {
    min_exec_time_ms: Option<u64>,
    max_exec_time_ms: Option<u64>,
    sql_contains: Option<String>,
    username: Option<String>,
    // 后续可继续追加字段
}
```

**链式 setter 签名** (builder.rs lines 25-28)：
```rust
// 完全照抄：接收 mut self，设置字段，返回 Self
pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
    self.encoding_hint = Some(hint);
    self
}
```
FilterBuilder 的每个 setter 遵循同一签名模板：
```rust
pub fn min_exec_time(mut self, ms: u64) -> Self {
    self.min_exec_time_ms = Some(ms);
    self
}

pub fn sql_contains(mut self, pattern: impl Into<String>) -> Self {
    self.sql_contains = Some(pattern.into());
    self
}
```

**build() / apply() 终结方法** (builder.rs lines 31-52)：
```rust
pub fn build(self) -> Result<LogParser, ParseError> { ... }
```
FilterBuilder 的终结方法不需要 Result，因为过滤本身不会失败。
推荐命名 `apply()`，接收一个迭代器并返回被过滤的迭代器：
```rust
pub fn apply<'a, I>(self, iter: I)
    -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a
where
    I: Iterator<Item = Result<Sqllog, ParseError>> + 'a,
{
    // 依次用 adapter 函数包裹
    let iter = if let Some(min) = self.min_exec_time_ms {
        // 用 Box<dyn Iterator> 或 enum-dispatch 解决类型擦除问题
        ...
    };
    iter
}
```
注意：由于链式 filter 每层类型不同，需用 `Box<dyn Iterator<...>>` 做类型擦除，
或者在 `apply` 内部把所有条件用 `.filter()` 闭包组合到单次 pass 中（更高效）：
```rust
pub fn apply<'a, I>(self, iter: I)
    -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a
where
    I: Iterator<Item = Result<Sqllog, ParseError>> + 'a,
{
    let min_ms = self.min_exec_time_ms.map(|v| v as f32);
    let max_ms = self.max_exec_time_ms.map(|v| v as f32);
    let sql_pat = self.sql_contains;
    let username = self.username;

    iter.filter(move |item| match item {
        Err(_) => false,
        Ok(rec) => {
            if let Some(min) = min_ms { if rec.exectime < min { return false; } }
            if let Some(max) = max_ms { if rec.exectime > max { return false; } }
            if let Some(ref pat) = sql_pat { if !rec.sql.contains(pat.as_str()) { return false; } }
            if let Some(ref user) = username { if rec.username != *user { return false; } }
            true
        }
    })
}
```

**`Default` / `new()` 构造** (builder.rs lines 17-22)：
```rust
pub fn new<P: AsRef<Path>>(path: P) -> Self {
    Self {
        path: path.as_ref().to_path_buf(),
        encoding_hint: None,
    }
}
```
FilterBuilder 无需路径参数，直接实现 `Default`（Rust 惯用法）并提供 `new()`：
```rust
impl Default for FilterBuilder {
    fn default() -> Self {
        Self {
            min_exec_time_ms: None,
            max_exec_time_ms: None,
            sql_contains: None,
            username: None,
        }
    }
}

impl FilterBuilder {
    pub fn new() -> Self { Self::default() }
}
```

---

### `src/filter/predicate.rs` (utility enum, transform) — 可选文件

**Analog:** `src/parser/encoding.rs`

encoding.rs 展示了项目中最简洁的 pub enum 写法 (encoding.rs lines 1-11)：
```rust
/// 文件编码提示，用于指示日志文件的字符编码。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FileEncodingHint {
    #[default]
    Auto,
    Utf8,
    Gb18030,
}
```

如果实现 predicate.rs，照此模式声明一个描述单条过滤规则的 enum：
```rust
// 建议骨架 — src/filter/predicate.rs
/// 单条过滤谓词，描述对 Sqllog 字段的一次比较。
#[derive(Clone, Debug)]
pub enum Predicate {
    MinExecTime(u64),   // ms
    MaxExecTime(u64),
    SqlContains(String),
    Username(String),
}

impl Predicate {
    /// 判断给定记录是否满足本谓词。
    pub fn matches(&self, rec: &Sqllog) -> bool {
        match self {
            Predicate::MinExecTime(ms) => rec.exectime >= *ms as f32,
            Predicate::MaxExecTime(ms) => rec.exectime <= *ms as f32,
            Predicate::SqlContains(pat) => rec.sql.contains(pat.as_str()),
            Predicate::Username(user) => rec.username == *user,
        }
    }
}
```
**注意：** planner 需决定是否引入 predicate.rs。若 FilterBuilder 的条件较少（<=5），
直接在 builder.rs 内用独立 Option 字段即可，无需拆分文件。

---

### `src/filter/mod.rs` (module root) — 更新

**Analog:** `src/parser/mod.rs` (lines 1-8)

parser/mod.rs 展示了本项目 mod.rs 的完整声明模式：
```rust
pub mod builder;
pub(crate) mod encoding;
pub mod iterator;

pub use builder::LogParserBuilder;
pub use encoding::FileEncodingHint;
pub use iterator::LogIterator;
```

现有 `src/filter/mod.rs` 只有一行（filter/mod.rs line 1）：
```rust
pub(crate) mod adapter;
```

更新后应追加 builder（及可选的 predicate）子模块声明，并按 parser/mod.rs 的风格重导出：
```rust
// src/filter/mod.rs 更新后
pub(crate) mod adapter;
pub mod builder;
// pub mod predicate; // 若实现 predicate.rs 则解注释

pub use builder::FilterBuilder;
// pub use predicate::Predicate;
```

**可见性规则（来自 parser/mod.rs 对比）：**
- 内部实现（adapter）保持 `pub(crate)`
- 对外 API（builder）用 `pub`
- 重导出条目用 `pub use`

---

### `src/lib.rs` (crate root) — 更新

**Analog:** 自身（lib.rs lines 82-89）

现有 lib.rs 的模块声明与重导出区块：
```rust
pub(crate) mod error;
pub(crate) mod filter;
pub(crate) mod parser;
pub(crate) mod record;

pub use error::ParseError;
pub use parser::{FileEncodingHint, LogIterator, LogParser, LogParserBuilder};
pub use record::Sqllog;
```

**关键观察：** `filter` 模块目前是 `pub(crate)`，因为 adapter 函数只供内部调用。
Phase 11 引入 `FilterBuilder` 作为公开 API 后，有两种选项：

选项 A（推荐）— 保持模块 `pub(crate)`，只重导出顶层符号：
```rust
pub(crate) mod filter;                   // 模块本身保持 crate 内可见
pub use filter::FilterBuilder;           // 只把 FilterBuilder 导到顶层
```

选项 B — 将 filter 改为 `pub` 模块（允许用户 `use crate::filter::Predicate`）：
```rust
pub mod filter;
pub use filter::FilterBuilder;
```

选项 A 与现有 parser/error/record 的 `pub(crate) mod` + `pub use` 风格完全一致，
**建议 planner 选择选项 A**。

lib.rs 最终重导出行应在现有 `pub use parser::{...}` 行后追加：
```rust
pub use filter::FilterBuilder;
```

---

## Sqllog 字段访问方式

所有字段已在 `parse_record_with_hint` 中一次性填充（record.rs lines 9-54），
FilterBuilder 可直接通过 `.` 访问，**无需调用任何 `parse_*` 方法**：

| 字段 | 类型 | 访问方式 | 典型过滤用途 |
|---|---|---|---|
| `ts` | `String` | `rec.ts` | 时间范围过滤 |
| `tag` | `Option<String>` | `rec.tag.as_deref()` | 按 [SEL]/[ORA] 过滤 |
| `ep` | `u8` | `rec.ep` | 按 EP 节点过滤 |
| `sess_id` | `String` | `rec.sess_id` | 按会话过滤 |
| `username` | `String` | `rec.username` | 按用户过滤 |
| `appname` | `String` | `rec.appname` | 按应用名过滤 |
| `client_ip` | `String` | `rec.client_ip` | 按客户端 IP 过滤 |
| `sql` | `String` | `rec.sql` | SQL 语句匹配 |
| `exectime` | `f32` | `rec.exectime` | 执行时间阈值（ms） |
| `rowcount` | `u32` | `rec.rowcount` | 影响行数过滤 |
| `exec_id` | `i64` | `rec.exec_id` | 按执行 ID 过滤 |

现有 adapter.rs 的直接访问示例 (adapter.rs lines 11-14)：
```rust
iter.filter(move |item| match item {
    Ok(sqllog) => sqllog.exectime >= threshold,
    Err(_) => false,
})
```
FilterBuilder.apply() 内的闭包应复用此 `Ok(rec) => ... / Err(_) => false` 结构。

---

## Shared Patterns

### 链式 Builder 方法签名
**来源:** `src/parser/builder.rs` lines 25-28
**适用:** `src/filter/builder.rs` 所有 setter 方法
```rust
pub fn <method_name>(mut self, value: <Type>) -> Self {
    self.<field> = Some(value);
    self
}
```

### 错误处理在过滤闭包中的惯用写法
**来源:** `src/filter/adapter.rs` lines 12-15
**适用:** `FilterBuilder::apply()` 内部闭包
```rust
iter.filter(move |item| match item {
    Ok(sqllog) => /* 条件判断 */,
    Err(_) => false,   // 解析错误记录直接过滤掉
})
```

### mod.rs 子模块声明风格
**来源:** `src/parser/mod.rs` lines 1-7
**适用:** `src/filter/mod.rs` 更新
```rust
pub mod <submodule>;       // 公开子模块
pub(crate) mod <internal>; // 内部子模块
pub use <submodule>::<Type>; // 重导出公开类型
```

### lib.rs 重导出风格
**来源:** `src/lib.rs` lines 87-89
**适用:** `src/lib.rs` 追加 FilterBuilder 重导出
```rust
pub use filter::FilterBuilder;
```

### 测试文件结构
**来源:** `tests/parser_filters.rs`
**适用:** 新测试文件 `tests/filter_builder.rs`

测试文件使用 `tempfile::NamedTempFile` 写入内联日志数据，
通过 `LogParserBuilder` 构建解析器，再链式调用 API 进行验证：
```rust
use dm_database_parser_sqllog::{FilterBuilder, LogParserBuilder};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
#[cfg(not(miri))]
fn test_filter_builder_min_exec_time() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "2025-11-17 16:09:41.100 (EP[0] ...) SELECT 1 EXECTIME: 50(ms) ...\n").unwrap();
    write!(file, "2025-11-17 16:09:41.200 (EP[0] ...) SELECT 2 EXECTIME: 200(ms) ...\n").unwrap();

    let parser = LogParserBuilder::new(file.path()).build().unwrap();
    let results: Vec<_> = FilterBuilder::new()
        .min_exec_time(100)
        .apply(parser.iter())
        .collect();

    assert_eq!(results.len(), 1);
    assert!(results[0].as_ref().unwrap().sql.contains("SELECT 2"));
}
```

---

## No Analog Found

无。所有新文件均有对应的现有模式可参考。

---

## Metadata

**Analog search scope:** `src/parser/`, `src/filter/`, `tests/`, `examples/`
**Files scanned:** 9
**Pattern extraction date:** 2026-05-22
