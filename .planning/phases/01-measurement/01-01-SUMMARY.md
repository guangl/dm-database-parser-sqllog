---
phase: 01-measurement
plan: 01
subsystem: testing
tags: [criterion, benchmark, throughput, multiline, performance-metrics]

requires: []
provides:
  - "扩展后的 benchmark，包含 GB/s（Throughput::Bytes）和 records/sec（Throughput::Elements）双维度吞吐量输出"
  - "多行合成语料库生成函数 generate_synthetic_log_multiline（20% 多行记录，精确取模控制）"
  - "parse_sqllog_metrics_5mb benchmark 变体，覆盖 parse_performance_metrics() 热路径"
affects: [02-correctness, 03-hotpath, 04-corealgo, 05-parallel]

tech-stack:
  added: []
  patterns:
    - "criterion Throughput::Bytes + Throughput::Elements 双变体模式（同一语料库两个变体，分别测 GB/s 和 records/sec）"
    - "取模计数控制合成语料库记录类型比例（record_index % 5 == 0 精确控制 20%）"

key-files:
  created: []
  modified:
    - benches/parser_benchmark.rs

key-decisions:
  - "使用 record_index % 5 == 0 取模而非随机数控制多行比例，确保语料库确定性可复现"
  - "parse_sqllog_metrics_5mb 复用多行语料库（而非单行），使 benchmark 反映真实热路径工作量"
  - "rps 变体在 bench 外预先统计 record_count，避免在 iter() 中引入额外测量噪声"

patterns-established:
  - "新增 benchmark 变体时：先 group.throughput() 再 group.bench_function()，吞吐量配置前置于对应变体"
  - "合成语料库临时文件（NamedTempFile）在函数末尾显式 drop，防止提前清理"

requirements-completed: [MEAS-01, MEAS-02, MEAS-03]

duration: 2min
completed: 2026-04-19
---

# Phase 01 Plan 01: Benchmark Extension Summary

**Criterion benchmark 扩展为双维度吞吐量（GB/s + records/sec）并新增多行语料库和 parse_performance_metrics() 热路径变体，共 5 个新 benchmark 变体**

## Performance

- **Duration:** 2 min
- **Started:** 2026-04-19T00:15:36Z
- **Completed:** 2026-04-19T00:17:16Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- 新增 `generate_synthetic_log_multiline` 函数，使用 `record_index % 5 == 0` 精确生成 20% 多行 SQL 记录（3 行换行 SQL body）
- 为现有 `parse_sqllog_file_5mb` 添加 `Throughput::Bytes`，新增 `parse_sqllog_file_5mb_rps`（`Throughput::Elements`）
- 新增多行语料库变体：`parse_sqllog_multiline_5mb`（GB/s）和 `parse_sqllog_multiline_5mb_rps`（records/sec）
- 新增 `parse_sqllog_metrics_5mb` 变体，在 bench 循环内调用 `parse_performance_metrics()`，覆盖真实热路径

## Task Commits

每个任务原子提交：

1. **Task 1: 新增多行合成语料库生成函数** - `ece369c` (feat)
2. **Task 2: 注册 5 个新 benchmark 变体** - `0ccb6f9` (feat)

**Plan metadata:** （待 SUMMARY 提交后记录）

## Files Created/Modified

- `benches/parser_benchmark.rs` - 新增 `generate_synthetic_log_multiline` 函数和 5 个 benchmark 变体（`parse_sqllog_file_5mb_rps`、`parse_sqllog_multiline_5mb`、`parse_sqllog_multiline_5mb_rps`、`parse_sqllog_metrics_5mb`），以及现有变体的 `Throughput::Bytes` 配置

## Decisions Made

- 使用取模计数（`record_index % 5 == 0`）而非随机数控制多行比例，确保语料库确定性，benchmark 结果可复现
- `parse_sqllog_metrics_5mb` 复用多行语料库，使测量结果反映含多行 SQL 的真实工作负载热路径
- `_rps` 变体在 bench 循环外预计算 `record_count`（通过 `filter_map(|r| r.ok()).count()`），避免统计误差

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 01 Plan 01 完成，benchmark 基础设施就绪
- `cargo bench --bench parser_benchmark` 现在可同时输出 GB/s 和 records/sec，覆盖单行/多行语料库和 parse_performance_metrics() 热路径
- Phase 01 Plan 02（baseline 记录）可立即执行

## Self-Check

- [x] `benches/parser_benchmark.rs` 存在并包含所有新函数和变体
- [x] commit `ece369c` 存在（Task 1）
- [x] commit `0ccb6f9` 存在（Task 2）
- [x] `cargo bench --bench parser_benchmark -- --list` 输出 5 个变体
- [x] 编译无错误

## Self-Check: PASSED

---
*Phase: 01-measurement*
*Completed: 2026-04-19*
