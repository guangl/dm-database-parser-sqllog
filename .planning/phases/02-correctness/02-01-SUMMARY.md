---
phase: 02-correctness
plan: 01
subsystem: testing
tags: [rust, encoding, gb18030, simdutf8, mmap]

requires:
  - phase: 01-measurement
    provides: baseline benchmark and test infrastructure

provides:
  - CORR-01: 全文件编码检测（消除 64 KB 截断误判）
  - CORR-03: find_indicators_split 验证守卫（消除 SQL body 含指标关键字时的假阳性切分）
  - 6 条回归测试（1 条 CORR-01 + 5 条 CORR-03）

affects: [03-hotpath, 04-corealgo, 05-parallel]

tech-stack:
  added: []
  patterns: [TDD RED-GREEN for bug fixes, validation guard before split acceptance]

key-files:
  created: []
  modified:
    - src/parser.rs
    - src/sqllog.rs
    - tests/sqllog_additional.rs

key-decisions:
  - "CORR-01: 改为 &mmap[..] 全文件扫描，接受轻微性能开销（simdutf8 ~50 GB/s，one-time）"
  - "CORR-03: 验证守卫调用 parse_indicators_from_bytes，若返回 None 则认为是伪指标，返回 len 不切分"

patterns-established:
  - "验证守卫模式：rfind 候选位置后须验证语义合法性，不能仅依赖关键字出现"

requirements-completed: [CORR-01, CORR-03]

duration: 0min
completed: 2026-04-20
---

# Plan 02-01: Correctness Bug Fixes (CORR-01 + CORR-03) Summary

**消除大文件 GB18030 误判（全文件编码扫描）和 SQL body 内含指标关键字时的错误切分（验证守卫），6 条回归测试全部通过**

## Performance

- **Duration:** — (commits pre-dated this summary)
- **Completed:** 2026-04-20
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- CORR-01: `src/parser.rs` 移除 `.min(65536)` 采样上限，改为扫描整个文件，消除大文件末尾 GB18030 字节被误判为 UTF-8 的 UB
- CORR-03: `src/sqllog.rs::find_indicators_split` 新增验证守卫：rfind 找到候选切分点后调用 `parse_indicators_from_bytes` 验证，若返回 `None`（伪指标）则返回 `len` 跳过切分
- 新增 6 条回归测试：`encoding_detection_gb18030_after_64kb_boundary` + 5 条 `find_indicators_split_*` 测试

## Task Commits

1. **Task 1: CORR-01 — 编码检测扩展至全文件 + 回归测试** - `59c0e25` (fix)
2. **Task 2: CORR-03 — find_indicators_split 验证守卫 + 五条假阳性测试** - `8602042` (fix)

## Files Created/Modified

- `src/parser.rs` - 第 38 行：`let sample = &mmap[..];`（移除 `.min(65536)`）
- `src/sqllog.rs` - `find_indicators_split` 末尾新增验证守卫（lines 231-235）
- `tests/sqllog_additional.rs` - 新增 6 条回归测试

## Decisions Made

- 全文件扫描的性能开销可接受（simdutf8 ~50 GB/s，逐文件执行一次）
- 验证守卫语义：`split < len && parse_indicators_from_bytes(&data[split..]).is_none()` → 返回 `len`

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

- 两个正确性 Bug 已修复，代码库安全地基稳固
- 所有测试通过（19 passed, 0 failed），clippy 无新警告
- 覆盖率基础良好（新测试只增不减）
- 03-hotpath 可以在此基础上安全进行热路径优化

---
*Phase: 02-correctness*
*Completed: 2026-04-20*

## Self-Check: PASSED
