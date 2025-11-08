# 测试文档

本项目包含了全面的测试套件，包括单元测试、集成测试、性能回归测试和边界情况测试。

## 测试结构

```
dm-database-parser-sqllog/
├── src/
│   ├── parser/tests.rs        # 解析器单元测试 (79个测试)
│   └── tools.rs               # 工具函数单元测试 (内嵌)
├── tests/
│   ├── integration_tests.rs   # 集成测试 (11个测试)
│   ├── performance_regression.rs # 性能回归测试 (7个测试)
│   └── edge_cases.rs          # 边界情况测试 (12个测试)
└── benches/
    ├── parser_bench.rs        # 解析器基准测试
    └── tools_bench.rs         # 工具函数基准测试
```

## 测试类型

### 1. 单元测试 (79个)

位于 `src/` 目录中，测试各个模块的功能：

**Parser 模块测试：**
- 记录解析（单行、多行、带指标）
- 元数据解析（有/无 IP）
- SQL 主体提取
- 指标提取和解析
- EP 字段解析
- 错误处理

**Tools 模块测试：**
- 时间戳验证
- 记录起始行检测
- 字段长度验证

运行单元测试：
```bash
cargo test --lib
```

### 2. 集成测试 (11个)

位于 `tests/integration_tests.rs`，测试端到端场景：

- ✅ 字符串解析完整流程
- ✅ 多行 SQL 解析
- ✅ 性能指标处理
- ✅ 文件读取（迭代器模式）
- ✅ 大文件处理（1000+ 条记录）
- ✅ 混合有效/无效行
- ✅ 空输入处理
- ✅ 特殊字符支持
- ✅ 超长 SQL（1000 列）
- ✅ 并发解析（10 线程）
- ✅ 仅无效行

运行集成测试：
```bash
cargo test --test integration_tests
```

### 3. 性能回归测试 (7个)

位于 `tests/performance_regression.rs`，确保性能不退化：

| 测试场景 | 性能目标 | 状态 |
|---------|---------|------|
| 1000 条单行记录解析 | < 100ms | ✅ |
| 1000 条 Sqllog 解析 | < 200ms | ✅ |
| 10000 条记录迭代 | < 1s | ✅ |
| 500 条多行 SQL | < 150ms | ✅ |
| 1000 条带指标记录 | < 250ms | ✅ |
| 50000 条记录（内存效率）| < 5s | ✅ |
| 吞吐量测试 | > 10000 条/秒 | ✅ |

运行性能回归测试（必须使用 release 模式）：
```bash
cargo test --test performance_regression --release
```

### 4. 边界情况测试 (12个)

位于 `tests/edge_cases.rs`，测试边界条件和错误处理：

- ✅ 时间戳边界（最小/最大时间，无效日期）
- ✅ EP 字段边界（0-255）
- ✅ 会话 ID 格式（十六进制、十进制）
- ✅ 用户名特殊字符
- ✅ 性能指标边界值
- ✅ SQL 语句类型（SELECT, INSERT, UPDATE等）
- ✅ 极端字段长度（空值、超长值）
- ✅ 空白字符处理
- ✅ 无效输入
- ✅ UTF-8 和 emoji 支持
- ✅ 事务 ID 特殊值
- ✅ 客户端 IP 格式（IPv4, IPv6）

运行边界情况测试：
```bash
cargo test --test edge_cases
```

### 5. 基准测试

位于 `benches/` 目录，使用 Criterion.rs 进行性能基准测试：

**parser_bench.rs:**
- 单行/多行/带指标记录解析
- RecordParser 吞吐量（10/100/1000/10000 条）
- SqllogParser 吞吐量（10/100/1000/10000 条）
- 混合记录处理
- 大文件处理（10k 条）

**tools_bench.rs:**
- 时间戳验证性能
- 记录起始行检测性能
- 各种行长度处理
- 早期退出优化

运行基准测试：
```bash
cargo bench
```

## 运行所有测试

```bash
# 运行所有测试（单元测试 + 集成测试）
cargo test --all-targets

# 运行所有测试 + 基准测试
cargo test --all-targets && cargo bench
```

## 测试覆盖率

当前测试统计：
- **单元测试**: 79 个
- **集成测试**: 11 个
- **性能回归测试**: 7 个
- **边界情况测试**: 12 个
- **基准测试**: 50+ 个场景

**总计**: 109 个测试用例 + 50+ 个基准场景

所有测试当前状态：✅ 100% 通过

## CI/CD 集成

建议在 CI/CD 流程中运行以下命令：

```bash
# 1. 运行所有测试
cargo test --all-targets

# 2. 运行性能回归测试（release 模式）
cargo test --test performance_regression --release

# 3. 确保没有未使用的依赖
cargo clean && cargo build --release
```

## 添加新测试

### 单元测试
在相应的模块文件中添加 `#[cfg(test)]` 模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        // 测试代码
    }
}
```

### 集成测试
在 `tests/` 目录下创建新文件或添加到现有文件：

```rust
use dm_database_parser_sqllog::*;

#[test]
fn test_my_integration() {
    // 测试代码
}
```

### 性能测试
添加到 `tests/performance_regression.rs`，设置时间限制：

```rust
#[test]
fn perf_my_test() {
    let start = Instant::now();
    // 执行操作
    assert!(start.elapsed() < Duration::from_millis(100));
}
```

### 基准测试
添加到 `benches/parser_bench.rs` 或创建新文件：

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn my_benchmark(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| {
            // 被测代码
        })
    });
}

criterion_group!(benches, my_benchmark);
criterion_main!(benches);
```

## 故障排查

### 性能测试失败
如果性能回归测试失败，检查：
1. 是否使用了 `--release` 模式
2. 系统负载是否过高
3. 是否有其他程序占用 CPU

### 基准测试结果不稳定
- 关闭其他应用程序
- 使用 `--sample-size` 增加采样数
- 检查电源设置（禁用节能模式）

## 测试最佳实践

1. **每次提交前运行测试**: `cargo test --all-targets`
2. **定期运行基准测试**: 检测性能退化
3. **添加新功能时**: 同时添加相应测试
4. **修复 bug 时**: 添加回归测试
5. **重构代码时**: 确保所有测试通过
