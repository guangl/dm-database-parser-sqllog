# ROADMAP: dm-database-parser-sqllog

## Milestones

- ✅ **v1.0 Performance Optimization** — Phases 1–5 (shipped 2026-04-26)
- ✅ **v1.1 API & Ergonomics** — Phases 6–9 (shipped 2026-05-19)

## Phases

<details>
<summary>✅ v1.0 Performance Optimization (Phases 1–5) — SHIPPED 2026-04-26</summary>

- [x] Phase 1: Measurement (2/2 plans) — completed 2026-04-20
- [x] Phase 2: Correctness (2/2 plans) — completed 2026-04-20
- [x] Phase 3: HotPath (2/2 plans) — completed 2026-04-24
- [x] Phase 4: CoreAlgo (1/1 plan) — completed 2026-04-25
- [x] Phase 5: Parallel (3/3 plans) — completed 2026-04-26

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>✅ v1.1 API & Ergonomics (Phases 6–9) — SHIPPED 2026-05-19</summary>

- [x] Phase 6: ErrorHandling (2/2 plans) — ParseError 行号追踪 + skip_errors() 错误策略
- [x] Phase 7: APIErgonomics (3/3 plans) — LogParserBuilder + 过滤方法 + 字段访问 + FromSqllog
- [x] Phase 8: Documentation (3/3 plans) — rustdoc 全覆盖 + crate-level examples + examples/
- [x] Phase 9: Publishing (1/1 plan) — CHANGELOG v1.1.0 + Cargo.toml + README 重写

Full details: `.planning/milestones/v1.1-ROADMAP.md`

</details>

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Measurement | v1.0 | 2/2 | Complete | 2026-04-20 |
| 2. Correctness | v1.0 | 2/2 | Complete | 2026-04-20 |
| 3. HotPath | v1.0 | 2/2 | Complete | 2026-04-24 |
| 4. CoreAlgo | v1.0 | 1/1 | Complete | 2026-04-25 |
| 5. Parallel | v1.0 | 3/3 | Complete | 2026-04-26 |
| 6. ErrorHandling | v1.1 | 2/2 | Complete | 2026-05-19 |
| 7. APIErgonomics | v1.1 | 3/3 | Complete | 2026-05-19 |
| 8. Documentation | v1.1 | 3/3 | Complete | 2026-05-19 |
| 9. Publishing | v1.1 | 1/1 | Complete | 2026-05-19 |

---
*Updated: 2026-05-19 — v1.1 milestone shipped*
