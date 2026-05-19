# ROADMAP: dm-database-parser-sqllog

## Milestones

- ✅ **v1.0 Performance Optimization** — Phases 1–5 (shipped 2026-04-26)
- 🔄 **v1.1 API & Ergonomics** — Phases 6–9 (active)

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

### v1.1 API & Ergonomics

- [x] **Phase 6: ErrorHandling** - 重构 ParseError，提供详细上下文，暴露迭代器错误策略 (completed 2026-05-19)
- [x] **Phase 7: APIErgonomics** - 新增 Builder、直接字段访问、过滤方法、FromSqllog trait (completed 2026-05-19)
- [ ] **Phase 8: Documentation** - rustdoc 全覆盖 + crate-level quick start + examples/ 目录
- [ ] **Phase 9: Publishing** - CHANGELOG、Cargo.toml metadata、README，crates.io 发布就绪

## Phase Details

### Phase 6: ErrorHandling

**Goal**: 调用方能够获取有意义的错误信息，并自主决定如何处理迭代过程中的解析错误
**Depends on**: Phase 5 (v1.0)
**Requirements**: ERR-01, ERR-02, ERR-03
**Success Criteria** (what must be TRUE):

  1. 调用方拿到 ParseError 时，能看到行号和原始内容片段，定位问题无需猜测
  2. 调用方可选择让 LogIterator 在遇到解析错误时跳过、收集或中止，行为由调用方控制而非库静默丢弃
  3. ParseError 可直接用于实现 `std::error::Error` 的错误处理链（`?` 运算符、`anyhow`、`thiserror` 等生态均可接入）

**Plans**: 2 plans

```
Plans:
**Wave 1**

- [x] 06-01-PLAN.md — ParseError 添加 line_number 字段，LogIterator 行号追踪

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 06-02-PLAN.md — skip_errors() 方法 + 行号与错误处理全套测试

```

### Phase 7: APIErgonomics

**Goal**: 用户可以用流畅的链式 API 配置解析器、直接访问字段、过滤记录，并将 Sqllog 映射到自定义类型
**Depends on**: Phase 6
**Requirements**: API-01, API-02, API-03, API-04
**Success Criteria** (what must be TRUE):

  1. 用户可以通过 `LogParserBuilder::new().threads(4).parallel_threshold(32 * 1024 * 1024).build()` 形式完成解析器配置，无需直接构造结构体
  2. 用户可以调用 `iter().filter_by_exec_time(100)` 或 `filter_by_sql_contains("SELECT")` 等方法过滤记录，不需要手动写闭包解构 parse_performance_metrics
  3. 用户可以直接调用 `sqllog.exec_time()` / `sqllog.row_count()` 取值，不需要解构元组或 match parse_performance_metrics 的返回
  4. 用户可以实现 `FromSqllog` trait 将 Sqllog 转为自定义业务类型，并通过 `.map(T::from_sqllog)` 组合使用

**Plans**: 3 plans

```
Plans:

**Wave 1**

- [x] 07-01-PLAN.md — LogParserBuilder 链式 API，移除 LogParser::from_path

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 07-02-PLAN.md — LogIterator 过滤方法（filter_by_exec_time / filter_by_sql_contains）
- [x] 07-03-PLAN.md — Sqllog 直接字段访问（exec_time/row_count）+ FromSqllog trait

```

### Phase 8: Documentation

**Goal**: 任何 Rust 开发者打开 docs.rs 页面或本地 `cargo doc` 后，能在 5 分钟内理解库用法并写出可运行代码
**Depends on**: Phase 7
**Requirements**: DOC-01, DOC-02, DOC-03
**Success Criteria** (what must be TRUE):

  1. `cargo doc --no-deps` 无任何 missing_docs 警告，所有公开类型、方法、字段均有说明文字
  2. crate 根文档中至少 3 个 `# Examples` 代码块可通过 `cargo test --doc` 运行且通过
  3. `examples/` 目录包含至少 2 个独立二进制示例（如 `filter_slow_queries.rs`），可用 `cargo run --example <name>` 直接执行

**Plans**: 3 plans

```
Plans:

- [ ] 08-01-PLAN.md — parser.rs 和 error.rs 公开 API 中文 rustdoc（LogParser、LogIterator、LogParserBuilder、过滤方法、parse_record + error.rs line_number 字段）
- [ ] 08-02-PLAN.md — sqllog.rs 新增 API rustdoc + lib.rs crate-level 文档（3 个可运行 # Examples）
- [ ] 08-03-PLAN.md — examples/ 目录（filter_slow_queries.rs、batch_export.rs）

```

**UI hint**: no

### Phase 9: Publishing

**Goal**: 库达到 crates.io 发布标准，版本历史可追溯，新用户能通过 README 独立完成集成
**Depends on**: Phase 8
**Requirements**: PUB-01, PUB-02, PUB-03
**Success Criteria** (what must be TRUE):

  1. `CHANGELOG.md` 存在且遵循 Keep a Changelog 格式，v1.0 和 v1.1 变更均有记录，`cargo changelog` 或人工审阅均可验证格式
  2. `cargo publish --dry-run` 成功，Cargo.toml 中 description、keywords、categories、repository、documentation 字段均已填写且无警告
  3. README.md 包含安装说明（`Cargo.toml` 片段）、Quick Start 代码、功能列表、v1.0 性能数据，新用户无需阅读源码即可上手

**Plans**: 1 plan

```
Plans:

- [ ] 09-01-PLAN.md — CHANGELOG v1.1.0、Cargo.toml metadata 完善、README 重写

```

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Measurement | v1.0 | 2/2 | Complete | 2026-04-20 |
| 2. Correctness | v1.0 | 2/2 | Complete | 2026-04-20 |
| 3. HotPath | v1.0 | 2/2 | Complete | 2026-04-24 |
| 4. CoreAlgo | v1.0 | 1/1 | Complete | 2026-04-25 |
| 5. Parallel | v1.0 | 3/3 | Complete | 2026-04-26 |
| 6. ErrorHandling | v1.1 | 2/2 | Complete   | 2026-05-19 |
| 7. APIErgonomics | v1.1 | 3/3 | Complete   | 2026-05-19 |
| 8. Documentation | v1.1 | 0/3 | Not started | - |
| 9. Publishing | v1.1 | 0/1 | Not started | - |

---
*Updated: 2026-05-19 — Phase 8 plans created (3 plans)*
