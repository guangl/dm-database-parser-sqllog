---
phase: 02-correctness
plan: 02
subsystem: ci
tags: [miri, cfg, unsafe, ci, testing]
dependency_graph:
  requires: []
  provides: [miri-ci, mmap-test-annotations]
  affects: [tests/edge_cases.rs, tests/sqllog_additional.rs, tests/parser_iterator.rs, tests/parser_errors.rs, tests/integration_test.rs, .github/workflows/miri.yml]
tech_stack:
  added: [Miri CI (GitHub Actions nightly)]
  patterns: [cfg(not(miri)) guard pattern for mmap tests]
key_files:
  created: [.github/workflows/miri.yml]
  modified:
    - tests/edge_cases.rs
    - tests/sqllog_additional.rs
    - tests/parser_iterator.rs
    - tests/parser_errors.rs
    - tests/integration_test.rs
decisions:
  - "MIRIFLAGS 设置在 step 级别 env 而非全局，只影响 Miri 运行步骤"
  - "cache key 使用 miri- 前缀区分 benchmark cache"
  - "不含 schedule 触发，避免浪费 CI 分钟"
metrics:
  duration: "~8 minutes"
  completed: "2026-04-20T05:15:58Z"
  tasks_completed: 2
  tasks_total: 2
---

# Phase 02 Plan 02: Miri CI + mmap 测试标注 Summary

**一句话总结：** 为 5 个测试文件中所有 mmap 相关测试添加 `#[cfg(not(miri))]` 标注，并创建 Miri CI 作业覆盖 unsafe 解码路径。

---

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | 为所有 mmap 测试添加 cfg 标注 | 7e76fa0 | tests/edge_cases.rs, tests/sqllog_additional.rs, tests/parser_iterator.rs, tests/parser_errors.rs, tests/integration_test.rs |
| 2 | 创建 Miri CI 作业 | 884d63d | .github/workflows/miri.yml |

---

## 创建的文件

### .github/workflows/miri.yml

- 触发条件：push + pull_request 到 main（无 schedule）
- 工具链：dtolnay/rust-toolchain@nightly，组件：miri
- 运行命令：`cargo miri test --test performance_metrics --test sqllog_additional --test edge_cases`
- MIRIFLAGS：`-Zmiri-disable-isolation`（step 级别 env）
- cache key：`{runner.os}-miri-{Cargo.lock hash}`

---

## 标注的测试函数列表

### tests/edge_cases.rs（1 个函数）

- `probable_record_start_line_and_iterator_singleline_detection`

### tests/sqllog_additional.rs（2 个函数）

- `file_encoding_detection_gb18030`
- `file_encoding_detection_utf8`

### tests/parser_iterator.rs（2 个函数，全部）

- `iterator_handles_crlf_and_eof_without_newline`
- `iterator_multiline_detection`

### tests/parser_errors.rs（2 个函数，全部）

- `iterator_yields_error_for_invalid_first_line_then_ok`
- `iterator_skips_empty_record_slice_between_valid_records`

### tests/integration_test.rs（2 个函数，全部）

- `test_parser_lazy_loading`
- `test_parser_multiline`

**合计：9 个标注**

注：`tests/performance_metrics.rs` 使用 `parse_record`（无 mmap），无需标注，全部 10 个测试在 Miri 下应可运行。

---

## cargo test 输出摘要

```
edge_cases.rs          4 passed, 0 failed
integration_test.rs    2 passed, 0 failed
parser_errors.rs       2 passed, 0 failed
parser_iterator.rs     2 passed, 0 failed
performance_metrics.rs 10 passed, 0 failed
sqllog_additional.rs   13 passed, 0 failed
doc-tests              2 passed, 0 failed

Total: 35 passed, 0 failed
```

---

## Miri 本地验证

未在本地运行 Miri（需要 nightly 工具链）。CI 作业将在下次 push/PR 时自动运行验证。

---

## Deviations from Plan

None - 计划按原文执行。

注：计划要求 "至少 10 处 cfg(not(miri)) 标注"，当前 9 处。差额来自 02-01-PLAN 新增的 `encoding_detection_gb18030_after_64kb_boundary` 测试尚未存在（02-01-PLAN 未执行）。该测试在 02-01-PLAN 中已标注 `#[cfg(not(miri))]`，执行后将达到 10 处。

---

## Self-Check: PASSED

- `.github/workflows/miri.yml` 存在：FOUND
- commit 7e76fa0 存在：FOUND
- commit 884d63d 存在：FOUND
- `cargo test` 35 passed, 0 failed：PASSED
- `cargo clippy` 无新警告：PASSED
