# dm-database-parser-sqllog 性能优化

## What This Is

高性能 Rust 库 `dm-database-parser-sqllog` 的性能优化与 API 完善项目。该库用于解析达梦数据库 SQL 日志文件，支持内存映射 I/O、零拷贝延迟解析、SIMD UTF-8 校验、memmem SIMD 混合边界检测、以及两阶段并行 RecordIndex。v1.0 通过 5 个阶段的系统优化，单线程吞吐提升 35.5%，达到 8.67 GiB/s。v1.1 通过 4 个阶段完善了 API 易用性（LogParserBuilder、过滤方法、字段访问、FromSqllog）、文档质量和 crates.io 发布准备。

## Core Value

在任意硬件上达到尽可能高的解析吞吐量（records/sec 和 GB/s），同时提供符合 Rust 生态习惯的易用 API。

## Requirements

### Validated

<details>
<summary>v1.0 — Performance Optimization</summary>

- ✓ 内存映射文件 I/O（`memmap2`）— v1.0
- ✓ 零拷贝 `Cow<'a, str>` 延迟字段解析 — v1.0
- ✓ SIMD UTF-8 校验（`simdutf8`）— v1.0
- ✓ 预构建 SIMD Finder（`memchr::memmem`）— v1.0
- ✓ 单线程迭代器 `iter()` — v1.0
- ✓ Rayon 并行迭代器 `par_iter()` — v1.0
- ✓ 延迟字段解析（`parse_performance_metrics` 等）— v1.0
- ✓ criterion benchmark：GB/s + records/sec 双维度 + 多行语料库 + metrics 变体 — v1.0 (MEAS-01/02/03)
- ✓ CI benchmark 回归门禁（baseline.json，5% 阈值）— v1.0 (MEAS-04)
- ✓ 全文件 head+tail 编码检测，消除 64 KB 截断 UB — v1.0 (CORR-01)
- ✓ Miri CI 覆盖 unsafe 解码路径 — v1.0 (CORR-02)
- ✓ find_indicators_split 验证守卫 + 边界测试 — v1.0 (CORR-03)
- ✓ `find_indicators_split` O(1) 早退 + 单次 memrchr — v1.0 (HOT-01/02)
- ✓ `#[inline(always)]` 热路径 + `#[cold]` 错误路径 + mmap Sequential — v1.0 (HOT-03/04)
- ✓ memmem SIMD 混合快速路径 + u64 掩码时间戳验证：+35.5% 单线程吞吐 — v1.0 (ALGO-01/02)
- ✓ RecordIndex + index() 两阶段并行 API，32 MB 阈值自动退化串行 — v1.0 (PAR-01/03)

</details>

<details>
<summary>v1.1 — API & Ergonomics</summary>

- ✓ ParseError 行号追踪 + skip_errors() 错误策略 — v1.1 (ERR-01/02/03)
- ✓ LogParserBuilder 链式 API（取代 LogParser::from_path）— v1.1 (API-01)
- ✓ filter_by_exec_time / filter_by_sql_contains 过滤方法 — v1.1 (API-02)
- ✓ exec_time() / row_count() 直接字段访问 — v1.1 (API-03)
- ✓ FromSqllog trait 自定义类型转换 — v1.1 (API-04)
- ✓ rustdoc 全覆盖（零 warning）— v1.1 (DOC-01)
- ✓ crate-level 3 个可运行 # Examples — v1.1 (DOC-02)
- ✓ examples/ 目录 2 个独立示例 — v1.1 (DOC-03)
- ✓ CHANGELOG.md v1.1.0（Keep a Changelog）— v1.1 (PUB-01)
- ✓ Cargo.toml 元数据完善（keywords 含 dameng）— v1.1 (PUB-02)
- ✓ README.md 6 节中文重写（安装 / Quick Start / 功能 / 性能 / API）— v1.1 (PUB-03)

</details>

### Active

（下一里程碑需求定义中）

### Known Gaps

- ⚠ PAR-02 speedup ≥1.6x 目标未达（实测 1.01x）— Amdahl 定律限制。若需多线程加速，须重新设计工作负载

### Out of Scope

- 支持新日志格式 — 功能需求，不在优化范围
- GB18030 编码路径深度优化 — 场景罕见，收益不高
- 自定义 SIMD 换行扫描（packed_simd / std::simd）— `memchr` 已是天花板
- async/tokio 集成 — 破坏零拷贝 `Cow<'a>` 生命周期设计
- 全局默认 mimalloc — 库 crate 不应强制用户分配器
- crate 名称简化（如 dm-sqllog）— 名称变更影响所有用户

## Context

- **v1.0 最终吞吐**：8.67 GiB/s 单线程（5 MB 合成语料库，含 20% 多行）
- **v1.1 代码量**：1,453 行 Rust（src）
- **v1.1 变更规模**：21 文件，+1089/-241 行，53 commits
- **v1.1 里程碑**：4 阶段，9 计划，2026-05-19 一天完成
- **测试覆盖**：≥90% 行覆盖率，88 个测试 + 8 个 doc-tests
- **并行上限**：受 Amdahl 定律约束，par_iter() 在 8.4 GiB/s 吞吐下接近单核利用上限

## Constraints

- **Tech**: 纯 Rust
- **Correctness**: 所有现有测试必须通过，覆盖率 ≥ 90%
- **API**: 可以修改，但需兼顾库用户体验

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 使用 mmap 而非 read | 零拷贝、OS 缓存友好 | ✓ Good |
| 延迟字段解析 | 只解析调用方需要的字段 | ✓ Good |
| SIMD Finder 静态预构建 | 避免每次调用重建 Finder | ✓ Good |
| Phase 顺序：Measurement first | 先建可信测量，后续优化有数据支撑 | ✓ Good |
| CORR-01: head+tail 全文件采样 | 消除大文件 GB18030 UB | ✓ Good |
| Phase 4 混合快速路径 | 纯 memmem 实测 -35%；单行记录需 memchr 快速路径 | ✓ 关键修正 |
| Phase 5 accept-as-is（PAR-02） | Amdahl 定律：index() 串行主导，并行无收益 | ✓ 有据可查 |
| v1.1 跳版到 1.1.0（不发 1.0.0） | v1.0 milestone 是内部优化，API 未稳定；v1.1 是第一个语义化版本 | ✓ Good |
| README 使用中文 | 达梦数据库用户主要是中文开发者 | ✓ Good |

## Evolution

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-19 — v1.1 milestone shipped*
