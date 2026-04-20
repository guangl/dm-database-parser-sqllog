# ROADMAP: dm-database-parser-sqllog 性能优化

**Project:** dm-database-parser-sqllog 性能优化
**Milestone:** v1 Performance Optimization
**Granularity:** Coarse
**Created:** 2026-04-18

---

## Phases

- [x] **Phase 1: Measurement** - 建立可信的基准测量基础设施，确保后续优化有可验证的数据支撑 *(completed 2026-04-20)*
- [ ] **Phase 2: Correctness** - 修复 unsafe 路径的正确性风险，为后续热路径改动建立安全地基
- [ ] **Phase 3: HotPath** - 零风险热路径微优化，可逐一回滚，每项改动独立可验证
- [ ] **Phase 4: CoreAlgo** - 核心算法重写，预期最高 ROI（15–45%）的单线程吞吐提升
- [ ] **Phase 5: Parallel** - 两阶段并行索引，解决当前字节级分块导致的负载不均问题

---

## Phase Details

### Phase 1: Measurement
**Goal**: 开发者可以用真实语料库衡量任意代码改动对吞吐量的影响，并在 CI 中自动捕获退化
**Depends on**: Nothing (first phase)
**Requirements**: MEAS-01, MEAS-02, MEAS-03, MEAS-04
**Success Criteria** (what must be TRUE):
  1. `cargo bench` 输出包含 GB/s 和 records/sec 两个吞吐量指标（不只是 ns/iter）
  2. benchmark 包含调用 `parse_performance_metrics()` 的变体，反映真实热路径工作量
  3. benchmark 使用含多行 SQL 的合成语料库，而非只有均匀单行记录
  4. CI 对比 `baseline.json`，吞吐退化超过 5% 时 pipeline 失败并报告具体数值
**Plans**: 2 plans
Plans:
- [x] 01-01-PLAN.md — 扩展 benchmark 变体（MEAS-01/02/03）：多行语料库 + Throughput 双单位 + metrics 变体
- [x] 01-02-PLAN.md — CI 回归门禁基础设施（MEAS-04）：check-regression.sh + benchmark.yml 门禁 + update-baseline.yml

### Phase 2: Correctness
**Goal**: 消除 unsafe 解码路径的已知正确性风险，使后续任意热路径改动不会踩到未定义行为
**Depends on**: Phase 1
**Requirements**: CORR-01, CORR-02, CORR-03
**Success Criteria** (what must be TRUE):
  1. 编码检测不再只采样前 64 KB，大文件（>64 KB）中的 GB18030 内容能被正确识别
  2. CI 运行 Miri，覆盖 unsafe 解码路径，Miri 检查无报错
  3. `find_indicators_split` 有测试用例覆盖 SQL body 内含 `EXECTIME:` 等指标关键字的场景，且结果正确
**Plans**: 2 plans
Plans:
- [ ] 02-01-PLAN.md — 代码修复（CORR-01/03）：全文件编码检测 + find_indicators_split 验证守卫 + 6 条新测试
- [x] 02-02-PLAN.md — Miri CI 基础设施（CORR-02）：miri.yml 作业 + 五个测试文件 cfg 标注

### Phase 3: HotPath
**Goal**: 通过低风险的内联提示、早退逻辑和 mmap 建议，消除热路径中的无谓开销
**Depends on**: Phase 2
**Requirements**: HOT-01, HOT-02, HOT-03, HOT-04
**Success Criteria** (what must be TRUE):
  1. `find_indicators_split` 在记录末尾不以 `.` 结尾时不执行任何 rfind 调用（可通过计数器或 benchmark 验证）
  2. `find_indicators_split` 使用单次反向字节扫描替代 3 次独立 rfind，现有测试全部通过
  3. `parse_performance_metrics` 被 `#[inline(always)]` 标注，错误路径被 `#[cold]` 标注，编译产物可验证
  4. 顺序读取大文件（>100 MB）时，mmap advise 生效，benchmark 吞吐不退化
**Plans**: TBD

### Phase 4: CoreAlgo
**Goal**: 重写记录边界检测的核心扫描算法，实现预期 15–45% 的单线程吞吐提升
**Depends on**: Phase 3
**Requirements**: ALGO-01, ALGO-02
**Success Criteria** (what must be TRUE):
  1. `LogIterator::next()` 使用 `memmem::Finder(b"\n20")` 单次扫描定位记录边界，不再有 `memchr(b'\n')` 逐行循环
  2. Phase 1 建立的 benchmark 显示单线程吞吐相比 Phase 3 基线提升 ≥10%（剔除语料库差异后）
  3. 时间戳检测使用打包 `u64` 掩码比较，所有现有测试通过
**Plans**: TBD

### Phase 5: Parallel
**Goal**: 通过两阶段索引扫描实现记录级均匀并行分区，并在小文件场景避免 Rayon 开销负收益
**Depends on**: Phase 4
**Requirements**: PAR-01, PAR-02, PAR-03
**Success Criteria** (what must be TRUE):
  1. `LogParser::index()` 返回 `RecordIndex`（记录起始位置列表），可独立调用
  2. `par_iter()` 使用 `RecordIndex` 实现记录级分区，多线程 benchmark 显示线性扩展（2 线程 ≥1.6x 单线程）
  3. 文件小于 32 MB 时，`par_iter()` 自动退化为串行，不引入 Rayon 调度开销
  4. 所有现有测试（单线程 + 多线程）通过，覆盖率维持 ≥90%
**Plans**: TBD

---

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Measurement | 2/2 | Complete | 2026-04-20 |
| 2. Correctness | 1/2 | In progress | - |
| 3. HotPath | 0/? | Not started | - |
| 4. CoreAlgo | 0/? | Not started | - |
| 5. Parallel | 0/? | Not started | - |

---

## Coverage Validation

| Requirement | Phase | Mapped |
|-------------|-------|--------|
| MEAS-01 | Phase 1 | ✓ |
| MEAS-02 | Phase 1 | ✓ |
| MEAS-03 | Phase 1 | ✓ |
| MEAS-04 | Phase 1 | ✓ |
| CORR-01 | Phase 2 | ✓ |
| CORR-02 | Phase 2 | ✓ |
| CORR-03 | Phase 2 | ✓ |
| HOT-01 | Phase 3 | ✓ |
| HOT-02 | Phase 3 | ✓ |
| HOT-03 | Phase 3 | ✓ |
| HOT-04 | Phase 3 | ✓ |
| ALGO-01 | Phase 4 | ✓ |
| ALGO-02 | Phase 4 | ✓ |
| PAR-01 | Phase 5 | ✓ |
| PAR-02 | Phase 5 | ✓ |
| PAR-03 | Phase 5 | ✓ |

**v1 coverage: 16/16 requirements mapped. No orphans.**

---
*Created: 2026-04-18*
*Updated: 2026-04-20 — Phase 2 Plan 02 completed (CORR-02: Miri CI + cfg annotations)*
