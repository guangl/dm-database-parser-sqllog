---
phase: 03-hotpath
reviewed: 2026-04-24T00:00:00Z
depth: standard
files_reviewed: 3
files_reviewed_list:
  - src/sqllog.rs
  - src/parser.rs
  - tests/performance_metrics.rs
findings:
  critical: 0
  warning: 1
  info: 1
  total: 2
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-04-24
**Depth:** standard
**Files Reviewed:** 3
**Status:** issues_found

## Summary

本次审查覆盖 Phase 03（HotPath）的四项优化：HOT-01（O(1) 早退）、HOT-02（单次反向 `:` 扫描）、HOT-03（`#[inline(always)]` / `#[cold]` 标注）、HOT-04（`mmap.advise(Advice::Sequential)`）。

逻辑正确性整体良好。HOT-01 的早退条件（末尾 `.`/`)`）覆盖了所有已知指标格式；CORR-03 守卫能够拦截假阳性。HOT-02 的 `scan_earliest_indicator` 实现与原 `FinderRev::rfind` 语义等价，`colon - 8`/`colon - 7` 减法在 `ends_with` 匹配保证下不会下溢。HOT-03 的 `#[cold]` 标注位置正确，`#[inline(always)]` 作用于非递归热路径函数，符合设计预期。

**主要问题：** HOT-04 在 `src/parser.rs` 中将 `Advice` 直接加入无条件 `use` 语句，而 `memmap2::Advice` 仅在 `#[cfg(unix)]` 下导出，导致 Windows 目标编译失败。

测试文件 `tests/performance_metrics.rs` 覆盖了所有主要路径（全指标、无指标、ORA tag、单指标、HOT-01 早退、HOT-02 假关键字），无质量问题。

---

## Warnings

### WR-01: `Advice` 无条件导入在 Windows 目标编译失败

**File:** `src/parser.rs:3`

**Issue:** `use memmap2::{Advice, Mmap};` 是无条件导入，但 `memmap2::Advice` 仅在 `#[cfg(unix)]` 下由 crate 导出（见 memmap2 0.9.9 `src/lib.rs` 第 66–67 行）。在 Windows 或其他非 Unix 目标上，此导入产生 `unresolved import` 编译错误，即使 `advise()` 调用本身已被 `#[cfg(unix)]` 正确门控（第 40–41 行）。

**Fix:** 将 `Advice` 的导入也限制到 Unix，与调用点的 `#[cfg(unix)]` 门控保持一致：

```rust
// 当前（有问题）
use memmap2::{Advice, Mmap};

// 修复后
#[cfg(unix)]
use memmap2::Advice;
use memmap2::Mmap;
```

---

## Info

### IN-01: `parse_performance_metrics` 的 `#[inline(always)]` 可能导致代码膨胀

**File:** `src/sqllog.rs:99`

**Issue:** `parse_performance_metrics` 函数体约 18 行，内部调用了 `find_indicators_split`（含反向字节扫描）、`decode_content_bytes`（unsafe，含多分支）、`strip_ora_prefix`、`parse_indicators_from_bytes`（含三个 SIMD 查找）。`#[inline(always)]` 强制所有调用点内联，对单一热路径调用点（benchmark）有益，但若库的多个消费者在不同调用点调用此方法，会导致二进制大小增加，且可能因 I-cache 压力抵消内联收益。

目前已知调用点仅为 `benches/parser_benchmark.rs` 的 `parse_sqllog_metrics_5mb` 变体，代码膨胀风险较低。但建议在基准测试数据支持前，可考虑使用 `#[inline]`（让编译器自行决策）。

**Fix:** 若基准测试证实 `#[inline(always)]` 带来可测量收益，则保留；否则改为普通 `#[inline]` 让编译器决策：

```rust
// 保守做法（让编译器决策）
#[inline]
pub fn parse_performance_metrics(&self) -> PerformanceMetrics<'a> {
```

---

_Reviewed: 2026-04-24_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
