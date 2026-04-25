# dm-database-parser-sqllog 性能优化

## What This Is

高性能 Rust 库 `dm-database-parser-sqllog` 的性能优化项目。该库用于解析达梦数据库 SQL 日志文件，当前已支持内存映射 I/O、零拷贝延迟解析、SIMD UTF-8 校验、以及 Rayon 并行迭代器。本项目目标是在现有基础上进一步压榨单线程和多线程吞吐量，不限制 API 变更。

## Core Value

在任意硬件上达到尽可能高的解析吞吐量（records/sec 和 GB/s）。

## Requirements

### Validated

- ✓ 内存映射文件 I/O（`memmap2`）— existing
- ✓ 零拷贝 `Cow<'a, str>` 延迟字段解析 — existing
- ✓ SIMD UTF-8 校验（`simdutf8`）— existing
- ✓ 预构建 SIMD Finder（`memchr::memmem`）— existing
- ✓ 单线程迭代器 `iter()` — existing
- ✓ Rayon 并行迭代器 `par_iter()` — existing
- ✓ 延迟字段解析（`parse_performance_metrics` 等）— existing

### Active

- [ ] 优化记录边界检测热路径（`LogIterator::next`）— Phase 4 目标
- [ ] 提升多线程分块策略 — Phase 5 目标

### Validated (Phase 3 — HotPath)

- ✓ `find_indicators_split` O(1) 早退逻辑（HOT-01）— Validated in Phase 3: HotPath
- ✓ 单次 `memrchr(b':')` 反向扫描替代 3 次 FinderRev（HOT-02）— Validated in Phase 3: HotPath
- ✓ `#[inline(always)]` 热路径 + `#[cold]` 错误路径标注（HOT-03）— Validated in Phase 3: HotPath
- ✓ mmap `Advice::Sequential` 顺序读取建议（HOT-04）— Validated in Phase 3: HotPath

### Out of Scope

- 支持新日志格式 — 功能需求，不在本次优化范围
- GB18030 编码路径深度优化 — 场景罕见，收益不高

## Context

- 当前 baseline（合成 5 MB 文件）：674,425 ns ≈ 7.6 GB/s（仅 `iter().count()`）
- 记录格式：单行约 206 字节，含时间戳 + 元数据 + SQL body + 性能指标
- 记录边界检测依赖逐行扫描 `memchr(b'\n')` + 8 字节时间戳模式匹配
- `parse_record_with_hint` 每次调用涉及多个 `memchr` 和 Finder 调用
- `find_indicators_split` 在末尾 256 字节窗口用 3 个反向 SIMD Finder
- `par_iter()` 已实现，但分块边界扫描仍为串行

## Constraints

- **Tech**: 纯 Rust，可引入新依赖（需评估编译时间和维护成本）
- **Correctness**: 所有现有测试必须通过，覆盖率 ≥ 90%
- **API**: 可以修改，但需兼顾库用户体验

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 使用 mmap 而非 read | 零拷贝、OS 缓存友好 | ✓ Good |
| 延迟字段解析 | 只解析调用方需要的字段 | ✓ Good |
| SIMD Finder 静态预构建 | 避免每次调用重建 Finder | ✓ Good |

## Evolution

This document evolves at phase transitions and milestone boundaries.

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
*Last updated: 2026-04-24 after Phase 3 (HotPath) completion*
