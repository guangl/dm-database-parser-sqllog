# Requirements: dm-database-parser-sqllog

**Defined:** 2026-05-19
**Milestone:** v1.1 — API & Ergonomics
**Core Value:** 在任意硬件上达到尽可能高的解析吞吐量（records/sec 和 GB/s）

## v1.1 Requirements

### ERR — 错误处理

- [ ] **ERR-01**: 用户可获取包含行号和原始内容的详细错误信息（ParseError 变体细化）
- [ ] **ERR-02**: 用户可选择跳过或捕获 LogIterator 内部的解析错误，而不是被默默丢弃
- [ ] **ERR-03**: ParseError 实现标准 `std::error::Error + Display + Debug` trait

### DOC — 文档与示例

- [ ] **DOC-01**: 所有 pub 类型、方法、字段均有 rustdoc 注释
- [ ] **DOC-02**: crate-level 文档包含 3-5 个可运行的 Quick Start 代码示例
- [ ] **DOC-03**: `examples/` 目录包含至少 2 个独立可运行示例（如 filter_slow_queries.rs）

### API — 新接口与易用性

- [ ] **API-01**: `LogParserBuilder` 支持线程数、并行阈值、编码提示的链式配置
- [ ] **API-02**: Iterator 提供专用过滤方法（`filter_by_exec_time`、`filter_by_sql_contains` 等）
- [ ] **API-03**: `Sqllog` 提供直接字段访问方法（`exec_time()`、`row_count()` 等）避免手动解构
- [ ] **API-04**: `FromSqllog` trait 允许用户将 `Sqllog` 映射到自定义类型

### PUB — 发布准备

- [ ] **PUB-01**: 新增 `CHANGELOG.md`（Keep a Changelog 格式，含 v1.0 历史和 v1.1 变更）
- [ ] **PUB-02**: `Cargo.toml` 完整填写 description、keywords、categories、repository、documentation 字段
- [ ] **PUB-03**: `README.md` 包含安装说明、快速开始、功能列表、性能数据

## Future Requirements

### 错误处理增强

- **ERR-04**: 结构化错误上下文（记录解析状态机中间状态，方便调试复杂格式错误）

### API 扩展

- **API-05**: 通用过滤适配器 `.filter_sqllog(|s| s.exec_time() > 100)` 形式（更灵活但增加抽象）
- **API-06**: 流式/增量解析 API（文件追加写入场景，需重新设计生命周期）

## Out of Scope

| Feature | Reason |
|---------|--------|
| async/tokio 集成 | 破坏零拷贝 `Cow<'a>` 生命周期设计 |
| GB18030 深度优化 | 场景罕见，收益不高 |
| 自定义 SIMD 换行扫描 | `memchr` 已是天花板 |
| 全局默认 mimalloc | 库 crate 不应强制用户分配器 |
| 支持新日志格式 | 超出本 milestone API 优化范围 |

## Traceability

*(由 roadmapper 填写)*

| Requirement | Phase | Status |
|-------------|-------|--------|
| ERR-01 | — | Pending |
| ERR-02 | — | Pending |
| ERR-03 | — | Pending |
| DOC-01 | — | Pending |
| DOC-02 | — | Pending |
| DOC-03 | — | Pending |
| API-01 | — | Pending |
| API-02 | — | Pending |
| API-03 | — | Pending |
| API-04 | — | Pending |
| PUB-01 | — | Pending |
| PUB-02 | — | Pending |
| PUB-03 | — | Pending |

**Coverage:**
- v1.1 requirements: 13 total
- Mapped to phases: 0 (pending roadmap)
- Unmapped: 13

---
*Requirements defined: 2026-05-19*
*Last updated: 2026-05-19 — initial v1.1 definition*
