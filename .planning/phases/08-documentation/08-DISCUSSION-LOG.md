# Phase 8: Documentation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 08-Documentation
**Areas discussed:** 文档语言 & 示例

---

## DOC-01/02: 文档语言

| Option | Description | Selected |
|--------|-------------|----------|
| 英文 | 面向 crates.io / docs.rs 国际社区。现有中文注释全部更换。 | |
| 中英双语 | 英文为主，标题/示例描述加中文翻译。方便国内用户但增加维护工作量。 | |
| 中文 | 保持现有中文注释风格。目标用户群是国内 DM 数据库生态用户。 | ✓ |

**User's choice:** 中文
**Notes:** 用户明确目标受众是国内 DM 数据库用户，与现有代码注释风格保持一致。

---

## DOC-03: examples/ 第二个示例场景

| Option | Description | Selected |
|--------|-------------|----------|
| batch_export.rs — 批量导出 | 读取所有记录，将 timestamp + sql + exec_time 导出为 CSV（写到 stdout）。展示 parse_performance_metrics + 收集场景。 | ✓ |
| custom_type_mapping.rs — FromSqllog 演示 | 实现 FromSqllog trait 将记录映射到自定义类型。最全面展示 Phase 7 新 API。 | |
| parse_gb18030.rs — GB18030 编码文件 | 处理中文内容的日志文件，展示编码自动检测能力。 | |

**User's choice:** batch_export.rs — 批量导出
**Notes:** 用户选择最实用的场景（CSV 导出），与 filter_slow_queries.rs 形成互补。

---

## DOC-02: Quick Start 3 个场景

| Option | Description | Selected |
|--------|-------------|----------|
| 基础迭代 + 过滤慢查询 + 批量导出 | 三个场景分别展示：基础 API、filter_by_exec_time 过滤、收集并指定导出字段。覆盖最常用场景。 | ✓ |
| 基础迭代 + GB18030 + 并行处理 | 展示不同编码和 par_iter 线程并行特性。 | |
| You decide | Planner 根据 DOC-02 需求和示例覆盖面自行选择。 | |

**User's choice:** 基础迭代 + 过滤慢查询 + 批量导出
**Notes:** 用户希望 Quick Start 覆盖实际使用最频繁的场景。

---

## Claude's Discretion

- 是否引入 `#![deny(missing_docs)]` 编译时强制 — planner 根据现有文档覆盖率决定是否在此阶段引入
- examples/ 文件的命令行参数解析方式（`std::env::args()` vs `clap`）— 保持最简，用 `std::env::args()`

## Deferred Ideas

- 英文或双语文档 — 若未来面向国际发布可在新 milestone 切换
- 交互式文档（docs.rs playground）— 超出本阶段范围
