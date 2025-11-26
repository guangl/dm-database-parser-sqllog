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
/// * `Iterator<Item = Result<Sqllog, ParseError>>` - 返回一个用于流式解析的迭代器，迭代项可能包含 `ParseError`，例如文件打开失败或解析错误
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
            Box::new(crate::parser::record_parser::SqllogIterator::new(
                record_parser,
            ))
        }
        Err(e) => Box::new(std::iter::once(Err(ParseError::FileNotFound {
            path: format!("{}: {}", path_ref.display(), e),
        }))),
    }
}
