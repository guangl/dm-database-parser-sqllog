# 实时 SQL 日志解析功能

本 PR 添加了实时监控和解析 SQL 日志文件的功能。

## 功能特性

- ✅ **文件监控**: 使用 `notify` crate 监控文件变化
- ✅ **增量读取**: 只读取新增的内容，提高效率
- ✅ **实时解析**: 自动解析新增的日志记录
- ✅ **回调处理**: 支持自定义回调函数处理每条日志
- ✅ **灵活配置**: 可选择从文件开头或末尾开始监控
- ✅ **定时停止**: 支持监控指定时长后自动停止

## 使用方法

### 1. 启用 `realtime` feature

在 `Cargo.toml` 中添加:

```toml
[dependencies]
dm-database-parser-sqllog = { version = "0.2", features = ["realtime"] }
```

### 2. 基本用法

```rust
use dm_database_parser_sqllog::realtime::RealtimeSqllogParser;

// 创建解析器（默认从文件末尾开始监控）
let parser = RealtimeSqllogParser::new("sqllog.txt")
    .expect("Failed to create parser");

// 启动监控
parser.watch(|sqllog| {
    println!("新日志: {} - {}", sqllog.ts, sqllog.body);
}).expect("Watch failed");
```

### 3. 从文件开头开始

```rust
let parser = RealtimeSqllogParser::new("sqllog.txt")
    .unwrap()
    .from_beginning()  // 从文件开头开始解析
    .unwrap();

parser.watch(|sqllog| {
    // 处理日志...
}).unwrap();
```

### 4. 监控指定时长

```rust
use std::time::Duration;

let parser = RealtimeSqllogParser::new("sqllog.txt").unwrap();

// 监控 60 秒后自动停止
parser.watch_for(Duration::from_secs(60), |sqllog| {
    println!("日志: {}", sqllog.body);
}).unwrap();
```

## 运行示例

### 完整示例

```bash
# 创建测试文件
touch sqllog.txt

# 运行监控程序（在一个终端）
cargo run --example realtime_watch --features realtime sqllog.txt 60

# 在另一个终端追加日志
echo '2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users' >> sqllog.txt
```

### 简单示例

```bash
cargo run --example simple_realtime --features realtime
```

## API 文档

### `RealtimeSqllogParser`

主要方法:

- `new(path)` - 创建新的实时解析器
- `from_beginning()` - 从文件开头开始监控
- `watch(callback)` - 启动持续监控
- `watch_for(duration, callback)` - 监控指定时长

## 测试

运行测试:

```bash
cargo test --features realtime
```

## 性能考虑

- 使用增量读取，只处理新增内容
- 缓冲跨行记录，确保完整性
- 低开销的文件监控
- 支持高频率日志写入

## 依赖

新增依赖:

- `notify = "7.1"` - 文件系统事件监控

## 兼容性

- 可选功能，不影响现有 API
- 通过 feature flag 控制
- 向后兼容

## 未来改进

可能的改进方向:

- [ ] 支持多文件监控
- [ ] 添加日志过滤器
- [ ] 支持日志轮转
- [ ] 添加性能统计
- [ ] 支持异步 API
