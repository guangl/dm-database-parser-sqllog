# dm-database-parser-sqllog

这是一个用于解析 dm（或类似）数据库产生的 sqllog 文本日志的 Rust 库。它提供了零或低内存分配的记录切分与解析工具，适合在高吞吐日志处理场景中使用。

## 主要特点
- 基于时间戳的记录切分（无额外分配的流式 API）；
- 使用双数组 Aho-Corasick（daachorse）进行高效模式匹配以识别记录起始行；
- 将每条记录解析为轻量引用（&str）的结构体，尽量避免复制；
- 提供方便的批量与流式解析接口，方便在不同场景下复用。

## 快速开始

前提
- 已安装 Rust 工具链（建议 stable）。

构建与测试

```bash
# 在项目根目录
cargo build
cargo test
```

## 示例用法

下面示例展示如何把整个日志文本拆分并解析：

```rust
use dm_database_parser_sqllog::{split_by_ts_records_with_errors, parse_record, for_each_record};

let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:joe trxid:0 stmt:1 appname:MyApp) SELECT 1"#;

// 将文本拆分为记录并获取前导错误
let (records, errors) = split_by_ts_records_with_errors(log_text);
println!("records: {:?}, leading errors: {:?}", records.len(), errors.len());

// 流式处理每条记录（零分配）
for_each_record(log_text, |rec| {
    let parsed = parse_record(rec);
    println!("ts={} body={}", parsed.ts, parsed.body);
});
```

## 导出 API 摘要

以下符号在 `crate` 根处导出并可直接使用：

- `ParseError` - 解析相关错误类型（从文本解析数值时的错误）。
- `split_by_ts_records_with_errors(text: &str) -> (Vec<&str>, Vec<&str>)` - 把完整日志拆成记录切片，并返回前导错误行。
- `split_into(text, records: &mut Vec<&str>, errors: &mut Vec<&str>)` - 将结果写入调用者提供的容器以避免分配。
- `for_each_record(text, f)` - 流式遍历记录并对每条调用回调（零分配）。
- `parse_records_with(text, f)` - 解析记录并通过回调返回 `ParsedRecord`（借用输入）。
- `parse_all(text) -> Vec<ParsedRecord<'_>>` - 将所有记录解析并返回 Vec（会分配）。
- `Sqllog` - 一个简单的可构建的记录结构（crate 内部类型）。

设计与注意事项

- 该库在解析时尽量借用传入的输入（&str / &[u8]），以减少分配和复制。
