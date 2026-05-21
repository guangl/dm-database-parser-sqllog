# MILESTONES

## v1.1 API & Ergonomics (Shipped: 2026-05-19)

**Phases completed:** 4 phases, 9 plans, 9 tasks
**Timeline:** 2026-05-19（1 天）
**Code:** 1,453 行 Rust（src）
**Commits:** 53

### Key Accomplishments

1. **ParseError 增强** — line_number 字段 + skip_errors() 错误策略，错误信息包含行号和原始内容
2. **LogParserBuilder 链式 API** — 取代 LogParser::from_path，支持 threads()/parallel_threshold()/encoding_hint()
3. **过滤方法 + 字段访问** — filter_by_exec_time/filter_by_sql_contains + exec_time()/row_count() + FromSqllog trait
4. **文档全覆盖** — rustdoc 零 warning + 3 个可运行 # Examples + 2 个独立示例
5. **crates.io 发布就绪** — CHANGELOG v1.1.0 + Cargo.toml 元数据 + README 6 节重写 + cargo publish --dry-run 通过

### Archive

- `.planning/milestones/v1.1-ROADMAP.md` — 完整阶段归档
- `.planning/milestones/v1.1-REQUIREMENTS.md` — 需求完成状态归档

---

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
