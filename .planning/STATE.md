# STATE: dm-database-parser-sqllog 性能优化

*This file is the project's working memory. Updated at phase transitions and plan completions.*

---

## Project Reference

**Core Value:** 在任意硬件上达到尽可能高的解析吞吐量（records/sec 和 GB/s）
**Current Milestone:** v1 Performance Optimization
**Phases:** 5 total (Measurement → Correctness → HotPath → CoreAlgo → Parallel)

---

## Current Position

**Current Phase:** Phase 1 — Measurement
**Current Plan:** None (not started)
**Phase Status:** Not started
**Milestone Status:** Not started

```
Progress: [ Phase 1 ][ Phase 2 ][ Phase 3 ][ Phase 4 ][ Phase 5 ]
           [  ----  ] [  ----  ] [  ----  ] [  ----  ] [  ----  ]
```

---

## Performance Metrics

**Baseline (at project start):**
- File: 5 MB synthetic, uniform single-line records (~206 bytes each)
- Benchmark: `iter().count()` only
- Throughput: ~7.6 GB/s, ~674,425 ns total

**Current:**
- Same as baseline (no optimizations applied yet)

**Targets (Phase 4 exit):**
- Single-thread: ≥10% improvement over Phase 3 baseline on realistic corpus
- Multi-thread (Phase 5 exit): ≥1.6x single-thread at 2 threads on large files (>32 MB)

---

## Accumulated Context

### Key Decisions

| Decision | Rationale | Phase |
|----------|-----------|-------|
| Phase order: Measurement first | 当前 benchmark 只测 `iter().count()`，不反映真实热路径；先修测量再优化 | Init |
| Correctness before hot-path | unsafe 解码仅采样 64 KB，大文件有 UB 风险；必须在热路径改动前修复 | Init |
| coarse granularity → 5 phases | 需求自然分为 5 组，依赖链清晰，coarse 粒度保留原始分组 | Init |

### Known Risks

- **CI SIMD 目标不匹配**（MODERATE）：CI 无 `target-cpu=native` 则 AVX2 缺失，MEAS-04 门禁阈值需在 CI 实际环境下标定 baseline
- **合成语料库与真实日志差异**（HIGH）：真实 DM 日志含多行 SQL、GB18030，吞吐可能低 2–5x；MEAS-03 必须覆盖此场景

### Todos

- [ ] Phase 1 开始前确认 CI 环境是否支持 `target-cpu=native`（影响 MEAS-04 门禁阈值标定）

### Blockers

None.

---

## Session Continuity

**Last updated:** 2026-04-18 — Roadmap created, STATE initialized
**Next action:** `/gsd-plan-phase 1` — plan Phase 1 (Measurement)

---
*Updated: 2026-04-18 after roadmap creation*
