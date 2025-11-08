# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- 添加迭代器模式的文件读取 API：`iter_records_from_file()` 和 `iter_sqllogs_from_file()`
- 支持大文件流式处理，避免一次性加载所有数据到内存
- 添加迭代器模式示例 `examples/iterator_mode.rs`
- 添加 `Clone` trait 到所有公共数据结构
- 添加可选的 serde 序列化支持（通过 `serde` feature）
- 为 `IndicatorsParts` 添加 `Copy` trait
- 完善所有公共 API 的文档注释
- 添加流式处理 API：`for_each_sqllog` 和 `for_each_sqllog_in_string`

### Changed
- **重命名 API**：`records_from_file()` → `iter_records_from_file()`，`sqllogs_from_file()` → `iter_sqllogs_from_file()`
  - 新命名更清晰地表达返回迭代器的语义，与 `parse_*` 函数区分开
  - 旧函数名保留为 deprecated 别名，确保向后兼容
- 重构 parser 模块为更清晰的子模块结构
- 优化性能：使用 `once_cell::Lazy` 进行静态初始化
- 改进错误处理：为 `ParseError` 添加 `Clone` 和 `PartialEq` trait
- 更新 `parse_records_from_file()` 和 `parse_sqllogs_from_file()` 内部使用迭代器，并添加内存使用警告
- 更新 README.md 添加迭代器模式使用指南和 API 对比表格

### Deprecated
- `records_from_file()` - 请使用 `iter_records_from_file()` 代替
- `sqllogs_from_file()` - 请使用 `iter_sqllogs_from_file()` 代替

### Fixed
- 修复文档测试中的编译错误
- 解决大文件解析时的内存占用问题

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
