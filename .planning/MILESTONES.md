# MILESTONES

## v1.0 — Performance Optimization

**Shipped:** 2026-04-26
**Phases:** 5 | **Plans:** 10
**Timeline:** 2026-04-18 → 2026-04-26（8 天）
**Code:** ~2,173 行 Rust（src + tests + benches）
**Commits:** 190

### Key Accomplishments

1. 建立 criterion benchmark 基础设施：GB/s + records/sec 双维度吞吐量，20% 多行合成语料库，CI 5% 回归门禁
2. 修复 unsafe 解码路径正确性风险：全文件 head+tail 编码检测，Miri CI 覆盖，find_indicators_split 验证守卫
3. 热路径微优化：O(1) 早退 + 单次 `memrchr` 扫描 + `#[inline(always)]` + mmap Sequential 建议
4. CoreAlgo 重写：memmem SIMD 混合快速路径 **+35.5% 单线程吞吐**（8.67 GiB/s），u64 掩码时间戳验证
5. 引入 `RecordIndex` + `LogParser::index()` 两阶段并行架构（PAR-01/PAR-03 完全达成）

### Known Gaps

- **PAR-02 speedup 目标未达**：par_iter() 实测 1.01x（目标 ≥1.6x）。根本原因：Amdahl 定律。index() 顺序扫描主导端到端时间，并行解析计算量极轻，线程扩展无收益。决策：accept-as-is。

### Archive

- `.planning/milestones/v1.0-ROADMAP.md` — 完整阶段归档
- `.planning/milestones/v1.0-REQUIREMENTS.md` — 需求完成状态归档

---
