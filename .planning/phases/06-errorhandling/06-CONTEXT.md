# Phase 6: ErrorHandling - Context

**Gathered:** 2026-05-19
**Status:** Ready for planning

<domain>
## Phase Boundary

重构 `ParseError` 类型以携带行号上下文，并在 `LogIterator` 上暴露错误处理策略（跳过错误），使调用方可获取有意义的错误信息并自主决定迭代过程中的错误行为。

**不在本 Phase 范围内：** 结构化错误上下文（ERR-04 Future）、新增 ParseError 变体细化、编码处理路径变更。

</domain>

<decisions>
## Implementation Decisions

### ERR-01: 行号追踪

- **D-01:** `LogIterator` 内部新增 `line_number: u64` 字段，初始值为 0，每遇到一个 `'\n'` 字节递增。返回错误时将当前行号写入 `ParseError` 变体的 `line_number` 字段。
- **D-02:** "行号"定义为**文件绝对行号**（从 1 开始），而非记录序号。调用方可用 grep/文本编辑器直接定位。
- **D-03:** 行号追踪轻微影响吞吐性能（每行一次递增），但这是 API 易用性阶段的合理权衡。不为此做性能豁免。

### ERR-02: 错误处理策略

- **D-04:** 在 `LogIterator` 上添加 `.skip_errors()` 便捷方法，返回 `impl Iterator<Item = Sqllog<'a>>`（即 `filter_map(Result::ok)` 的封装）。
- **D-05:** 不引入 `ErrorPolicy` enum。调用方仍可通过 `iter()` 返回 `Result` 自行处理，或调用 `.skip_errors()` 跳过错误。

### ERR-03: std::error::Error 实现

- **D-06:** `ParseError` 已通过 `thiserror` 实现 `std::error::Error + Display + Debug`。需验证现有各变体的 Display 消息在添加 `line_number` 后仍然清晰。

### ParseError 变体修改

- **D-07:** 保持现有变体不变（`InvalidFormat`、`FileNotFound`、`InvalidRecordStartLine`、`IntParseError`、`IoError`）。在需要行号的变体（`InvalidFormat`、`InvalidRecordStartLine`、`IntParseError`）中新增 `line_number: u64` 字段。`FileNotFound` 和 `IoError` 不适合添加行号。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 需求规格
- `.planning/REQUIREMENTS.md` — ERR-01、ERR-02、ERR-03 完整需求定义
- `.planning/ROADMAP.md` — Phase 6 Success Criteria（3 条验收标准）

### 现有代码（必读）
- `src/error.rs` — 现有 `ParseError` 枚举定义（全部变体 + thiserror 注解）
- `src/parser.rs` — `LogIterator` 结构体和 `Iterator` impl（行号追踪需在此添加 `line_number` 字段）
- `src/lib.rs` — 公开 API 导出列表（`ParseError`、`LogIterator` 均已导出）

</canonical_refs>

<code_context>
## Existing Code Insights

### 可复用资产
- `thiserror::Error` derive：已在 `ParseError` 上使用，新增字段直接通过宏语法支持
- `LogIterator.pos: usize`：现有字节偏移追踪，行号追踪可并排添加为 `line_number: u64`

### 已有模式
- `ParseError::InvalidFormat { raw: String }` / `ParseError::IntParseError { field, value, raw }` — 已有命名字段风格，`line_number` 字段遵循同一模式追加
- `LogIterator::next()` 中的 `memchr(b'\n', data)` 调用 — 每次 `'\n'` 检测即为行号递增点

### 集成点
- `LogIterator::next()` — 行号递增逻辑加入此方法；错误构造时注入 `line_number`
- `src/lib.rs` — `skip_errors()` 方法属于 `LogIterator` impl，无需新增导出

</code_context>

<specifics>
## Specific Ideas

- `.skip_errors()` 方法签名建议：`pub fn skip_errors(self) -> impl Iterator<Item = Sqllog<'a>> + 'a`，内部 `self.filter_map(Result::ok)`
- 行号应从 1 开始计数（用户友好的惯例）
- `par_iter()` 不追踪全局行号（分区扫描无法维护连续行号），仅 `iter()` 支持行号，这一限制应在文档中说明

</specifics>

<deferred>
## Deferred Ideas

- **ERR-04**（结构化错误上下文）— Future requirements，记录解析状态机中间状态，属于未来增强
- `par_iter()` 行号支持 — 分区并行架构下全局行号需要额外协调开销，暂缓

</deferred>

---

*Phase: 06-ErrorHandling*
*Context gathered: 2026-05-19*
