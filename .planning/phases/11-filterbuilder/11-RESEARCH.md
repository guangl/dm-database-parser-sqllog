# Phase 11: FilterBuilder - Research

**Researched:** 2026-05-22
**Domain:** Rust 迭代器适配器 / Builder 模式 / 组合谓词过滤
**Confidence:** HIGH

---

## Executive Summary

`Sqllog` 结构体所有字段均为 `String` / `Option<String>` / 基础数值类型（owned），
**没有生命周期参数**。这一关键事实消除了需求描述中提到的"零拷贝 Cow 生命周期挑战"——
闭包不需要捕获任何带生命周期的引用，`FilterBuilder::build()` 可以直接返回
`Box<dyn Fn(&Sqllog) -> bool + Send + Sync>`，无任何生命周期标注难题。

推荐方案 D（结构体持有 `Vec<Box<dyn Fn(&Sqllog) -> bool + Send + Sync>>`）：
API 最简洁，AND 语义天然满足，单次迭代无中间分配，与 `Iterator::filter` 无缝集成，
同时支持 Phase 12 async 在 `spawn_blocking` 内传递 `Filter`（Send + Sync）。

**Primary recommendation:** 方案 D — `FilterBuilder` 持有动态谓词列表，`build()` 返回 `Filter` 结构体，
`LogIterator` 新增 `apply_filter(filter: &Filter)` 方法，`Filter` 公开导出以支持 Phase 12。

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FILTER-01 | ts（时间戳字符串）supports contains / eq / starts_with / ends_with | ts 字段为 String，直接调用对应 str 方法，无需 parse_on_demand |
| FILTER-02 | tag（Option\<String\>）存在性检查和值匹配 | tag 为 Option\<String\>，is_some() + 值匹配均在 String 上操作 |
| FILTER-03 | ep（u8）eq / gt / lt / between | ep 为直接 u8，数值比较最廉价，宜放谓词链最前 |
| FILTER-04 | sess_id / thrd_id / username / trxid / statement / appname / client_ip 四种字符串谓词 | 均为 String，同 FILTER-01 |
| FILTER-05 | sql contains / eq / starts_with / ends_with | sql 为 String，同上 |
| FILTER-06 | exectime（f32）gt / lt / between | 直接字段访问，无 parse-on-demand 成本 |
| FILTER-07 | rowcount（u32）eq / gt / lt / between | 直接字段访问 |
| FILTER-08 | exec_id（i64）eq / gt / lt / between | 直接字段访问 |
| FILTER-09 | 多条件链式 AND，单次迭代，无中间 Vec 分配 | Iterator::filter 本身是惰性的；Vec\<Box\<dyn Fn\>\> 遍历即 AND |
| FILTER-10 | 与 LogParser::iter() 无缝集成 | LogIterator::apply_filter 委托给 filter::adapter，与现有 filter_by_exec_time 风格一致 |
</phase_requirements>

---

## 关键发现：Sqllog 无生命周期

读取 `src/record.rs` 后确认：

```rust
pub struct Sqllog {
    pub ts: String,           // owned
    pub tag: Option<String>,  // owned
    pub ep: u8,
    pub sess_id: String,      // owned
    pub thrd_id: String,
    pub username: String,
    pub trxid: String,
    pub statement: String,
    pub appname: String,
    pub client_ip: String,
    pub sql: String,          // owned
    pub exectime: f32,
    pub rowcount: u32,
    pub exec_id: i64,
}
```

需求描述中提到的"零拷贝 Cow 生命周期"是 **[ASSUMED]** 的旧版设计描述，
当前代码库中 `Sqllog` 已经是 fully-owned 结构体。[VERIFIED: src/record.rs 直接读取]

`LogIterator<'a>` 的 `'a` 生命周期仅描述"迭代器持有 `LogParser` 数据的引用"，
与 `Sqllog` 本身无关——每条 `next()` 返回 `Result<Sqllog, ParseError>`，Sqllog 已是 owned。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| FilterBuilder 链式 API | `src/filter/builder.rs` | — | 构建谓词列表，不依赖 parser 层 |
| Filter 谓词执行 | `src/filter/adapter.rs` | — | 扩展现有 adapter，保持单向依赖 |
| LogIterator 集成方法 | `src/parser/iterator.rs` | filter::adapter | 委托调用，与现有 filter_by_* 风格一致 |
| Filter 公开类型导出 | `src/lib.rs` | — | Phase 12 async 需要接收 Filter 参数 |
| 字段级谓词 pred_* 函数 | `src/filter/builder.rs` 或内联闭包 | — | 紧凑实现，不需要独立模块 |

---

## Architecture Decision

### 选定方案：方案 D（动态谓词 Vec + Filter 结构体）

**设计理由：**

1. API 最简洁，用户不需要了解类型状态机（方案 C 的编译器错误提示难以理解）
2. 谓词数量通常 < 10，`Vec<Box<dyn Fn>>` 的间接调用开销在 4M records/sec 目标下可接受
   （每次调用约增加 1-2ns，总开销 < 5%；若成为瓶颈，方案升级路径是 enum 谓词）
3. `Filter` 结构体可 `+ Send + Sync`，直接满足 Phase 12 async `spawn_blocking` 传递需求
4. 现有 `filter_by_exec_time` / `filter_by_sql_contains` 可以保留（向后兼容），
   同时新增 `apply_filter` 作为 FilterBuilder 的集成点

**不选方案 B（monomorphic 字段位图）的原因：**
- 每字段都需要 Option 存储谓词值（14 字段 × 4 谓词类型 = 最多 56 个 Option），结构体臃肿
- 不支持同一字段多个谓词（e.g., `ts_contains("2024").ts_starts_with("2024-06")`）
- 可组合性差

**不选方案 C（typestate）的原因：**
- 泛型爆炸：每个链式方法都产生新类型，30+ 方法的组合会让编译变慢
- 类型复杂度对 IDE 提示和错误信息不友好

### 代码骨架

```rust
// src/filter/builder.rs

use crate::record::Sqllog;

/// 组合过滤器，持有所有谓词的 AND 组合。
///
/// 通过 [`FilterBuilder`] 构建。
pub struct Filter {
    predicates: Vec<Box<dyn Fn(&Sqllog) -> bool + Send + Sync>>,
}

impl Filter {
    /// 对给定记录运行所有谓词，全部通过则返回 true（AND 语义）。
    #[inline]
    pub fn matches(&self, record: &Sqllog) -> bool {
        self.predicates.iter().all(|pred| pred(record))
    }
}

/// 链式构建组合过滤器。
///
/// 所有条件以 AND 语义组合，调用 [`build()`](FilterBuilder::build) 生成 [`Filter`]。
///
/// # 示例
/// ```rust
/// use dm_database_parser_sqllog::FilterBuilder;
///
/// let filter = FilterBuilder::new()
///     .ts_contains("2024-06")
///     .exec_time_gt(100.0)
///     .sql_contains("SELECT")
///     .build();
/// ```
pub struct FilterBuilder {
    predicates: Vec<Box<dyn Fn(&Sqllog) -> bool + Send + Sync>>,
}

impl FilterBuilder {
    pub fn new() -> Self {
        Self { predicates: Vec::new() }
    }

    fn add<F>(mut self, pred: F) -> Self
    where
        F: Fn(&Sqllog) -> bool + Send + Sync + 'static,
    {
        self.predicates.push(Box::new(pred));
        self
    }

    pub fn build(self) -> Filter {
        Filter { predicates: self.predicates }
    }

    // ── FILTER-01: ts ──
    pub fn ts_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.ts.contains(&pattern))
    }
    pub fn ts_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.ts == value)
    }
    pub fn ts_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.ts.starts_with(&prefix))
    }
    pub fn ts_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.ts.ends_with(&suffix))
    }

    // ── FILTER-02: tag ──
    pub fn tag_is_some(self) -> Self {
        self.add(|r| r.tag.is_some())
    }
    pub fn tag_is_none(self) -> Self {
        self.add(|r| r.tag.is_none())
    }
    pub fn tag_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.tag.as_deref() == Some(&value))
    }
    pub fn tag_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.tag.as_deref().is_some_and(|t| t.contains(&pattern)))
    }

    // ── FILTER-03: ep ──
    pub fn ep_eq(self, value: u8) -> Self {
        self.add(move |r| r.ep == value)
    }
    pub fn ep_gt(self, value: u8) -> Self {
        self.add(move |r| r.ep > value)
    }
    pub fn ep_lt(self, value: u8) -> Self {
        self.add(move |r| r.ep < value)
    }
    pub fn ep_between(self, min: u8, max: u8) -> Self {
        self.add(move |r| r.ep >= min && r.ep <= max)
    }

    // ── FILTER-04: 七个字符串元数据字段（sess_id / thrd_id / username / trxid / statement / appname / client_ip）
    // 每个字段 4 个方法，用宏生成以避免重复

    // sess_id
    pub fn sess_id_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.sess_id.contains(&pattern))
    }
    pub fn sess_id_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.sess_id == value)
    }
    pub fn sess_id_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.sess_id.starts_with(&prefix))
    }
    pub fn sess_id_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.sess_id.ends_with(&suffix))
    }
    // ... username / thrd_id / trxid / statement / appname / client_ip 同上（宏展开）

    // ── FILTER-05: sql ──
    pub fn sql_contains(self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.add(move |r| r.sql.contains(&pattern))
    }
    pub fn sql_eq(self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.add(move |r| r.sql == value)
    }
    pub fn sql_starts_with(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.add(move |r| r.sql.starts_with(&prefix))
    }
    pub fn sql_ends_with(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.add(move |r| r.sql.ends_with(&suffix))
    }

    // ── FILTER-06: exectime ──
    pub fn exec_time_gt(self, min_ms: f32) -> Self {
        self.add(move |r| r.exectime > min_ms)
    }
    pub fn exec_time_lt(self, max_ms: f32) -> Self {
        self.add(move |r| r.exectime < max_ms)
    }
    pub fn exec_time_between(self, min_ms: f32, max_ms: f32) -> Self {
        self.add(move |r| r.exectime >= min_ms && r.exectime <= max_ms)
    }

    // ── FILTER-07: rowcount ──
    pub fn rowcount_eq(self, value: u32) -> Self {
        self.add(move |r| r.rowcount == value)
    }
    pub fn rowcount_gt(self, value: u32) -> Self {
        self.add(move |r| r.rowcount > value)
    }
    pub fn rowcount_lt(self, value: u32) -> Self {
        self.add(move |r| r.rowcount < value)
    }
    pub fn rowcount_between(self, min: u32, max: u32) -> Self {
        self.add(move |r| r.rowcount >= min && r.rowcount <= max)
    }

    // ── FILTER-08: exec_id ──
    pub fn exec_id_eq(self, value: i64) -> Self {
        self.add(move |r| r.exec_id == value)
    }
    pub fn exec_id_gt(self, value: i64) -> Self {
        self.add(move |r| r.exec_id > value)
    }
    pub fn exec_id_lt(self, value: i64) -> Self {
        self.add(move |r| r.exec_id < value)
    }
    pub fn exec_id_between(self, min: i64, max: i64) -> Self {
        self.add(move |r| r.exec_id >= min && r.exec_id <= max)
    }
}

impl Default for FilterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## Integration Design

### 集成方式选择

选择**方案 A + 方案 C 的混合**：

- `LogIterator` 新增 `apply_filter(filter: Filter)` 方法（委托给 `filter::adapter`）
- `Filter` 实现 `Fn(&Sqllog) -> bool` 语义（通过 `matches` 方法），adapter 函数接受泛型谓词

**拒绝方案 D（专门的 FilteredIter<'a>）：**
`Iterator::filter` 是标准库设施，直接使用即可，不需要自定义包装类型。

### adapter.rs 扩展

```rust
// src/filter/adapter.rs 新增

use crate::filter::builder::Filter;

/// 使用 Filter 过滤迭代器，错误记录被丢弃（与 filter_by_exec_time 行为一致）。
pub(crate) fn apply_filter<I>(
    iter: I,
    filter: Filter,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    iter.filter(move |item| match item {
        Ok(sqllog) => filter.matches(sqllog),
        Err(_) => false,
    })
}

/// 与 apply_filter 相同，但保留错误记录。
pub(crate) fn apply_filter_keep_errors<I>(
    iter: I,
    filter: Filter,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    iter.filter(move |item| match item {
        Ok(sqllog) => filter.matches(sqllog),
        Err(_) => true,   // 错误透传
    })
}
```

### iterator.rs 集成

```rust
// src/parser/iterator.rs 新增方法

use crate::filter::builder::Filter;

impl<'a> LogIterator<'a> {
    // 现有方法保留（向后兼容）
    pub fn filter_by_exec_time(self, min_ms: u64) -> impl Iterator<...> { ... }
    pub fn filter_by_sql_contains(self, pattern: &'a str) -> impl Iterator<...> { ... }

    // 新增：FILTER-10
    /// 应用 FilterBuilder 产出的组合过滤器，错误记录被丢弃。
    pub fn apply_filter(
        self,
        filter: Filter,
    ) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
        adapter::apply_filter(self, filter)
    }
}
```

### lib.rs 导出

```rust
// src/lib.rs 新增导出
pub use filter::builder::{Filter, FilterBuilder};
```

---

## API Surface（用户侧调用示例）

对应 Success Criteria 的完整 API 草图：

```rust
use dm_database_parser_sqllog::{FilterBuilder, LogParserBuilder};

// SC-1: 链式构建组合过滤器（所有 14 字段均有对应方法）
let filter = FilterBuilder::new()
    .ts_contains("2024-06")          // FILTER-01
    .tag_is_some()                   // FILTER-02
    .ep_eq(0)                        // FILTER-03
    .username_eq("alice")            // FILTER-04
    .sql_contains("SELECT")          // FILTER-05
    .exec_time_gt(1.0)               // FILTER-06
    .rowcount_gt(10)                 // FILTER-07
    .exec_id_between(1000, 9999)     // FILTER-08
    .build();

// SC-3: 字符串字段四种谓词
let filter2 = FilterBuilder::new()
    .ts_starts_with("2024")
    .ts_ends_with(".000")
    .sql_starts_with("SELECT")
    .sql_ends_with(";")
    .build();

// SC-4: AND 语义，单次迭代（FILTER-09）
// SC-5: 与 LogParser::iter() 无缝集成（FILTER-10）
let parser = LogParserBuilder::new("sqllog.txt").build()?;
let results: Vec<_> = parser
    .iter()
    .apply_filter(filter)     // 单次迭代，无中间 Vec
    .collect();

// 空过滤器（无条件）匹配所有记录
let all = FilterBuilder::new().build();
assert!(all.matches(&record));
```

---

## Implementation Map

| FILTER-* | 实现位置 | 关键类型/方法 |
|----------|---------|-------------|
| FILTER-01 | `src/filter/builder.rs` | `FilterBuilder::ts_*` × 4 |
| FILTER-02 | `src/filter/builder.rs` | `FilterBuilder::tag_is_some / tag_is_none / tag_eq / tag_contains` |
| FILTER-03 | `src/filter/builder.rs` | `FilterBuilder::ep_*` × 4 |
| FILTER-04 | `src/filter/builder.rs` | 7 字段 × 4 方法 = 28 个方法（宏展开辅助） |
| FILTER-05 | `src/filter/builder.rs` | `FilterBuilder::sql_*` × 4 |
| FILTER-06 | `src/filter/builder.rs` | `FilterBuilder::exec_time_gt / lt / between`（无 eq，f32 浮点比较无意义） |
| FILTER-07 | `src/filter/builder.rs` | `FilterBuilder::rowcount_*` × 4 |
| FILTER-08 | `src/filter/builder.rs` | `FilterBuilder::exec_id_*` × 4 |
| FILTER-09 | `src/filter/adapter.rs` | `apply_filter` — `Iterator::filter` 天然满足 AND + 惰性 |
| FILTER-10 | `src/parser/iterator.rs` | `LogIterator::apply_filter` 委托 adapter |

**新增文件：**
- `src/filter/builder.rs` — FilterBuilder + Filter 全部实现
- `src/filter/mod.rs` 修改：新增 `pub mod builder;`

**修改文件：**
- `src/filter/adapter.rs` — 新增 `apply_filter` / `apply_filter_keep_errors`
- `src/parser/iterator.rs` — 新增 `apply_filter` 方法
- `src/lib.rs` — 新增 `pub use filter::builder::{Filter, FilterBuilder};`

---

## 字段访问成本分析（短路求值顺序建议）

所有 `Sqllog` 字段在解析时**一次性填充**（parse_record_with_hint 是 eager），
没有 lazy/on-demand 字段。因此不存在"访问成本"差异——所有字段都已在内存中。

短路求值（`Vec::iter().all()`）的意义在于**提前返回 false**，因此建议用户
将**选择性最高（最多记录被过滤）的谓词放在链式调用的最前面**。

谓词自然成本排序（CPU 指令数角度，非访问成本）：

| 排名 | 字段类型 | 成本 | 理由 |
|------|---------|------|------|
| 1（最低）| ep、rowcount、exec_id 等数值 | 1-2 指令 | 直接比较 |
| 2 | ts（固定 23 字符） | 比较可 SIMD | 短固定长度字符串 |
| 3 | sess_id / thrd_id 等短字符串 | contains/eq | 通常短 |
| 4（最高）| sql | contains 可能很慢 | SQL 可能很长（多行） |

---

## 方法命名约定

为保持与现有 API 风格一致：

| 字段 | 前缀 | 示例 |
|------|------|------|
| ts | `ts_` | `ts_contains`, `ts_eq` |
| tag | `tag_` | `tag_is_some`, `tag_eq` |
| ep | `ep_` | `ep_eq`, `ep_gt` |
| sess_id | `sess_id_` | `sess_id_eq` |
| thrd_id | `thrd_id_` | `thrd_id_contains` |
| username | `username_` | `username_eq` |
| trxid | `trxid_` | `trxid_eq` |
| statement | `statement_` | `statement_eq` |
| appname | `appname_` | `appname_contains` |
| client_ip | `client_ip_` | `client_ip_starts_with` |
| sql | `sql_` | `sql_contains`, `sql_eq` |
| exectime | `exec_time_` | `exec_time_gt`（注意：不是 `exectime_`） |
| rowcount | `rowcount_` | `rowcount_gt` |
| exec_id | `exec_id_` | `exec_id_between` |

`exec_time_` 前缀（而非 `exectime_`）与现有 `filter_by_exec_time` 保持语义一致性。

**exectime 不提供 eq 方法**：f32 浮点相等比较在实践中几乎无意义，省略避免用户误用。
可以用 `exec_time_between(x - 0.001, x + 0.001)` 替代。

---

## 宏辅助方案（减少重复代码）

7 个字符串元数据字段（FILTER-04）各有 4 个方法，共 28 个方法。
建议使用 `macro_rules!` 在 `builder.rs` 内部减少重复：

```rust
// 仅在 builder.rs 内部使用，不对外暴露
macro_rules! impl_str_filter {
    ($field:ident, $prefix:ident) => {
        paste::paste! {
            pub fn [<$prefix _contains>](self, pattern: impl Into<String>) -> Self {
                let pattern = pattern.into();
                self.add(move |r| r.$field.contains(&pattern))
            }
            pub fn [<$prefix _eq>](self, value: impl Into<String>) -> Self {
                let value = value.into();
                self.add(move |r| r.$field == value)
            }
            pub fn [<$prefix _starts_with>](self, prefix: impl Into<String>) -> Self {
                let prefix = prefix.into();
                self.add(move |r| r.$field.starts_with(&prefix))
            }
            pub fn [<$prefix _ends_with>](self, suffix: impl Into<String>) -> Self {
                let suffix = suffix.into();
                self.add(move |r| r.$field.ends_with(&suffix))
            }
        }
    };
}
```

**注意**：`paste` crate 需要加入 `[dev-dependencies]` 或 `[dependencies]`。
如果不想引入新依赖，可以手写展开（28 个方法，每个约 5 行，共 140 行，可接受）。
**推荐手写展开**，避免 `paste` 依赖增加编译时间和外部依赖。

---

## Pitfalls（已知陷阱与缓解措施）

### Pitfall 1：循环依赖风险

**现象：** `src/filter/builder.rs` 导入 `crate::record::Sqllog`；
如果 `filter` 模块同时被 `parser::iterator` 引用，而 `parser` 又引用 `filter`，
需要确认不产生循环。

**现状分析：**

```
lib.rs
├── parser (引用 filter::adapter + record)
├── filter (引用 record 只)
└── record (无外部依赖)
```

`filter::builder` 只引用 `crate::record::Sqllog`，不引用 `parser` 任何类型。
`parser::iterator` 引用 `crate::filter::adapter`（已存在，无问题）+ 新增 `crate::filter::builder::Filter`。
**不产生循环。**

**缓解：** `filter::builder` 禁止引用任何 `crate::parser::*` 类型。

### Pitfall 2：filter_by_exec_time 行为不一致

**现象：** 现有 `filter_by_exec_time(100u64)` 接受 `u64`（毫秒），
新 FilterBuilder 的 `exec_time_gt(f32)` 接受 `f32`（毫秒，与 `exectime` 字段类型一致）。
用户可能混淆。

**缓解：** 
- 保留现有方法不修改（向后兼容）
- 新 `exec_time_gt(f32)` rustdoc 明确注明"参数单位为毫秒，类型为 f32"
- 不废弃现有方法（Phase 11 不做 deprecation）

### Pitfall 3：apply_filter 错误记录处理语义

**现象：** 现有 `filter_by_exec_time` 遇到 `Err(_)` 返回 `false`（丢弃错误记录）。
`apply_filter` 应保持相同行为（丢弃）还是提供两种版本？

**决策：**
- `apply_filter(filter)` — 与现有方法一致，丢弃错误记录（Err 被过滤掉）
- `apply_filter_keep_errors(filter)` — 透传错误记录（供需要完整错误处理的用户）
- 两者均加入 adapter.rs，LogIterator 暴露两个方法

### Pitfall 4：tag_is_some / tag_is_none 方向

**现象：** `tag` 为 `Option<String>`，用户可能想"仅处理有 tag 的记录"或"仅处理无 tag 的记录"。

**决策：** 同时提供 `tag_is_some()` 和 `tag_is_none()`，避免用户用错方向导致 bug。

### Pitfall 5：f32 浮点比较精度

**现象：** `exec_time_gt(100.0)` 在某些平台上因 f32 精度可能出现边界误差。

**缓解：**
- 不提供 `exec_time_eq`（f32 eq 无意义）
- between 使用闭区间 `>=` / `<=`，符合用户直觉
- rustdoc 注明"f32 精度约 7 位有效数字"

### Pitfall 6：宏展开导致 rustdoc 方法文档缺失

**现象：** 若使用 `paste` + 宏展开，`cargo doc` 可能不会为每个方法生成单独文档。

**缓解：** 手写展开所有 28 个方法，每个方法有独立 rustdoc 注释。
代码量约 140 行，在 CLAUDE.md 的"函数 < 40 行"约束内（每个函数约 5 行）。

### Pitfall 7：FilterBuilder 不实现 Clone 的影响

**现象：** `FilterBuilder` 持有 `Box<dyn Fn + Send + Sync>`，无法 derive `Clone`。
用户无法"复用"一个 builder 产出多个 Filter。

**决策：** 不实现 Clone（`Box<dyn Fn>` 无法 clone）。
用户需要多个 Filter 时，重新调用 `FilterBuilder::new()` 链式构建。
这是正常 builder 模式的限制，不需要解决。

---

## Test Strategy

### 单元测试（`src/filter/builder.rs` 内 `#[cfg(test)]` 块）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Sqllog;

    fn make_record() -> Sqllog {
        Sqllog {
            ts: "2024-06-01 10:00:00.000".to_string(),
            tag: Some("SEL".to_string()),
            ep: 2,
            sess_id: "0xABC".to_string(),
            thrd_id: "123".to_string(),
            username: "alice".to_string(),
            trxid: "0".to_string(),
            statement: "0x1".to_string(),
            appname: "myapp".to_string(),
            client_ip: "10.0.0.1".to_string(),
            sql: "SELECT * FROM users".to_string(),
            exectime: 150.0,
            rowcount: 10,
            exec_id: 999,
        }
    }

    // 测试每个字段的每种谓词
    #[test] fn test_ts_contains() { ... }
    #[test] fn test_ts_eq() { ... }
    #[test] fn test_tag_is_some() { ... }
    #[test] fn test_tag_is_none() { ... }
    #[test] fn test_ep_between() { ... }
    #[test] fn test_exec_time_gt() { ... }
    // ...

    // FILTER-09: AND 语义
    #[test]
    fn test_multiple_predicates_all_must_pass() {
        let record = make_record();
        let filter = FilterBuilder::new()
            .ts_contains("2024")
            .exec_time_gt(100.0)
            .sql_contains("SELECT")
            .build();
        assert!(filter.matches(&record));

        // 其中一个不满足，整体 false
        let strict = FilterBuilder::new()
            .ts_contains("2024")
            .exec_time_gt(200.0)  // 150.0 不满足
            .build();
        assert!(!strict.matches(&record));
    }

    // 空过滤器匹配所有
    #[test]
    fn test_empty_filter_matches_all() {
        let filter = FilterBuilder::new().build();
        assert!(filter.matches(&make_record()));
    }
}
```

### 集成测试（`tests/filter_builder.rs` 新增）

使用 `tempfile` + 真实 LogParser 的端到端测试：

```rust
// tests/filter_builder.rs
use dm_database_parser_sqllog::{FilterBuilder, LogParserBuilder};

#[test]
#[cfg(not(miri))]
fn test_apply_filter_single_iteration() { ... }

#[test]
#[cfg(not(miri))]
fn test_apply_filter_no_match() { ... }

#[test]
#[cfg(not(miri))]
fn test_apply_filter_multiple_conditions_and_semantics() { ... }

#[test]
#[cfg(not(miri))]
fn test_apply_filter_keep_errors() { ... }
```

### 覆盖率目标

现有覆盖率 94.93%（line），新增 `src/filter/builder.rs` 全部路径需测到。
建议覆盖：
- 每个字段至少 1 个谓词的 true / false 两种情况
- 空过滤器
- 多谓词 AND（至少 3 条件）
- tag_is_some / tag_is_none 两种方向

---

## Standard Stack

### Core（无新依赖）

| 组件 | 来源 | Purpose |
|------|------|---------|
| `Box<dyn Fn(&Sqllog) -> bool + Send + Sync>` | std | 动态谓词存储 |
| `Vec::iter().all()` | std | AND 短路求值 |
| `Iterator::filter` | std | 惰性过滤，单次迭代 |
| `impl Into<String>` | std | 方法参数人机体验 |

**不引入新 crate 依赖**。现有 `Cargo.toml` 已满足所有需求。

---

## Environment Availability

Step 2.6: 无外部依赖（纯 Rust 代码变更），SKIPPED。

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test + criterion（bench） |
| Config file | 无独立配置文件（cargo test） |
| Quick run command | `cargo test filter` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FILTER-01 | ts 字段四种谓词 | unit | `cargo test filter::builder::tests` | ❌ Wave 0 创建 |
| FILTER-02 | tag 存在性和值匹配 | unit | `cargo test filter::builder::tests::test_tag` | ❌ Wave 0 创建 |
| FILTER-03 | ep 数值谓词 | unit | `cargo test filter::builder::tests::test_ep` | ❌ Wave 0 创建 |
| FILTER-04 | 7 个字符串字段 | unit | `cargo test filter::builder::tests::test_sess_id` etc | ❌ Wave 0 创建 |
| FILTER-05 | sql 字段 | unit | `cargo test filter::builder::tests::test_sql` | ❌ Wave 0 创建 |
| FILTER-06 | exectime 浮点谓词 | unit | `cargo test filter::builder::tests::test_exec_time` | ❌ Wave 0 创建 |
| FILTER-07 | rowcount 谓词 | unit | `cargo test filter::builder::tests::test_rowcount` | ❌ Wave 0 创建 |
| FILTER-08 | exec_id 谓词 | unit | `cargo test filter::builder::tests::test_exec_id` | ❌ Wave 0 创建 |
| FILTER-09 | 多条件 AND，无中间分配 | unit + integration | `cargo test test_multiple_predicates` | ❌ Wave 0 创建 |
| FILTER-10 | LogIterator::apply_filter | integration | `cargo test --test filter_builder` | ❌ Wave 0 创建 |

### Wave 0 Gaps

- [ ] `src/filter/builder.rs` — FilterBuilder + Filter + `#[cfg(test)]` 单元测试块
- [ ] `tests/filter_builder.rs` — 集成测试（apply_filter 端到端）

---

## Security Domain

FilterBuilder 不涉及认证、会话、加密或外部网络调用，无 ASVS 相关安全需求。

V5 Input Validation：用户输入的过滤字符串（pattern）作为 `str::contains` 等的参数，
不作为正则表达式解析，不存在 ReDoS 风险。SKIPPED。

---

## Open Questions

1. **`apply_filter` 对错误记录的默认行为**
   - What we know: 现有 `filter_by_exec_time` / `filter_by_sql_contains` 丢弃错误记录
   - What's unclear: 用户是否需要在多条件过滤时仍看到错误记录？
   - Recommendation: 默认丢弃（与现有一致），额外提供 `apply_filter_keep_errors` 变体；
     planner 可选择只实现一个减少 API 表面

2. **exectime eq 是否完全不提供？**
   - What we know: f32 浮点比较存在精度问题
   - What's unclear: 是否有真实用例（如查找 exectime == 0.0 的记录）
   - Recommendation: 省略，用 between(0.0, 0.001) 可以替代

3. **tag_starts_with / tag_ends_with 是否需要？**
   - What we know: FILTER-02 只要求"存在性检查和值匹配"，没有提字符串谓词变体
   - What's unclear: 是否遗漏了 tag 的 contains / starts_with / ends_with
   - Recommendation: 照 Success Criteria SC-2 严格实现即可（tag_is_some + tag_eq + tag_contains），
     starts_with / ends_with 可作为 Claude's Discretion 决定是否添加

4. **是否需要 FilterBuilder::from_closure(F: Fn(&Sqllog) -> bool)?**
   - What we know: 用户可能有 FilterBuilder 覆盖不到的自定义条件
   - What's unclear: 是否影响 Phase 11 scope
   - Recommendation: 可提供 `custom(F: Fn(&Sqllog) -> bool + Send + Sync + 'static)` 逃生口，
     让用户注入任意谓词；实现成本低（一行），API 完整性高

5. **方法计数与函数长度约束**
   - What we know: CLAUDE.md 要求函数 < 40 行；FilterBuilder 有 50+ 个方法
   - What's unclear: 链式方法每个约 4 行，无问题；add() 辅助函数约 5 行；全部满足
   - Recommendation: 无问题，但 planner 应确认 builder.rs 文件总长度（预计约 350 行）

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Sqllog 使用 owned String，已无 Cow 生命周期 | 关键发现 | 已通过 src/record.rs 直接验证，风险极低 |
| A2 | par_iter 在 ROADMAP SC-5 的提及是展望性描述，Phase 11 不实现 | Integration Design | 若 Phase 11 需要 par_iter，Filter 已满足 Send+Sync，可传入 Rayon 迭代器 |
| A3 | paste crate 不引入（手写展开） | 实现方案 | 若宏展开更难维护，可改为引入 paste；不影响运行时行为 |

---

## Sources

### Primary (HIGH confidence)

- `src/record.rs` — Sqllog 结构体完整字段定义（直接读取）[VERIFIED]
- `src/filter/adapter.rs` — 现有 filter_by_exec_time / filter_by_sql_contains 实现（直接读取）[VERIFIED]
- `src/parser/iterator.rs` — LogIterator 结构和现有 filter 方法（直接读取）[VERIFIED]
- `src/lib.rs` — 公开 API 入口（直接读取）[VERIFIED]
- `.planning/REQUIREMENTS.md` — FILTER-01 ~ FILTER-10 完整需求（直接读取）[VERIFIED]
- `.planning/ROADMAP.md` — Phase 11 Success Criteria（直接读取）[VERIFIED]

### Secondary (MEDIUM confidence)

- Rust 标准库 Iterator::filter / Vec::iter().all() 行为 [ASSUMED: 与训练知识一致，标准库稳定 API]
- `Box<dyn Fn() + Send + Sync>` 满足 'static 要求的设计 [ASSUMED: Rust 所有权规则，稳定]

---

## Metadata

**Confidence breakdown:**
- FilterBuilder 设计方案: HIGH — 基于直接代码分析，Sqllog owned 结构已确认
- 与 iterator 集成: HIGH — 与现有 adapter 模式完全一致
- 性能评估: MEDIUM — 动态分发开销为理论估算，未实测

**Research date:** 2026-05-22
**Valid until:** 2026-06-22（标准库 API 稳定，30 天内有效）
