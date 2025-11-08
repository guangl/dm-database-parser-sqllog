# dm-database-parser-sqllog

[![Crates.io](https://img.shields.io/crates/v/dm-database-parser-sqllog.svg)](https://crates.io/crates/dm-database-parser-sqllog)
[![Documentation](https://docs.rs/dm-database-parser-sqllog/badge.svg)](https://docs.rs/dm-database-parser-sqllog)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

一个高性能的达梦数据库 sqllog 日志解析库，提供零分配或低分配的记录切分与解析功能。

## 主要特点

- **零分配解析**：基于时间戳的记录切分，使用流式 API 避免额外内存分配
- **高效模式匹配**：使用双数组 Aho-Corasick（daachorse）进行高效模式匹配
- **轻量级结构**：解析结果使用引用（`&str`），避免不必要的字符串复制
- **灵活的 API**：提供批量解析、流式解析等多种使用方式
- **高性能**：适合在高吞吐日志处理场景中使用

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
dm-database-parser-sqllog = "0.1"
```

## 快速开始

### 基本用法

```rust
use dm_database_parser_sqllog::{split_by_ts_records_with_errors, parse_record};

let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1"#;

// 将文本拆分为记录并获取前导错误
let (records, errors) = split_by_ts_records_with_errors(log_text);
println!("找到 {} 条记录，{} 条前导错误", records.len(), errors.len());

// 解析记录
for record in records {
    let parsed = parse_record(record);
    println!("用户: {}, 事务ID: {}, 执行时间: {:?}ms",
             parsed.user, parsed.trxid, parsed.execute_time_ms);
}
```

### 流式处理（零分配）

```rust
use dm_database_parser_sqllog::{for_each_record, parse_records_with};

let log_text = r#"..."#; // 大量日志文本

// 方法 1: 流式处理记录
for_each_record(log_text, |rec| {
    println!("记录: {}", rec.lines().next().unwrap_or(""));
});

// 方法 2: 流式解析记录
parse_records_with(log_text, |parsed| {
    println!("用户: {}, 事务ID: {}", parsed.user, parsed.trxid);
});
```

### 重用缓冲区（避免重复分配）

```rust
use dm_database_parser_sqllog::{split_into, parse_into};

let mut records = Vec::new();
let mut errors = Vec::new();
let mut parsed_records = Vec::new();

// 在循环中重用缓冲区
for log_text in log_files {
    split_into(log_text, &mut records, &mut errors);
    parse_into(log_text, &mut parsed_records);
    // 处理解析结果...
}
```

## 更多示例

查看 `examples/` 目录获取更多使用示例：

- `basic.rs` - 基本使用示例
- `streaming.rs` - 流式处理示例
- `reuse_buffers.rs` - 重用缓冲区示例

运行示例：

```bash
cargo run --example basic
cargo run --example streaming
cargo run --example reuse_buffers
```

## API 文档

完整的 API 文档请查看 [docs.rs](https://docs.rs/dm-database-parser-sqllog)。

### 主要类型和函数

- [`ParsedRecord`] - 解析后的日志记录结构体
- [`ParseError`] - 解析错误类型
- [`RecordSplitter`] - 记录切分迭代器
- [`split_by_ts_records_with_errors`] - 拆分日志为记录和错误
- [`parse_record`] - 解析单条记录
- [`for_each_record`] - 流式处理记录
- [`parse_records_with`] - 流式解析记录

## 性能特性

- **零分配切分**：`RecordSplitter` 和 `for_each_record` 使用引用，不分配新内存
- **高效匹配**：使用 daachorse 双数组 Aho-Corasick 自动机进行 O(n) 模式匹配
- **批量处理优化**：提供 `split_into` 和 `parse_into` 以重用缓冲区

## 性能测试

项目包含完整的基准测试套件，用于验证和监控性能：

```bash
# 运行所有基准测试
cargo bench

# 运行特定基准测试
cargo bench --bench parser_bench
cargo bench --bench performance_test

# criterion 默认会生成 HTML 报告
cargo bench --bench parser_bench
# 报告保存在 target/criterion/ 目录
```

基准测试包括：
- 不同大小日志文件的解析性能（10 到 100,000 条记录）
- 各种 API 的性能对比（`parse_all` vs `for_each_record` vs `parse_records_with`）
- 内存效率验证（零分配特性）
- 大型文件处理性能

详细的基准测试结果可以在 GitHub Actions 的 artifacts 中查看，或运行 `cargo bench` 后在 `target/criterion/` 目录查看 HTML 报告。

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

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件


## 相关链接

- [Crates.io](https://crates.io/crates/dm-database-parser-sqllog)
- [文档](https://docs.rs/dm-database-parser-sqllog)
- [GitHub](https://github.com/guangl/dm-parser-sqllog)
- [性能测试报告](docs/PERFORMANCE_BENCHMARK.md)
