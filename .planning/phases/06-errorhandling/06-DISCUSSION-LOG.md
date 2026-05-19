# Phase 6: ErrorHandling - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 06-ErrorHandling
**Areas discussed:** 错误策略 & 行号

---

## ERR-02: 错误处理策略

| Option | Description | Selected |
|--------|-------------|----------|
| .skip_errors() 便捷方法 | LogIterator 上添加 .skip_errors() 返回 impl Iterator<Item=Sqllog>。调用最简单，符合 Rust 迭代器造就文化。 | ✓ |
| ErrorPolicy enum | 新增 ErrorPolicy { Skip, Collect(Vec), Abort } 枚举，在构造器或迭代器上设置。更灵活但增加 API 复杂度。 | |
| 保持现状 + 用户自己 filter_map | 现有 iter() 已返回 Result，用户用 .filter_map(Result::ok) 跳过错误。不新增 API，但 ERR-02 需求明确要库支持这个场景。 | |

**User's choice:** .skip_errors() 便捷方法
**Notes:** 用户认可最简单的方案，Rust 标准库风格。

---

## ERR-01: 行号定义

| Option | Description | Selected |
|--------|-------------|----------|
| 文件绝对行号 | 迭代器内部维护 line_number: u64 计数器。调用者看到「第 42 行出错」，能用 grep 定位。小幅影响吞吐性能。 | ✓ |
| 记录序号 | 返回第几条记录出错（record_index: usize）。实现更简单，但用户无法用文本工具直接定位。 | |
| 不追踪行号，保留原始内容就够 | ParseError 已包含 raw 字段。不加行号避免迭代器状态开销。 | |

**User's choice:** 文件绝对行号
**Notes:** 用户明确需要可 grep 的行号，接受轻微性能权衡。

---

## ParseError 变体细化

| Option | Description | Selected |
|--------|-------------|----------|
| 保持现有变体，只添加 line_number 字段 | 在 InvalidFormat / InvalidRecordStartLine / IntParseError 中添加 line_number: u64。最小改动。 | ✓ |
| 新增细化变体 | 添加 MissingTimestamp, MissingMeta 等。更精确但可能破坏现有用户的 match 分支。 | |
| You decide | Planner 根据现有代码自行判断。 | |

**User's choice:** 保持现有变体，只添加 line_number 字段
**Notes:** 用户倾向最小破坏性变更。

---

## Claude's Discretion

- `par_iter()` 是否支持行号：分区并行架构下全局行号需额外协调，暂不实现，在文档中说明此限制。

## Deferred Ideas

- **ERR-04**（结构化错误上下文）— Future requirements，非本阶段范围
- `par_iter()` 行号支持 — 架构复杂，暂缓
