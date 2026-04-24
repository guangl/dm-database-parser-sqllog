# Phase 4: CoreAlgo - Research

**Researched:** 2026-04-24
**Domain:** Rust 字节扫描算法优化 — memmem 子串搜索、u64 掩码比较
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** `find_next_record_start()` 与 `LogIterator::next()` 一并改为 `memmem` 实现，统一消除逐行 memchr 循环。

**D-02:** `memmem` 定位到候选位置 `found_at` 后，用 `memchr(b'\n', &data[..found_at]).is_some()` 判断 `is_multiline`，保留 `parse_record_with_hint` 的单行快速路径。

**D-03:** `Finder(b"\n20")` 以 `LazyLock<Finder<'static>>` 形式定义为模块级静态变量，命名 `FINDER_RECORD_START`，与 `FINDER_CLOSE_META` 风格一致。

**D-04:** 时间戳检测改为两次 LE u64 load + 掩码方案（lo = data[0..8]，hi = data[8..16]），位置 16/19 额外处理（第三次 u64 load 或两次单字节比较由 planner 决定）。

### Claude's Discretion

- 第三个字节窗口（位置 16, 19）：第三次 u64 load 还是两次单字节比较——由 planner 根据代码复杂度决定。
- `find_next_record_start` 内部 loop 结构调整细节（是否保留"跳过首行"逻辑）。

### Deferred Ideas (OUT OF SCOPE)

None — 讨论阶段无延期项。

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ALGO-01 | `LogIterator::next()` 使用预构建 `memmem::Finder(b"\n20")` 替代 `memchr(b'\n')` 逐行循环 | memmem::Finder API 已验证；LazyLock 模式在代码库中已有先例（FINDER_CLOSE_META）；算法逻辑在本文"架构模式"节详述 |
| ALGO-02 | 时间戳检测改为打包 u64 掩码比较，替代 8 个独立字节比较分支 | 三组掩码常量已通过 Python 验证，正例/负例均通过 |

</phase_requirements>

---

## Summary

Phase 4 目标是用两项算法改进替换 `LogIterator::next()` 和 `find_next_record_start()` 中的逐行扫描逻辑。第一项（ALGO-01）：以 `memmem::Finder(b"\n20")` 单次子串搜索跳过所有无关字节，彻底消除内层 `while let Some(idx) = memchr(b'\n', ...)` 循环。第二项（ALGO-02）：时间戳 8 字节位置检测从 8 个独立 if 分支改为 2–3 次 LE u64 load + 位掩码比较，减少分支预测开销并允许编译器向量化。

两项改动均在现有代码框架内进行：`memmem::Finder` 已经 import，`LazyLock<Finder<'static>>` 模式已有 `FINDER_CLOSE_META` 先例，无需新增依赖。所有现有测试（19 个单元测试 + 2 个 doc-tests，已在 Phase 3 后全部通过）必须在改动后继续通过。Phase 1 建立的 benchmark 基础设施用于验证 ≥10% 吞吐提升门禁。

`nyquist_validation: false`——无需编写新测试文件，但所有现有测试必须通过。

**Primary recommendation:** 用 `FINDER_RECORD_START.find_iter(data)` + u64 掩码内联函数替换两处逐行 memchr 循环；u64 掩码用 `is_timestamp_start(bytes: &[u8]) -> bool` 单独提取，方便测试和复用。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 记录边界检测（memmem 扫描） | `LogIterator::next()` | `find_next_record_start()` | 两处都有同样的逐行逻辑，D-01 决定同步改写 |
| 时间戳合法性验证 | 内联于 `LogIterator::next()` | 抽取为 `is_timestamp_start()` 辅助函数 | 同一逻辑需在两个调用点复用，提取函数避免重复 |
| is_multiline 提示 | `LogIterator::next()` | — | `memchr` 在 found_at 范围内检测内嵌换行，结果传给 `parse_record_with_hint` |
| Benchmark 回归验证 | `benches/parser_benchmark.rs` | CI `check-regression.sh` | Phase 1 已建立，Phase 4 直接复用 |

---

## Standard Stack

### Core（已在 Cargo.toml 中，无需新增）

| Library | Version (锁定) | Purpose | Why Standard |
|---------|----------------|---------|--------------|
| `memchr` | 2.8.0 | `memmem::Finder` 子串搜索 + `memchr` 字节搜索 | SIMD 加速，业界标准，已是项目依赖 [VERIFIED: cargo tree] |
| `std::sync::LazyLock` | stable (Rust 1.80+) | 静态 Finder 惰性初始化 | 已在 parser.rs 中用于 FINDER_CLOSE_META [VERIFIED: src/parser.rs:10,19] |

**无需新增依赖。** [VERIFIED: Cargo.toml]

### 关键 API

`memchr::memmem::Finder` 已验证 API [VERIFIED: Context7 /burntsushi/memchr]:

```rust
// 构造（一次性开销）
let finder = Finder::new(b"\n20");

// 搜索（返回第一个命中位置，相对于 haystack 起始）
finder.find(haystack: &[u8]) -> Option<usize>

// 搜索所有命中（返回迭代器）
finder.find_iter(haystack: &[u8]) -> impl Iterator<Item = usize>
```

`Finder<'static>` 用于 `LazyLock`，需 `Finder::new(b"\n20").into_owned()` 或直接 `Finder::new` 并泄漏。已有代码直接写 `Finder::new(b") ")` 作为 LazyLock 体——编译器推断 `'static` 生命周期（字节字面量是 `&'static [u8]`）。[VERIFIED: src/parser.rs:19]

---

## Architecture Patterns

### System Architecture Diagram（Phase 4 后的数据流）

```
File (mmap)
    │
    ▼
LogIterator::next()
    │
    ├─ FINDER_RECORD_START.find_iter(data)
    │       │
    │       ├─ 候选命中 found_at ──► is_timestamp_start(&data[found_at+1..])
    │       │                              │
    │       │                    ┌─ True ──┤ 记录边界确认
    │       │                    │         │  is_multiline = memchr('\n', &data[..found_at]).is_some()
    │       │                    │         │  record_slice = &data[..found_at]
    │       │                    │         └─► parse_record_with_hint(slice, is_multiline, encoding)
    │       │                    │
    │       │                    └─ False ─► 继续 find_iter（下一个候选）
    │       │
    │       └─ find_iter 耗尽 ──► 最后一条记录（record_end = data.len()）
    │
    └─ 返回 Option<Result<Sqllog<'a>, ParseError>>
```

### 推荐代码结构

现有文件结构不变，只修改 `src/parser.rs`：

```
src/
├── parser.rs       # 修改：添加 FINDER_RECORD_START、is_timestamp_start()、
│                   #       重写 LogIterator::next()、find_next_record_start()
├── sqllog.rs       # 不变
├── error.rs        # 不变
└── lib.rs          # 不变
```

### Pattern 1: LazyLock 静态 Finder（ALGO-01）

**What:** 模块级惰性初始化静态 Finder，首次调用时构建，之后线程安全共享。
**When to use:** 需要多线程共享同一 Finder 而不重复构造开销时。

```rust
// Source: src/parser.rs:19（现有 FINDER_CLOSE_META 模式，直接照抄）
// [VERIFIED: src/parser.rs:19]
static FINDER_RECORD_START: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b"\n20"));
```

### Pattern 2: memmem find_iter 驱动的记录扫描（ALGO-01）

**What:** 用 `find_iter` 替代逐行 memchr 循环，单次 SIMD 扫描跳过所有无关字节。
**When to use:** 在大缓冲区中查找以特定字节序列开头的"记录"边界。

```rust
// 重写后的 LogIterator::next() 核心扫描逻辑（伪代码）
let data = &self.data[self.pos..];
let mut found_boundary: Option<usize> = None;

for candidate in FINDER_RECORD_START.find_iter(data) {
    // candidate 是 '\n' 的位置，candidate+1 是候选时间戳起始
    let ts_start = candidate + 1;
    if ts_start + 23 <= data.len()
        && is_timestamp_start(&data[ts_start..ts_start + 23])
    {
        found_boundary = Some(candidate);
        break;
    }
    // 验证失败：继续 find_iter 找下一个候选
}

let (record_end, next_start) = match found_boundary {
    Some(idx) => (idx, idx + 1),
    None => (data.len(), data.len()),
};

let is_multiline = memchr(b'\n', &data[..record_end]).is_some();
```

### Pattern 3: u64 掩码时间戳验证（ALGO-02）

**What:** 两次（或三次）LE u64 load + 位掩码比较，替代 8 个独立字节 if 分支。
**When to use:** 需要以最低分支开销验证固定偏移字节值时。

已验证的掩码常量（[VERIFIED: Python 计算 + 正/负/边界例子验证]）：

```rust
// 时间戳格式: "20YY-MM-DD HH:MM:SS.mmm"
//              0123456789012345678901234
//
// LO 窗口 data[0..8]：检查位置 0('2'), 1('0'), 4('-'), 7('-')
const LO_MASK: u64     = 0xFF0000FF0000FFFF;
const LO_EXPECTED: u64 = 0x2D00002D00003032;  // LE: '2'=0x32,'0'=0x30,'-'=0x2D,'-'=0x2D

// HI 窗口 data[8..16]：检查位置 10(' '), 13(':')  （偏移 2, 5）
const HI_MASK: u64     = 0x0000FF0000FF0000;
const HI_EXPECTED: u64 = 0x00003A0000200000;  // LE: ' '=0x20,':'=0x3A

// TH 窗口 data[16..24]：检查位置 16(':'), 19('.')  （偏移 0, 3）
const TH_MASK: u64     = 0x00000000FF0000FF;
const TH_EXPECTED: u64 = 0x000000002E00003A;  // LE: ':'=0x3A,'.'=0x2E

#[inline(always)]
fn is_timestamp_start(bytes: &[u8]) -> bool {
    debug_assert!(bytes.len() >= 23);
    // SAFETY: 调用前已检查 bytes.len() >= 23
    let lo = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let hi = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    // 位置 16(':') 和 19('.') 通过第三次 load 或单字节比较处理
    (lo & LO_MASK == LO_EXPECTED)
        && (hi & HI_MASK == HI_EXPECTED)
        && bytes[16] == b':'
        && bytes[19] == b'.'
}
```

**注意：** `bytes[16]` 和 `bytes[19]` 两次单字节比较 vs 第三次 u64 load，由 planner 选择。单字节比较更简单，第三次 load 更一致——两者性能差距极小（覆盖 2 个字节的掩码优化空间有限）。[ASSUMED: 性能差异可忽略，但未实测]

### Pattern 4: find_next_record_start 重写（ALGO-01，D-01）

**What:** 同样用 memmem 替代逐行 memchr 循环。
**When:** par_iter() 分块边界定位。

```rust
fn find_next_record_start(data: &[u8], from: usize) -> usize {
    // 1. 跳到 from 之后的行首
    let line_start = match memchr(b'\n', &data[from..]) {
        Some(nl) => from + nl + 1,
        None => return data.len(),
    };
    // 2. 检查 line_start 本身是否是时间戳行（无前置\n，Finder 不会命中）
    if line_start + 23 <= data.len()
        && is_timestamp_start(&data[line_start..line_start + 23])
    {
        return line_start;
    }
    // 3. 用 FINDER_RECORD_START 在剩余数据中搜索
    for candidate in FINDER_RECORD_START.find_iter(&data[line_start..]) {
        let ts_start = line_start + candidate + 1;
        if ts_start + 23 <= data.len()
            && is_timestamp_start(&data[ts_start..ts_start + 23])
        {
            return ts_start;
        }
    }
    data.len()
}
```

### Anti-Patterns to Avoid

- **保留内层 while-memchr 循环：** 改用 memmem 的核心目的就是消除它。如果重写后还有 `while let Some(idx) = memchr(b'\n', ...)` 驱动的循环，ALGO-01 目标未达成。
- **每次调用重新构造 Finder：** `Finder::new(b"\n20")` 有一次性预处理开销（SIMD 向量预计算），必须用 LazyLock 静态化。
- **unsafe 字节 load：** u64 load 用 `u64::from_le_bytes(slice.try_into().unwrap())` 而非 unsafe pointer cast，safe Rust 编译器会优化为同等指令。
- **TH 窗口过度优化：** 第三次完整 u64 load 为 TH 窗口仅需检查 2 字节，代码复杂度 > 收益；两次单字节比较 `bytes[16] == b':' && bytes[19] == b'.'` 更清晰。[ASSUMED]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 子串搜索（Boyer-Moore / SIMD 多字节匹配） | 自写 SIMD 换行扫描 | `memchr::memmem::Finder` | REQUIREMENTS.md Out-of-Scope 明确排除；memchr 已是天花板 [VERIFIED: REQUIREMENTS.md] |
| LazyLock 单例 | 手写 `OnceLock` + `unsafe` | `std::sync::LazyLock` | stable since Rust 1.80，代码库已用 [VERIFIED: src/parser.rs:10] |
| LE u64 转换 | unsafe 指针 cast | `u64::from_le_bytes(slice.try_into().unwrap())` | 编译器优化为 `MOV`，safe，无 UB 风险 |

---

## Common Pitfalls

### Pitfall 1: FINDER_RECORD_START 搜索范围包含第一条记录起始

**What goes wrong:** `data` 从文件开头开始时（`self.pos == 0`），第一条记录以 `20YY-MM-DD` 开头，没有前置 `\n`。`FINDER_RECORD_START.find(data)` 不会命中第一条记录的开头，而是找到第一条记录结束后的 `\n20`。这是**正确行为**——`self.pos` 从 0 开始，第一次 `find_iter` 确实应该找到第一条记录的结束边界（下一条记录的前置 `\n`），而第一条记录从 `data[0]` 到 `found_at-1`。
**Why it happens:** `\n20` 不匹配行首的 `20`。
**How to avoid:** 不需要特殊处理——第一条记录自然地被提取为 `data[0..found_at]`。
**Warning signs:** 测试中第一条记录被跳过，或返回从文件中间开始的切片。

### Pitfall 2: find_next_record_start 的行首检测缺失

**What goes wrong:** `from` 之后跳到 `line_start`，`line_start` 本身可能是时间戳行，但 `FINDER_RECORD_START` 在 `data[line_start..]` 中不会命中其起始位置（没有前置 `\n`），导致跳过了正确的边界。
**Why it happens:** Finder 搜索 `\n20`，行首没有 `\n`。
**How to avoid:** 跳到 `line_start` 后先单独检查一次 `is_timestamp_start(&data[line_start..])` 再启动 Finder 循环。
**Warning signs:** par_iter 测试失败，或多线程下记录被截断/跳过。

### Pitfall 3: u64 load 越界

**What goes wrong:** `is_timestamp_start` 接受 `&[u8]`，若长度 < 23（甚至 < 16）则 `try_into().unwrap()` panic。
**Why it happens:** 在 `LogIterator::next()` 中忘记先检查 `ts_start + 23 <= data.len()`。
**How to avoid:** 在调用 `is_timestamp_start` 前**始终**先做长度检查。
**Warning signs:** `cargo test` 中 panic at `try_into` unwrap。

### Pitfall 4: is_multiline 判断范围错误

**What goes wrong:** D-02 规定 `memchr(b'\n', &data[..found_at]).is_some()`，其中 `found_at` 是从 `data[self.pos..]` 开始的偏移。若误用 `data[..record_end]` 而 `record_end` 含末尾 CR，或使用了绝对偏移，则 `is_multiline` 可能误判。
**Why it happens:** 相对/绝对偏移混淆。
**How to avoid:** 明确 `data = &self.data[self.pos..]`，`found_at` 和 `record_end` 均是 `data` 内的相对偏移。
**Warning signs:** `iterator_multiline_detection` 测试失败，或单行记录被误判为多行导致性能回归。

### Pitfall 5: 候选点验证失败后的搜索偏移

**What goes wrong:** `find_iter` 返回的是 `\n` 的位置，下一个候选点从 `\n20` 命中位置 + 1 继续，但 `find_iter` 本身是无状态迭代器，天然处理这一点——只要不手动 break 后重新搜索就没问题。
**Why it happens:** 若改为手动 `find()` + 偏移管理，容易跳过候选点或无限循环。
**How to avoid:** 优先使用 `find_iter` 迭代器，让 memchr 管理搜索偏移。

---

## Code Examples

### 完整的 is_timestamp_start 函数

```rust
// Source: 推导自 src/parser.rs:137-144 现有逻辑 + Python 验证
// [VERIFIED: Python mask calculation + 正/负/边界测试]

const LO_MASK: u64     = 0xFF0000FF0000FFFF;
const LO_EXPECTED: u64 = 0x2D00002D00003032;
const HI_MASK: u64     = 0x0000FF0000FF0000;
const HI_EXPECTED: u64 = 0x00003A0000200000;

/// 检查 bytes[0..23] 是否符合时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"
/// 调用前需确保 bytes.len() >= 23
#[inline(always)]
fn is_timestamp_start(bytes: &[u8]) -> bool {
    let lo = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let hi = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    (lo & LO_MASK == LO_EXPECTED)
        && (hi & HI_MASK == HI_EXPECTED)
        && bytes[16] == b':'
        && bytes[19] == b'.'
}
```

### 重写后的 LogIterator::next() 骨架

```rust
// Source: 基于 src/parser.rs:113-183 重写方案
// [VERIFIED: 与现有代码结构对齐]

fn next(&mut self) -> Option<Self::Item> {
    loop {
        if self.pos >= self.data.len() {
            return None;
        }

        let data = &self.data[self.pos..];
        let mut found_boundary: Option<usize> = None;

        // ALGO-01: 用 memmem 单次扫描替代逐行 memchr 循环
        for candidate in FINDER_RECORD_START.find_iter(data) {
            let ts_start = candidate + 1;
            if ts_start + 23 <= data.len()
                && is_timestamp_start(&data[ts_start..ts_start + 23])
            {
                found_boundary = Some(candidate);
                break;
            }
        }

        let (record_end, next_start) = match found_boundary {
            Some(idx) => (idx, idx + 1),
            None => (data.len(), data.len()),
        };

        // D-02: is_multiline 检测
        let is_multiline = memchr(b'\n', &data[..record_end]).is_some();

        let record_slice = &data[..record_end];
        self.pos += next_start;

        // Trim trailing CR
        let record_slice = if record_slice.ends_with(b"\r") {
            &record_slice[..record_slice.len() - 1]
        } else {
            record_slice
        };

        if record_slice.is_empty() {
            continue;
        }

        return Some(parse_record_with_hint(
            record_slice,
            is_multiline,
            self.encoding,
        ));
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 8 个独立 if 字节比较 | u64 掩码比较（本 Phase） | Phase 4 | 减少分支预测开销，允许编译器 CSE |
| `while memchr('\n')` 逐行循环 | `memmem::find_iter("\n20")` 跳跃扫描（本 Phase） | Phase 4 | 减少总扫描字节数（SIMD 跨步），消除内层循环 |
| 每次调用重建 Finder | `LazyLock<Finder<'static>>` 一次构建（本 Phase） | Phase 4 | 消除 Finder 构造开销 |

---

## Open Questions

1. **第三个 u64 窗口（位置 16, 19）：单字节 vs 第三次 load**
   - 什么已知：两种方案功能等价，掩码常量 `TH_MASK/TH_EXPECTED` 已计算好
   - 什么不确定：实际 codegen 差异（两次 `movzx` + compare 对一次 `movq` + and + compare）
   - 建议：用两次单字节比较（`bytes[16] == b':' && bytes[19] == b'.'`），代码更清晰，差异在噪声范围内

2. **is_multiline 的 memchr 额外开销**
   - 什么已知：D-02 决定添加此检测；每条记录额外一次 memchr 调用
   - 什么不确定：单行记录占多数时（合成语料库 80%），此 memchr 是否会抵消 memmem 带来的收益
   - 建议：接受此开销，因为 `parse_record_with_hint` 的单行快速路径价值超过这次 memchr；若 benchmark 显示退化可改为扫描时计数换行符

---

## Environment Availability

Step 2.6: SKIPPED — Phase 4 为纯代码改动，无外部工具/服务依赖。所有依赖（memchr 2.8.0）已在 Cargo.lock 中锁定。[VERIFIED: cargo tree]

---

## Validation Architecture

`nyquist_validation: false` — 跳过此节。

---

## Security Domain

Phase 4 为纯性能算法优化，无新增输入解析、认证、加密或权限控制逻辑。ASVS 不适用。

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | 两次单字节比较（bytes[16]/bytes[19]）与第三次 u64 load 性能差异可忽略 | Pattern 3, Anti-Patterns | 若 load+mask 更快，可补充第三次 load；不影响正确性 |
| A2 | is_multiline 的额外 memchr 开销不会抵消 memmem 收益（单线程 ≥10% 净提升可达） | Open Questions | 若 benchmark 未达标，需考虑移除 is_multiline 检测或改变策略 |

---

## Sources

### Primary (HIGH confidence)
- `src/parser.rs` — 完整现有实现，重写目标代码 [VERIFIED: 直接读取]
- `src/parser.rs:19` — FINDER_CLOSE_META LazyLock 模式 [VERIFIED: 直接读取]
- `Cargo.toml` + `cargo tree` — memchr 2.8.0 依赖确认 [VERIFIED: 工具输出]
- Context7 `/burntsushi/memchr` — Finder API、find_iter、into_owned 文档 [VERIFIED: Context7]
- Python 脚本 — LO_MASK / LO_EXPECTED / HI_MASK / HI_EXPECTED / TH_MASK / TH_EXPECTED 正例+负例+边界验证 [VERIFIED: 本次 session 计算]

### Secondary (MEDIUM confidence)
- `.planning/phases/04-corealgo/04-CONTEXT.md` — 已锁定决策 D-01 至 D-04 [VERIFIED: 直接读取]

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — memchr 已在依赖中，LazyLock 已有先例，无新依赖
- Architecture: HIGH — 重写目标代码已读取，算法边界案例已分析
- Pitfalls: HIGH — 基于现有代码结构推导，边界案例（行首时间戳、末尾记录、SQL body 内 \n20）已覆盖
- u64 掩码常量: HIGH — Python 计算并经正/负/边界三类样本验证

**Research date:** 2026-04-24
**Valid until:** 2026-05-24（memchr API 稳定，掩码常量不变）
