---
phase: 06-errorhandling
plan: 01
name: ParseError line_number support
completed_date: 2026-05-19
duration: 0.5h
tags:
  - rust
  - errors
  - line-number
requires:
  - cfg(test): none
provides:
  - ParseError line_number field
  - LogIterator line_number tracking
affects:
  - src/error.rs
  - src/parser.rs
tech-stack:
  added: []
  patterns:
    - "LogIterator::next() counts '\\n' bytes to track file line number"
    - "par_iter() sets line_number to 0 (no global tracking in partitioned scan)"
key-files:
  created: []
  modified:
    - src/error.rs (ParseError enum 3 variants gain line_number: u64 + Display update)
    - src/parser.rs (LogIterator line_number tracking, function signatures)
decisions:
  - D-01: LogIterator adds line_number: u64, incremented per '\n' byte
  - D-02: Line numbers start at 1 (file absolute line number)
  - D-03: Performance impact accepted for API ergonomics
  - D-06: Display messages include line_number info
  - D-07: Only InvalidFormat/InvalidRecordStartLine/IntParseError get line_number; FileNotFound/IoError unchanged
metrics:
  duration_minutes: 30
  tasks_completed: 3
  files_modified: 2
  commits: 3
---

# Phase 6 Plan 1: ParseError line_number support

为 ParseError 的 3 个变体添加 `line_number: u64` 字段，LogIterator 在迭代过程中追踪文件绝对行号，错误发生时行号写入 ParseError。

## Commits

| Hash | Message |
|------|---------|
| `2627760` | feat(06-01): add line_number field to ParseError variants |
| `ed1f50a` | feat(06-01): add line_number tracking to LogIterator |
| `e503019` | feat(06-01): propagate line_number through parse_record_with_hint and make_invalid_format_error |

## ParseError Changes

**InvalidFormat** -- 新增 `line_number: u64`
- Display: `"invalid format at line {line_number} | raw: {raw}"`

**InvalidRecordStartLine** -- 新增 `line_number: u64`
- Display: `"invalid record start line at line {line_number}: line does not match expected format | raw: {raw}"`

**IntParseError** -- 新增 `line_number: u64`
- Display: `"failed to parse {field} as integer at line {line_number}: {value} | raw: {raw}"`

**FileNotFound** -- 不变（不含 line_number）
**IoError** -- 不变（不含 line_number）

## LogIterator Line Number Tracking

- `LogIterator` 结构体新增 `line_number: u64` 字段
- `LogParser::iter()` 初始值 `line_number: 1`（文件绝对行号从 1 开始）
- `par_iter()` 内部 LogIterator 初始值 `line_number: 0`（分区扫描无法维护全局行号，0 表示不可用）
- `LogIterator::next()` 每轮迭代：保存 `current_line` 用于错误构造，在 `self.pos` 前进后统计消耗字节中的 `'\n'` 数量累加到 `self.line_number`
- 行号更新在空记录 `continue` 之前执行，确保空行跳过时行号仍正确

## Function Signature Changes

- `make_invalid_format_error(raw_bytes: &[u8], line_number: u64) -> ParseError` -- 新增 line_number 参数
- `parse_record_with_hint(..., line_number: u64) -> Result<...>` -- 新增 line_number 参数
- `parse_record()` -> `parse_record_with_hint(..., 0)` -- 直接调用时 line_number=0
- `LogIterator::next()` -> `parse_record_with_hint(..., current_line)` -- 迭代器调用时携带实际行号

## Testing

- `cargo build`: 通过
- `cargo test`: 全部 60 个测试通过（无回归）
- `cargo clippy --lib -- -D warnings`: 库代码无警告
- `cargo llvm-cov --workspace --all-features --fail-under-lines 90`: 行覆盖率 90.50%（通过）

## Deviations from Plan

无 -- 按计划精确执行。

## Known Stubs

无

## Threat Flags

无

## Self-Check: PASSED

所有文件变更已验证，所有提交已确认存在，所有测试通过。
