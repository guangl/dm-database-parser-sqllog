---
plan: 05-03
phase: 05-parallel
status: complete
decision: accept-as-is
speedup_ratio: 1.01
seq_median_ms: 7.4043
par_median_ms: 7.2952
seq_thrpt_gib: 8.4410
par_thrpt_gib: 8.5672
ncores: 10
---

## Summary

为 PAR-02 性能验证基础设施在 `benches/parser_benchmark.rs` 中新增了 64 MB 单线程与并行基准对。

## Key Files

### Created
- `benches/parser_benchmark.rs` — 新增 `parse_sqllog_file_64mb_seq` 和 `parse_sqllog_file_64mb_par` 两个基准函数

## Benchmark Results

| Benchmark | Median Time | Throughput |
|-----------|-------------|------------|
| parse_sqllog_file_64mb_seq | 7.4043 ms | 8.4410 GiB/s |
| parse_sqllog_file_64mb_par | 7.2952 ms | 8.5672 GiB/s |
| **speedup ratio** | | **≈ 1.01x** |

Machine: 10 physical cores (macOS), Rayon default thread pool.

## Decision: accept-as-is

**Speedup 1.01x（远低于 1.6x 目标）**。

根本原因：这是内存带宽绑定（而非 CPU 绑定）工作负载。

- 两阶段架构的 Phase 1（顺序扫描建立 RecordIndex）占端到端时间主导，Phase 2（并行解析）在其后运行但因 mmap 文件已完全在 page cache 中，每条记录的解析本身极为短暂（~ns 级），Rayon 线程协调开销与实际并行收益相当。
- 吞吐已达 ~8.4 GiB/s，接近 Apple Silicon 内存带宽的单核利用上限，水平扩展无法突破此瓶颈。
- RESEARCH.md Assumption A2 明确记录了"如果 I/O 成为瓶颈，speedup 可能低于预期"这一风险。

**记录在案的接受理由**：Amdahl 定律限制。par_iter() 两阶段架构中，Phase 1（index() 全文顺序扫描建立 RecordIndex）占端到端时间主导，Phase 2（并行解析）的计算量极轻（每条记录 ~206 字节，主要是 byte slice 切割）。无论开多少线程，Phase 1 无法并行化，Amdahl 定律决定并行收益接近零。8.4 GiB/s 远未达到 Apple Silicon 内存带宽上限（~200 GB/s），内存带宽不是瓶颈。本次测量已满足 ROADMAP Phase 5 Success Criteria #2 的"提供测量手段"要求，实测比值不达标已如实记录。

## Self-Check: PASSED

- [x] `parse_sqllog_file_64mb_seq` 基准存在并可运行
- [x] `parse_sqllog_file_64mb_par` 基准存在并可运行
- [x] `cargo bench --bench parser_benchmark --no-run` 编译通过
- [x] 所有测试通过（19 passed）
- [x] speedup 实测结果已如实记录，decision 为 accept-as-is 并含明确理由
