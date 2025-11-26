# dm-database-parser-sqllog

[![Crates.io](https://img.shields.io/crates/v/dm-database-parser-sqllog.svg)](https://crates.io/crates/dm-database-parser-sqllog)
[![Documentation](https://docs.rs/dm-database-parser-sqllog/badge.svg)](https://docs.rs/dm-database-parser-sqllog)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Coverage](https://img.shields.io/badge/coverage-94.07%25-brightgreen.svg)](docs/COVERAGE.md)

一个高性能的达梦数据库 sqllog 日志解析库，提供零分配或低分配的记录切分与解析功能。

## 主要特点

- **零分配解析**：基于时间戳的记录切分，使用流式 API 避免额外内存分配
- **轻量级结构**：解析结果使用引用（`&str`），避免不必要的字符串复制
- **详细的错误信息**：所有解析错误都包含原始数据，便于调试和问题定位

## 安装

在你的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dm-database-parser-sqllog = "0.4"
```

### 作为库使用

```rust
use dm_database_parser_sqllog::iter_records_from_file;

for result in iter_records_from_file("large_log.sqllog") {
    match result {
        Ok(sqllog) => {
            // 处理每条日志
        }
        Err(e) => eprintln!("解析错误: {}", e),
    }
}
```

更多用法请参考 `examples/` 目录，所有示例均为库用法，无可执行入口。

## 构建与测试

```bash
# 构建库
cargo build

# 运行测试
cargo test

# 生成文档
cargo doc --open
```

## API 文档

完整的 API 文档请查看 [docs.rs](https://docs.rs/dm-database-parser-sqllog)。

### 文件解析 API（推荐）

- [`iter_records_from_file`] - 从文件流式读取 SQL 日志，返回一个迭代器（内存高效，批量缓冲 + 并行处理）

### 核心类型

- [`Sqllog`] - SQL 日志结构体（包含时间戳、元数据、SQL 正文等）
- [`ParseError`] - 解析错误类型（包含详细错误信息）

## 设计与注意事项

- 所有 API 都直接返回解析好的 `Sqllog`，无需手动调用解析方法
- 自动使用批量缓冲 + 并行处理优化性能
- 适合处理大型日志文件（1GB 文件约 2.5 秒）
- 流式 API 内存占用低，适合超大文件或需要提前中断的场景

## 测试

本项目包含了全面的测试套件:

- **107 个测试用例**: 78 个集成测试 + 29 个单元测试
- **50+ 个基准场景**: 使用 Criterion.rs 进行性能基准测试
- **100% 通过率**: 所有测试当前状态均为通过
- **94.69% 代码覆盖率**: 行覆盖率,函数覆盖率达 98.80%

### 测试结构

**集成测试** (`tests/` 目录):
- `api.rs` - API 函数测试 (14 tests)
- `record.rs` - Record 结构测试 (9 tests)
- `record_parser.rs` - RecordParser 迭代器测试 (9 tests)
- `parse_functions.rs` - 核心解析函数测试 (46 tests)

**单元测试** (源码中):
- `sqllog.rs` - Sqllog 结构体测试 (8 tests)
- `tools.rs` - 工具函数测试 (21 tests)

**Benchmark 测试** (`benches/` 目录):
- `parse_file_bench.rs` - API 函数性能测试

### 测试覆盖率

**当前覆盖率: 94.69%** ✅

| 模块 | 行覆盖率 | 函数覆盖率 |
|------|----------|------------|
| parser/api.rs | 89.66% | 100.00% |
| parser/parse_functions.rs | 90.71% | 95.65% |
| parser/record.rs | 100.00% | 100.00% |
| parser/record_parser.rs | 96.72% | 100.00% |
| sqllog.rs | 100.00% | 100.00% |
| tools.rs | 96.07% | 100.00% |

覆盖功能:
- ✅ 所有解析函数(parse_record, parse_meta, parse_indicators)
- ✅ 所有错误路径和边界情况
- ✅ 流式 API
- ✅ 多行记录处理
- ✅ Windows/Unix 换行符兼容
- ✅ 大数据集处理(1GB+ 文件)

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件


## 相关链接

- [Crates.io](https://crates.io/crates/dm-database-parser-sqllog)
- [文档](https://docs.rs/dm-database-parser-sqllog)
- [GitHub](https://github.com/guangl/dm-parser-sqllog)
