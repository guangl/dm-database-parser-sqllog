# Phase 7: APIErgonomics - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 07-APIErgonomics
**Areas discussed:** Builder & 过滤 API

---

## API-01: LogParserBuilder 与 from_path 共存方式

| Option | Description | Selected |
|--------|-------------|----------|
| Builder 作为额外入口 | from_path 保留不动（向下兼容），LogParserBuilder 是可选的新入口。简单情况不需要 Builder。 | |
| 完全替代 from_path | 移除 from_path，统一入口为 Builder。API 更统一，是 breaking change。 | ✓ |
| from_path 也变成 Builder | LogParser::from_path("...").threads(4).build() 形式。 | |

**User's choice:** 完全替代 from_path
**Notes:** 用户接受 breaking change（0.9.1 → 1.1.0，semver 允许）。API 统一性优先。

---

## API-02: 过滤方法位置

| Option | Description | Selected |
|--------|-------------|----------|
| LogIterator 上直接添加方法 | impl LogIterator 中新增 fn filter_by_exec_time(self, min_ms: u64) -> impl Iterator<...>。简单直接，无需新类型。 | ✓ |
| 独立 Trait SqllogFilter | 定义 trait SqllogFilter 并为 LogIterator impl。可由用户为任意迭代器实现，但方案复杂。 | |
| 返回新的 FilteredIter 类型 | 专门的适配器类型，类型安全，但公共 API 面暴露更多类型。 | |

**User's choice:** LogIterator 上直接添加方法
**Notes:** 最简单方案，无需新类型，直接在现有 impl 块扩展。

---

## API-03: exec_time() / row_count() 签名

| Option | Description | Selected |
|--------|-------------|----------|
| Option<u64> | fn exec_time(&self) -> Option<u64>。None 表示字段不存在或解析失败。调用最简洁。 | |
| Result<Option<u64>, ParseError> | 区分「字段不存在」和「解析失败」，更精确但调用繁琐。 | ✓ |
| u64（panic on failure） | 不适合库 crate。 | |

**User's choice:** Result<Option<u64>, ParseError>
**Notes:** 用户希望区分「无 indicators」和「解析出错」两种情况。接受调用稍繁琐。

---

## API-04: FromSqllog trait 设计

| Option | Description | Selected |
|--------|-------------|----------|
| 消费所有权（推荐） | trait FromSqllog { fn from_sqllog(s: Sqllog<'_>) -> Self; }。类似标准库 From trait。 | ✓ |
| 带生命周期的借用设计 | trait FromSqllog<'a> { fn from_sqllog(s: &'a Sqllog<'a>) -> Self; }。支持保持对 Sqllog 数据的悬挂引用，但生命周期命名复杂。 | |
| You decide | Planner 根据实际生命周期限制决定。 | |

**User's choice:** 消费所有权
**Notes:** 简单，与 From trait 风格一致。用户可在 from_sqllog 内部 clone 需要保留的数据。

---

## Claude's Discretion

- `parse_record` 是否从公开导出中移除 — 由 planner 根据向后兼容性决定
- 编码提示参数名称（`encoding_hint` vs `encoding`）— 由 planner 决定

## Deferred Ideas

- **API-05**（通用过滤适配器 `.filter_sqllog(closure)`）— Future requirements
- **API-06**（流式/增量解析）— Out of Scope，破坏零拷贝生命周期
