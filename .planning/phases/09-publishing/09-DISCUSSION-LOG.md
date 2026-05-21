# Phase 9: Publishing - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 09-Publishing
**Areas discussed:** 版本号策略

---

## PUB-02: 版本号

| Option | Description | Selected |
|--------|-------------|----------|
| 1.0.0 | v1.0 milestone 已绘制为第一个正式发布。v1.1 milestone 完成后发 1.0.0，语义明确。 | |
| 1.1.0 | v1.0 先发 1.0.0，v1.1 发 1.1.0。当前 milestone 内先升 1.0.0 再升 1.1.0，或直接跳到 1.1.0。 | ✓ |
| 0.9.1 保持不动 | 这个 milestone 不暴露版本号，看后续情况再定。 | |

**User's choice:** 1.1.0
**Notes:** 用户选择直接跳到 1.1.0（从 0.9.1），将 v1.0 和 v1.1 milestone 的成果一并在 1.1.0 发布。

---

## PUB-01: CHANGELOG.md 记录范围

| Option | Description | Selected |
|--------|-------------|----------|
| 只包含 v1.1 变更 | v1.0 milestone 的 git 历史已在仓库，CHANGELOG.md 从 v1.1 开始记录。维护工作量小。 | ✓ |
| v1.0 + v1.1 全部 | 根据 git log 整理 v1.0 变更并写入 CHANGELOG.md，然后添加 v1.1 内容。历史更完整。 | |
| You decide | Planner 根据 Keep a Changelog 规范定义内容。 | |

**User's choice:** 只包含 v1.1 变更
**Notes:** 用户认为 git 历史已足够，CHANGELOG 只需从首次正式发布开始记录。

---

## PUB-03: README.md 处理方式

| Option | Description | Selected |
|--------|-------------|----------|
| 全面重写 | 按 PUB-03 需求重写：安装说明 + Quick Start + 功能列表 + v1.0 性能数据 + API 概览。现有 README 内容不确定是否符合要求。 | ✓ |
| 在现有 README 基础上补充 | 保留已有内容，只添加缺少的部分（如 Cargo.toml 安装片段、性能数据）。 | |

**User's choice:** 全面重写
**Notes:** 用户倾向干净的全面重写，确保结构和内容完全符合发布要求。

---

## Claude's Discretion

- CHANGELOG.md 的 v1.1.0 发布日期 — 占位符 `YYYY-MM-DD`，实际发布时填写
- `description` 字段是否需要更新为英文（面向 crates.io 搜索）— 与文档语言决策保持一致（中文）

## Deferred Ideas

- 实际执行 `cargo publish` — 用户手动操作，不纳入执行计划
- crate 名称简化（如 `dm-sqllog`）— 影响所有用户，暂不处理
- 英文 README — 若未来面向国际社区在新 milestone 添加
