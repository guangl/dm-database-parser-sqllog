# Requirements: dm-database-parser-sqllog 性能优化

**Defined:** 2026-04-18
**Core Value:** 在任意硬件上达到尽可能高的解析吞吐量（records/sec 和 GB/s）

## v1 Requirements

### Measurement（测量基础设施）

- [ ] **MEAS-01**: benchmark 以 GB/s 和 records/sec 报告吞吐量（criterion::Throughput）
- [ ] **MEAS-02**: benchmark 包含 `parse_performance_metrics()` 调用变体（反映真实调用路径）
- [ ] **MEAS-03**: benchmark 包含含多行 SQL 的真实分布合成语料库
- [ ] **MEAS-04**: CI 加入 benchmark 回归门禁（对比 baseline.json，超过 5% 退化则失败）

### Correctness（正确性加固）

- [ ] **CORR-01**: 编码检测采样范围扩展至整个文件（或足够大的样本），消除 64 KB 截断导致的误分类
- [ ] **CORR-02**: Miri 加入 CI，覆盖 unsafe 解码路径
- [ ] **CORR-03**: `find_indicators_split` 针对 SQL body 内含指标关键字（如 `EXECTIME:`）的场景有测试用例

### HotPath（零风险热路径优化）

- [ ] **HOT-01**: `find_indicators_split` 在记录末尾不以 `.` 结尾时快速返回，跳过 3 次 rfind
- [ ] **HOT-02**: `find_indicators_split` 改为单次反向字节扫描，替代 3 个独立 `rfind` 调用
- [ ] **HOT-03**: `find_indicators_split` 标注 `#[inline(always)]`，错误路径标注 `#[cold]`
- [ ] **HOT-04**: `LogParser::from_path` 调用 `mmap.advise(Advice::Sequential)`

### CoreAlgo（核心算法优化）

- [ ] **ALGO-01**: `LogIterator::next()` 使用预构建 `memmem::Finder(b"\n20")` 替代 `memchr(b'\n')` 逐行循环
- [ ] **ALGO-02**: 时间戳检测改为打包 `u64` 掩码比较，替代 8 个独立字节比较分支

### Parallel（并行优化）

- [ ] **PAR-01**: 引入 `LogParser::index()` 返回 `RecordIndex`（记录起始位置 `Vec<usize>`），支持两阶段扫描
- [ ] **PAR-02**: `par_iter()` 改用 `RecordIndex` 实现记录级均匀分区（替代当前字节级分块）
- [ ] **PAR-03**: `par_iter()` 在文件小于阈值（建议 32 MB）时自动退化为串行迭代

## v2 Requirements

### Advanced（高级优化）

- **ADV-01**: PGO（Profile-Guided Optimization）构建流水线
- **ADV-02**: `mimalloc` 可选 feature flag（`features = ["fast-alloc"]`），仅 GB18030 路径受益
- **ADV-03**: BOLT 二进制优化（待 Rust 工具链支持成熟后评估）

## Out of Scope

| Feature | Reason |
|---------|--------|
| 自定义 SIMD 换行扫描（packed_simd / std::simd） | `memchr` 已是天花板；引入 nightly 依赖得不偿失 |
| GB18030 路径深度优化 | 罕见场景，实际收益不值得复杂度投入 |
| async/tokio 集成 | 破坏零拷贝 `Cow<'a>` 生命周期设计 |
| 全局默认 mimalloc | 库 crate 不应强制用户分配器 |
| 新日志格式支持 | 功能需求，不在本次优化范围 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| MEAS-01 | Phase 1 | Pending |
| MEAS-02 | Phase 1 | Pending |
| MEAS-03 | Phase 1 | Pending |
| MEAS-04 | Phase 1 | Pending |
| CORR-01 | Phase 2 | Pending |
| CORR-02 | Phase 2 | Pending |
| CORR-03 | Phase 2 | Pending |
| HOT-01 | Phase 3 | Pending |
| HOT-02 | Phase 3 | Pending |
| HOT-03 | Phase 3 | Pending |
| HOT-04 | Phase 3 | Pending |
| ALGO-01 | Phase 4 | Pending |
| ALGO-02 | Phase 4 | Pending |
| PAR-01 | Phase 5 | Pending |
| PAR-02 | Phase 5 | Pending |
| PAR-03 | Phase 5 | Pending |

**Coverage:**
- v1 requirements: 16 total
- Mapped to phases: 16
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-18*
*Last updated: 2026-04-18 after initial definition*
