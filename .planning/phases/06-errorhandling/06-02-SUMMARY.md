---
phase: 06-errorhandling
plan: 02
name: LogIterator::skip_errors() + error tests
completed_date: 2026-05-19
duration: 0.5h
tags:
  - rust
  - errors
  - skip-errors
  - tests
requires:
  - 06-01 (line_number support)
provides:
  - LogIterator::skip_errors() method
  - 5 tests covering line_number and skip_errors
affects:
  - src/parser.rs
  - tests/parser_errors.rs
tech-stack:
  added: []
  patterns:
    - "LogIterator::skip_errors() delegates to filter_map(Result::ok)"
    - "Invalid records for testing must use timestamp-prefixed lines without meta section to trigger parse errors"
key-files:
  created: []
  modified:
    - src/parser.rs (LogIterator::skip_errors() method)
    - tests/parser_errors.rs (5 new tests: line_number, skip_errors, Display, std::error::Error)
decisions:
  - D-04: skip_errors() returns impl Iterator<Item = Sqllog<'a>>, delegates to filter_map(Result::ok)
  - D-05: No ErrorPolicy enum introduced; simple skip semantics
metrics:
  duration_minutes: 30
  tasks_completed: 2
  files_modified: 2
  commits: 2
---

# Phase 6 Plan 2: LogIterator::skip_errors() + Tests

添加 `LogIterator::skip_errors()` 便捷方法，以及 5 个覆盖行号追踪和错误跳过行为的新测试。

## Commits

| Hash | Message |
|------|---------|
| `59b74c0` | feat(06-errorhandling): add LogIterator::skip_errors() method |
| `a6d7d22` | test(06-02): add 5 tests for line_number tracking and skip_errors |

## skip_errors() Method

```rust
/// Returns an iterator that skips parse errors, yielding only successfully parsed Sqllog records.
///
/// # Example
/// ```
/// for log in parser.iter().skip_errors() {
///     println!("{}", log.body());
/// }
/// ```
///
/// Note: `par_iter()` returns a Rayon ParallelIterator, not LogIterator, so skip_errors()
/// is not available in parallel mode.
pub fn skip_errors(self) -> impl Iterator<Item = Sqllog<'a>> + 'a {
    self.filter_map(Result::ok)
}
```

## New Tests (tests/parser_errors.rs)

| Test | Description | Miri |
|------|-------------|------|
| `test_skip_errors_filters_invalid_records` | 3 行：有效/无效/有效，断言 skip_errors() 返回 2 个 Sqllog | no |
| `test_error_contains_correct_line_number` | 2 行：有效/无效，断言错误中 line_number == 2 | no |
| `test_line_number_after_multiple_valid_records` | 5 行：3 有效/1 无效/1 有效，断言 line_number == 4 | no |
| `test_parse_error_impl_std_error` | 编译期验证 ParseError 实现了 std::error::Error | yes |
| `test_error_display_contains_line_number` | 构造 ParseError::InvalidFormat { line_number: 42 }，验证 Display 包含 "42" 和 "line" | yes |

## Testing

- `cargo build`: 通过
- `cargo test --test parser_errors`: 7/7 测试通过（2 个原有 + 5 个新增）
- `cargo test`: 全部 workspace 测试通过

## Deviations from Plan

无 — 按计划精确执行。注意：测试 2-3 中的无效记录必须使用带时间戳前缀但不含元数据部分的行（例如 `"2025-11-17 16:09:41.124 BAD WITHOUT META\n"`），以便 LogIterator 将其标识为记录起始行，从而触发 ParseError。纯无效行（不含时间戳前缀）将被视为前一记录的正文续行。

## Known Stubs

无

## Threat Flags

无

## Self-Check: PASSED

所有文件变更已验证，所有提交已确认存在，全部 7 个测试通过。
