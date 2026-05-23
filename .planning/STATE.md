---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Refactor, Filter & Async
status: milestone_archived
last_updated: 2026-05-23T00:00:00+08:00
progress:
  total_phases: 3
  completed_phases: 3
  total_plans: 6
  completed_plans: 6
  percent: 100
stopped_at: Milestone archived (v2.0 complete)
---

# STATE: dm-database-parser-sqllog

*This file is the project's working memory. Updated at phase transitions and plan completions.*

---

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-23 after v2.0)

**Core value:** 在任意硬件上达到尽可能高的解析吞吐量（records/sec 和 GB/s），同时提供符合 Rust 生态习惯的易用 API。
**Current focus:** v2.0 里程碑已归档，等待下一里程碑规划

---

## Shipped Versions

| Version | Name | Shipped | Phases | Highlights |
|---------|------|---------|--------|-----------|
| v1.0 | Performance Optimization | 2026-04-26 | 1–5 | 8.67 GiB/s 单线程（+35.5%） |
| v1.1 | API & Ergonomics | 2026-05-19 | 6–9 | LogParserBuilder + FilterBuilder API + crates.io |
| v2.0 | Refactor, Filter & Async | 2026-05-23 | 10–12 | 功能分层重组 + 56 谓词 FilterBuilder + tokio async |

---

## Performance Metrics

**Baseline (at project start):**

- File: 5 MB synthetic, uniform single-line records (~206 bytes each)
- Throughput: ~7.6 GB/s, ~674,425 ns total（仅 iter().count()）

**Final (v1.0 shipped):**

- memmem 混合快速路径（Phase 4） + 两阶段并行架构（Phase 5）
- Single-thread: **8.67 GiB/s（+35.5% vs Phase 3 基线）**
- Parallel (10 cores): 8.57 GiB/s（speedup ≈ 1.01x — Amdahl 定律限制）

---

## Known Gaps

- **PAR-02 speedup 1.01x（目标 ≥1.6x）**：已 accept-as-is，理由：index() 串行扫描主导，Amdahl 定律决定并行无收益

---

## Accumulated Context

### Key Decisions

| Decision | Rationale |
|----------|-----------|
| v2.0 3 个阶段（粒度 coarse）| 7+10+4 需求自然对应重构/过滤/异步三层构建顺序 |
| Phase 12 async 返回 Vec<Sqllog<'static>> | mmap 是同步内存访问；spawn_blocking 内部需 owned 数据，打破 Cow<'a> 生命周期 |
| tokio 作为可选 feature | 不应强制所有用户引入 tokio 依赖树 |

---

## Session Continuity

**Last updated:** 2026-05-23 — v2.0 milestone archived (3 phases, 6 plans, 21 requirements)
**Next action:** `/gsd:new-milestone` 规划下一里程碑

---
*Archived: 2026-05-23 — v2.0 Refactor, Filter & Async complete*
