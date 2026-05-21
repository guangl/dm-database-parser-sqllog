# Requirements: dm-database-parser-sqllog

**Defined:** 2026-05-22
**Core Value:** 在任意硬件上达到尽可能高的解析吞吐量（records/sec 和 GB/s），同时提供符合 Rust 生态习惯的易用 API。

## v2.0 Requirements

### Refactor（代码重构）

- [ ] **REFACTOR-01**: 开发者可通过 `src/parser/` 子模块找到所有解析相关代码（LogParser、LogParserBuilder、迭代器）
- [ ] **REFACTOR-02**: 开发者可通过 `src/filter/` 子模块找到所有过滤相关代码（FilterBuilder、各字段谓词）
- [ ] **REFACTOR-03**: 开发者可通过 `src/async_api/` 子模块找到所有异步接口代码
- [ ] **REFACTOR-04**: `src/record.rs` 包含 `Sqllog` 结构体定义，`src/error.rs` 包含 `ParseError`
- [ ] **REFACTOR-05**: `tools.rs` 中的字节级工具函数分配到合适子模块，不作为公开 API 暴露
- [ ] **REFACTOR-06**: `lib.rs` 顶层重导出所有公开类型，用户侧 `use dm_database_parser_sqllog::LogParser` 等路径保持有效
- [ ] **REFACTOR-07**: `examples/` 和 rustdoc 示例更新以反映新模块结构和新 API

### Filter（全字段过滤）

- [ ] **FILTER-01**: 用户可对 `ts`（时间戳字符串）进行 contains / eq / starts_with / ends_with 过滤
- [ ] **FILTER-02**: 用户可对 `tag`（Option<String>）进行存在性检查和值匹配过滤
- [ ] **FILTER-03**: 用户可对 `ep`（u8）进行 eq / gt / lt / between 过滤
- [ ] **FILTER-04**: 用户可对 `sess_id`、`thrd_id`、`username`、`trxid`、`statement`、`appname`、`client_ip` 进行 contains / eq / starts_with / ends_with 过滤
- [ ] **FILTER-05**: 用户可对 `sql` 进行 contains / eq / starts_with / ends_with 过滤
- [ ] **FILTER-06**: 用户可对 `exectime`（f32）进行 gt / lt / between 过滤
- [ ] **FILTER-07**: 用户可对 `rowcount`（u32）进行 eq / gt / lt / between 过滤
- [ ] **FILTER-08**: 用户可对 `exec_id`（i64）进行 eq / gt / lt / between 过滤
- [ ] **FILTER-09**: 用户可将多个过滤条件链式 AND 组合，通过一次迭代完成多条件筛选
- [ ] **FILTER-10**: FilterBuilder 与现有 `LogParser` 迭代器无缝集成（返回 `impl Iterator<Item = Sqllog>`）

### Async（异步 API）

- [ ] **ASYNC-01**: 用户可在 `async fn` 中直接 `await` 解析整个日志文件，无需手写 `spawn_blocking`
- [ ] **ASYNC-02**: 异步 API 内部使用 `tokio::task::spawn_blocking` 封装同步 mmap 解析，不破坏零拷贝内核路径
- [ ] **ASYNC-03**: tokio 依赖通过 `features = ["async"]` 可选引入，不使用异步 API 的用户无需依赖 tokio
- [ ] **ASYNC-04**: 异步 API 支持过滤条件传入（与 FilterBuilder 集成）

## Future Requirements

### Stream API

- **STREAM-01**: 用户可通过 `impl Stream<Item = Sqllog>` 逐条异步处理超大日志文件
- **STREAM-02**: Stream 支持背压（backpressure），避免全量加载到内存

### OR 过滤组合

- **FILTER-OR-01**: 用户可将多个过滤条件以 OR 逻辑组合（当前仅 AND）

## Out of Scope

| Feature | Reason |
|---------|--------|
| async/await 内部解析（真正异步 mmap）| mmap 本质是同步内存访问，tokio 无法异步化内核页故障 |
| 正则表达式过滤 | 引入 regex 依赖较重，用户可通过 `.filter(closure)` 自行实现 |
| SQL 语法解析（SELECT/WHERE 分析）| 超出日志解析范畴，是独立功能域 |
| 并行过滤（Rayon）| 不在本里程碑，可在 v2.1 评估 |
| GB18030 编码路径深度优化 | 场景罕见，收益不高 |
| crate 名称简化 | 名称变更影响所有用户 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| REFACTOR-01 | — | Pending |
| REFACTOR-02 | — | Pending |
| REFACTOR-03 | — | Pending |
| REFACTOR-04 | — | Pending |
| REFACTOR-05 | — | Pending |
| REFACTOR-06 | — | Pending |
| REFACTOR-07 | — | Pending |
| FILTER-01 | — | Pending |
| FILTER-02 | — | Pending |
| FILTER-03 | — | Pending |
| FILTER-04 | — | Pending |
| FILTER-05 | — | Pending |
| FILTER-06 | — | Pending |
| FILTER-07 | — | Pending |
| FILTER-08 | — | Pending |
| FILTER-09 | — | Pending |
| FILTER-10 | — | Pending |
| ASYNC-01 | — | Pending |
| ASYNC-02 | — | Pending |
| ASYNC-03 | — | Pending |
| ASYNC-04 | — | Pending |

**Coverage:**
- v2.0 requirements: 21 total
- Mapped to phases: 0（roadmap 待创建）
- Unmapped: 21 ⚠️

---
*Requirements defined: 2026-05-22*
*Last updated: 2026-05-22 after milestone v2.0 definition*
