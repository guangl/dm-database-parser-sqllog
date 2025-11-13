# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2025-11-13

### Added

- **全面的 Benchmark 测试体系**：新增 5 个 benchmark 文件，覆盖所有核心组件
  - `benches/api_bench.rs` - API 函数性能测试（3 组测试）
  - `benches/parse_functions_bench.rs` - 解析函数性能测试（8 组测试，60+ 场景）
  - `benches/record_bench.rs` - Record 结构性能测试（6 组测试）
  - `benches/record_parser_bench.rs` - RecordParser 迭代器性能测试（6 组测试）
  - `benches/tools_bench.rs` - 工具函数性能测试（7 组测试）

- **Benchmark 文档**：
  - `BENCHMARKS.md` - 完整的 benchmark 文档和最佳实践
  - `BENCHMARK_QUICK_START.md` - 快速入门指南

- **示例代码**：
  - `examples/perf_full_test.rs` - 完整解析性能测试
  - `examples/perf_record_only.rs` - 仅记录识别性能测试
  - `examples/streaming_parse.rs` - 流式解析示例

### Changed

- **测试结构重组**：将所有测试从 `src/parser/tests.rs` 迁移到 `tests/` 目录
  - 创建 4 个集成测试文件（`api.rs`, `record.rs`, `record_parser.rs`, `parse_functions.rs`）
  - 保持单元测试在源码中（`sqllog.rs`, `tools.rs`）
  - 测试统计：107 个测试（78 集成 + 29 单元）

- **API 变更**：
  - `parse_records_from_file` 现在返回 `(Vec<Sqllog>, Vec<ParseError>)` 元组
  - 将部分内部函数暴露为 `pub`（通过 `__test_helpers` 模块供测试使用）

- **文档更新**：
  - 新增 `TESTS.md` - 详细的测试说明文档
  - 更新 `README.md` 反映新的测试和 benchmark 结构
  - 删除 `docs/BENCHMARK_API.md`（整合到新文档中）

### Performance

- 测试覆盖率：94.69%（行覆盖），98.80%（函数覆盖），93.92%（区域覆盖）
- Benchmark 覆盖：30+ 组测试场景，60+ 个具体测试
- 核心函数性能：
  - `parse_record` 单行：~470 ns
  - `parse_record` 多行：~480 ns
  - `is_record_start_line` 有效行：~20-45 ns
  - `is_record_start_line` 无效行：~0.8 ns

## [0.3.0] - 2025-01-24

### Added

- **实时监控功能**：全新的 `realtime` 特性，支持实时监控 SQL 日志文件变化
  - `RealtimeSqllogParser` - 核心实时监控解析器
  - `watch()` - 持续监控日志文件，自动捕获新增记录
  - `watch_for()` - 监控指定时长后自动停止
  - `from_beginning()` - 从文件开头开始处理所有记录
  - 自动处理文件轮转、截断等场景
  - 精确的文件位置跟踪和断点续传支持
  - 完善的错误处理和恢复机制

- **新增依赖**：
  - `notify = "8.2"` - 跨平台文件系统监控（可选特性）

- **示例代码**：
  - `examples/realtime_watch.rs` - 实时监控示例

- **测试覆盖**：
  - 新增 108 个实时监控专项测试
  - 测试总数从 130 增加到 268
  - 实时监控模块代码覆盖率: 91.17% (行覆盖)

### Changed

- 整体代码覆盖率从 98.47% 调整至 94.07%（由于新增大量实时监控代码）
- 更新项目描述，强调实时监控能力
- 更新 README.md，新增实时监控使用说明
- 更新 Cargo.toml 版本至 0.3.0

### Documentation

- 新增 REALTIME_FEATURE.md 详细文档
- 更新 README.md 安装说明，包含 realtime 特性的启用方式
- 更新相关链接，添加实时监控特性文档

## [0.2.0]

### Changed
- **错误信息增强**：所有 `ParseError` 变体现在都包含原始数据用于调试
  - 从元组变体改为结构体变体（如 `InvalidEpFormat { value, raw }`）
  - 所有错误消息格式统一为：`错误描述 | raw: 原始数据`
  - `FileNotFound` 错误现在包含完整的文件路径和系统错误信息
- **is_record_start_line 优化**：修复字段数量验证逻辑，确保至少 5 个必需字段
- **测试改进**：更新测试用例以适配新的错误格式和字段验证规则

### Fixed
- 修复了 `is_record_start_line` 在字段不足时仍可能返回 true 的问题
- 修复了所有测试用例以匹配新的结构体变体错误格式

## [0.1.3] - 2025-11-09

### Added
- **测试覆盖率**：达到 98.47% 的代码覆盖率（远超 80% 目标）
- **API 覆盖率测试**：新增 21 个 API 测试用例 (`tests/api_coverage.rs`)
  - `for_each_sqllog` 系列函数测试
  - `parse_*_from_file` 函数测试
  - `iter_*_from_file` 函数测试
  - deprecated API 向后兼容性测试
- **集成测试**：11 个端到端场景测试 (`tests/integration_tests.rs`)
  - 文件读取迭代器测试
  - 大文件处理测试（1000+ 条记录）
  - 并发解析测试（10 线程）
  - 混合有效/无效行处理
- **性能回归测试**：7 个性能基准测试 (`tests/performance_regression.rs`)
  - 1000 条记录 < 100ms
  - 10000 条迭代 < 1s
  - 吞吐量 > 10000 条/秒
- **边界情况测试**：12 个边界条件测试 (`tests/edge_cases.rs`)
  - 时间戳边界、EP 字段边界
  - 特殊字符、UTF-8、emoji 支持
  - 客户端 IP 格式（IPv4/IPv6）
- **文档**：
  - `docs/TESTING.md` - 完整的测试文档和指南
  - `docs/COVERAGE.md` - 详细的覆盖率报告
  - `docs/BENCHMARK_TOOLS.md` - 性能基准测试说明
- **CI/CD**：GitHub Actions 工作流
  - `ci.yml` - 持续集成
  - `benchmark.yml` - 性能基准测试
  - `release.yml` - 自动发布
- **示例代码**：
  - `examples/iterator_mode.rs` - 迭代器模式使用
  - `examples/parse_from_file.rs` - 文件解析示例
  - `examples/stream_processing.rs` - 流式处理示例

### Changed
- **模块化重构**：将 `parser.rs` (1038+ 行) 拆分为 7 个子模块
  - `constants.rs` - 常量定义
  - `record.rs` - Record 结构
  - `record_parser.rs` - Record 解析器
  - `sqllog_parser.rs` - Sqllog 解析器
  - `parse_functions.rs` - 核心解析函数
  - `api.rs` - 便捷 API
  - `tests.rs` - 单元测试
- **API 重命名**（保持向后兼容）：
  - `records_from_file()` → `iter_records_from_file()`
  - `sqllogs_from_file()` → `iter_sqllogs_from_file()`
- **文档改进**：
  - README.md 添加覆盖率徽章（98.47%）
  - 更新测试统计（130 个测试用例）
  - 添加迭代器模式使用指南
- **性能优化**：
  - 流式处理避免内存溢出
  - 迭代器模式提升大文件处理效率

### Deprecated
- `records_from_file()` - 请使用 `iter_records_from_file()` 代替（自 0.1.3 起）
- `sqllogs_from_file()` - 请使用 `iter_sqllogs_from_file()` 代替（自 0.1.3 起）

### Fixed
- 修复文件读取 API 的覆盖率问题（从 26.44% 提升到 96.55%）
- 完善错误处理测试

### Statistics
- **测试数量**：130 个（单元测试 79 + 集成测试 11 + 性能回归 7 + 边界情况 12 + API 覆盖 21）
- **代码覆盖率**：98.47%（行覆盖），99.28%（函数覆盖）
- **通过率**：100%（130/130）
- **性能**：吞吐量 > 10000 条/秒

## [0.1.2] - 2025-11-09

### Added
- 模块化 parser 结构
- 详细的文档注释和示例
- 流式处理支持

### Changed
- 性能优化：单次迭代验证、预分配内存
- 使用 `once_cell` 替代 lazy_static

## [0.1.1] - Previous Release

### Added
- 基本的日志解析功能
- Record 和 Sqllog 数据结构
- 批量解析 API

## [0.1.0] - Initial Release

### Added
- 初始版本发布
- SQL 日志解析器基础功能
