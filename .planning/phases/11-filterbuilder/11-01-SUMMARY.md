---
phase: 11-filterbuilder
plan: "01"
subsystem: filter
tags: [filter, builder-pattern, predicate, chain-api]
dependency_graph:
  requires: [src/record.rs]
  provides: [src/filter/builder.rs, src/filter/mod.rs]
  affects: [Plan 11-02 (wiring)]
tech_stack:
  added: []
  patterns: [builder-pattern, dyn-dispatch, type-alias]
key_files:
  created:
    - src/filter/builder.rs
  modified:
    - src/filter/mod.rs
decisions:
  - "使用 Predicate 类型别名（Box<dyn Fn(&Sqllog) -> bool + Send + Sync>）解决 clippy::type_complexity"
  - "Filter/FilterBuilder 加 #[allow(dead_code)] 临时标注，待 Plan 11-02 接入 lib.rs 后移除"
  - "7 个字符串元数据字段手写展开 28 个方法（不引入 paste crate），满足每方法独立 rustdoc"
  - "exectime 不提供 eq 方法（f32 浮点精度问题，用 between 替代）"
metrics:
  duration: "347s"
  completed_date: "2026-05-22"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 2
---

# Phase 11 Plan 01: FilterBuilder 核心 API 构建 Summary

**一句话总结：** 新建 `src/filter/builder.rs` 实现 `FilterBuilder` 链式构建器 + `Filter` 谓词容器，覆盖 14 个 Sqllog 字段全部 50 个公开谓词方法（FILTER-01~09）；更新 `src/filter/mod.rs` 声明并重导出公开类型。

---

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | 创建 src/filter/builder.rs，实现 Filter + FilterBuilder + 50 个方法 + 单元测试 | 3dbaced | src/filter/builder.rs（新建，753 行） |
| 2 | 更新 src/filter/mod.rs 声明 builder 子模块并重导出 | 7f2da24 | src/filter/mod.rs（修改，5 行） |

---

## API 表面统计

### src/filter/builder.rs（753 行）

| 字段分类 | 方法数 | 方法前缀 | 需求 |
|---------|--------|---------|------|
| ts（String）| 4 | ts_ | FILTER-01 |
| tag（Option<String>）| 4 | tag_ | FILTER-02 |
| ep（u8）| 4 | ep_ | FILTER-03 |
| sess_id（String）| 4 | sess_id_ | FILTER-04 |
| thrd_id（String）| 4 | thrd_id_ | FILTER-04 |
| username（String）| 4 | username_ | FILTER-04 |
| trxid（String）| 4 | trxid_ | FILTER-04 |
| statement（String）| 4 | statement_ | FILTER-04 |
| appname（String）| 4 | appname_ | FILTER-04 |
| client_ip（String）| 4 | client_ip_ | FILTER-04 |
| sql（String）| 4 | sql_ | FILTER-05 |
| exectime（f32）| 3（无 eq）| exec_time_ | FILTER-06 |
| rowcount（u32）| 4 | rowcount_ | FILTER-07 |
| exec_id（i64）| 4 | exec_id_ | FILTER-08 |
| **合计** | **55** | — | — |

（Filter::matches + FilterBuilder::new + build + add 私有方法 = 4 个基础方法 + 50 个谓词方法 + 1 个 Default 实现 = 共 55 个方法接口）

### src/filter/mod.rs（5 行）

```
pub(crate) mod adapter;
pub mod builder;

#[allow(unused_imports)] // allow until Plan 11-02 wires in lib.rs exports
pub use builder::{Filter, FilterBuilder};
```

---

## 单元测试统计

- **新增测试数：** 38 个
- **测试结果：** `test result: ok. 38 passed; 0 failed; 0 ignored`
- **整体测试数：** 89 passed（lib unit tests，含原有 51 + 新增 38）

### 测试覆盖分布

| 测试组 | 测试数 |
|--------|--------|
| FILTER-01（ts 4 方法）| 4 |
| FILTER-02（tag 4 方法）| 4 |
| FILTER-03（ep 4 方法）| 4 |
| FILTER-04（7 字段各 1 代表性测试）| 7 |
| FILTER-05（sql 4 方法）| 4 |
| FILTER-06（exectime 3 方法）| 3 |
| FILTER-07（rowcount 4 方法）| 4 |
| FILTER-08（exec_id 4 方法）| 4 |
| FILTER-09（AND 语义 + 空过滤器 + Default）| 4 |
| **合计** | **38** |

---

## 构建与质量

| 检查项 | 结果 |
|--------|------|
| `cargo build` | 通过（0 警告）|
| `cargo build --tests` | 通过 |
| `cargo test --lib filter::builder` | 38 passed; 0 failed |
| `cargo test`（整体）| 所有 suites 0 failed |
| `cargo clippy --all-targets -- -D warnings` | 0 错误 0 警告 |
| 函数 < 40 行约束（CLAUDE.md）| 满足，awk 验证无 TOO LONG 输出 |
| 新依赖（Cargo.toml 变更）| 无 |

---

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] 引入 Predicate 类型别名解决 clippy::type_complexity**
- **Found during:** Task 2 运行 `cargo clippy --all-targets -- -D warnings` 时
- **Issue:** `Vec<Box<dyn Fn(&Sqllog) -> bool + Send + Sync>>` 在 Filter 和 FilterBuilder 字段定义处触发 `clippy::type_complexity` 错误（-D warnings 升级为 error）
- **Fix:** 在文件头部引入 `type Predicate = Box<dyn Fn(&Sqllog) -> bool + Send + Sync>;` 类型别名，两个结构体字段改为 `Vec<Predicate>`
- **Files modified:** src/filter/builder.rs
- **Commit:** 7f2da24（包含在 Task 2 提交中）
- **Impact on acceptance criteria:** `grep -c 'Box<dyn Fn(&Sqllog) -> bool + Send + Sync>' src/filter/builder.rs` 返回 1（而非要求的 ≥ 2），因为类型别名定义只有 1 处。语义上类型仍然正确：`Predicate = Box<dyn Fn(&Sqllog) -> bool + Send + Sync>`，两个结构体均使用该别名。

**2. [Rule 2 - Missing] Filter/FilterBuilder impl 块添加 #[allow(dead_code)]**
- **Found during:** Task 2 连接 mod.rs 后，`cargo build` 出现 dead_code 警告（Filter::matches 和 FilterBuilder 所有方法）
- **Issue:** `src/lib.rs` 中 `filter` 模块是 `pub(crate)`，Filter/FilterBuilder 对 crate 外不可见，clippy 报 dead_code
- **Fix:** 按 Plan 选项 1，在 Filter impl 块和 FilterBuilder impl 块各加 `#[allow(dead_code)]`；mod.rs 的 pub use 行加 `#[allow(unused_imports)]`
- **Files modified:** src/filter/builder.rs, src/filter/mod.rs
- **Plan 11-02 待办：** 在 lib.rs 完成顶层重导出后，移除 builder.rs 中的 4 处 `#[allow(dead_code)]` 和 mod.rs 中的 `#[allow(unused_imports)]`

---

## 留给 Plan 11-02 的衔接点

1. **类型已公开：** `Filter` 和 `FilterBuilder` 已在 `src/filter/builder.rs` 中声明为 `pub`；`src/filter/mod.rs` 已 `pub use builder::{Filter, FilterBuilder};`
2. **Send + Sync 满足：** `Filter` 持有 `Vec<Predicate>`，`Predicate = Box<dyn Fn(&Sqllog) -> bool + Send + Sync>`，可安全在 `spawn_blocking` 中传递（Phase 12 async 需求）
3. **待移除的临时标注（Plan 11-02 任务）：**
   - `src/filter/builder.rs` 第 12 行：`#[allow(dead_code)]`（Filter struct）
   - `src/filter/builder.rs` 第 20 行：`#[allow(dead_code)]`（impl Filter）
   - `src/filter/builder.rs` 第 44 行：`#[allow(dead_code)]`（FilterBuilder struct）
   - `src/filter/builder.rs` 第 49 行：`#[allow(dead_code)]`（impl FilterBuilder）
   - `src/filter/mod.rs` 第 4 行：`#[allow(unused_imports)]`
4. **lib.rs 重导出（Plan 11-02 Task 1）：** 在 `src/lib.rs` 添加 `pub use filter::builder::{Filter, FilterBuilder};`，或先改 `pub mod filter;`（现为 pub(crate)）再让 filter::Filter / FilterBuilder 直接对外可见

---

## Known Stubs

无——所有谓词方法完整实现，无 placeholder 或硬编码空值。

---

## Threat Flags

无——FilterBuilder 不涉及认证、网络、文件访问或信任边界，无新安全攻击面。

---

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| src/filter/builder.rs 存在 | FOUND |
| src/filter/mod.rs 存在 | FOUND |
| .planning/phases/11-filterbuilder/11-01-SUMMARY.md 存在 | FOUND |
| 3dbaced 提交存在 | FOUND |
| 7f2da24 提交存在 | FOUND |
