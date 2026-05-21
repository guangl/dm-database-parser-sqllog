# ROADMAP: dm-database-parser-sqllog

## Milestones

- ✅ **v1.0 Performance Optimization** — Phases 1–5 (shipped 2026-04-26)
- ✅ **v1.1 API & Ergonomics** — Phases 6–9 (shipped 2026-05-19)
- 🚧 **v2.0 Refactor, Filter & Async** — Phases 10–12 (active)

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

### v2.0 Refactor, Filter & Async

- [ ] **Phase 10: Restructure** - 将 src/ 重组为功能分层子模块，保持所有现有测试通过
- [ ] **Phase 11: FilterBuilder** - 实现全字段可组合链式过滤，与 LogParser 迭代器集成
- [ ] **Phase 12: AsyncAPI** - 引入 tokio async 封装及 feature flag，与 FilterBuilder 集成

## Phase Details

### Phase 10: Restructure

**Goal**: src/ 目录按功能分层重组完毕，库用户侧导入路径不变，所有现有测试继续通过

**Depends on**: Nothing (first phase of milestone)

**Requirements**: REFACTOR-01, REFACTOR-02, REFACTOR-03, REFACTOR-04, REFACTOR-05, REFACTOR-06, REFACTOR-07

**Success Criteria** (what must be TRUE):
  1. `src/parser/` 子模块包含 LogParser、LogParserBuilder、LogIterator 等所有解析相关代码；`src/filter/` 子模块包含所有过滤相关骨架；`src/async_api/` 子模块存在（即使暂为空模块）；`src/record.rs` 包含 Sqllog，`src/error.rs` 包含 ParseError
  2. `use dm_database_parser_sqllog::LogParser` / `LogParserBuilder` / `Sqllog` / `ParseError` 等所有原有公开导入路径在用户侧仍然有效（通过 lib.rs 重导出验证）
  3. `tools.rs` 中的字节级工具函数已并入对应子模块，不出现在公开 API（`cargo doc --open` 中不可见）
  4. `cargo test` 全量通过，`cargo clippy -- -D warnings` 零警告，覆盖率 ≥90%
  5. `examples/` 目录和 lib.rs 顶层 rustdoc 示例已更新，`cargo test --doc` 全部通过

**Plans**: TBD

---

### Phase 11: FilterBuilder

**Goal**: 用户可对所有 14 个 Sqllog 字段链式组合过滤条件，并通过 LogParser 迭代器直接使用

**Depends on**: Phase 10

**Requirements**: FILTER-01, FILTER-02, FILTER-03, FILTER-04, FILTER-05, FILTER-06, FILTER-07, FILTER-08, FILTER-09, FILTER-10

**Success Criteria** (what must be TRUE):
  1. 用户可以通过 `FilterBuilder::new().ts_contains("2024").exec_time_gt(1.0).build()` 等链式调用构造组合过滤器，所有 14 个字段均有对应过滤方法
  2. 字符串字段（ts、tag 值、sess_id、thrd_id、username、trxid、statement、appname、client_ip、sql）各自支持 contains / eq / starts_with / ends_with 四种谓词；`tag` 还支持存在性检查（`tag_is_some()`）
  3. 数值字段（ep: u8、exectime: f32、rowcount: u32、exec_id: i64）各自支持适合类型的 eq / gt / lt / between 谓词
  4. 多条件链式调用自动以 AND 语义组合，单次迭代遍历完成多条件筛选（无中间 Vec 分配）
  5. `LogParser::iter()` 或 `LogParser::par_iter()` 可直接接受 FilterBuilder 产出的过滤器，`cargo test` 全量通过，覆盖率 ≥90%

**Plans**: TBD

---

### Phase 12: AsyncAPI

**Goal**: 用户可在 tokio async 运行时中直接 await 解析日志文件，且 tokio 依赖为可选 feature

**Depends on**: Phase 11

**Requirements**: ASYNC-01, ASYNC-02, ASYNC-03, ASYNC-04

**Success Criteria** (what must be TRUE):
  1. 用户可在 `async fn` 中调用 `parse_file_async(path).await` 得到日志记录集合，无需手动调用 `spawn_blocking`
  2. 内部实现通过 `tokio::task::spawn_blocking` 封装同步 mmap 解析，日志记录以 `Vec<Sqllog<'static>>` 形式返回（owned，生命周期独立）
  3. `Cargo.toml` 中 tokio 仅在 `features = ["async"]` 时引入；不声明该 feature 的项目 `cargo build` 后不引入 tokio 依赖树
  4. async API 接受 `FilterBuilder` 参数，用户可传入过滤条件，结果已在 `spawn_blocking` 内部完成过滤
  5. `cargo test --features async` 全量通过，覆盖率 ≥90%（含 async 代码路径）

**Plans**: TBD

---

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
| 10. Restructure | v2.0 | 0/? | Not started | - |
| 11. FilterBuilder | v2.0 | 0/? | Not started | - |
| 12. AsyncAPI | v2.0 | 0/? | Not started | - |

---
*Updated: 2026-05-22 — v2.0 roadmap created*
