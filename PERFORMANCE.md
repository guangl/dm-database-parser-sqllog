# 性能测试报告

本文档描述了库的性能测试方法和结果。

## 基准测试套件

项目包含两个主要的基准测试套件：

### 1. parser_bench.rs

核心解析功能的基准测试，包括：

- **记录拆分性能**：测试 `split_by_ts_records_with_errors` 在不同大小日志文件上的性能
- **RecordSplitter 性能**：测试迭代器的创建和迭代性能
- **单条记录解析**：测试 `parse_record` 的性能
- **批量解析**：测试 `parse_all` 的性能
- **流式处理**：测试 `for_each_record` 和 `parse_records_with` 的性能
- **API 对比**：比较不同 API 的性能差异

### 2. performance_test.rs

性能和压力测试，包括：

- **大型文件解析**：测试处理 1000 到 100,000 条记录的性能
- **内存效率**：验证零分配特性
- **不同记录类型**：测试各种 SQL 操作类型的解析性能

## 运行基准测试

```bash
# 运行所有基准测试
cargo bench

# 运行特定基准测试
cargo bench --bench parser_bench
cargo bench --bench performance_test

# 快速测试（减少迭代次数）
cargo bench --bench parser_bench -- --quick

# 生成 HTML 报告
cargo bench --bench parser_bench
# 报告保存在 target/criterion/ 目录
```

## 性能指标

基准测试测量以下指标：

- **执行时间**：每个操作的耗时（纳秒、微秒、毫秒）
- **吞吐量**：每秒处理的记录数
- **内存分配**：验证零分配特性（通过对比不同 API）

## 持续集成

基准测试在以下情况自动运行：

- 推送到 main/master 分支
- 创建 Pull Request
- 每周定时运行（周一）
- 手动触发

测试结果会作为 artifacts 上传到 GitHub Actions，可以下载查看详细的 HTML 报告。

## 性能优化建议

基于基准测试结果，以下是一些性能优化建议：

1. **使用流式 API**：对于大型日志文件，使用 `for_each_record` 或 `parse_records_with` 而不是 `parse_all`
2. **重用缓冲区**：在循环中处理多个文件时，使用 `split_into` 和 `parse_into` 重用缓冲区
3. **避免不必要的解析**：如果只需要记录切分，使用 `RecordSplitter` 而不是完整解析

## 性能目标

- **小文件（< 100 条记录）**：< 50µs
- **中等文件（100-1000 条记录）**：< 500µs
- **大文件（1000-10000 条记录）**：< 5ms
- **超大文件（> 10000 条记录）**：线性扩展，无明显性能退化
