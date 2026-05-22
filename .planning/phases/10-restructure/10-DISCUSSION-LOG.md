# Phase 10: Restructure - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-22
**Phase:** 10-Restructure
**Areas discussed:** parser/ 内部结构, filter/ 骨架深度, async_api/ 骨架形态

---

## parser/ 内部结构

| Option | Description | Selected |
|--------|-------------|----------|
| 整体移为 mod.rs | 521 行全部进 src/parser/mod.rs，最小化改动 | |
| 拆分子文件 | 按职责分：mod.rs / builder.rs / iterator.rs | ✓ |

**User's choice:** 拆分子文件

---

### 子文件数量

| Option | Description | Selected |
|--------|-------------|----------|
| 3 个：mod.rs + builder.rs + iterator.rs | mod.rs 放 LogParser + parse_record；builder.rs 放 LogParserBuilder；iterator.rs 放 LogIterator | ✓ |
| 2 个：mod.rs + iterator.rs | mod.rs 放 LogParser + LogParserBuilder | |
| 你决定 | 由计划器根据职责单一原则判断 | |

**User's choice:** 3 个（+ 追加了 encoding.rs 存放 FileEncodingHint）

---

### parse_record() 可见性

| Option | Description | Selected |
|--------|-------------|----------|
| 保持 pub | 继续在 lib.rs 重导出，无 breaking change | |
| 降为 pub(crate) | 移除 lib.rs re-export，v2.0 breaking change | ✓ |

**User's choice:** 降为 pub(crate)
**Notes:** 发现 parse_record 在 4 个测试文件 ~40+ 处被直接调用，需迁移到 parser/ 内部 #[cfg(test)] 模块

---

### parse_record 测试迁移

| Option | Description | Selected |
|--------|-------------|----------|
| 迁入 parser/ 内部模块测试 | 在 src/parser/ 内加 #[cfg(test)] 模块，直接测试 pub(crate) 函数 | ✓ |
| 保持 pub，不作 breaking change | 保留 lib.rs re-export | |

**User's choice:** 迁入 parser/ 内部模块测试

---

### FileEncodingHint 归属

| Option | Description | Selected |
|--------|-------------|----------|
| parser/mod.rs | 和 LogParser 放在一起 | |
| parser/encoding.rs | 单独文件封装，可扩展 | ✓ |

**User's choice:** parser/encoding.rs

---

### 私有辅助函数归属

| Option | Description | Selected |
|--------|-------------|----------|
| 与使用它们的主类共居 | is_timestamp_start → iterator.rs；parse_record_with_hint → mod.rs | ✓ |
| parser/utils.rs 或 helpers.rs | 集中一处，但产生跨文件依赖 | |

**User's choice:** 就近原则，与使用者共居

---

### par_iter() 归属

| Option | Description | Selected |
|--------|-------------|----------|
| 了解归属 | — | |

**Notes:** par_iter() 在当前代码库中不存在（src/ 和 tests/ 均无此符号），不适用于 Phase 10

---

## filter/ 骨架深度

| Option | Description | Selected |
|--------|-------------|----------|
| 仅空 mod.rs | Phase 10 只创建占位，现有过滤方法保留在 parser/ | |
| 现有过滤方法迁入 filter/ | filter_by_exec_time / filter_by_sql_contains 迁入 | ✓ |
| FilterBuilder 类型桩 | 创建 pub struct FilterBuilder 定义但不实现方法 | |

**User's choice:** 现有过滤方法迁入 filter/

---

### 迁入后 API 形态

| Option | Description | Selected |
|--------|-------------|----------|
| 保持迭代器适配器形式 | 方法保持不变，仅移动到 filter/ | |
| 适配新 FilterBuilder 调用形式 | filter_by_exec_time → FilterBuilder::exec_time_gt 等 | ✓ |

**User's choice:** 适配新 FilterBuilder 形式

---

### Phase 10 与 Phase 11 边界

| Option | Description | Selected |
|--------|-------------|----------|
| Phase 10：2 个现有过滤 → FilterBuilder 框架；Phase 11：剩余 12 字段 | 提前验证 API 形状 | |
| Phase 10：仅移动代码；Phase 11：所有 FilterBuilder 实现 | 纯重构，不引入新公开 API | ✓ |

**User's choice:** Phase 10 仅移动代码，Phase 11 实现所有 FilterBuilder

---

### 过滤方法可见性

| Option | Description | Selected |
|--------|-------------|----------|
| 保持公开 | 无 breaking change | |
| 降为 pub(crate)，Phase 11 再公开 | FilterBuilder 才是最终公开 API | ✓ |

**User's choice:** 降为 pub(crate)

---

### filter/ 内部文件结构

| Option | Description | Selected |
|--------|-------------|----------|
| 仅 mod.rs | 2 个方法不需要拆分 | |
| mod.rs + adapter.rs | 迭代器适配器逻辑独立文件 | ✓ |

**User's choice:** mod.rs + adapter.rs

---

### filter/adapter.rs 函数类型签名

| Option | Description | Selected |
|--------|-------------|----------|
| 泛型迭代器 | `fn filter_by_exec_time<I: Iterator<...>>(iter: I, ...)` | ✓ |
| 直接传入 LogIterator | `fn filter_by_exec_time<'a>(iter: LogIterator<'a>, ...)` | |
| 保持方法形式（不移动） | 留在 parser/iterator.rs | |

**User's choice:** 泛型迭代器签名

---

### filter/ 与 LogIterator 集成

| Option | Description | Selected |
|--------|-------------|----------|
| filter/ 定义 trait，LogIterator impl | 模块边界清晰 | |
| filter/ 定义函数，parser/ 直接调用 | 简单直接 | ✓ |

**User's choice:** filter/ 定义函数，parser/ 直接调用

---

## async_api/ 骨架形态

| Option | Description | Selected |
|--------|-------------|----------|
| 纯空 mod.rs | 只有空模块声明 | ✓ |
| parse_file_async 签名框架 | 包含 unimplemented!() 的函数桩 | |
| TODO 注释骨架 | 空模块 + 说明注释 | |

**User's choice:** 纯空 mod.rs

---

### async_api 在 lib.rs 中的暴露

| Option | Description | Selected |
|--------|-------------|----------|
| 不暴露 | Phase 10 不添加到 lib.rs，Phase 12 再加 | ✓ |
| 立即添加 #[cfg(feature = "async")] 占位 | 提前在 lib.rs 占位 | |

**User's choice:** 不暴露

---

## Claude's Discretion

- `parser/mod.rs` 命名（vs `parser/core.rs`）：使用标准 `mod.rs`
- 拆分后各子文件的具体代码行数分配：由实际代码决定

## Deferred Ideas

- par_iter() / Rayon 并行迭代器：当前不存在于代码库，Phase 10 不涉及
- 完整 FilterBuilder 公开 API：Phase 11
- tokio feature flag 和 async API：Phase 12
