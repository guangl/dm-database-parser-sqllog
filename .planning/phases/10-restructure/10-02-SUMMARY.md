# 10-02 SUMMARY — Parser Submodule Split + Test Migration

## Status: COMPLETE

## 各子文件最终行数

| 文件 | 行数 | 内容 |
|------|------|------|
| src/parser/mod.rs | 801 | LogParser、parse_record(pub(crate))、parse_record_with_hint、静态 finder、#[cfg(test)] 迁入测试 |
| src/parser/builder.rs | 54 | LogParserBuilder struct + 完整 impl |
| src/parser/iterator.rs | 178 | LogIterator、Iterator impl、filter 委托调用、is_timestamp_start（私有） |
| src/parser/encoding.rs | 11 | pub enum FileEncodingHint |

## 迁入的测试用例（按来源分组）

| 来源 | 操作 | 迁入用例数 |
|------|------|-----------|
| tests/performance_metrics.rs | 整体迁移 + git rm | 16 |
| tests/sqllog_additional.rs | 整体迁移 + git rm | 19 |
| tests/parser_coverage.rs | 部分迁移（3 个），保留 2 个 | 3 |
| tests/edge_cases.rs | 部分迁移（3 个），保留 1 个 | 3 |
| tests/parser_errors.rs | 部分迁移（1 个），保留 8 个 | 1 |

合计迁入：42 个测试用例；tests/ 中保留：24 个

## LogIterator::filter_by_* 可见性修订说明

CONTEXT.md 原决策 D-07 要求 filter_by_exec_time / filter_by_sql_contains 降为 pub(crate)。
RESEARCH.md Pitfall 5 指出 examples/filter_slow_queries.rs 直接调用 filter_by_exec_time——降为 pub(crate) 后 examples 无法编译，违反 REFACTOR-07。
**采纳 RESEARCH 推荐方案**：两个方法保持 `pub` 可见性；方法体改为委托调用 filter::adapter 泛型函数（D-06 保持）。

## cargo test 实际结果

75 个测试全部通过，0 失败：
- src/parser/mod.rs 内部测试：51 通过（含 42 迁入 + 9 原有）
- tests/edge_cases.rs：1 通过（保留）
- tests/integration_test.rs：2 通过
- tests/parser_coverage.rs：2 通过（保留）
- tests/parser_errors.rs：8 通过（保留）
- tests/parser_filters.rs：6 通过
- tests/parser_iterator.rs：2 通过
- doc-tests：3 通过

## 遗留 warning

`warning: function 'parse_record' is never used`——parse_record 是 pub(crate)，仅在 #[cfg(test)] 块中被调用。非测试编译目标下 Rust 会报此 warning。由 Plan 10-03 通过 clippy 修复（预期方案：添加 `#[cfg_attr(not(test), allow(dead_code))]` 或在非测试路径添加使用点）。

## 下一计划（10-03）衔接点

- src/lib.rs 当前状态：mod error、parser、record（缺 filter）；pub use 列表不含 parse_record（已在本计划移除）
- 需在 lib.rs 添加 `pub(crate) mod filter;`（D-15）
- 运行 D-17 三项硬验收：cargo test + cargo clippy -- -D warnings + cargo llvm-cov --fail-under-lines 90
- parse_record unused warning 需在 clippy 通过前修复
