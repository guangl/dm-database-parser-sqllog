# 实时解析功能快速参考

## 快速开始

```rust
use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};

// 1. 创建解析器
let mut parser = RealtimeParser::new(ParserConfig::default())?;

// 2. 解析新记录
parser.parse_new_records(|parsed| {
    println!("{}: {}", parsed.user, parsed.body);
})?;
```

## 常用场景

### 场景 1: 定期检查新日志

```rust
use std::time::Duration;

let config = ParserConfig {
    file_path: "sqllog.log".into(),
    poll_interval: Duration::from_secs(1),
    buffer_size: 8192,
};

let mut parser = RealtimeParser::new(config)?;

loop {
    let count = parser.parse_new_records(|parsed| {
        // 处理新记录
    })?;

    if count > 0 {
        println!("处理了 {} 条新记录", count);
    }

    std::thread::sleep(Duration::from_secs(5));
}
```

### 场景 2: 持续监听（自动轮询）

```rust
let mut parser = RealtimeParser::new(config)?;

// 自动每秒检查一次，有新记录时立即处理
parser.watch(|parsed| {
    println!("实时: {}", parsed.body);
})?; // 阻塞
```

### 场景 3: 保存和恢复进度

```rust
// 保存进度
let position = parser.position();
std::fs::write("progress.txt", position.to_string())?;

// 恢复进度
let position: u64 = std::fs::read_to_string("progress.txt")?.parse()?;
parser.seek_to(position);
```

## API 方法

| 方法 | 功能 | 返回值 |
|------|------|--------|
| `new(config)` | 创建解析器 | `Result<RealtimeParser>` |
| `parse_new_records(callback)` | 解析新增记录 | `Result<usize>` |
| `parse_all(callback)` | 从头解析所有记录 | `Result<usize>` |
| `watch(callback)` | 持续监听（阻塞） | `Result<()>` |
| `seek_to(position)` | 设置文件位置 | `()` |
| `position()` | 获取当前位置 | `u64` |
| `reset()` | 重置状态 | `()` |

## 运行示例和测试

```bash
# 运行示例
cargo run --example realtime

# 运行测试
cargo test --test integration_realtime

# 运行所有测试
cargo test
```

## 配置参数

```rust
ParserConfig {
    file_path: PathBuf,      // 日志文件路径
    poll_interval: Duration,  // 轮询间隔（默认 1秒）
    buffer_size: usize,      // 缓冲区大小（默认 8KB）
}
```

## 性能建议

- 小文件（<1MB）：`buffer_size = 4096` (4KB)
- 中等文件（1-100MB）：`buffer_size = 8192` (8KB，默认)
- 大文件（>100MB）：`buffer_size = 16384` (16KB) 或更大
- 高频更新：`poll_interval = Duration::from_millis(500)`
- 低频更新：`poll_interval = Duration::from_secs(5)`
