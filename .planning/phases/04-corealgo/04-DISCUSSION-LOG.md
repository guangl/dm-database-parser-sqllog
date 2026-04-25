# Phase 4: CoreAlgo - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-24
**Phase:** 04-CoreAlgo
**Areas discussed:** find_next_record_start 覆盖范围, is_multiline 检测方案, u64 掩码布局策略, 静态 Finder 放置

---

## find_next_record_start 覆盖范围

| Option | Description | Selected |
|--------|-------------|----------|
| 一起改（推荐） | 两个函数逻辑等价，统一改掉；Phase 5 par_iter 重写也受益 | ✓ |
| 只改 LogIterator::next() | ALGO-01 只明确要求 next()，find_next_record_start 留到 Phase 5 | |

**User's choice:** 一起改  
**Notes:** 两个函数是同一套逻辑，统一处理更一致。

---

## is_multiline 检测方案

| Option | Description | Selected |
|--------|-------------|----------|
| 额外 memchr 检测（推荐） | memmem 找到 found_at 后，用 memchr(b'\n', &data[..found_at]).is_some() 判断 | ✓ |
| 默认多行=true | 始终传 is_multiline=true，省一次 memchr，但丢失单行快速路径 | |
| 去掉 is_multiline 优化 | parse_record_with_hint 统一用 memchr 找第一行末，删除两套路径 | |

**User's choice:** 额外 memchr 检测  
**Notes:** 保留 is_multiline 优化路径，多一次 memchr 代价可接受。

---

## u64 掩码布局策略

| Option | Description | Selected |
|--------|-------------|----------|
| 两次 u64 load + 掩码（推荐） | load bytes[0..8] 和 bytes[8..16] 各一次，掩码提取目标位置后比较 | ✓ |
| 单次 u64 load，只检查位置 0-7 | 仅前 8 字节做掩码，后 4 个位置（10,13,16,19）仍逐字节比较 | |

**User's choice:** 两次 u64 load + 掩码  
**Notes:** 更彻底的向量化，覆盖更多关键位置。

---

## 静态 Finder 放置

| Option | Description | Selected |
|--------|-------------|----------|
| LazyLock 静态变量（推荐） | 命名 FINDER_RECORD_START，与 FINDER_CLOSE_META 风格一致 | ✓ |
| 内嵌到 LogIterator 结构体 | 每个迭代器自带 Finder 字段，适合未来不同过滤策略 | |

**User's choice:** LazyLock 静态变量  
**Notes:** 与现有代码风格保持一致，无需修改 LogIterator 结构体。

---

## Claude's Discretion

- 第三个字节窗口（位置 16, 19）的具体实现（第三次 u64 load vs 两次单字节比较）
- `find_next_record_start` 内部跳过首行逻辑的保留方式

## Deferred Ideas

None.
