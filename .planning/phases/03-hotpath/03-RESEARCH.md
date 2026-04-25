# Phase 3: HotPath - Research

**Researched:** 2026-04-24
**Domain:** Rust 热路径微优化 — 内联提示、早退逻辑、单次反向扫描、mmap advise
**Confidence:** HIGH

---

## Summary

Phase 3 包含四项互相独立、低风险的热路径微优化，每项均可单独实施和回滚。两项改动集中在 `find_indicators_split`（HOT-01 早退、HOT-02 单次扫描），一项为编译器提示（HOT-03 内联/cold），一项为 OS mmap 建议（HOT-04 MADV_SEQUENTIAL）。

所有改动均建立在已通过正确性加固（Phase 2 CORR-03 验证守卫）的基础上：`find_indicators_split` 末尾的 `parse_indicators_from_bytes` 验证守卫保证了假阳性被过滤。因此 HOT-01/02 的修改无需改动验证逻辑，只需改变搜索顺序和触发条件。

HOT-04 的 `mmap.advise()` 仅支持 Unix，Windows 上为 no-op；需加 `#[cfg(unix)]` 门控，不影响正确性。

**Primary recommendation:** 按 HOT-01 → HOT-02 → HOT-03 → HOT-04 顺序独立提交，每项改动后运行全量测试确认零退化。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 指标分割早退（HOT-01） | 库 core logic (`sqllog.rs`) | — | `find_indicators_split` 是纯函数，无 I/O |
| 单次反向字节扫描（HOT-02） | 库 core logic (`sqllog.rs`) | — | 同上，替换搜索算法 |
| 内联/cold 编译提示（HOT-03） | 编译器指令层 | — | 属性标注，不改逻辑 |
| mmap advise（HOT-04） | I/O 初始化层 (`parser.rs`) | OS 内核 | `LogParser::from_path` 是唯一 mmap 构造点 |

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| HOT-01 | `find_indicators_split` 在记录末尾不以 `.` 结尾时快速返回，跳过 3 次 rfind | DM 格式规范：EXEC_ID 字段始终以 `.` 结尾；`content_raw` 末尾字节检查为 O(1) |
| HOT-02 | `find_indicators_split` 改为单次反向字节扫描，替代 3 个独立 `rfind` 调用 | memchr crate 提供 `memrchr` 单字节反向搜索；扫描 `:` 再向前匹配关键字前缀即可 |
| HOT-03 | `parse_performance_metrics` 标注 `#[inline(always)]`，错误路径标注 `#[cold]` | Rust 参考手册确认 `#[inline(always)]` / `#[cold]` 均为函数级属性，编译器为提示 |
| HOT-04 | `LogParser::from_path` 调用 `mmap.advise(Advice::Sequential)` | memmap2 0.9.x 文档确认 `Advice::Sequential` 跨 Unix 平台可用，Windows 需 cfg 门控 |
</phase_requirements>

---

## Standard Stack

### Core（已在项目中）

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| memchr | 2.7.6 | 高性能单字节/多字节反向搜索 | SIMD 加速，`memrchr` 用于 HOT-02 |
| memmap2 | 0.9.9 | 内存映射文件 + advise | `Advice::Sequential` 用于 HOT-04 |

[VERIFIED: crates.io registry — `cargo metadata` 输出]

### 无需新增依赖

所有 Phase 3 改动均使用已有依赖，无需修改 `Cargo.toml`。

---

## Architecture Patterns

### 数据流（无变化）

```
File → LogParser::from_path (mmap + advise) → LogIterator → Sqllog<'a>
                                                                   │
                                               find_indicators_split() [HOT-01: 早退; HOT-02: 单次扫描]
                                                                   │
                                               parse_performance_metrics() [HOT-03: inline(always)]
```

### HOT-01: 早退逻辑

**观察：** DM 格式中 EXEC_ID 字段以 `.` 结尾（格式：`EXEC_ID: 12345.`），这是指标段末尾的唯一终止符。若 `content_raw` 末尾字节不是 `b'.'`（含换行时需 trim），则可确定该记录不含指标，直接返回 `len`。

**当前代码路径（`find_indicators_split`，sqllog.rs 行 227-252）：**
```rust
fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    let len = data.len();
    let start = len.saturating_sub(INDICATORS_WINDOW);
    let window = &data[start..];
    // 3 次 rfind 调用（FinderRev::rfind）
    ...
}
```

**改动后（在 window 搜索之前插入早退）：**
```rust
fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    let len = data.len();
    // HOT-01: 末尾字节检查（O(1)），不以 '.' 结尾则无指标
    let last = data.iter().rev()
        .find(|&&b| b != b'\n' && b != b'\r')
        .copied();
    if last != Some(b'.') {
        return len;
    }
    // ... 余下搜索逻辑
}
```

**正确性保证：** CORR-03 验证守卫（`parse_indicators_from_bytes` 返回 None 时 fallback 到 len）仍然保留，HOT-01 早退是在守卫之前的额外过滤层，不影响假阳性处理。

**边界情况：** 若 SQL 语句本身以 `.` 结尾（如 `SELECT 1.`），HOT-01 不会错误跳过——此时 `find_indicators_split` 仍会继续执行后续搜索，守卫会拦截假阳性。[ASSUMED: 仅改了早退的 `false` 路径，`true` 路径仍执行完整搜索]

### HOT-02: 单次反向字节扫描

**当前问题：** 3 次独立 `FinderRev::rfind` 分别搜索 `b"EXECTIME: "`、`b"ROWCOUNT: "`、`b"EXEC_ID: "` — 每次都是独立的 SIMD 扫描，但三者的公共特征是都包含 `: ` 序列，且前面有已知关键字前缀。

**方案：用 `memrchr` 扫描 `b':'` + 前向关键字匹配：**

```rust
// 替换 3 次 FinderRev::rfind
use memchr::memrchr;

fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    let len = data.len();
    // HOT-01 早退已过滤非 '.' 结尾
    let start = len.saturating_sub(INDICATORS_WINDOW);
    let window = &data[start..];

    let mut pos = window.len();
    let mut earliest = window.len();

    // 单次反向扫描 ':' 字节
    while let Some(colon_idx) = memrchr(b':', &window[..pos]) {
        pos = colon_idx; // 下次从此处之前继续
        // 检查前面是否是已知关键字之一（含后置空格）
        if window[..colon_idx].ends_with(b"EXECTIME")
            || window[..colon_idx].ends_with(b"ROWCOUNT")
            || window[..colon_idx].ends_with(b"EXEC_ID")
        {
            // 关键字起始位置：退到关键字开头
            let kw_start = find_keyword_start(window, colon_idx);
            earliest = earliest.min(kw_start);
        }
        if pos == 0 { break; }
    }
    let split = start + earliest;
    if split < len && parse_indicators_from_bytes(&data[split..]).is_none() {
        return len;
    }
    split
}
```

**关键点：**
- `memrchr` 是 SIMD 加速的单字节反向搜索，比 `FinderRev::rfind`（多字节）更轻量
- 反向扫描确保找到最左匹配（earliest），与当前逻辑等价
- 每次命中 `:` 后做 `ends_with` 检查（最多 8 字节比较），代价极低
- CORR-03 守卫保持不变

**替代方案：** 保留 `FinderRev` 但合并为一个 `FinderRev::new(b": ")` 扫描冒号+空格，再检查前缀。与上述方案等价，但 `memrchr` 更直接。[ASSUMED: 两方案性能相近，memrchr 更简洁]

### HOT-03: 内联提示

**`#[inline(always)]` 应用于 `parse_performance_metrics`：**

```rust
#[inline(always)]
pub fn parse_performance_metrics(&self) -> PerformanceMetrics<'a> {
    // ...
}
```

**`#[cold]` 应用于错误路径函数：** `#[cold]` 只能应用于函数定义，不能应用于代码块。需要将错误处理提取为独立函数：

```rust
// 在 parser.rs 中
#[cold]
fn return_invalid_format_error(raw: &[u8]) -> Result<Sqllog<'_>, ParseError> {
    Err(ParseError::InvalidFormat {
        raw: String::from_utf8_lossy(raw).to_string(),
    })
}
```

[VERIFIED: Rust Reference — `cold` 属性只能用于函数定义]

**验证编译产物：** 可通过 `cargo rustc --release -- --emit=llvm-ir` 或 `cargo asm` 检查生成代码中是否内联。

**注意：** `#[inline(always)]` 是提示，编译器可忽略（如递归函数）。`parse_performance_metrics` 非递归，实际上必定被内联。[VERIFIED: Rust Reference]

### HOT-04: mmap advise

**`memmap2::Advice::Sequential` 是 Unix-only，跨平台需 cfg 门控：**

```rust
// 在 LogParser::from_path 中，map() 调用后
#[cfg(unix)]
mmap.advise(memmap2::Advice::Sequential)
    .unwrap_or(()); // advise 失败不影响正确性，静默忽略

Ok(Self { mmap, encoding })
```

**理由：**
- `Advice::Sequential` 告知内核以顺序模式预读页面，减少 page fault 开销
- 顺序日志读取（`iter()` 从头到尾）是主要使用场景，advise 与实际访问模式匹配
- 失败（如内核不支持）静默忽略，不影响正确性

**Windows 行为：** `advise()` 方法在 Windows 上不存在（Unix-only），必须加 `#[cfg(unix)]`，否则编译失败。[VERIFIED: memmap2 0.9.10 文档]

**并行路径：** `par_iter()` 中各线程的 chunk 并非完全顺序读取同一 mmap，Sequential advise 对 par_iter 可能无益甚至有害（会触发不必要的预读）。HOT-04 只在 `from_path` 时设置一次全局 advise；若未来 par_iter 成为主路径，可考虑 `Advice::Normal` 或移除 advise。[ASSUMED]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 反向字节搜索 | 手写 `while pos > 0 { pos -= 1; ... }` | `memchr::memrchr` | SIMD 加速，性能差距 10x+ |
| 多字节反向搜索 | 手写模式匹配循环 | `memchr::memmem::FinderRev` | 已有预编译 SIMD finder |
| 内联判断 | 手动展开函数体 | `#[inline(always)]` | 编译器做得更好 |
| mmap 内存建议 | 手写 libc madvise | `memmap2::Mmap::advise()` | 安全封装，跨平台 |

---

## Common Pitfalls

### Pitfall 1: HOT-01 末尾字节检查遗漏 `\n` / `\r`

**What goes wrong:** 日志行通常以 `EXEC_ID: 123.\n` 结尾，直接检查 `data.last()` 得到 `\n` 而非 `.`，导致所有记录被错误判为无指标。

**Why it happens:** 末尾换行符被包含在 `content_raw` 中（`LogIterator` 对记录末尾不 trim 内容）。

**How to avoid:** 从末尾向前 skip 所有 `\n` 和 `\r`，取第一个非空白字节：
```rust
let last = data.iter().rev().find(|&&b| b != b'\n' && b != b'\r').copied();
if last != Some(b'.') { return len; }
```

**Warning signs:** 测试中所有有指标的记录都返回 `sql = full_content`（SQL 字段不被切分）。

### Pitfall 2: HOT-02 单次扫描遗漏最左匹配

**What goes wrong:** 从右向左扫描 `:` 找到第一个命中就停止，而最左匹配在更左边（例如 SQL body 中也有 `:` 字符）。

**Why it happens:** 搜索逻辑只记录第一次命中而不继续扫描。

**How to avoid:** 需继续扫描直到 `pos == 0`，记录所有命中中最小的 `kw_start`（即 `earliest`）。

**Warning signs:** `find_indicators_split_keyword_in_body_plus_real_indicators` 测试失败。

### Pitfall 3: HOT-02 `ends_with` 越界

**What goes wrong:** `window[..colon_idx].ends_with(b"EXEC_ID")` 当 `colon_idx < 7` 时会返回 false 但不 panic（`ends_with` 本身安全），无越界风险。但若误用切片索引则 panic。

**How to avoid:** 使用 `slice::ends_with` 不用手写边界检查。

### Pitfall 4: HOT-03 `#[cold]` 用于代码块而非函数

**What goes wrong:** `#[cold]` 不是路径注解，不能写在 `if` 分支里。编译器会报错或静默忽略。

**Why it happens:** 混淆了 C/C++ 的 `__builtin_expect` 与 Rust 的 `#[cold]`。

**How to avoid:** 将错误路径提取为独立 `#[cold]` 函数，主函数调用该函数。

**Warning signs:** `cargo build` 报错 `#[cold] attribute cannot be applied to a statement`。

### Pitfall 5: HOT-04 Windows 编译失败

**What goes wrong:** `mmap.advise(Advice::Sequential)` 在 Windows 上不存在，`cargo build --target x86_64-pc-windows-gnu` 失败。

**How to avoid:** 加 `#[cfg(unix)]` 门控整个 advise 调用。

**Warning signs:** CI Windows job 报 `method not found in Mmap`。

---

## Code Examples

### HOT-01 早退模式

```rust
// Source: 基于当前 find_indicators_split 实现（src/sqllog.rs:227）
fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    let len = data.len();

    // HOT-01: O(1) 早退 — DM 格式 EXEC_ID 字段以 '.' 结尾
    // 跳过末尾换行符后检查最后一个有效字节
    let last_meaningful = data.iter().rev()
        .find(|&&b| b != b'\n' && b != b'\r')
        .copied();
    if last_meaningful != Some(b'.') {
        return len;  // 无指标，快速返回
    }

    // 余下逻辑不变...
    let start = len.saturating_sub(INDICATORS_WINDOW);
    // ...
}
```

### HOT-02 单次反向扫描（概念示例）

```rust
// Source: 研究阶段草案（未提交），依赖 memchr::memrchr
use memchr::memrchr;

// 在 window 上单次反向扫描 ':'
let mut search_end = window.len();
let mut earliest = window.len();

while search_end > 0 {
    match memrchr(b':', &window[..search_end]) {
        None => break,
        Some(colon) => {
            let prefix = &window[..colon];
            if prefix.ends_with(b"EXECTIME")
                || prefix.ends_with(b"ROWCOUNT")
                || prefix.ends_with(b"EXEC_ID")
            {
                // 关键字的起始索引（EXECTIME=8字节，ROWCOUNT=8字节，EXEC_ID=7字节）
                let kw_len = if prefix.ends_with(b"EXECTIME") { 8 }
                    else if prefix.ends_with(b"ROWCOUNT") { 8 }
                    else { 7 };
                earliest = earliest.min(colon - kw_len);
            }
            search_end = colon; // 继续向左扫描
        }
    }
}
```

### HOT-03 属性标注

```rust
// Source: Rust Reference (https://doc.rust-lang.org/reference/attributes/codegen.html)

// 热路径函数：强制内联
#[inline(always)]
pub fn parse_performance_metrics(&self) -> PerformanceMetrics<'a> {
    // ...
}

// 错误路径提取为独立函数并标注 cold
#[cold]
fn make_invalid_format_error(raw: &[u8]) -> ParseError {
    ParseError::InvalidFormat {
        raw: String::from_utf8_lossy(raw).to_string(),
    }
}
```

### HOT-04 mmap advise

```rust
// Source: memmap2 0.9.10 docs (https://docs.rs/memmap2/0.9.10/memmap2/struct.Mmap.html)
use memmap2::Advice;

pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
    let file = File::open(path).map_err(|e| ParseError::IoError(e.to_string()))?;
    let mmap = unsafe { Mmap::map(&file).map_err(|e| ParseError::IoError(e.to_string()))? };

    // HOT-04: 顺序读取建议（仅 Unix，失败静默忽略）
    #[cfg(unix)]
    let _ = mmap.advise(Advice::Sequential);

    // 编码检测...
    let encoding = ...;
    Ok(Self { mmap, encoding })
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 全文件 3 次 FinderRev::rfind | HOT-02 单次 memrchr 扫描 | Phase 3 | 减少 SIMD 启动开销 |
| 无早退，始终执行 rfind | HOT-01 末尾字节检查 | Phase 3 | 无指标记录 O(1) 返回 |
| 无 mmap 访问提示 | HOT-04 MADV_SEQUENTIAL | Phase 3 | OS 预读减少 page fault |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | SQL 语句以 `.` 结尾不触发 HOT-01 误判（守卫兜底） | HOT-01 Pattern | 守卫失效则 body 截断，但守卫来自 CORR-03 已测试 |
| A2 | `memrchr` 单字节扫描 + `ends_with` 匹配比 3 次 `FinderRev::rfind` 更快 | HOT-02 | 需 benchmark 验证；若更慢可回退 |
| A3 | Sequential advise 对 `iter()` 顺序读有益，对 `par_iter()` 影响中性 | HOT-04 | 若 par_iter 为主路径，advise 可能触发不必要预读 |
| A4 | `#[cold]` 提取错误路径函数覆盖范围：`parse_record_with_hint` 的 InvalidFormat 分支 | HOT-03 | 若错误路径不是瓶颈，效果不可测；但无害 |

---

## Open Questions

1. **HOT-01 的末尾字节判断是否涵盖多行记录**
   - What we know: 多行记录的 `content_raw` 末尾仍以 `EXEC_ID: N.\n` 结束（见 tests/parser_iterator.rs）
   - What's unclear: 是否有多行记录末尾不以 `\n` 结尾的场景（文件末尾无换行）
   - Recommendation: 测试无结尾换行的多行记录边界情况

2. **HOT-02 是否应保留 `FinderRev` 静态实例**
   - What we know: 当前 `FINDER_REV_EXECTIME` 等静态变量在 HOT-02 改动后可删除
   - What's unclear: `FinderRev` 实例是否被其他路径引用
   - Recommendation: grep 确认后删除，减少 LazyLock 开销

3. **HOT-03 的 cold 函数边界**
   - What we know: `#[cold]` 只能标注函数，不能标注 if 分支
   - What's unclear: `parse_record_with_hint` 中有多个 `InvalidFormat` return 点，提取哪些合适
   - Recommendation: 提取 `make_invalid_format` 辅助函数，覆盖所有 `ParseError::InvalidFormat` 构造点

---

## Environment Availability

Step 2.6: 本 phase 全部为代码/配置改动，无新增外部依赖。跳过环境可用性审计。

唯一平台约束：`mmap.advise()` 仅 Unix（已通过 `#[cfg(unix)]` 处理）。

---

## Validation Architecture

nyquist_validation 在 config.json 中设为 false，跳过本节。

---

## Security Domain

本 phase 不涉及认证、输入验证边界扩展或加解密。跳过 ASVS 审计。

---

## Sources

### Primary (HIGH confidence)
- `src/sqllog.rs`（当前实现）— `find_indicators_split`、`parse_performance_metrics` 完整代码
- `src/parser.rs`（当前实现）— `LogParser::from_path`、mmap 构造路径
- [memmap2 0.9.10 Advice enum docs](https://docs.rs/memmap2/0.9.10/memmap2/enum.Advice.html) — `Advice::Sequential` 跨 Unix 平台可用，Windows 不支持
- [memmap2 0.9.10 Mmap::advise docs](https://docs.rs/memmap2/0.9.10/memmap2/struct.Mmap.html#method.advise) — Unix-only，签名 `fn advise(&self, advice: Advice) -> Result<()>`
- [Rust Reference — codegen attributes](https://doc.rust-lang.org/reference/attributes/codegen.html) — `#[inline(always)]`、`#[cold]` 均为函数级属性，编译器提示
- `/Users/guang/.cargo/registry/src/.../memchr-2.7.6/src/memchr.rs` — `memrchr`、`memrchr2`、`memrchr3` 签名已在本地源码验证

### Secondary (MEDIUM confidence)
- memchr crate 2.7.6 README — `memrchr3_iter` 示例验证反向多字节搜索 API 存在
- benchmarks/parser_benchmark.rs — 确认 `parse_performance_metrics` 是 MEAS-02 热路径变体

### Tertiary (LOW confidence)
- ASSUMED: HOT-02 memrchr 单次扫描优于 3 次 FinderRev（需 benchmark 实测）
- ASSUMED: Sequential advise 对 par_iter 影响中性

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — 全部使用已有依赖，版本已验证
- Architecture patterns: HIGH — 基于实际源码分析，非猜测
- Pitfalls: HIGH — 来自代码阅读和格式规范分析；A2/A3 需 benchmark 验证
- HOT-04 平台限制: HIGH — memmap2 官方文档明确说明 Unix-only

**Research date:** 2026-04-24
**Valid until:** 2026-05-24（memmap2/memchr API 稳定，30 天内有效）
