# dm-database-parser-sqllog

[![Crates.io](https://img.shields.io/crates/v/dm-database-parser-sqllog.svg)](https://crates.io/crates/dm-database-parser-sqllog)
[![Documentation](https://docs.rs/dm-database-parser-sqllog/badge.svg)](https://docs.rs/dm-database-parser-sqllog)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Coverage](https://img.shields.io/badge/coverage-98.47%25-brightgreen.svg)](docs/COVERAGE.md)

一个高性能的达梦数据库 sqllog 日志解析库，提供零分配或低分配的记录切分与解析功能。

## 主要特点

- **零分配解析**：基于时间戳的记录切分，使用流式 API 避免额外内存分配
- **高效模式匹配**：使用双数组 Aho-Corasick（daachorse）进行高效模式匹配
- **轻量级结构**：解析结果使用引用（`&str`），避免不必要的字符串复制
- **灵活的 API**：提供批量解析、流式解析等多种使用方式
- **详细的错误信息**：所有解析错误都包含原始数据，便于调试和问题定位
- **高性能**：适合在高吞吐日志处理场景中使用

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dm-database-parser-sqllog = "0.1"
```

## 快速开始

### 基本用法

### 从字符串解析

```rust
use dm_database_parser_sqllog::{parse_records_from_string, parse_sqllogs_from_string};

let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"#;

// 方法 1: 解析为 Record 列表（自动跳过无效行）
let records = parse_records_from_string(log_text);
println!("找到 {} 条记录", records.len());

// 方法 2: 解析为 Sqllog 列表（包含成功和失败的）
let results = parse_sqllogs_from_string(log_text);
for result in results {
    match result {
        Ok(sqllog) => {
            println!("用户: {}, 事务ID: {}, SQL: {}",
                sqllog.meta.username, sqllog.meta.trxid, sqllog.body);

            // 获取性能指标（如果有）
            if let Some(time) = sqllog.execute_time() {
                println!("执行时间: {:.2}ms", time);
            }
        }
        Err(e) => eprintln!("解析错误: {}", e),
    }
}
```

### 流式处理（回调模式）

```rust
use dm_database_parser_sqllog::for_each_sqllog_in_string;

let log_text = r#"..."#; // 大量日志文本

// 对每个 Sqllog 调用回调函数（适合大数据流式处理）
let count = for_each_sqllog_in_string(log_text, |sqllog| {
    println!("EP: {}, 用户: {}, SQL: {}",
        sqllog.meta.ep, sqllog.meta.username, sqllog.body);
}).unwrap();

println!("处理了 {} 条记录", count);
```


### 从文件读取

#### 方式一：迭代器模式（推荐用于大文件）

对于大文件（> 100MB），推荐使用迭代器模式，一次只加载一条记录，避免内存溢出：

```rust
use dm_database_parser_sqllog::{iter_records_from_file, iter_sqllogs_from_file};

// 迭代处理原始记录
let mut record_count = 0;
let mut error_count = 0;

for result in iter_records_from_file("large_log.sqllog")? {
    match result {
        Ok(record) => {
            record_count += 1;
            println!("记录 {}: {}", record_count, record.start_line());
        }
        Err(e) => {
            error_count += 1;
            eprintln!("错误 {}: {}", error_count, e);
        }
    }
}

println!("成功: {}, 失败: {}", record_count, error_count);

// 迭代处理解析后的 SQL 日志（带性能统计）
let mut total_time = 0.0;
let mut count = 0;
let mut slow_queries = 0;

for result in iter_sqllogs_from_file("large_log.sqllog")? {
    match result {
        Ok(sqllog) => {
            if let Some(time) = sqllog.execute_time() {
                total_time += time;
                count += 1;
                if time > 100.0 {
                    slow_queries += 1;
                    println!("慢查询: {:.2}ms - {}", time, sqllog.body);
                }
            }
        }
        Err(e) => eprintln!("解析错误: {}", e),
    }
}

println!("平均执行时间: {:.2}ms", total_time / count as f64);
println!("慢查询数量: {}", slow_queries);

// 使用迭代器组合器（筛选慢查询）
let slow_queries: Vec<_> = iter_sqllogs_from_file("large_log.sqllog")?
    .filter_map(Result::ok)  // 忽略解析错误
    .filter(|log| log.execute_time().map_or(false, |t| t > 100.0))
    .take(10)  // 只取前 10 条
    .collect();

println!("找到 {} 条慢查询", slow_queries.len());
```

#### 方式二：一次性加载（适合小文件）

对于小文件（< 100MB），可以一次性加载所有记录：

```rust
use dm_database_parser_sqllog::{parse_records_from_file, parse_sqllogs_from_file};

// 一次性加载所有记录（包含成功和失败的）
let (records, errors) = parse_records_from_file("small_log.sqllog")?;
println!("成功解析 {} 条记录，遇到 {} 个错误", records.len(), errors.len());

// 一次性加载所有 SQL 日志（包含成功和失败的）
let (sqllogs, parse_errors) = parse_sqllogs_from_file("small_log.sqllog")?;
println!("成功解析 {} 条 SQL 日志，遇到 {} 个解析错误", sqllogs.len(), parse_errors.len());

// 处理解析错误
for error in parse_errors {
    eprintln!("解析错误: {}", error);
}
```

### 错误处理和调试

所有解析错误都包含详细的原始数据，便于调试和定位问题：

```rust
use dm_database_parser_sqllog::{iter_sqllogs_from_file, ParseError};

for result in iter_sqllogs_from_file("log.sqllog")? {
    match result {
        Ok(sqllog) => {
            // 处理成功的记录
        }
        Err(e) => {
            // 错误信息包含原始数据
            match e {
                ParseError::InvalidRecordStartLine { raw } => {
                    eprintln!("无效的记录起始行: {}", raw);
                }
                ParseError::LineTooShort { length, raw } => {
                    eprintln!("行太短 (长度: {}): {}", length, raw);
                }
                ParseError::InsufficientMetaFields { count, raw } => {
                    eprintln!("字段不足 (只有 {} 个): {}", count, raw);
                }
                ParseError::InvalidEpFormat { value, raw } => {
                    eprintln!("EP 格式错误 '{}' 在: {}", value, raw);
                }
                ParseError::FileNotFound { path } => {
                    eprintln!("文件未找到: {}", path);
                }
                _ => eprintln!("其他错误: {}", e),
            }
        }
    }
}
```

所有错误类型的 `Display` 实现都遵循格式：`错误描述 | raw: 原始数据`，例如：
```
invalid EP format: EPX0] | raw: EPX0] sess:123 thrd:456 user:alice trxid:0 stmt:999 appname:app
```

这使得在生产环境中快速定位问题变得更加容易。


**API 对比**：

| API | 返回类型 | 内存占用 | 适用场景 |
|-----|---------|---------|---------|
| `iter_records_from_file()` | `RecordParser<BufReader<File>>` | 低（流式） | 大文件（> 100MB） |
| `iter_sqllogs_from_file()` | `SqllogParser<BufReader<File>>` | 低（流式） | 大文件（> 100MB） |
| `parse_records_from_file()` | `(Vec<Record>, Vec<io::Error>)` | 高（一次性） | 小文件（< 100MB） |
| `parse_sqllogs_from_file()` | `(Vec<Sqllog>, Vec<ParseError>)` | 高（一次性） | 小文件（< 100MB） |

**选择建议**：
- ✓ **迭代器模式** (`iter_*`)：一次只处理一条记录，支持 GB 级大文件，可使用 `.filter()`, `.take()` 等组合器
- ✓ **一次性加载** (`parse_*`)：简单直接，适合需要多次遍历或随机访问的场景

## 更多示例

查看 `examples/` 目录获取更多使用示例：

- `parse_example.rs` - 基本解析示例
- `iterator_mode.rs` - 迭代器模式示例（推荐用于大文件）
- `parse_from_file.rs` - 从文件读取和解析
- `stream_processing.rs` - 流式处理示例
- `using_parsers.rs` - 直接使用 RecordParser 和 SqllogParser
- `error_messages.rs` - 错误处理示例
- `parse_records.rs` - Record 解析示例
- `performance_demo.rs` - 性能演示

运行示例：

```bash
cargo run --example parse_example
cargo run --example iterator_mode
cargo run --example parse_from_file
cargo run --example stream_processing
```

## API 文档

完整的 API 文档请查看 [docs.rs](https://docs.rs/dm-database-parser-sqllog)。

### 主要类型

- [`Sqllog`] - 解析后的 SQL 日志结构体（包含时间戳、元数据、SQL 文本、性能指标等）
- [`Record`] - 原始日志记录结构（包含起始行和总行数）
- [`ParseError`] - 解析错误类型

### 核心解析器

- [`RecordParser`] - 记录解析迭代器，将日志文本按时间戳切分为记录
- [`SqllogParser`] - SQL 日志解析迭代器，将记录解析为 `Sqllog` 结构体

### 字符串解析 API

- [`parse_records_from_string`] - 从字符串解析为 `Record` 列表
- [`parse_sqllogs_from_string`] - 从字符串解析为 `Result<Sqllog, ParseError>` 列表

### 文件解析 API（迭代器模式）

- [`iter_records_from_file`] - 从文件读取并返回 `RecordParser` 迭代器（推荐用于大文件）
- [`iter_sqllogs_from_file`] - 从文件读取并返回 `SqllogParser` 迭代器（推荐用于大文件）

### 文件解析 API（一次性加载）

- [`parse_records_from_file`] - 从文件读取并返回 `(Vec<Record>, Vec<io::Error>)`
- [`parse_sqllogs_from_file`] - 从文件读取并返回 `(Vec<Sqllog>, Vec<ParseError>)`

### 流式处理 API（回调模式）

- [`for_each_sqllog`] - 对每个 `Sqllog` 调用回调函数（接受 `Read` trait）
- [`for_each_sqllog_in_string`] - 从字符串流式处理 `Sqllog`
- [`for_each_sqllog_from_file`] - 从文件流式处理 `Sqllog`

## 设计与注意事项

- 该库在解析时尽量借用传入的输入（`&str` / `&[u8]`），以减少分配和复制
- 所有解析结果的生命周期都绑定到输入文本，确保内存安全
- 适合处理大型日志文件，支持流式处理

## 构建与测试

```bash
# 构建
cargo build

# 运行测试
cargo test

# 运行性能测试
cargo bench

# 查看性能报告
# 详细的性能测试报告：docs/PERFORMANCE_BENCHMARK.md
# HTML 报告：target/criterion/report/index.html

# 运行示例
cargo run --example basic

# 生成文档
cargo doc --open
```

## 性能

- **RecordParser**: 467 MiB/s 吞吐量，每秒处理约 310 万条记录
- **SqllogParser**: 130 MiB/s 吞吐量，每秒处理约 91 万条记录
- **时间戳验证**: 纳秒级（~2.7 ns）
- **记录行识别**: 百纳秒级（~160-190 ns）
- **单条记录解析**: 微秒级（~656 ns - 1.1 µs）

对于典型的 1 GB 日志文件（约 400 万条记录）：
- RecordParser: ~1.3 秒
- SqllogParser: ~4.4 秒

详细性能测试报告请查看：**[docs/PERFORMANCE_BENCHMARK.md](docs/PERFORMANCE_BENCHMARK.md)**

## 测试

本项目包含了全面的测试套件：

- **130 个测试用例**：单元测试 + 集成测试 + 性能回归测试 + 边界情况测试 + API 覆盖率测试
- **50+ 个基准场景**：使用 Criterion.rs 进行性能基准测试
- **100% 通过率**：所有测试当前状态均为通过
- **98.47% 代码覆盖率**：远超 80% 的行业标准

### 运行测试

```bash
# 运行所有测试
cargo test --all-targets

# 运行性能回归测试（必须使用 release 模式）
cargo test --test performance_regression --release

# 运行基准测试
cargo bench

# 生成覆盖率报告
cargo llvm-cov --all-features --workspace
```

### 测试类型

- **单元测试 (79个)**：测试各个模块的功能
- **集成测试 (11个)**：端到端场景测试
- **性能回归测试 (7个)**：确保性能不退化
- **边界情况测试 (12个)**：测试边界条件和错误处理
- **API 覆盖率测试 (21个)**：确保所有 API 都被测试

详细测试文档请查看：**[docs/TESTING.md](docs/TESTING.md)**
代码覆盖率报告请查看：**[docs/COVERAGE.md](docs/COVERAGE.md)**

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件


## 相关链接

- [Crates.io](https://crates.io/crates/dm-database-parser-sqllog)
- [文档](https://docs.rs/dm-database-parser-sqllog)
- [GitHub](https://github.com/guangl/dm-parser-sqllog)
- [性能测试报告](docs/PERFORMANCE_BENCHMARK.md)
