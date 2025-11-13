//! 便捷 API 函数
//!
//! 提供了一组方便使用的高层 API，用于快速解析 SQL 日志。

use crate::error::ParseError;
use crate::parser::record::Record;
use crate::parser::record_parser::RecordParser;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// 从文件读取并返回 Record 迭代器（流式处理）
///
/// 这是一个便捷函数，从文件读取日志并返回 `RecordParser` 迭代器。
/// 使用迭代器可以避免一次性加载所有数据到内存，适合处理大文件。
///
/// # 参数
///
/// * `path` - 日志文件路径
///
/// # 返回
///
/// * `Ok(RecordParser<BufReader<File>>)` - Record 迭代器
/// * `Err(ParseError)` - 文件打开错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::iter_records_from_file;
///
/// let parser = iter_records_from_file("sqllog.txt")?;
///
/// let mut record_count = 0;
/// let mut error_count = 0;
///
/// for result in parser {
///     match result {
///         Ok(record) => {
///             record_count += 1;
///             println!("记录 {}: {}", record_count, record.start_line());
///         }
///         Err(err) => {
///             error_count += 1;
///             eprintln!("错误 {}: {}", error_count, err);
///         }
///     }
/// }
///
/// println!("成功: {} 条, 错误: {} 个", record_count, error_count);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn iter_records_from_file<P>(path: P) -> Result<RecordParser<BufReader<File>>, ParseError>
where
    P: AsRef<Path>,
{
    let path_ref = path.as_ref();
    let file = File::open(path_ref).map_err(|e| ParseError::FileNotFound {
        path: format!("{}: {}", path_ref.display(), e),
    })?;
    let reader = BufReader::new(file);
    Ok(RecordParser::new(reader))
}

/// 从文件读取并收集所有 Records 和错误（内存模式）
///
/// ⚠️ **警告**：此函数会将所有结果加载到内存中，不适合处理大文件。
/// 对于大文件，请使用 `iter_records_from_file()` 返回的迭代器。
///
/// # 参数
///
/// * `path` - 日志文件路径
///
/// # 返回
///
/// * `Ok((Vec<Record>, Vec<std::io::Error>))` - 成功解析的 records 和遇到的错误
/// * `Err(ParseError)` - 文件打开错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::parse_records_from_file;
///
/// // 仅适用于小文件
/// let (records, errors) = parse_records_from_file("small_log.txt")?;
///
/// println!("成功解析 {} 条记录", records.len());
/// println!("遇到 {} 个错误", errors.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_records_from_file<P>(path: P) -> Result<(Vec<Record>, Vec<std::io::Error>), ParseError>
where
    P: AsRef<Path>,
{
    let mut records = Vec::new();
    let mut errors = Vec::new();

    for result in iter_records_from_file(path)? {
        match result {
            Ok(record) => records.push(record),
            Err(err) => errors.push(err),
        }
    }

    Ok((records, errors))
}
