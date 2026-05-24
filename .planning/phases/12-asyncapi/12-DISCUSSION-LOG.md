# Phase 12: AsyncAPI - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-22
**Phase:** 12-AsyncAPI
**Areas discussed:** API 入口形态, Filter 整合方式, tokio 最小依赖, 错误类型设计, AsyncLogParser 完整帕号

---

## API 入口形态

| Option | Description | Selected |
|--------|-------------|----------|
| 自由函数 `parse_file_async` | 单一函数 `pub async fn parse_file_async(path) -> Result<Vec<Sqllog>, ParseError>` | |
| `LogParserBuilder` 异步扩展 | `LogParserBuilder::new(path).build_async().await?` | |
| 独立 `AsyncLogParser` 结构体 | `AsyncLogParser::new(path).parse().await?` | ✓ |

**User's choice:** 独立 AsyncLogParser 结构体  
**Notes:** 用户偏好独立结构体，不扩展现有 LogParserBuilder，API 风格与 LogParserBuilder 一致（builder 方法链）。

---

### 返回类型

| Option | Description | Selected |
|--------|-------------|----------|
| `Vec<Sqllog>` 只含成功 | 解析失败记录静默丢弃，类似 `skip_errors()` | ✓（Claude 决定）|
| `Vec<Result<Sqllog, ParseError>>` | 包含所有条目含错误，与 LogIterator 语义对齐 | |

**User's choice:** Claude 决定  
**Notes:** Claude 选择 `Result<Vec<Sqllog>, AsyncError>`，单条记录解析失败静默丢弃。

---

## Filter 整合方式

| Option | Description | Selected |
|--------|-------------|----------|
| `.with_filter(filter)` 方法链 | `AsyncLogParser::new(path).with_filter(filter).parse().await?` | ✓ |
| `parse(Option<Filter>)` 参数 | `.parse(Some(filter))` / `.parse(None)` | |
| 分开两个方法 `parse` / `parse_filtered` | 类似现有 `filter_by_exec_time` 风格 | |

**User's choice:** `.with_filter(filter)` 方法链  
**Notes:** 与 Rust builder pattern 一致；过滤在 `spawn_blocking` 内部执行，减少返回给 async 上下文的数据量。

---

## tokio 最小依赖

| Option | Description | Selected |
|--------|-------------|----------|
| `rt` only | 最小依赖，用户自选运行时 | ✓（Claude 决定）|
| `rt + rt-multi-thread` | 强制多线程运行时 | |

**User's choice:** Claude 决定  
**Notes:** 库 crate 最小依赖原则，只拉 `rt`。测试用 dev-dependencies 加 `macros`。

---

## 错误类型设计

| Option | Description | Selected |
|--------|-------------|----------|
| `ParseError::IoError` 包装 JoinError | 用现有 `ParseError` 封装，返回类型不变 | |
| 新增 `AsyncError` 枚举 | `Parse(ParseError) \| Panic(String)`，语义更精确 | ✓ |
| 直接 `.expect()` JoinError | JoinError = 内部 panic，不封装给用户 | |

**User's choice:** 新增 `AsyncError` 枚举  
**Notes:** 用户认为语义分离比简单更重要。

### `From<ParseError>` 实现

| Option | Description | Selected |
|--------|-------------|----------|
| 实现 `From<ParseError>` | 内部 `?` 运算符自动转换，减少样板代码 | ✓ |
| 不实现 | 只在外层 match 转换一次 | |

**User's choice:** 实现 `From<ParseError>`

---

## AsyncLogParser 完整帕号确认

确认的完整签名：

```rust
pub struct AsyncLogParser { ... }
impl AsyncLogParser {
    pub fn new(path: impl AsRef<Path>) -> Self
    pub fn encoding_hint(self, hint: FileEncodingHint) -> Self
    pub fn with_filter(self, filter: Filter) -> Self
    pub async fn parse(self) -> Result<Vec<Sqllog>, AsyncError>
}
```

**User's choice:** 确认，就这个

---

## Claude's Discretion

- 返回类型选 `Result<Vec<Sqllog>, AsyncError>`（单条错误静默丢弃）
- tokio feature：库侧仅 `rt`，dev-deps 加 `macros`
- `AsyncError` 实现 `std::error::Error` trait（公开错误类型标准做法）

## Deferred Ideas

- **Stream API**（STREAM-01/02）：用户暗示过 `impl Stream` 逐条处理，已标注为 Future Requirements
- **Phase 13–15**：调用参数中包含 `13,14,15`，但这些 phase 不在 ROADMAP.md 中，本次只处理 Phase 12
