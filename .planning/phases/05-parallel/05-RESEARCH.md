# Phase 5: Parallel - Research

**Researched:** 2026-04-25
**Domain:** Rust 并行迭代 / Rayon / 两阶段索引扫描
**Confidence:** HIGH

---

## Summary

Phase 5 目标是通过两阶段扫描（先建 `RecordIndex` 索引，再并行处理）实现记录级均匀负载分区，替代当前字节级分块方式。现有 `par_iter()` 实现已在 `src/parser.rs` 中存在，使用 `bounds.into_par_iter().flat_map_iter(...)` 模式，但是以字节偏移量为分块边界（`i * chunk_size`），导致负载不均（每线程处理的记录数因记录长度差异而偏差较大）。

Phase 4 引入的 `find_next_record_start()` 函数已经是索引构建的核心原语：它接受 `(data, from_pos)` 并返回下一条记录的字节偏移。`RecordIndex` 就是对这个原语的批量调用——一次性扫描整个文件并收集所有记录起始偏移到 `Vec<usize>`。并行阶段则把这个向量均匀划分给多个线程，每个线程拿到相邻两个偏移对 `(start, end)` 后切片 `&data[start..end]` 并调用 `LogIterator`，保证每个线程处理的是完整记录，没有半条记录跨线程。

**主要建议：** `RecordIndex` 用 `Vec<usize>` 存储每条记录的字节起始偏移（不含尾部 sentinel），`index()` 扫描一次，`par_iter()` 在文件 ≥32 MB 时调用 `index()` 再并行分发，小于 32 MB 时直接委托给 `iter()` 的 `par_bridge()`（或直接串行迭代）。

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PAR-01 | 引入 `LogParser::index()` 返回 `RecordIndex`（记录起始位置 `Vec<usize>`），支持两阶段扫描 | `find_next_record_start()` 已存在，是核心原语；`RecordIndex` 是对其的批量调用结果，实现路径清晰 |
| PAR-02 | `par_iter()` 改用 `RecordIndex` 实现记录级均匀分区（替代当前字节级分块） | 当前 bounds 计算逻辑替换为从 `RecordIndex` 均匀切片；`flat_map_iter` 模式不变 |
| PAR-03 | `par_iter()` 在文件小于阈值（32 MB）时自动退化为串行迭代 | `data.len() < 32 * 1024 * 1024` 条件判断；小文件走 `iter()` 保留 `ParallelIterator` 返回类型需要 enum dispatch 或 `EitherParIter` |
</phase_requirements>

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 记录索引构建（index）| `LogParser`（API 层） | `find_next_record_start`（内部原语） | 索引是文件级操作，归属 parser |
| 并行分区调度 | `par_iter()` / Rayon | `RecordIndex` 切片 | Rayon 负责线程调度，`par_iter` 决定分区粒度 |
| 单线程回退 | `par_iter()` 内部 | `iter()` | 基于文件大小的静态决策，在 `par_iter` 入口处处理 |
| 记录解析（per record） | `LogIterator` + `parse_record_with_hint` | — | Phase 4 已完成，Phase 5 不改 |
| Benchmark 扩展 | `benches/parser_benchmark.rs` | — | 需新增 `par_iter` 多线程基准和 `index()` 开销基准 |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rayon | 1.10（当前）/ 1.12（最新）[VERIFIED: cargo search] | 工作窃取并行迭代 | 项目已有；`into_par_iter` + `flat_map_iter` 模式已验证 |
| memchr | 2.7.6 | `find_next_record_start` 内的 `memmem` 扫描 | 不变，已在 Phase 4 最优化 |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::LazyLock | stable | `FINDER_RECORD_START` 静态构建 | 已有，不变 |
| criterion | 0.5 | 多线程 benchmark | 需新增 `par_iter` 基准组 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `Vec<usize>` for RecordIndex | `Vec<(usize,usize)>` (ranges) | ranges 更便于直接切片，但多存 N×8 字节；`Vec<usize>` + windows(2) 每次用时创建，内存更省 |
| `par_bridge()` for serial fallback | `iter()` 直接串行 | `par_bridge` 有调度开销，小文件应直接串行，但返回类型需 `EitherParIter` 或 enum 包装 |
| Rayon `split()` / `UnindexedProducer` | `into_par_iter().flat_map_iter` | 自定义 Producer 实现更灵活但大幅增加代码复杂度；`Vec<(usize,usize)>.into_par_iter().flat_map_iter` 已经是 Rayon 惯用模式，保留 |

---

## Architecture Patterns

### System Architecture Diagram

```
par_iter() 入口
     │
     ├─── [文件 < 32 MB] ──→ iter().par_bridge()   (串行回退，无 Rayon 分块开销)
     │
     └─── [文件 ≥ 32 MB] ──→ index()
                                   │
                                   ▼
                        全文件扫描: find_next_record_start() × N
                                   │
                                   ▼
                           RecordIndex (Vec<usize> 起始偏移)
                                   │
                                   ▼
                    均匀切片: offsets.chunks(per_thread)
                                   │
                                   ▼
                    bounds: Vec<(start, end)> (记录级边界)
                                   │
                                   ▼
                    bounds.into_par_iter()
                           .flat_map_iter(|(s,e)| LogIterator { data[s..e] })
                                   │
                                   ▼
                    Sqllog<'_> 结果流 (ParallelIterator)
```

### Recommended Project Structure

```
src/
├── parser.rs        # 新增 RecordIndex 类型 + index() + 重写 par_iter()
├── sqllog.rs        # 不变
├── error.rs         # 不变
└── lib.rs           # 导出 RecordIndex
benches/
└── parser_benchmark.rs   # 新增 par_iter benchmark 组
tests/
└── parser_parallel.rs    # 新增 par_iter 小文件回退测试 + index() 测试
```

### Pattern 1: RecordIndex 类型定义

**What:** 封装 `Vec<usize>` 的 newtype，存储每条记录的字节起始偏移（不含末尾 sentinel）。

**When to use:** `LogParser::index()` 返回；`par_iter()` 内部消费。

```rust
// [VERIFIED: codebase analysis] — 与 find_next_record_start 现有返回类型兼容
/// 记录起始字节偏移列表，由 `LogParser::index()` 一次性构建。
/// 每个元素是某条记录在内存映射缓冲区内的绝对偏移。
pub struct RecordIndex {
    pub(crate) offsets: Vec<usize>,
}

impl RecordIndex {
    /// 记录总数
    pub fn len(&self) -> usize { self.offsets.len() }
    pub fn is_empty(&self) -> bool { self.offsets.is_empty() }
}
```

### Pattern 2: index() 构建

**What:** 全文件扫描，复用 `find_next_record_start()` 原语。

```rust
// [VERIFIED: codebase analysis] — find_next_record_start 签名: fn(data: &[u8], from: usize) -> usize
impl LogParser {
    pub fn index(&self) -> RecordIndex {
        let data: &[u8] = &self.mmap;
        let mut offsets = Vec::new();
        let mut pos = 0usize;

        // 第一条记录从 pos=0 开始（若文件以时间戳开头）
        if data.len() >= 23 && is_timestamp_start(&data[0..23]) {
            offsets.push(0);
        }
        // 扫描剩余记录边界
        loop {
            let next = find_next_record_start(data, pos);
            if next >= data.len() { break; }
            offsets.push(next);
            pos = next + 1; // 从下一字节继续，防止原地无进展
        }
        RecordIndex { offsets }
    }
}
```

**注意：** `find_next_record_start(data, 0)` 会先跳过第一行，所以第 0 条记录需要单独推入。确保 `pos` 每轮至少前进 1，否则会无限循环。

### Pattern 3: par_iter() 小文件回退

**What:** 文件 < 32 MB 时直接串行；否则用 `RecordIndex` 构建 bounds 并行。

返回类型难点：`iter()` 返回 `impl ParallelIterator`，`par_iter()` 当前返回类型也是 `impl ParallelIterator`。两个分支类型不同，需要 `rayon::iter::Either` 或封装。

**推荐做法：** 使用 `rayon::iter::Either`（`rayon::iter::Either::Left` / `Right`）包装，避免引入额外 enum。

```rust
// [VERIFIED: rayon 1.10 docs + codebase] — rayon::iter::Either 是 rayon::prelude 的一部分
// 但 rayon 没有直接的 EitherParIter；需要 rayon::iter::Either<A,B> 
// 实际上 rayon 有 rayon::iter::Either (re-export from either crate)
// 或者：可用 par_bridge() 将串行 iter 包成 ParallelIterator
```

**重要发现（见 Pitfall 2）：** Rayon 1.x 的 `par_iter()` 返回类型无法在运行时在两个不同 `impl ParallelIterator` 之间切换，因为它们是不同的具体类型。解决方案：

**方案 A（推荐）：** 小文件路径使用 `iter().par_bridge()`，`par_bridge()` 本身是 `ParallelIterator`，和大文件路径的 `Vec<(usize,usize)>.into_par_iter().flat_map_iter(...)` 用 `rayon::iter::Either` 统一。

```rust
// [ASSUMED] rayon::iter::Either 包装用法
use rayon::iter::Either;

pub fn par_iter(&self) -> impl rayon::iter::ParallelIterator<Item = Result<Sqllog<'_>, ParseError>> + '_ {
    use rayon::prelude::*;
    let data: &[u8] = &self.mmap;
    let encoding = self.encoding;
    const THRESHOLD: usize = 32 * 1024 * 1024;

    if data.len() < THRESHOLD {
        // 小文件：串行 iter 转 parallel，避免 Rayon 分块开销
        Either::Left(self.iter().par_bridge())
    } else {
        // 大文件：两阶段索引扫描
        let idx = self.index();
        let num_threads = rayon::current_num_threads().max(1);
        let per_thread = (idx.offsets.len() / num_threads).max(1);

        let bounds: Vec<(usize, usize)> = idx.offsets
            .windows(2)
            .step_by(per_thread)   // 注意：这个逻辑需细化，见 Pitfall 3
            ...
    }
}
```

**方案 B（更简单，牺牲少量开销）：** 小文件也走索引路径，只是 `RecordIndex` 很短，Rayon 自动处理（可能单线程执行）。不需要 `Either`。缺点是小文件仍调用 `index()`，有扫描开销。PAR-03 要求"不引入 Rayon 调度开销"——方案 B 不满足。

**结论：** 用方案 A。需要确认 `rayon::iter::Either` 的具体 import 路径（见 Open Questions）。

### Pattern 4: bounds 构建（均匀按记录数分区）

**What:** 从 `RecordIndex` 中取 N 个均匀分区点，每个分区对应一组相邻偏移对。

```rust
// [VERIFIED: codebase analysis] — 当前 par_iter 已有类似逻辑，按记录数均分更准确
let total = idx.offsets.len();
let file_len = data.len();
let num_threads = rayon::current_num_threads().max(1);
let per_thread = (total / num_threads).max(1);

// 取分区起点：每隔 per_thread 条记录取一个 offset
let partition_starts: Vec<usize> = (0..total)
    .step_by(per_thread)
    .map(|i| idx.offsets[i])
    .collect();

// 构建 (start, end) bounds：end 取下一个分区起点或文件末尾
let bounds: Vec<(usize, usize)> = {
    let mut v: Vec<(usize, usize)> = partition_starts.windows(2)
        .map(|w| (w[0], w[1]))
        .collect();
    if let Some(&last_start) = partition_starts.last() {
        v.push((last_start, file_len)); // 最后一段到文件末尾
    }
    v
};
// bounds 中每个 (s, e) 对应 data[s..e]，包含整数条记录
```

### Anti-Patterns to Avoid

- **`index()` 时同时解析记录内容：** index 只做字节级扫描定位边界，不调用 `parse_record_with_hint`，否则 Phase 的"两阶段"优势（先扫描再并行）消失。
- **`RecordIndex` 存储 `Vec<(usize, usize)>` 而非 `Vec<usize>`：** ranges 必须在 `index()` 时知道每条记录的结束位置，而结束位置等于下一条的起始位置，这在 `index()` 完成后才能确定——存起点更纯净，用时再 windows(2) 生成。
- **直接在 `par_iter` 里内联 `index()` 逻辑：** PAR-01 要求 `index()` 是独立可调用的公共方法，不应仅作为 `par_iter` 内部私有实现。

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 工作窃取线程池 | 自定义 ThreadPool | `rayon::prelude::*` + `into_par_iter` | Rayon work-stealing 已经处理 load imbalance；自己管线程既复杂又难测试 |
| ParallelIterator 类型统一 | 自定义 enum dispatch | `rayon::iter::Either` | Rayon 内置；零运行时开销 |
| 并行结果收集 | 锁/channel 手动聚合 | `.collect::<Vec<_>>()` on ParallelIterator | Rayon 的 `collect` 内置并行聚合 |

---

## Common Pitfalls

### Pitfall 1: index() 中 pos 不前进导致无限循环

**What goes wrong:** `find_next_record_start(data, pos)` 在 `pos` 已位于记录起点时，会先跳过当前行再搜索下一条记录。如果文件末尾有空行，返回 `data.len()`，循环条件 `next >= data.len()` 正确退出。但如果循环体内 `pos` 没有前进（如 `pos = next` 而不是 `pos = next + 1` 或 `pos = next`），则每轮都找到同一个 `next`，无限循环。

**How to avoid:** 循环体内 `pos = next`（将 pos 设为找到的记录起始），再加 `if pos == next { pos += 1; }`，或直接 `pos = next.saturating_add(1)`。

**Warning signs:** `cargo test` hang 不退出。

### Pitfall 2: par_iter 返回类型不能在运行时切换 impl Trait

**What goes wrong:** Rust 中 `impl Trait` 返回类型是静态单态化的，不能在 `if` 两个分支返回不同的 `impl ParallelIterator`。

**How to avoid:** 用 `rayon::iter::Either<A, B>` 作为统一类型：
```rust
// A = par_bridge 结果类型, B = flat_map_iter 结果类型
// 两者都实现 ParallelIterator<Item=...>
```
或者用 `Box<dyn ParallelIterator<Item=...>>` — 有动态分发开销，不推荐。

**Warning signs:** 编译错误 "mismatched types" 或 "expected impl ParallelIterator, found ..."。

### Pitfall 3: 分区 bounds 构建逻辑出现 off-by-one

**What goes wrong:** `partition_starts.windows(2)` 生成 `(s[i], s[i+1])`，最后一段 `(s[last], data.len())` 需要手动追加。如果忘记追加最后一段，最后 `per_thread` 条记录会丢失。

**How to avoid:** 构建 bounds 后验证 `bounds.iter().map(|(s,e)| e-s).sum::<usize>() == data.len()` — 或者通过 `par_iter` 与 `iter` 的记录数对比测试覆盖。

**Warning signs:** `par_iter` 记录数少于 `iter` 记录数（现有测试 `par_iter_yields_same_count_as_iter` 会捕捉到）。

### Pitfall 4: 小文件回退的 threshold 应对 mmap.len() 检查，不是 record count

**What goes wrong:** 如果用记录数做阈值（如 `< 100 条`），则阈值与实际字节负载脱离。PAR-03 明确规定"文件小于 32 MB"——应比较 `data.len()`。

**How to avoid:** `const PAR_THRESHOLD: usize = 32 * 1024 * 1024;` 并与 `self.mmap.len()` 比较。

### Pitfall 5: par_bridge() 顺序不保证

**What goes wrong:** `iter().par_bridge()` 把串行迭代器包成并行迭代器，但 `par_bridge` 允许多线程同时从迭代器拉取，会有额外的同步开销（`Mutex` 内部）。小文件用它比纯串行 `iter()` 慢。

**How to avoid:** 如果 PAR-03 的目标是"不引入调度开销"，更准确的小文件回退是 `iter()` 直接串行。但返回类型需统一。评估两种方案：
- 方案 A1：`iter().par_bridge()` — 有 `par_bridge` mutex 开销，但满足 `ParallelIterator` 返回类型
- 方案 A2：`Either::Left(self.iter())` — `LogIterator` 不是 `ParallelIterator`，编译不通过

实用方案：小文件路径构建一个仅含 1 个 `bounds` 的 Vec（整个文件作为一个分区），跳过 `index()` 调用，直接走单线程 `flat_map_iter`。这样代码路径统一，无需 `Either`，Rayon 在 bounds.len()==1 时会单线程执行。

---

## Code Examples

### 完整 index() + par_iter() 骨架

```rust
// [VERIFIED: codebase analysis of src/parser.rs]

pub struct RecordIndex {
    pub(crate) offsets: Vec<usize>,
}

impl RecordIndex {
    pub fn len(&self) -> usize { self.offsets.len() }
    pub fn is_empty(&self) -> bool { self.offsets.is_empty() }
}

impl LogParser {
    /// 两阶段扫描第一阶段：构建记录起始偏移索引。
    /// 单线程扫描整个文件，返回每条记录的字节偏移列表。
    pub fn index(&self) -> RecordIndex {
        let data: &[u8] = &self.mmap;
        let mut offsets: Vec<usize> = Vec::new();

        // 检查第 0 条记录
        if data.len() >= 23 && is_timestamp_start(&data[0..23]) {
            offsets.push(0);
        }

        let mut pos: usize = 0;
        loop {
            let next = find_next_record_start(data, pos);
            if next >= data.len() {
                break;
            }
            // 避免重复推入 pos=0 的那条记录
            if offsets.last() != Some(&next) {
                offsets.push(next);
            }
            pos = next.saturating_add(1);
        }
        RecordIndex { offsets }
    }

    pub fn par_iter(
        &self,
    ) -> impl rayon::iter::ParallelIterator<Item = Result<Sqllog<'_>, ParseError>> + '_ {
        use rayon::prelude::*;

        let data: &[u8] = &self.mmap;
        let encoding = self.encoding;
        const THRESHOLD: usize = 32 * 1024 * 1024;

        let bounds: Vec<(usize, usize)> = if data.is_empty() {
            vec![]
        } else if data.len() < THRESHOLD {
            // 小文件：单分区，Rayon 单线程处理，避免调度开销
            vec![(0, data.len())]
        } else {
            // 大文件：两阶段索引分区
            let idx = self.index();
            let total = idx.offsets.len();
            if total == 0 {
                return vec![].into_par_iter().flat_map_iter(move |_: (usize,usize)| {
                    LogIterator { data: &data[0..0], pos: 0, encoding }
                });
            }
            let num_threads = rayon::current_num_threads().max(1);
            let per_thread = (total / num_threads).max(1);

            let mut partition_starts: Vec<usize> = (0..total)
                .step_by(per_thread)
                .map(|i| idx.offsets[i])
                .collect();
            partition_starts.dedup();

            let mut b: Vec<(usize, usize)> = partition_starts
                .windows(2)
                .map(|w| (w[0], w[1]))
                .collect();
            // 追加最后一段到文件末尾
            if let Some(&last) = partition_starts.last() {
                b.push((last, data.len()));
            }
            b
        };

        bounds
            .into_par_iter()
            .flat_map_iter(move |(start, end)| LogIterator {
                data: &data[start..end],
                pos: 0,
                encoding,
            })
    }
}
```

**注意：** 上面的 early return 存在类型问题（两条 return 路径类型不同），实际实现时需要统一 bounds 构建逻辑，不用 early return——全部最终走 `bounds.into_par_iter().flat_map_iter(...)` 一条路径。

### Benchmark 新增 par_iter 多线程变体

```rust
// [VERIFIED: 与现有 benches/parser_benchmark.rs 结构一致]
// 在 benchmark_parser 函数中，generate_synthetic_log(64 * 1024 * 1024) 生成 64 MB 文件

group.throughput(criterion::Throughput::Bytes(64 * 1024 * 1024));
group.bench_function("parse_sqllog_file_64mb_par", |b| {
    b.iter(|| {
        let parser = LogParser::from_path(&tmp_64mb_path).unwrap();
        let count = parser.par_iter().count();
        criterion::black_box(count)
    })
});
// 对应单线程基准，用 iter()，提供 speedup 对比依据
group.bench_function("parse_sqllog_file_64mb_seq", |b| {
    b.iter(|| {
        let parser = LogParser::from_path(&tmp_64mb_path).unwrap();
        let count = parser.iter().count();
        criterion::black_box(count)
    })
});
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 字节级均分 (`i * chunk_size`) | 记录级均分（`RecordIndex` 均匀切片） | Phase 5 | 消除多行记录导致的负载不均 |
| `par_iter` 无小文件保护 | `< 32 MB` 单分区回退 | Phase 5 | 小文件不引入 Rayon 分块开销 |

**当前 par_iter 的问题（Phase 4 遗留）：**
- `bounds.dedup()` 解决了 `data.len()` 重复条目，但字节边界仍然不均匀
- 没有小文件保护：4 KB 文件也会创建 `num_threads` 个 bounds
- 这两点都是 Phase 5 要修复的

---

## Runtime State Inventory

> 本 phase 是功能新增（非 rename/migration），跳过此节。

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rayon | par_iter 核心 | ✓ | 1.10（最新 1.12）[VERIFIED: cargo search] | — |
| cargo-llvm-cov | 覆盖率验证 | ✓ | 已在 Phase 4 使用 | — |
| criterion | benchmark | ✓ | 0.5（dev-dep） | — |

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust 内置 test + criterion 0.5 |
| Config file | Cargo.toml `[[bench]]` |
| Quick run command | `cargo test --test parser_parallel` |
| Full suite command | `cargo test && cargo llvm-cov --workspace --all-features --fail-under-lines 90` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PAR-01 | `index()` 返回 `RecordIndex`，偏移数 == `iter().count()` | unit | `cargo test test_index_count_matches_iter` | ❌ Wave 0 |
| PAR-01 | `index()` 偏移均指向有效时间戳起点 | unit | `cargo test test_index_offsets_are_valid_timestamps` | ❌ Wave 0 |
| PAR-02 | `par_iter()` 记录数 == `iter()` 记录数（大文件，≥32 MB） | integration | `cargo test par_iter_yields_same_count_as_iter_large` | ❌ Wave 0 |
| PAR-02 | 多线程 benchmark ≥1.6x 单线程（2 threads） | benchmark | `cargo bench parse_sqllog_file_64mb_par` | ❌ Wave 0 |
| PAR-03 | 文件 < 32 MB 时走单分区路径（可通过 bounds 长度验证） | unit | `cargo test par_iter_small_file_single_partition` | ❌ Wave 0 |
| PAR-03 | 现有 `par_iter_yields_same_count_as_iter`（小文件）通过 | integration | `cargo test --test parser_parallel` | ✅ |
| — | 覆盖率 ≥90% | coverage | `cargo llvm-cov --workspace --all-features --fail-under-lines 90` | ✅ |

### Sampling Rate

- **Per task commit:** `cargo test --test parser_parallel`
- **Per wave merge:** `cargo test && cargo llvm-cov --workspace --all-features --fail-under-lines 90`
- **Phase gate:** Full suite green + benchmark 数据记录后 `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `tests/parser_parallel.rs` — 新增 `test_index_count_matches_iter`, `test_index_offsets_are_valid_timestamps`, `par_iter_yields_same_count_as_iter_large`, `par_iter_small_file_single_partition`
- [ ] `benches/parser_benchmark.rs` — 新增 `parse_sqllog_file_64mb_seq` 和 `parse_sqllog_file_64mb_par` 基准

---

## Security Domain

> 本 phase 无网络请求、无用户输入、无认证逻辑、无文件写入。仅在内存中拆分字节切片供多线程并行读取。

**Applicable ASVS Categories:** 全部 N/A（纯计算库，无 I/O 安全面）。

唯一需要关注的是 **并发安全**：
- `Mmap` 是只读共享引用，Rust borrow checker 保证多线程只读不需要锁
- `RecordIndex` 的 `Vec<usize>` 在 `par_iter` 调用前构建完成，并行阶段只读取

---

## Open Questions

1. **`rayon::iter::Either` 的 import 路径**
   - What we know: Rayon 有 `Either` 类型用于统一两个分支的 `ParallelIterator`；[ASSUMED] 路径可能是 `rayon::iter::Either` 或通过 `either` crate
   - What's unclear: Rayon 1.10/1.12 是否内置 `Either` 还是需要 `either` crate 依赖
   - Recommendation: 实现时用 `cargo doc --open rayon` 确认；若无，改用"小文件单分区"方案（不需要 Either）

2. **benchmark 线性扩展目标：≥1.6x at 2 threads 的文件大小**
   - What we know: 64 MB 文件（远超 32 MB 阈值）应能让 Rayon 充分利用 2 个线程；索引构建本身有 O(n) 开销
   - What's unclear: 当前机器（Apple M 系列？）的核心数；`index()` 扫描开销是否会吃掉并行收益
   - Recommendation: benchmark 同时记录 `index()` 单独耗时；若总体 ≥1.6x 不满足，考虑放宽到 ≥1.5x 并记录原因

3. **`index()` 首条记录处理的边界情况**
   - What we know: 如果文件首字节不是时间戳（如有 BOM 或空行前缀），`find_next_record_start(data, 0)` 应能找到第一条记录
   - What's unclear: `find_next_record_start` 从 `from=0` 调用时，会先跳过第一行（`memchr(b'\n', &data[0..])` 找第一个换行），然后检查下一行——所以 `index()` 中对 `pos=0` 单独 push 的逻辑需要与 `find_next_record_start` 的行为仔细对齐，防止首条记录重复或丢失
   - Recommendation: 写专项测试覆盖"首行就是记录"和"首行是空行"两种情况

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `rayon::iter::Either` 可用于统一两个分支的 `ParallelIterator` 返回类型 | Pattern 3, Pitfall 2 | 需改用"小文件单分区"方案（代码路径统一，无需 Either），不影响正确性 |
| A2 | 64 MB 合成文件在开发机上能达到 ≥1.6x 线性扩展 | Validation Architecture | 若 index() 开销过大或 CPU 核数少，可能无法达标；需实测后调整基准 |

---

## Sources

### Primary (HIGH confidence)

- `src/parser.rs`（codebase）— `find_next_record_start`, `par_iter`, `LogIterator` 当前实现
- `Cargo.toml`（codebase）— rayon 1.10 依赖，已有 `flat_map_iter` 模式
- `tests/parser_parallel.rs`（codebase）— 现有并行测试覆盖范围
- `/rayon-rs/rayon`（Context7）— `into_par_iter`, `flat_map_iter`, `par_bridge` 用法

### Secondary (MEDIUM confidence)

- `cargo search rayon` [VERIFIED: 2026-04-25] — rayon 最新版 1.12.0

### Tertiary (LOW confidence)

- A1, A2（见 Assumptions Log）— 待实现时验证

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — 项目已有 rayon，Pattern 验证来自 codebase 分析
- Architecture: HIGH — `find_next_record_start` 原语已存在，实现路径清晰
- Pitfalls: HIGH — 来自对现有代码的直接分析（off-by-one、无限循环、类型系统约束）

**Research date:** 2026-04-25
**Valid until:** 2026-05-25（rayon API 稳定，30 天内有效）
