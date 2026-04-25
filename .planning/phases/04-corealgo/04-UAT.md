---
status: testing
phase: 04-corealgo
source: [04-01-PLAN.md]
started: 2026-04-25T00:00:00Z
updated: 2026-04-25T00:00:00Z
---

## Current Test

number: 5
name: 覆盖率 ≥90%
expected: |
  cargo llvm-cov --workspace --all-features --fail-under-lines 90 通过，
  总行覆盖率不低于 90%。
awaiting: user response

## Tests

### 1. cargo test 全部通过
expected: cargo test 输出 "21 passed; 0 failed"，无 panic
result: pass

### 2. 内层 memchr 逐行循环已消除
expected: grep -n "while let Some.*memchr" src/parser.rs 返回空（ALGO-01 核心目标）
result: pass

### 3. FINDER_RECORD_START.find_iter 在主要路径使用
expected: grep -c "FINDER_RECORD_START\.find_iter" src/parser.rs 返回 2（LogIterator::next 和 find_next_record_start 各一处）
result: issue
reported: "grep 返回 3，比预期多一处。LogIterator::next 的多行慢速路径中有两处 find_iter 调用（行 127 和 142），find_next_record_start 有一处（行 200）。"
severity: minor

### 4. is_timestamp_start() + u64 掩码常量存在
expected: grep 到 fn is_timestamp_start（行 404）、LO_MASK（行 396）、LO_EXPECTED 常量，证明 ALGO-02 已实现
result: pass

### 5. 覆盖率 ≥90%
expected: cargo llvm-cov --workspace --all-features --fail-under-lines 90 通过，总行覆盖率 ≥90%
result: issue
reported: "总行覆盖率 80.69%（parser.rs: 70.35%，sqllog.rs: 90.64%），低于 90% 门槛，命令以非零码退出。"
severity: major

### 6. Benchmark 单线程吞吐提升 ≥10%
expected: cargo bench 显示 parse_sqllog_file_5mb 相比 Phase 3 基线（674,425 records/sec）提升 ≥10%
result: issue
reported: "parse_sqllog_file_5mb: 吞吐下降 12-13%（regression）。parse_sqllog_multiline_5mb 提升约 +10%。parse_sqllog_metrics_5mb 吞吐下降约 18%。"
severity: major

## Summary

total: 6
passed: 3
issues: 3
pending: 0
skipped: 0

## Gaps

- truth: "cargo llvm-cov --workspace --all-features --fail-under-lines 90 通过，总行覆盖率 ≥90%"
  status: failed
  reason: "User reported: 总行覆盖率 80.69%（parser.rs 70.35%），低于 90% 阈值"
  severity: major
  test: 5
  artifacts: []
  missing: []

- truth: "cargo bench parse_sqllog_file_5mb 吞吐相比 Phase 3 基线提升 ≥10%"
  status: failed
  reason: "User reported: parse_sqllog_file_5mb 吞吐下降 12-13%，parse_sqllog_metrics_5mb 下降 18%，仅 multiline 提升约 10%"
  severity: major
  test: 6
  artifacts: []
  missing: []

- truth: "grep -c FINDER_RECORD_START.find_iter src/parser.rs 输出 2（LogIterator::next 和 find_next_record_start 各一处）"
  status: failed
  reason: "User reported: 实际返回 3，LogIterator::next 多行路径包含两处 find_iter 调用"
  severity: minor
  test: 3
  artifacts: []
  missing: []
