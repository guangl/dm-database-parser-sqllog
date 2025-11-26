//! 便捷 API 函数
//!
//! 提供了一组方便使用的高层 API，用于快速解析 SQL 日志。

use crate::error::ParseError;
use crate::parser::record_parser::RecordParser;
use crate::sqllog::Sqllog;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

// SqllogIterator 已移入 record_parser.rs 并非公共导出

/// 从文件读取并返回 Sqllog 迭代器（流式处理）
///
/// 这是一个便捷函数，从文件读取日志并返回 `SqllogIterator` 迭代器。
/// 使用迭代器可以避免一次性加载所有数据到内存，适合处理大文件。
///
/// # 参数
///
/// * `path` - 日志文件路径
///
/// # 返回
///
/// * `Ok(Iterator)` - 返回一个用于流式解析的迭代器，迭代项是 `Result<Sqllog, ParseError>`
/// * `Err(ParseError)` - 文件打开错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::iter_records_from_file;
///
/// let parser = iter_records_from_file("sqllog.txt");
///
/// let mut sqllog_count = 0;
/// let mut error_count = 0;
///
/// for result in parser {
///     match result {
///         Ok(sqllog) => {
///             sqllog_count += 1;
///             println!("Sqllog {}: 用户={}, SQL={}",
///                 sqllog_count, sqllog.meta.username, sqllog.body);
///         }
///         Err(err) => {
///             error_count += 1;
///             eprintln!("错误 {}: {}", error_count, err);
///         }
///     }
/// }
///
/// println!("成功: {} 条, 错误: {} 个", sqllog_count, error_count);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn iter_records_from_file<P>(path: P) -> Box<dyn Iterator<Item = Result<Sqllog, ParseError>>>
where
    P: AsRef<Path>,
{
    let path_ref = path.as_ref();
    match File::open(path_ref) {
        Ok(file) => {
            let reader = BufReader::new(file);
            let record_parser = RecordParser::new(reader);
            // 返回一个隐藏的具体迭代器实现（crate 内部定义）
            Box::new(crate::parser::record_parser::SqllogIterator::new(record_parser))
        }
        Err(e) => Box::new(std::iter::once(Err(ParseError::FileNotFound {
            path: format!("{}: {}", path_ref.display(), e),
        }))),
    }
}

/// 从文件读取并并行解析为 Sqllog（高性能版本）
///
/// 此函数使用并行处理，将日志文件解析为 Sqllog 列表。
/// 适合处理大文件（GB 级别），先识别所有记录，然后并行解析。
///
/// # 性能
///
/// - 1GB 文件（300万条记录）：约 2.5-2.7 秒
/// - 与流式处理性能相当（流式也使用批量并行优化）
/// - 内存使用：批量加载所有记录到内存
///
/// # 参数
///
/// * `path` - 日志文件路径
///
/// # 返回
///
/// * `Ok((Vec<Sqllog>, Vec<ParseError>))` - 成功解析的 Sqllog 和遇到的错误
/// * `Err(ParseError)` - 文件打开错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::parse_records_from_file;
///
/// let (sqllogs, errors) = parse_records_from_file("large_log.txt");
///
/// println!("成功解析 {} 条 SQL 日志", sqllogs.len());
/// println!("遇到 {} 个错误", errors.len());
///
/// for sqllog in sqllogs.iter().take(10) {
///     println!("用户: {}, SQL: {}", sqllog.meta.username, sqllog.body);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_records_from_file<P>(path: P) -> (Vec<Sqllog>, Vec<ParseError>)
where
    P: AsRef<Path>,
{
    // 直接使用流式迭代器收集所有结果（内部已经使用批量并行优化）
    let mut sqllogs = Vec::new();
    let mut errors = Vec::new();

    for result in iter_records_from_file(path) {
        match result {
            Ok(sqllog) => sqllogs.push(sqllog),
            Err(err) => errors.push(err),
        }
    }

    (sqllogs, errors)
}
