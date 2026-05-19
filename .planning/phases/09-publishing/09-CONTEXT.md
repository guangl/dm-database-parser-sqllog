# Phase 9: Publishing - Context

**Gathered:** 2026-05-19
**Status:** Ready for planning

<domain>
## Phase Boundary

将库准备为 crates.io 发布标准：新增 `CHANGELOG.md`（v1.1 变更），将 `Cargo.toml` 版本号更新至 `1.1.0`，全面重写 `README.md`（含安装、Quick Start、功能列表、性能数据），确保 `cargo publish --dry-run` 通过。

**依赖：** Phase 8（Documentation）— README 的 Quick Start 与 lib.rs 示例保持一致。

**不在本 Phase 范围内：** 实际执行 `cargo publish`（仅到 dry-run 为止）、crate 名称变更。

</domain>

<decisions>
## Implementation Decisions

### PUB-01: CHANGELOG.md

- **D-01:** 新建 `CHANGELOG.md`，遵循 [Keep a Changelog](https://keepachangelog.com/) 格式。
- **D-02:** 只记录 **v1.1 变更**，不追溯 v1.0 历史（v1.0 已在 git log 中可查）。
- **D-03:** v1.1.0 条目包含分类：`Added`（新 API：Builder、过滤方法、exec_time() 等、FromSqllog、examples）、`Changed`（ParseError 添加 line_number）、`Fixed`（如有）。

### PUB-02: 版本号

- **D-04:** 将 `Cargo.toml` 中 `version` 从 `0.9.1` 更新为 `1.1.0`。
- **D-05:** v1.0 milestone 的工作体现在 v1.1.0 中一并发布（不单独发 1.0.0）。这是第一个正式语义化版本发布。

### PUB-03: README.md

- **D-06:** **全面重写** README.md，内容结构：
  1. 标题 + 一句话描述（中文）
  2. 安装说明（`Cargo.toml` 片段，版本 `1.1.0`）
  3. Quick Start 代码示例（与 lib.rs 保持一致，3 个场景）
  4. 功能列表（零拷贝、内存映射、GB18030、par_iter、Builder API 等）
  5. v1.0 性能数据（8.67 GiB/s 单线程，基于 5 MB 合成语料库）
  6. API 概览（LogParserBuilder、Sqllog 方法、过滤、FromSqllog）
- **D-07:** README 语言与 rustdoc 保持一致，使用**中文**。

### Cargo.toml metadata 验证

- **D-08:** 验证以下字段均已正确填写：`description`、`keywords`、`categories`、`repository`、`documentation`。当前 `homepage` 字段有错别字（`dm-parser-sqllog` 而非 `dm-database-parser-sqllog`），需修正。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 需求规格
- `.planning/REQUIREMENTS.md` — PUB-01、PUB-02、PUB-03 完整需求定义
- `.planning/ROADMAP.md` — Phase 9 Success Criteria（3 条验收标准）

### 现有文件（必读）
- `Cargo.toml` — 当前 metadata 状态（version: 0.9.1，homepage 有错别字）
- `README.md` — 现有内容（全面重写，但确认现有结构）
- `.planning/phases/08-documentation/08-CONTEXT.md` — Quick Start 场景决策，README 应与之保持一致

### 格式规范
- [Keep a Changelog v1.1.1](https://keepachangelog.com/en/1.1.0/) — CHANGELOG.md 格式参考

### 性能数据来源
- `.planning/STATE.md` §Performance Metrics — v1.0 最终吞吐：8.67 GiB/s 单线程

</canonical_refs>

<code_context>
## Existing Code Insights

### 当前 Cargo.toml 状态
- `name`: `dm-database-parser-sqllog` — 保持不变
- `version`: `0.9.1` → 改为 `1.1.0`
- `description`: 中文描述已有，检查是否需要更新以反映 v1.1 新 API
- `keywords`: `["sqllog", "parser", "dm-database"]` — 考虑添加 `"dm"` 或 `"dameng"`
- `homepage`: `https://github.com/guangl/dm-parser-sqllog` — 错误，应为 `https://github.com/guangl/dm-database-parser-sqllog`
- `repository` / `documentation` 字段已有

### 验证命令
- `cargo publish --dry-run` — 最终验收门禁
- `cargo package --list` — 检查打包文件列表（`exclude` 字段是否正确）

</code_context>

<specifics>
## Specific Ideas

- README 中性能数据引用：`> 8.67 GiB/s 单线程（5 MB 合成语料库，含 20% 多行记录，Apple M 系列芯片）`
- CHANGELOG v1.1.0 发布日期在实际发布时填写，占位符 `YYYY-MM-DD`
- `cargo publish --dry-run` 成功后，实际发布由用户手动执行（不在自动化计划内）

</specifics>

<deferred>
## Deferred Ideas

- 实际执行 `cargo publish` — 用户手动操作，不纳入执行计划
- crate 名称简化（如 `dm-sqllog`）— 名称变更影响所有用户，暂不处理
- i18n README（英文版）— 若未来面向国际社区可在新 milestone 添加

</deferred>

---

*Phase: 09-Publishing*
*Context gathered: 2026-05-19*
