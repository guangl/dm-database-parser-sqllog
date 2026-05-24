# Changelog

All notable changes to this project will be documented in this file.

## [2.0.0] - 2026-05-23

### Added

- **FilterBuilder**：56 个链式谓词方法，覆盖全部 14 个字段（ts、tag、ep、sess_id、thrd_id、username、trxid、statement、appname、client_ip、sql、exectime、rowcount、exec_id），AND 语义组合，通过 `Filter` 对象传入迭代器。
- **AsyncLogParser**（可选 feature `async`）：基于 tokio `spawn_blocking` 的异步封装，`AsyncLogParser::new(path).with_filter(filter).parse().await` 返回 `Vec<Sqllog>`。
- **`apply_filter` / `apply_filter_keep_errors`**：`LogIterator` 新增两个方法，支持将 `Filter` 应用于顺序迭代。
- 代码按功能分层：`parser/`、`filter/`、`async_api/`、`record.rs`、`error.rs`。

### Changed

- 模块结构重组，内部实现分拆至子目录，公开 API 不变。

## [1.1.0] - 2026-05-21

### Changed (Breaking)

- **Sqllog 扁平化**：`MetaParts` 和 `PerformanceMetrics` 结构体已移除，所有字段直接平铺到 `Sqllog`。
- **去除惰性解析**：所有字段在解析时一次性填充，不再有延迟方法。`body()`、`parse_meta()`、`parse_indicators()`、`parse_performance_metrics()`、`exec_time()`、`row_count()` 等方法全部移除，改为直接字段访问。
- **去除生命周期**：`Sqllog<'a>` → `Sqllog`，所有 `Cow<'a, str>` 替换为 `String`。
- **去除 unsafe**：移除所有 `unsafe` 代码块，全 safe Rust 实现。
- **去除 mmap**：`LogParser` 改用 `Vec<u8>` 持有文件内容，移除 `memmap2` 依赖。
- **去除并行迭代**：移除 `par_iter()`、`RecordIndex`、`index()`，移除 `rayon` 依赖。
- **去除 FromSqllog**：移除 `FromSqllog` trait。
- **LogParserBuilder 简化**：移除 `threads()` 和 `parallel_threshold()` 方法。
- **依赖精简**：移除 `memmap2`、`rayon`、`simdutf8`、`fast-float` 依赖。

### Migration

```rust
// before (v1.0)
let body = record.body();
let meta = record.parse_meta();
let pm = record.parse_performance_metrics();
let et = record.exec_time()?;

// after (v1.1)
let body = &record.sql;
let username = &record.username;
let exectime = record.exectime;
let rowcount = record.rowcount;
```

## [1.0.0] - 2026-05-19

### Added

- **`LogParserBuilder` 链式构建 API**
- **过滤方法**：`filter_by_exec_time`、`filter_by_sql_contains`
- **直接字段访问**：`exec_time()`、`row_count()`
- **`FromSqllog` trait**
- 独立运行示例

### Changed

- `ParseError` 增强：添加 `line_number` 字段
- 公开 API rustdoc 全覆盖

## [0.9.1] - 2026-04-13

### Changed
- 热路径性能优化（单线程 -20.5%）
- 依赖升级

## [0.9.0] - 2026-04-04

### Changed
- 最小化公开 API（破坏性变更）：子模块改为 `pub(crate)`

## [0.8.0] - 2026-04-04

### Added
- `LogParser::par_iter()` 基于 rayon 的并行迭代器

### Changed
- 性能优化（单线程 +33%）

## [0.7.0] - 2026-04-02

### Added
- `parse_performance_metrics()` 方法
- ORA tag 前缀自动去除
- 合并 `IndicatorsParts` 到 `PerformanceMetrics`

## [0.6.1] - 2026-01-31

### Added
- 文件级编码检测
- 提取方括号标签 `[SEL]`/`[ORA]`

### Fixed
- GB18030 编码解码问题

## [0.6.0] - 2025-12-02

### Changed
- 完全惰性解析重构
- 性能提升至 >400 万条/秒

## [0.5.0] - 2025-11-29

### Changed
- 引入 `Cow` 实现零拷贝解析

## Earlier versions

See git history for versions prior to 0.5.0.
