# dm-database-parser-sqllog

[![Crates.io](https://img.shields.io/crates/v/dm-database-parser-sqllog.svg)](https://crates.io/crates/dm-database-parser-sqllog)
[![Documentation](https://docs.rs/dm-database-parser-sqllog/badge.svg)](https://docs.rs/dm-database-parser-sqllog)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Coverage](https://img.shields.io/badge/coverage-%E2%89%A5%2090%25-brightgreen.svg)](docs/COVERAGE.md)

一个高性能的达梦数据库 sqllog 日志解析库，提供零分配或低分配的记录切分与解析功能。

## 主要特点

- **零分配解析**：基于时间戳的记录切分，使用流式 API 和 `Cow` 类型避免额外内存分配
- **完全惰性解析**：仅在需要时解析 SQL 正文和性能指标，大幅提升扫描速度
- **极致性能**：单线程处理速度超过 400 万条/秒（>1GB/s）
- **轻量级结构**：解析结果使用引用（`&str`），避免不必要的字符串复制
- **详细的错误信息**：所有解析错误都包含原始数据，便于调试和问题定位

## 安装

在你的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dm-database-parser-sqllog = "0.6"
```

### 作为库使用

```rust
use dm_database_parser_sqllog::LogParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = LogParser::from_path("large_log.sqllog")?;
    for result in parser.iter() {
        match result {
            Ok(sqllog) => {
                // 处理每条日志
                println!("SQL: {}", sqllog.body());
            }
            Err(e) => eprintln!("解析错误: {}", e),
        }
    }
    Ok(())
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

### 覆盖率 ≥ 90%（已在 CI 强制）

本仓库使用 `cargo-llvm-cov` 统计并卡住覆盖率下限为 90%。本地运行：

```powershell
# 安装 cargo-llvm-cov（一次性）
cargo install cargo-llvm-cov

# 运行并在达不到 90% 时失败
cargo llvm-cov --workspace --all-features --fail-under-lines 90
```

### 基准作为性能基线（已在 CI 强制）

我们将 Criterion 基准的当前结果作为“性能基线”。任何改动不得慢于该基线（容忍度默认 0%）。

首次或更新基线（会运行基准并写入 `benchmarks/baseline.json`）：

```powershell
pwsh ./scripts/export_criterion_baseline.ps1
```

对比当前实现与基线（慢于基线将退出码 1）：

```powershell
pwsh ./scripts/check_criterion_against_baseline.ps1 -Baseline 'benchmarks/baseline.json' -TolerancePercent 0
```

CI 中已在 Windows 上执行上述校验以避免环境差异导致波动。

## API 文档

完整的 API 文档请查看 [docs.rs](https://docs.rs/dm-database-parser-sqllog)。

### 文件解析 API（推荐）

- [`LogParser`] - 从文件流式读取 SQL 日志，返回一个迭代器（内存映射 + 零拷贝）

### 核心类型

- [`Sqllog`] - SQL 日志结构体（包含时间戳、元数据、SQL 正文等）
- [`ParseError`] - 解析错误类型（包含详细错误信息）

## 设计与注意事项

- 所有 API 都直接返回解析好的 `Sqllog`，无需手动调用解析方法
- 采用内存映射 (mmap) 技术，适合处理大型日志文件（1GB 文件 < 1 秒）
- 流式 API 内存占用低，适合超大文件或需要提前中断的场景
- `body()` 和 `indicators_raw()` 方法采用惰性求值，仅在调用时进行分割和 UTF-8 转换

## 测试

本项目包含了全面的测试套件:

- **集成测试**: `tests/` 目录下的集成测试覆盖了常见场景
- **Benchmark 测试**: 使用 Criterion.rs 进行性能基准测试，确保高性能
- **100% 通过率**: 所有测试当前状态均为通过

### 测试结构

**集成测试** (`tests/` 目录):
- `integration_test.rs` - 核心功能集成测试

**Benchmark 测试** (`benches/` 目录):
- `parser_benchmark.rs` - 解析器性能测试

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件


## 相关链接

- [Crates.io](https://crates.io/crates/dm-database-parser-sqllog)
- [文档](https://docs.rs/dm-database-parser-sqllog)
- [GitHub](https://github.com/guangl/dm-parser-sqllog)
