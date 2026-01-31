# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.1] - 2026-01-31

### Added
- **文件级编码检测**：通过采样前 64KB 自动识别日志文件的编码（UTF‑8 或 GB18030），并将结果缓存到 `LogParser::encoding`，避免对每条记录进行重复猜测，提升解析稳定性与性能。
- **提取方括号标签**：新增 `Sqllog.tag: Option<Cow<'a, str>>`，自动提取记录前缀的 `[SEL]`、`[ORA]` 等标签（若无则为 `None`）。
- **集成回归测试**：增加针对 `dmsql_OA01_20260127_15` 文件夹的集成测试，确保解析过程中无替换字符（�）并且没有解析错误。

### Fixed
- 修复 GB18030 编码的 meta 与 body 解码问题，防止中文字段被乱码处理。  
- 修复当 `appname:` 为空且紧随的 token 为 `ip:` / `ip::` / `ip:::` 时字段错位的问题。


## [0.6.0] - 2025-12-02

### Changed
- **重大性能优化**：重构 `Sqllog` 结构体，实现完全惰性解析。
  - 引入 `content_raw` 字段存储原始字节，推迟 `body` 和 `indicators` 的分割与解析。
  - `LogIterator` 引入上下文提示（Context Hinting），大幅减少单行日志的扫描开销。
  - 解析性能提升至 >400万条/秒（单线程）。
- **API 变更**：
  - `Sqllog` 的 `body` 字段变更为 `body()` 方法。
  - `Sqllog` 的 `indicators_raw` 字段变更为 `indicators_raw()` 方法。
  - 移除了 `Sqllog` 中的 `body` 和 `indicators_raw` 公共字段。

## [0.5.0] - 2025-11-29

### Changed
- 初始性能优化版本，引入 `Cow` 实现零拷贝解析。

## [0.4.3] - 2025-11-26

### Changed
- 完善所有核心模块的库化文档注释，明确 API 用法和 feature 控制
- 测试辅助 API 通过 feature `test-helpers` 隐藏，普通用户不可见
- README 增加 crates.io、docs.rs、CI、feature 说明、examples 目录说明等内容
- 代码结构进一步规范，所有内部类型和工具均不暴露给普通用户

### Fixed
- 保证所有 feature、文档、注释与 crates.io 规范一致
- 修正部分注释和文档遗漏

## [0.4.1] - 2025-11-20

### Changed
- 升级依赖：rayon 升级到 1.11.0，memchr 升级到 2.7.6，thiserror 升级到 2.0.17
- 移除可选依赖和特性相关代码（serde、notify），简化 Cargo.toml
- 修正仓库链接，repository 字段改为实际地址
- 清理和优化 README 文档，删除不再支持的 API 示例和说明
- 优化 parser 相关代码结构，去除无用重导出和条件编译
- 同步依赖锁文件，移除无用依赖
- 将 `SqllogIterator` 设为 crate 内部实现并从 `api.rs` 中移除，避免将内部实现暴露为公共类型

### Fixed
- 修复 release.yml 的 CI 触发条件和 secrets 判断语法，兼容 GitHub Actions 标准
- 修复 CI 只在 Rust 源码和 Cargo 文件变更时触发

## [0.4.0] - 2025-11-13

### Added

- **全面的 Benchmark 测试体系**：新增 5 个 benchmark 文件，覆盖所有核心组件
  - `benches/api_bench.rs` - API 函数性能测试（3 组测试）
  - `benches/parse_functions_bench.rs` - 解析函数性能测试（8 组测试，60+ 场景）
  - `benches/record_bench.rs` - Record 结构性能测试（6 组测试）
  - `benches/record_parser_bench.rs` - Record 解析器性能测试（6 组测试）
  - `benches/tools_bench.rs` - 工具函数性能测试（7 组测试）

- **Benchmark 文档**：
  - `BENCHMARKS.md` - 完整的 benchmark 文档和最佳实践
  - `BENCHMARK_QUICK_START.md` - 快速入门指南

- **示例代码**：
  - `examples/perf_full_test.rs` - 完整解析性能测试
  - `examples/perf_record_only.rs` - 仅记录识别性能测试
  - `examples/streaming_parse.rs` - 流式解析示例

- **更多内容继续...**