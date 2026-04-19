---
phase: 01-measurement
plan: "02"
subsystem: ci-infrastructure
tags: [ci, benchmark, regression-gate, baseline]
dependency_graph:
  requires: []
  provides: [ci-regression-gate, baseline-update-workflow]
  affects: [.github/workflows/benchmark.yml]
tech_stack:
  added: []
  patterns: [bash-script, github-actions-workflow-dispatch, python3-json-processing]
key_files:
  created:
    - scripts/check-regression.sh
    - .github/workflows/update-baseline.yml
  modified:
    - .github/workflows/benchmark.yml
decisions:
  - "5% 退化阈值（THRESHOLD=5），基于 D-11 规范"
  - "baseline key 格式为 {group}/{bench_name}，不含 _rps 变体（D-09）"
  - "update-baseline.yml 使用 python3 + json 模块提取 mean.point_estimate，无额外依赖"
metrics:
  duration_seconds: 300
  completed_date: "2026-04-19"
  tasks_completed: 2
  tasks_total: 2
  files_created: 2
  files_modified: 1
---

# Phase 1 Plan 02: CI 回归门禁基础设施 Summary

## One-liner

基于 criterion estimates.json 的 5% 阈值回归门禁脚本（check-regression.sh）及手动触发 baseline 重标定 workflow（update-baseline.yml）。

## What Was Built

创建了完整的 CI 回归门禁基础设施，包含三个关键产物：

1. **scripts/check-regression.sh** — 本地可复现的回归检测脚本，遍历 baseline.json 所有 key，对比 criterion estimates.json 中的 mean.point_estimate，超过 5% 时 exit 1 并输出 baseline/current/regression 三行报告（D-10 格式）
2. **benchmark.yml** — 在 Run parser benchmarks 之后插入 Check regression vs baseline step，调用 check-regression.sh
3. **update-baseline.yml** — 手动触发（workflow_dispatch）的 baseline 重标定 workflow，运行 bench 后通过 python3 提取所有非 _rps 变体的 mean.point_estimate，commit message 含 [skip ci] 避免 CI 循环触发

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | 创建 CI 回归门禁脚本 | 619a680 | scripts/check-regression.sh (new) |
| 2 | 修改 benchmark.yml 添加门禁 step，创建 update-baseline.yml | 6e872b6 | .github/workflows/benchmark.yml (modified), .github/workflows/update-baseline.yml (new) |

## Verification Results

- `bash -n scripts/check-regression.sh` 语法通过
- benchmark.yml 中 Check regression step（第51行）位于 Run parser benchmarks（第48行）之后
- update-baseline.yml 包含 workflow_dispatch、[skip ci]、_rps 跳过逻辑
- benchmarks/baseline.json 格式有效，key 为 `parser_group/parse_sqllog_file_5mb`

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all implementation is complete and functional.

## Threat Flags

未发现计划外的新安全面。T-01-02（update-baseline.yml git push）和 T-01-03（check-regression.sh 无限循环）均在计划 threat_model 中覆盖，按 accept 处置。

## Self-Check

- [x] scripts/check-regression.sh 存在且有可执行权限
- [x] .github/workflows/update-baseline.yml 存在
- [x] .github/workflows/benchmark.yml 已修改，含 Check regression step
- [x] 提交 619a680 存在
- [x] 提交 6e872b6 存在
