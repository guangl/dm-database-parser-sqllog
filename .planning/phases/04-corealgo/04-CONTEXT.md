# Phase 4: CoreAlgo - Context

**Gathered:** 2026-04-24
**Status:** Ready for planning

<domain>
## Phase Boundary

重写 `LogIterator::next()` 的记录边界检测算法：用 `memmem::Finder(b"\n20")` 单次扫描替代 `memchr(b'\n')` 逐行循环，并将时间戳检测改为打包 `u64` 掩码比较，预期实现 15–45% 单线程吞吐提升。

**不在本 Phase 范围内：** 多线程并行分区优化（Phase 5）、新字段解析优化、编码处理路径。

</domain>

<decisions>
## Implementation Decisions

### ALGO-01: memmem 覆盖范围

- **D-01:** `find_next_record_start()`（供 `par_iter()` 分块用）与 `LogIterator::next()` 一并改为 `memmem` 实现。两者逻辑等价，统一改掉避免代码不一致；Phase 5 的 `par_iter` 重写也能从中受益。

### ALGO-01: is_multiline 检测

- **D-02:** `memmem` 定位到下一条记录起始位置 `found_at` 后，用 `memchr(b'\n', &data[..found_at]).is_some()` 判断当前记录是否含内嵌换行，以此传入 `is_multiline` 提示。每条记录多一次 `memchr`，但保留 `parse_record_with_hint` 的单行快速路径。

### ALGO-01: 静态 Finder 放置

- **D-03:** `Finder(b"\n20")` 以 `LazyLock<Finder<'static>>` 形式定义为模块级静态变量，命名 `FINDER_RECORD_START`，与现有 `FINDER_CLOSE_META` 风格一致，多线程共享无需重复构造。

### ALGO-02: u64 掩码布局

- **D-04:** 时间戳 8 个关键字节位置（0,1,4,7,10,13,16,19）不连续，采用**两次 LE u64 load + 掩码**方案：
  - `lo = u64::from_le_bytes(data[0..8])`：覆盖位置 0(`'2'`), 1(`'0'`), 4(`'-'`), 7(`'-'`)
  - `hi = u64::from_le_bytes(data[8..16])`：覆盖位置 10(`' '`), 13(`':'`)；位置 16(`':'`), 19(`'.'`) 不在 hi 范围内，用额外两次单字节比较处理（或第三次 u64 load 取 bytes[16..24]）
  - 掩码仅保留目标字节对应 bit，其余置零后与期望常量比较
  - 具体 `LO_MASK` / `HI_MASK` / `LO_EXPECTED` / `HI_EXPECTED` 常量值由 planner/executor 根据 LE 字节顺序推导

### Claude's Discretion

- 第三个字节窗口（位置 16,19）是做第三次 u64 load 还是两次单字节比较——由 planner 根据实际代码复杂度决定
- `find_next_record_start` 内部 loop 结构调整细节（是否保留"跳过首行"逻辑）

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 需求规格
- `.planning/REQUIREMENTS.md` — ALGO-01、ALGO-02 完整需求定义
- `.planning/ROADMAP.md` — Phase 4 Success Criteria（3 条验收标准）

### 现有代码（必读）
- `src/parser.rs` — `LogIterator::next()`（第 113–183 行）、`find_next_record_start()`（第 188–219 行）：算法重写目标
- `src/parser.rs` — `FINDER_CLOSE_META` LazyLock 定义（第 19 行）：新 Finder 的命名/风格参考
- `benches/parser_benchmark.rs` — benchmark 变体，重写后必须保持吞吐 ≥10% 提升

### 架构参考
- `.planning/phases/01-measurement/01-CONTEXT.md` — baseline 标定说明，CI 回归门禁 5% 阈值

</canonical_refs>

<code_context>
## Existing Code Insights

### 重写目标代码
- `LogIterator::next()`（`src/parser.rs:113`）：当前用 `while let Some(idx) = memchr(b'\n', &data[scan_pos..])` 逐行扫描，找到下一条时间戳行后返回当前记录切片
- `find_next_record_start()`（`src/parser.rs:188`）：同样的逐行模式，用于 `par_iter()` 的分块边界定位

### 可复用模式
- `FINDER_CLOSE_META: LazyLock<Finder<'static>>`（`src/parser.rs:19`）：新 `FINDER_RECORD_START` 照此定义
- `parse_record_with_hint(record_slice, is_multiline, encoding)` API 保持不变，`is_multiline` 语义不变，只改检测来源

### 时间戳检测现状
- 当前 8 个 `if` 条件：`next_bytes[0]==b'2' && next_bytes[1]==b'0' && next_bytes[4]==b'-' && next_bytes[7]==b'-' && next_bytes[10]==b' ' && next_bytes[13]==b':' && next_bytes[16]==b':' && next_bytes[19]==b'.'`
- 重写后改为两次 u64 load + 掩码，同样逻辑，同样覆盖 8 个位置

### Integration Points
- `memmem::Finder` 已在 `use memchr::memmem::Finder;` 中 import（`src/parser.rs:1`），无需新增依赖
- `memchr` 仍保留用于 `is_multiline` 检测和 `parse_record_with_hint` 内部，不能全量删除

</code_context>

<specifics>
## Specific Ideas

- `FINDER_RECORD_START` 搜索 `b"\n20"` 而非 `b"\n2"`，前者误报率更低（减少非时间戳 `\n2` 候选点）
- memmem 命中后仍需 23 字节全量验证（u64 掩码），因为 SQL body 内可能出现 `\n20xx` 字串
- 成功路径：`memmem` 找到候选 → u64 掩码验证 → 确认是时间戳 → 记录边界；失败路径：候选点验证失败 → 继续 `memmem` 找下一个候选

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 04-corealgo*
*Context gathered: 2026-04-24*
