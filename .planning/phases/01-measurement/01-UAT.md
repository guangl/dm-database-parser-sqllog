---
status: complete
phase: 01-measurement
source:
  - .planning/phases/01-measurement/01-01-SUMMARY.md
  - .planning/phases/01-measurement/01-02-SUMMARY.md
started: 2026-04-20T00:00:00Z
updated: 2026-04-20T01:00:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Benchmark list includes all new variants
expected: |
  运行 `cargo bench --bench parser_benchmark -- --list`，输出中包含以下 5 个新变体：
  parse_sqllog_file_5mb_rps、parse_sqllog_multiline_5mb、parse_sqllog_multiline_5mb_rps、parse_sqllog_metrics_5mb（以及原有的 parse_sqllog_file_5mb）
result: pass

### 2. Regression gate passes on current baseline
expected: |
  先运行 `cargo bench --bench parser_benchmark`，再运行 `bash scripts/check-regression.sh`。
  所有 benchmark 输出 PASS: ... (N%)，脚本以 exit 0 结束，最后一行打印 "All benchmarks within threshold."
result: pass

### 3. Regression gate null protection works
expected: |
  在 baseline.json 中临时添加一个不存在的 key（如 "parser_group/fake_bench"），运行 `bash scripts/check-regression.sh`。
  脚本应打印 "WARNING: estimates file not found: ..." 并 skip，不应 crash 或产生除零错误，最终 exit 0（其它 key 均 pass）。
  测试完成后还原 baseline.json。
result: pass

### 4. update-baseline.yml has contents:write permission
expected: |
  查看 `.github/workflows/update-baseline.yml`，在 `update-baseline` job 下能看到：
  ```
  permissions:
    contents: write
  ```
  该字段存在且位于 job 级别（不是顶层）。
result: pass

### 5. benchmark.yml uses correct action versions
expected: |
  查看 `.github/workflows/benchmark.yml`，确认：
  - `upload-artifact` 使用 `@v4`（不是 v7）
  - `github-script` 使用 `@v7`（不是 v9）
  - PR comment 脚本调用 `execSync('bash scripts/check-regression.sh')` 而非读取 parser_bench.html
result: pass

## Summary

total: 5
passed: 5
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
