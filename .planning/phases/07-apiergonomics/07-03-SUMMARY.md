---
phase: 07-apiergonomics
plan: 03
subsystem: sqllog-field-access
tags:
  - rust
  - field-access
  - trait
requires:
  - 07-01
affects:
  - src/sqllog.rs
  - src/lib.rs
  - tests/parser_from_sqllog.rs
key-files:
  created:
    - tests/parser_from_sqllog.rs
  modified:
    - src/sqllog.rs
    - src/lib.rs
metrics:
  duration: "~10 min"
  completed_date: "2026-05-19"
---

# Phase 07 Plan 03: Sqllog 字段访问 + FromSqllog Trait

本 plan 在 Sqllog 上新增 `exec_time()` 和 `row_count()` 直接字段访问方法（API-03，已由 07-01 或 07-02 完成），定义 `FromSqllog` trait 并公开导出（API-04），创建 5 个全通过测试。

## 实现内容

### exec_time() / row_count() — 已在基 commit 中

`exec_time()` 和 `row_count()` 方法在进入本 agent 的基 commit `1e49354` 时已存在。签名和实现与计划一致：

```rust
pub fn exec_time(&self) -> Result<Option<u64>, ParseError>
pub fn row_count(&self) -> Result<Option<u64>, ParseError>
```

### FromSqllog trait — 本 plan 新增

```rust
pub trait FromSqllog {
    fn from_sqllog(s: Sqllog<'_>) -> Self;
}
```

定义在 `src/sqllog.rs` 的 `PerformanceMetrics` 结构体之后。用户通过 `.map(MyType::from_sqllog)` 在迭代器链中使用。

### lib.rs 导出

添加 `FromSqllog` 到 sqllog 模块的 pub use 列表中。

### 测试文件 `tests/parser_from_sqllog.rs`（5 个测试）

| 测试 | 场景 | 断言 |
|------|------|------|
| `test_from_sqllog_maps_record` | tempfile + `LogParserBuilder` + `.map(TestRecord::from_sqllog)` | records.len()==1, 含 "2025", 含 "SELECT 1" |
| `test_exec_time_returns_value` | EXECTIME 200ms | `Ok(Some(200))` |
| `test_exec_time_returns_none_when_missing` | 无指标 | `Ok(None)` |
| `test_row_count_returns_value` | ROWCOUNT 42 | `Ok(Some(42))` |
| `test_from_sqllog_trait_object_safety` | 编译期泛型 bound 验证 | 编译通过 |

## 验证结果

- [x] `cargo build` 编译通过
- [x] `cargo test --test parser_from_sqllog` 5 个测试全部通过
- [x] `cargo test` 全 workspace 通过（100+ 测试）
- [x] `cargo clippy --all-targets -- -D warnings` 无警告
- [x] `cargo fmt` 格式化无变更

## Deviations from Plan

### 1. [Rule 1 - Bug] FromSqllog trait 的 doc example 中 impl 块的生命周期标注

**Found during:** Task 2（doc test 编译失败）

**Issue:** doc example 使用 `impl<'a> FromSqllog for MyRecord` 和 `fn from_sqllog(s: Sqllog<'a>)`，但 trait 方法签名使用匿名生命周期 `fn from_sqllog(s: Sqllog<'_>)`。生命周期不匹配导致 doc test 编译失败。

**Fix:** 将 doc example 改为 `impl FromSqllog for MyRecord` 和 `fn from_sqllog(s: Sqllog<'_>)`。

**Files modified:** `src/sqllog.rs`（doc 注释部分）

**Commit:** `d3280de`

### 2. [Rule 1 - Bug] test 5 trait object safety — FromSqllog 非对象安全

**Found during:** Task 2

**Issue:** 计划要求 test 5 接受 `&dyn FromSqllog`，但 `FromSqllog::from_sqllog` 返回 `Self` 且没有 `Self: Sized` 约束，因此 trait 不是对象安全的。`&dyn FromSqllog` 无法编译。

**Fix:** 将 test 5 改为使用泛型 trait bound `fn _use_trait_bound<T: FromSqllog>(_t: T) {}`，验证 trait 可在编译期作为泛型约束使用。这仍然实现了"编译时验证 FromSqllog 可以作为 trait 引用"的计划意图。

**Files modified:** `tests/parser_from_sqllog.rs`

**Commit:** `d3280de`

## 测试覆盖

- `test_from_sqllog_maps_record` 验证 `.map()` 迭代器链模式
- `test_exec_time_returns_value` / `test_row_count_returns_value` 验证有值场景
- `test_exec_time_returns_none_when_missing` 验证无指标场景
- `test_from_sqllog_trait_object_safety` 验证 trait 可编译且可作为泛型 bound

## Commits

- `d3280de` — feat(07-apiergonomics): add FromSqllog trait, lib.rs export, and 5 tests
