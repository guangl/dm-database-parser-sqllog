---
phase: 03-hotpath
plan: "01"
subsystem: sqllog
tags: [performance, hot-path, memrchr, early-exit]
dependency_graph:
  requires: []
  provides: [HOT-01-early-exit, HOT-02-single-scan]
  affects: [src/sqllog.rs]
tech_stack:
  added: []
  patterns: [O(1)-early-exit, single-pass-reverse-scan]
key_files:
  modified:
    - src/sqllog.rs
    - tests/performance_metrics.rs
decisions:
  - "HOT-01 早退条件扩展为 '.' 或 ')' 以兼容 EXECTIME/ROWCOUNT only 记录"
  - "HOT-02 扫描语义：每个关键字只取最右命中（等价于原 FinderRev::rfind），防止 body 中假关键字干扰"
  - "scan_earliest_indicator 提取为独立辅助函数，保证 find_indicators_split 在 40 行以内"
metrics:
  duration: "~15min"
  completed: "2026-04-24"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
---

# Phase 03 Plan 01: HOT-01/02 热路径优化 Summary

**一句话：** `find_indicators_split` 增加 O(1) 末尾字节早退（HOT-01）和单次 `memrchr(b':')` 反向扫描替代 3 次 `FinderRev::rfind`（HOT-02），减少无指标记录和有指标记录的搜索开销。

---

## HOT-01/02 改动摘要

### HOT-01：早退逻辑

在 `find_indicators_split` 开头插入末尾字节检查：

- 从 `content_raw` 末尾向前跳过 `\n`/`\r`，取最后一个有效字节
- 若不是 `'.'`（EXEC_ID 终止符）也不是 `')'`（EXECTIME/ROWCOUNT 终止符），则记录无指标，直接返回 `len`
- 有指标的路径仍继续执行后续搜索，CORR-03 守卫保持原位

### HOT-02：单次反向字节扫描

提取辅助函数 `scan_earliest_indicator(window: &[u8]) -> usize`：

- 单次 `memrchr(b':')` 从右向左扫描窗口
- 每次命中后用 `ends_with` 检查前缀是否为 `EXECTIME`/`ROWCOUNT`/`EXEC_ID`
- 每个关键字只记录**最右**命中（等价于原 `FinderRev::rfind` 语义），防止 SQL body 中假关键字干扰
- 三个关键字最右命中中取最左（最小索引）作为分割点

---

## 删除的静态变量

| 变量名 | 类型 | 说明 |
|--------|------|------|
| `FINDER_REV_EXECTIME` | `LazyLock<FinderRev<'static>>` | HOT-02 后不再需要 |
| `FINDER_REV_ROWCOUNT` | `LazyLock<FinderRev<'static>>` | HOT-02 后不再需要 |
| `FINDER_REV_EXEC_ID` | `LazyLock<FinderRev<'static>>` | HOT-02 后不再需要 |

同时删除 `use memchr::memmem::FinderRev`，新增 `use memchr::memrchr`。

---

## 新增测试

### HOT-01 测试（Task 1）

| 测试名 | 验证点 |
|--------|--------|
| `hot01_early_exit_no_dot_suffix` | 纯 SQL（无 `.` 结尾）被早退，body_len == content_raw.len() |
| `hot01_early_exit_newline_suffix` | 末尾 `\n` 的无指标记录被早退 |
| `hot01_dot_suffix_no_real_indicators_guarded` | SQL 以 `.` 结尾但无真实指标，CORR-03 守卫拦截 |
| `hot01_dot_suffix_with_real_indicators` | 末尾 `.` 含真实指标，正常分割解析 |

### HOT-02 测试（Task 2）

| 测试名 | 验证点 |
|--------|--------|
| `hot02_fake_keyword_in_body_plus_real_indicators` | body 含假 EXECTIME:，真实指标仍正确解析 |
| `hot02_multiple_colons_in_body` | body 含多个 `:` 字符（URL），不影响 split |
| `hot02_exec_id_only_split_correct` | 仅 EXEC_ID 记录，split 正确 |

---

## `cargo test` 输出摘要

```
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

所有测试通过，`cargo clippy -- -D warnings` 无 error。

---

## Commits

| Task | Commit | 描述 |
|------|--------|------|
| Task 1 | 19fddbb | feat(03-01): HOT-01 早退逻辑 — 非指标记录 O(1) 返回 |
| Task 2 | 9a4f72d | feat(03-01): HOT-02 单次反向字节扫描替换 3 次 FinderRev |

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] HOT-01 早退条件扩展以兼容 EXECTIME/ROWCOUNT only 记录**

- **Found during:** Task 1 GREEN 阶段运行 `cargo test`
- **Issue:** 计划描述 "有指标记录必以 '.' 结尾"，但现有测试 `performance_metrics_exectime_only` 和 `performance_metrics_rowcount_only` 使用只含 EXECTIME 或 ROWCOUNT（无 EXEC_ID）的记录，末尾为 `)` 而非 `.`，被错误早退导致 2 个测试失败。
- **Fix:** 将早退条件从 `!= Some(b'.')` 扩展为 `!= Some(b'.') && != Some(b')'`，兼容所有合法指标格式
- **Files modified:** `src/sqllog.rs`
- **Commit:** 19fddbb

**2. [Rule 1 - Bug] HOT-02 扫描语义修正为最右命中**

- **Found during:** Task 2 GREEN 阶段运行 `cargo test`
- **Issue:** 初版实现对每个关键字记录**最左**命中（最小索引），但原 `FinderRev::rfind` 返回的是**最右**命中。当 SQL body 中包含假 `EXECTIME:` 关键字时，初版实现将假关键字位置记录为最左命中，导致 `hot02_fake_keyword_in_body_plus_real_indicators` 测试失败（分割点偏左，sql 字段截断）。
- **Fix:** 改为每个关键字只记录第一次（最右）命中，后续更左的同名命中忽略，与原 `FinderRev::rfind` 语义等价
- **Files modified:** `src/sqllog.rs`（`scan_earliest_indicator` 函数）
- **Commit:** 9a4f72d

---

## Self-Check: PASSED

| Item | Status |
|------|--------|
| src/sqllog.rs | FOUND |
| tests/performance_metrics.rs | FOUND |
| 03-01-SUMMARY.md | FOUND |
| commit 19fddbb | FOUND |
| commit 9a4f72d | FOUND |
