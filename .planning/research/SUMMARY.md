# Research Summary — dm-database-parser-sqllog 性能优化

## Executive Summary

项目当前基线已处于极高水位（~7.6 GB/s，5 MB 合成文件单线程）。架构（mmap、零拷贝 `Cow` 切片、延迟字段解析、`memchr`/`simdutf8` SIMD）是正确的，不需要重构。剩余性能收益来自现有设计内的算法精化。

**单项最高 ROI 改动**：用已有依赖中的 `memmem::Finder(b"\n20")` 替换 `LogIterator::next()` 里的 `memchr(b'\n')` 逐行循环，预计提升 15–30%。

**最大威胁**：测量的是错误的东西。当前 benchmark 只对均匀单行记录调用 `iter().count()`；真实 DM 日志含多行 SQL、变长记录、GB18030 内容，实际吞吐可能低 2–5x。任何热路径优化之前必须先升级测量基础设施。

**次要威胁**：`unsafe` 解码路径的正确性风险——编码检测仅采样前 64 KB，大文件若 64 KB 后出现 GB18030 内容会导致 UB。

---

## Recommended Stack

**保留所有现有依赖**。唯一变更：将 `mimalloc` 从 `dev-dependencies` 提升为可选 feature `fast-alloc`。

**明确拒绝**：`packed_simd`（停维护）、`std::simd`（nightly-only）、`aho-corasick`（256 字节窗口内不如 3 个 Finder 快）、`highway-rs`（C++ FFI）。

---

## Key Optimization Opportunities（按 ROI 排序）

| 排名 | 改动 | 预期收益 | 风险 | 阶段 |
|------|------|---------|------|------|
| 1 | `memmem::Finder(b"\n20")` 替换 `memchr(b'\n')` 循环 | 15–30% | 低 | 4 |
| 2 | 打包 `u64` 时间戳比较替换 8 个分支 | 5–15% | 低 | 4 |
| 3 | `find_indicators_split` 早退启发式 | 0–30% | 无（2 行代码） | 3 |
| 4 | `find_indicators_split` 单次反向扫描（替换 3 次 rfind） | 2–5% | 无 | 3 |
| 5 | `#[inline(always)]` + `#[cold]` 注解 | 2–8% | 无 | 3 |
| 6 | `mmap.advise(Advice::Sequential)` | 冷读大文件有效 | 无 | 3 |
| 7 | `RecordIndex` 两阶段扫描 + Rayon 完美分区 | 大文件（>100 MB）有效 | 中 | 5 |
| 8 | PGO 构建流水线 | 5–20% | 中 | 6 |

---

## Critical Pitfalls

1. **合成 benchmark 掩盖真实性能**（CRITICAL）— 必须先加真实语料库和 `parse_performance_metrics()` 变体，再改任何热路径
2. **`unsafe from_utf8_unchecked` 仅采样 64 KB**（CRITICAL）— 大文件存在 UB 风险，热路径优化前需修复
3. **`iter().count()` 不测量实际热路径**（HIGH）— `parse_performance_metrics()` 额外增加 2–4x 工作量
4. **Rayon 对小文件有负收益**（MODERATE）— 需要文件大小阈值门禁
5. **CI SIMD 目标不匹配**（MODERATE）— CI 无 `target-cpu=native` 则 AVX2 缺失，吞吐减半

---

## Suggested Phase Order

1. **Phase 1 — 测量基础设施**（必须先做，零风险）
2. **Phase 2 — 正确性加固**（unsafe 路径优化前的前提）
3. **Phase 3 — 零风险热路径小优化**（可验证、可逐一回滚）
4. **Phase 4 — 核心算法优化**（最高 ROI，需 Phase 1 验证）
5. **Phase 5 — 并行与大文件优化**（单线程路径优化完后再做）
6. **Phase 6 — 构建流水线 / 高级优化**（可选，边际收益递减）

---

## Research Confidence

| 领域 | 置信度 | 说明 |
|------|--------|------|
| 技术栈 | HIGH | 完整代码库分析 |
| 优化方向 | HIGH | 增益幅度为估算，Phase 1 后可验证 |
| 架构 | HIGH | 全部来自直接代码阅读 |
| Pitfall | HIGH（关键项）/ MEDIUM（平台相关） | — |
