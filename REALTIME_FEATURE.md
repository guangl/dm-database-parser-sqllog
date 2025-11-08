# 实时解析功能

本分支添加了实时解析 sqllog 文件的功能，支持增量读取和持续监听日志变化。

## 新增功能

### 核心特性

- ✅ **增量解析**：只读取和解析文件中的新增内容
- ✅ **状态维护**：自动跟踪文件读取位置
- ✅ **持续监听**：可以在后台持续监控文件变化
- ✅ **零拷贝设计**：保持原有的高性能特性
- ✅ **灵活配置**：可配置轮询间隔、缓冲区大小等参数

### 新增模块

- `src/realtime.rs` - 实时解析核心模块
  - `RealtimeParser` - 实时解析器结构体
  - `ParserConfig` - 解析器配置

### API 概览

```rust
use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};
use std::time::Duration;

// 创建解析器
let config = ParserConfig {
    file_path: "sqllog.log".into(),
    poll_interval: Duration::from_secs(1),
    buffer_size: 8192,
};
let mut parser = RealtimeParser::new(config)?;

// 解析新增记录
parser.parse_new_records(|parsed| {
    println!("用户: {}, SQL: {}", parsed.user, parsed.body);
})?;
```

## 使用示例

### 1. 增量解析

适用于定期检查日志文件是否有新增内容：

```rust
use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};

let mut parser = RealtimeParser::new(ParserConfig::default())?;

// 第一次解析
let count1 = parser.parse_new_records(|parsed| {
    println!("记录: {}", parsed.body);
})?;
println!("解析了 {} 条记录", count1);

// ... 文件有新内容写入 ...

// 第二次解析（只处理新增部分）
let count2 = parser.parse_new_records(|parsed| {
    println!("新记录: {}", parsed.body);
})?;
println!("解析了 {} 条新记录", count2);
```

### 2. 持续监听模式

适用于需要实时监控日志的场景：

```rust
use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};
use std::time::Duration;

let config = ParserConfig {
    file_path: "sqllog.log".into(),
    poll_interval: Duration::from_secs(1), // 每秒检查一次
    buffer_size: 8192,
};

let mut parser = RealtimeParser::new(config)?;

// 持续监听（阻塞）
parser.watch(|parsed| {
    println!("实时捕获: 用户={}, SQL={}", parsed.user, parsed.body);
})?;
```

### 3. 从指定位置开始

适用于恢复中断的解析任务：

```rust
let mut parser = RealtimeParser::new(config)?;

// 从特定位置开始
parser.seek_to(1024); // 从文件偏移 1024 字节处开始

parser.parse_new_records(|parsed| {
    // 处理记录
})?;

// 获取当前位置（用于保存状态）
let current_pos = parser.position();
```

### 4. 完整解析

需要从头解析整个文件时：

```rust
let mut parser = RealtimeParser::new(config)?;

let total = parser.parse_all(|parsed| {
    println!("记录: {}", parsed.body);
})?;
println!("总共解析了 {} 条记录", total);
```

## 运行示例

本分支包含一个完整的实时解析示例：

```bash
cargo run --example realtime
```

示例展示了三种使用场景：
1. 增量解析模式
2. 从头完整解析
3. 持续监听模式（演示 5 秒）

## 运行测试

运行实时解析相关测试：

```bash
# 运行所有测试
cargo test

# 只运行实时解析集成测试
cargo test --test integration_realtime

# 查看测试输出
cargo test --test integration_realtime -- --nocapture
```

测试覆盖：
- ✅ 基本解析功能
- ✅ 增量解析
- ✅ 完整解析
- ✅ 位置定位和重置
- ✅ 空文件处理
- ✅ 文件不存在错误处理
- ✅ 多次增量读取
- ✅ 大缓冲区处理

## 性能特性

- **低内存占用**：采用流式处理，不会一次性加载整个文件
- **零拷贝解析**：继承原有解析器的零拷贝特性
- **可配置缓冲区**：根据场景调整缓冲区大小优化性能
- **状态缓存**：自动处理跨读取边界的不完整记录

## 配置选项

```rust
pub struct ParserConfig {
    /// 日志文件路径
    pub file_path: PathBuf,
    /// 轮询间隔（用于监听模式）
    pub poll_interval: Duration,
    /// 读取缓冲区大小（字节）
    pub buffer_size: usize,
}
```

默认配置：
- `file_path`: `"sqllog.log"`
- `poll_interval`: `1秒`
- `buffer_size`: `8192字节` (8KB)

## 使用场景

1. **数据库监控**：实时监控数据库 SQL 执行情况
2. **性能分析**：持续收集慢查询和执行统计
3. **安全审计**：实时检测可疑的数据库操作
4. **日志采集**：作为日志采集系统的数据源
5. **增量导入**：定期增量导入日志到分析系统

## 注意事项

1. `watch()` 方法会阻塞当前线程，建议在单独的线程或异步任务中使用
2. 文件轮询会消耗一定的系统资源，合理设置 `poll_interval`
3. 对于特别大的文件，建议适当增加 `buffer_size` 以提高性能
4. 解析器会自动处理不完整记录（跨读取边界的记录）

## 后续优化方向

- [ ] 支持异步 I/O (tokio)
- [ ] 使用系统文件监听 API（如 inotify、FSEvents）替代轮询
- [ ] 添加错误恢复和重试机制
- [ ] 支持日志轮转检测
- [ ] 提供性能监控指标

## 代码结构

```
src/
├── realtime.rs         # 实时解析核心模块
├── lib.rs              # 导出 realtime 模块
└── error.rs            # 添加 FileNotFound 错误

examples/
└── realtime.rs         # 实时解析完整示例

tests/
└── integration_realtime.rs  # 集成测试
```

## 与现有 API 的兼容性

实时解析功能作为新增模块，完全不影响现有的解析 API：

- `parse_record()` - 单记录解析
- `parse_all()` - 批量解析
- `for_each_record()` - 流式处理
- `RecordSplitter` - 记录切分迭代器

所有现有功能和测试保持不变，可以放心使用。
