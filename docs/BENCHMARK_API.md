# API Benchmark 测试报告

本文档记录了 dm-database-parser-sqllog 库对外公开 API 的性能测试结果。

## 测试环境

- Rust 版本: 2024 edition
- 测试工具: criterion 0.5
- 样本数: 10
- 测试数据: sqllogs 目录下的两个真实日志文件

## 测试的 API

### 1. `iter_records_from_file` - 迭代式读取

从文件中迭代读取 Record，适合流式处理大文件。

**性能结果：**

| 文件 | 平均时间 | 范围 |
|------|---------|------|
| dmsql_DSC0_20250812_092516.log | 582.79 ms | 556.04 ms - 609.61 ms |
| dmsql_OASIS_DB1_20251020_151030.log | 7.98 s | 7.11 s - 8.81 s |

### 2. `parse_records_from_file` - 批量读取

一次性将所有 Record 读入内存，适合小文件的批量处理。

**性能结果：**

| 文件 | 平均时间 | 范围 |
|------|---------|------|
| dmsql_DSC0_20250812_092516.log | 659.45 ms | 632.86 ms - 692.14 ms |
| dmsql_OASIS_DB1_20251020_151030.log | 7.03 s | 6.23 s - 8.47 s |

### 3. `RecordParser` - 流式解析器

直接使用底层的流式解析器，提供最大的灵活性。

**性能结果：**

| 文件 | 平均时间 | 范围 |
|------|---------|------|
| dmsql_DSC0_20250812_092516.log | 259.85 ms | 234.02 ms - 308.66 ms |
| dmsql_OASIS_DB1_20251020_151030.log | 5.91 s | 4.93 s - 6.99 s |

## API 对比分析

使用 dmsql_DSC0_20250812_092516.log 进行直接对比：

| API | 平均时间 | 相对性能 |
|-----|---------|---------|
| RecordParser (直接使用) | 259.85 ms | **最快 (1.0x)** |
| parse_records_from_file | 275.81 ms | 1.06x |
| iter_records_from_file | 358.52 ms | 1.38x |

## 结论与建议

### 性能总结

1. **RecordParser 最快**: 直接使用 `RecordParser` 性能最好，因为它避免了额外的包装层
2. **parse_records_from_file 次之**: 批量读取的性能接近 RecordParser，适合中小型文件
3. **iter_records_from_file 最慢**: 迭代式 API 有一定开销，但仍然保持在可接受范围内

### 使用建议

- **大文件处理** (>100MB): 使用 `RecordParser` 或 `iter_records_from_file`，避免一次性加载全部内容到内存
- **中小文件** (<100MB): 使用 `parse_records_from_file`，代码更简洁，性能也很好
- **需要自定义控制**: 使用 `RecordParser`，提供了最大的灵活性和最佳性能

### 性能特点

所有 API 都包含了完整的解析流程（Record 分割 + Sqllog 解析），实际性能差异主要来自：

1. **内存分配**: `parse_records_from_file` 一次性分配所有 Record 的内存
2. **迭代器开销**: `iter_records_from_file` 有额外的迭代器包装开销
3. **直接访问**: `RecordParser` 直接访问底层实现，无额外开销

## 运行 Benchmark

```bash
# 运行所有 benchmark
cargo bench --bench api_bench

# 只运行特定 benchmark
cargo bench --bench api_bench -- iter_records_from_file

# 查看 HTML 报告
# 报告位于: target/criterion/report/index.html
```

## 测试文件说明

测试使用 sqllogs 目录下的真实达梦数据库日志文件：

- **dmsql_DSC0_20250812_092516.log**: 中等大小的日志文件，包含完整的 SQL 执行记录
- **dmsql_OASIS_DB1_20251020_151030.log**: 大型日志文件，用于测试大文件处理性能

每个测试都会：
1. 读取并分割 Record
2. 将每个 Record 解析为 Sqllog 结构
3. 验证解析成功的记录数量

这样确保测试的是完整的端到端性能，而不仅仅是 I/O 性能。
