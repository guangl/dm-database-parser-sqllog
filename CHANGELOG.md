# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
