# Phase 4: CoreAlgo - Pattern Map

**Mapped:** 2026-04-24
**Files analyzed:** 1 (src/parser.rs — 唯一修改目标)
**Analogs found:** 1 / 1 (文件即自身，所有模式提取自同一文件的现有代码)

---

## File Classification

| 修改文件 | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `src/parser.rs` | iterator / utility | streaming (byte scanning) | `src/parser.rs` 自身现有代码 | exact (原地重写) |

说明：本 Phase 只修改 `src/parser.rs`，不新增文件。模式全部从该文件现有代码提取。

---

## Pattern Assignments

### `src/parser.rs` — 需要修改的三个代码区域

#### 区域 1：新增静态 `FINDER_RECORD_START`

**Analog：** `src/parser.rs` 第 17–19 行 `FINDER_CLOSE_META`

**现有模式（lines 17–19）：**
```rust
/// Pre-built SIMD searcher for the `") "` meta-close pattern.
/// Avoids rebuilding the Finder on every record parse.
static FINDER_CLOSE_META: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b") "));
```

**照此模式新增（插入到第 19 行之后）：**
```rust
/// Pre-built SIMD searcher for the `"\n20"` record-start pattern.
/// Shared across threads via LazyLock; constructed once on first use.
static FINDER_RECORD_START: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b"\n20"));
```

关键点：
- `LazyLock<Finder<'static>>` 类型不变
- `LazyLock::new(|| Finder::new(...))` 闭包形式不变
- 字节字面量 `b"\n20"` 是 `&'static [u8]`，编译器推断 `Finder<'static>` 生命周期，无需 `into_owned()`
- 命名规范：`FINDER_` 前缀 + 描述用途的大写名称

---

#### 区域 2：重写 `LogIterator::next()`（lines 113–183）

**Analog：** `src/parser.rs` 第 113–183 行（现有实现，照结构保留，改内层扫描算法）

**现有核心扫描循环（lines 120–153）——需要替换：**
```rust
let mut scan_pos = 0;
let mut found_next = None;
let mut is_multiline = false;

while let Some(idx) = memchr(b'\n', &data[scan_pos..]) {
    let newline_idx = scan_pos + idx;
    let next_line_start = newline_idx + 1;

    if next_line_start >= data.len() {
        break;
    }

    let check_len = std::cmp::min(23, data.len() - next_line_start);
    if check_len == 23 {
        let next_bytes = &data[next_line_start..next_line_start + 23];
        if next_bytes[0] == b'2'
            && next_bytes[1] == b'0'
            && next_bytes[4] == b'-'
            && next_bytes[7] == b'-'
            && next_bytes[10] == b' '
            && next_bytes[13] == b':'
            && next_bytes[16] == b':'
            && next_bytes[19] == b'.'
        {
            found_next = Some(newline_idx);
            break;
        }
    }

    is_multiline = true;
    scan_pos = next_line_start;
}
```

**替换为（ALGO-01 + ALGO-02 合并后逻辑）：**
```rust
let mut found_boundary: Option<usize> = None;

for candidate in FINDER_RECORD_START.find_iter(data) {
    // candidate 是 '\n' 的位置；candidate+1 是候选时间戳起始
    let ts_start = candidate + 1;
    if ts_start + 23 <= data.len()
        && is_timestamp_start(&data[ts_start..ts_start + 23])
    {
        found_boundary = Some(candidate);
        break;
    }
    // 验证失败：继续 find_iter 找下一个候选，find_iter 自动管理偏移
}

let (record_end, next_start) = match found_boundary {
    Some(idx) => (idx, idx + 1),
    None => (data.len(), data.len()),
};

// D-02: 在确认的边界范围内检测是否含内嵌换行
let is_multiline = memchr(b'\n', &data[..record_end]).is_some();
```

**保持不变的后续代码（lines 161–182）：**
```rust
let record_slice = &data[..record_end];
self.pos += next_start;

// Trim trailing CR if present
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
```

---

#### 区域 3：重写 `find_next_record_start()`（lines 188–219）

**Analog：** `src/parser.rs` 第 188–219 行（现有实现）

**现有逐行扫描 loop（lines 197–218）——需要替换：**
```rust
loop {
    if pos + 23 > data.len() {
        return data.len();
    }
    let peek = &data[pos..pos + 23];
    if peek[0] == b'2'
        && peek[1] == b'0'
        && peek[4] == b'-'
        && peek[7] == b'-'
        && peek[10] == b' '
        && peek[13] == b':'
        && peek[16] == b':'
        && peek[19] == b'.'
    {
        return pos;
    }
    match memchr(b'\n', &data[pos..]) {
        Some(nl) => pos += nl + 1,
        None => return data.len(),
    }
}
```

**替换为（ALGO-01，保留行首检测逻辑避免 Pitfall 2）：**
```rust
// 先检查 line_start 本身是否是时间戳行（Finder 不会命中无前置\n的行首）
if pos + 23 <= data.len() && is_timestamp_start(&data[pos..pos + 23]) {
    return pos;
}

// 用 FINDER_RECORD_START 在剩余数据中搜索
for candidate in FINDER_RECORD_START.find_iter(&data[pos..]) {
    let ts_start = pos + candidate + 1;
    if ts_start + 23 <= data.len()
        && is_timestamp_start(&data[ts_start..ts_start + 23])
    {
        return ts_start;
    }
}
data.len()
```

**保持不变的前置"跳到行首"代码（lines 189–195）：**
```rust
let mut pos = from;
// Skip to start of next line
if let Some(nl) = memchr(b'\n', &data[pos..]) {
    pos += nl + 1;
} else {
    return data.len();
}
```

---

#### 区域 4：新增 `is_timestamp_start()` 辅助函数

**Analog：** `src/parser.rs` 第 407–412 行 `make_invalid_format_error`（同为私有辅助函数，带 `#[inline]` / `#[cold]` 属性的函数风格参考）

**现有辅助函数模式（lines 406–412）：**
```rust
/// 将原始字节转换为 InvalidFormat 错误（错误路径，标注 cold 避免影响热路径代码布局）
#[cold]
fn make_invalid_format_error(raw_bytes: &[u8]) -> ParseError {
    ParseError::InvalidFormat {
        raw: String::from_utf8_lossy(raw_bytes).to_string(),
    }
}
```

**照此风格新增（插入到 `make_invalid_format_error` 之前，约第 406 行）：**
```rust
// u64 掩码常量：验证时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"
// 字节位置：0('2'), 1('0'), 4('-'), 7('-'), 10(' '), 13(':'), 16(':'), 19('.')
const LO_MASK: u64     = 0xFF0000FF0000FFFF; // data[0..8]：位置 0,1,4,7
const LO_EXPECTED: u64 = 0x2D00002D00003032; // LE: '2'=0x32,'0'=0x30,'-'=0x2D,'-'=0x2D
const HI_MASK: u64     = 0x0000FF0000FF0000; // data[8..16]：位置 10,13（偏移 2,5）
const HI_EXPECTED: u64 = 0x00003A0000200000; // LE: ' '=0x20,':'=0x3A

/// 检查 bytes[0..23] 是否符合时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"。
/// 调用前需确保 bytes.len() >= 23（由调用方做长度检查）。
#[inline(always)]
fn is_timestamp_start(bytes: &[u8]) -> bool {
    debug_assert!(bytes.len() >= 23);
    let lo = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let hi = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    // 位置 16(':') 和 19('.') 用两次单字节比较（比第三次 u64 load 更清晰）
    (lo & LO_MASK == LO_EXPECTED)
        && (hi & HI_MASK == HI_EXPECTED)
        && bytes[16] == b':'
        && bytes[19] == b'.'
}
```

关键点：
- `#[inline(always)]` 对热路径函数（每条记录都调用）
- `#[cold]` 对错误路径函数（与此相反）
- `debug_assert!` 不影响 release 性能，文档化前置条件
- `try_into().unwrap()` 而非 unsafe 指针 cast（编译器优化为等价 MOV 指令）

---

## Shared Patterns

### Import 块（不需修改）

**Source:** `src/parser.rs` 第 1–10 行

```rust
use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
#[cfg(unix)]
use memmap2::Advice;
use memmap2::Mmap;
use simdutf8::basic::from_utf8 as simd_from_utf8;
use std::borrow::Cow;
use std::fs::File;
use std::path::Path;
use std::sync::LazyLock;
```

`Finder` 和 `LazyLock` 已导入，`memchr` 函数已导入。无需新增 use 语句。

---

### 长度检查前置模式

**Source:** `src/parser.rs` 第 133 行（现有代码）
**Apply to:** 所有调用 `is_timestamp_start` 的地方

```rust
// 调用 is_timestamp_start 前，始终先做长度检查（防止 Pitfall 3）
if ts_start + 23 <= data.len()
    && is_timestamp_start(&data[ts_start..ts_start + 23])
```

两个调用点（`LogIterator::next()` 和 `find_next_record_start()`）都必须遵循此模式。

---

### 相对偏移管理模式

**Source:** `src/parser.rs` 第 119、162 行

```rust
// data 是 self.data[self.pos..] 的切片——所有偏移都是相对于 data 的
let data = &self.data[self.pos..];
// ...
self.pos += next_start; // next_start 是 data 内的相对偏移
```

`is_multiline` 检测范围 `&data[..record_end]` 和 `found_boundary`/`record_end`/`next_start` 均为相对于 `data` 的偏移，不是绝对文件偏移（防止 Pitfall 4）。

---

## No Analog Found

无。本 Phase 唯一修改文件 `src/parser.rs` 同时也是自身的 analog，所有模式均可从现有代码直接提取。

---

## Metadata

**Analog search scope:** `src/parser.rs`（唯一修改目标，RESEARCH.md 已明确）
**Files scanned:** 4（src/parser.rs, src/sqllog.rs, src/error.rs, src/lib.rs）
**Pattern extraction date:** 2026-04-24

**关键约束备忘：**
- `memchr` import 必须保留（`is_multiline` 检测和 `parse_record_with_hint` 内部仍在使用）
- `FINDER_RECORD_START` 搜索 `b"\n20"` 而非 `b"\n2"`（减少 SQL body 内误报）
- `find_iter` 优先于手动 `find()` + 偏移管理（防止 Pitfall 5）
- u64 load 用 safe `try_into().unwrap()`，不用 unsafe 指针 cast
