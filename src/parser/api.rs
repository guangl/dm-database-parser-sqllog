//! 便捷 API 函数
//!
//! 提供了一组方便使用的高层 API，用于快速解析 SQL 日志。

use crate::error::ParseError;
use crate::parser::record::Record;
use crate::parser::record_parser::RecordParser;
use crate::parser::sqllog_parser::SqllogParser;
use crate::sqllog::Sqllog;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// 从字符串解析记录
///
/// 这是一个便捷函数，将字符串内容解析为 `Record` 列表。
/// 会自动跳过无效的行，只返回成功解析的记录。
///
/// # 参数
///
/// * `content` - 包含日志内容的字符串
///
/// # 返回
///
/// 返回成功解析的 `Record` 列表
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::parse_records_from_string;
///
/// let log = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
/// 2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"#;
///
/// let records = parse_records_from_string(log);
/// assert_eq!(records.len(), 2);
/// ```
pub fn parse_records_from_string(content: &str) -> Vec<Record> {
    let cursor = std::io::Cursor::new(content.as_bytes());
    RecordParser::new(cursor).filter_map(|r| r.ok()).collect()
}

/// 从字符串直接解析为 Sqllog 列表
///
/// 这是一个便捷函数，将字符串内容解析为 `Sqllog` 列表。
/// 返回所有解析结果（包括成功和失败的）。
///
/// # 参数
///
/// * `content` - 包含日志内容的字符串
///
/// # 返回
///
/// 返回 `Result<Sqllog, ParseError>` 列表
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::parse_sqllogs_from_string;
///
/// let log = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
/// let results = parse_sqllogs_from_string(log);
///
/// for result in results {
///     match result {
///         Ok(sqllog) => println!("成功: {}", sqllog.body),
///         Err(e) => eprintln!("错误: {}", e),
///     }
/// }
/// ```
pub fn parse_sqllogs_from_string(content: &str) -> Vec<Result<Sqllog, ParseError>> {
    let cursor = std::io::Cursor::new(content.as_bytes());
    SqllogParser::new(cursor).collect()
}

/// 流式解析 Sqllog，对每个解析后的记录调用回调函数
///
/// 这个函数适合处理大文件，因为它不会将所有记录加载到内存中。
/// 它会逐条读取并解析记录，然后立即调用回调函数处理。
///
/// # 参数
///
/// * `reader` - 实现了 `Read` trait 的数据源（如 File、&[u8] 等）
/// * `callback` - 处理每个 `Sqllog` 的回调函数
///
/// # 返回
///
/// * `Ok(usize)` - 成功处理的记录数量
/// * `Err(ParseError)` - 解析或读取错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::for_each_sqllog;
/// use std::fs::File;
/// use std::io::BufReader;
///
/// let file = File::open("sqllog.log")?;
/// let reader = BufReader::new(file);
///
/// let count = for_each_sqllog(reader, |sqllog| {
///     println!("EP: {}, 用户: {}, SQL: {}",
///         sqllog.meta.ep,
///         sqllog.meta.username,
///         sqllog.body);
/// })?;
///
/// println!("处理了 {} 条记录", count);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn for_each_sqllog<R, F>(reader: R, mut callback: F) -> Result<usize, ParseError>
where
    R: Read,
    F: FnMut(&Sqllog),
{
    let parser = SqllogParser::new(reader);
    let mut count = 0;

    for result in parser {
        match result {
            Ok(sqllog) => {
                callback(&sqllog);
                count += 1;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(count)
}

/// 从字符串流式解析 Sqllog，对每个解析后的记录调用回调函数
///
/// 这是 `for_each_sqllog` 的字符串版本，适合处理内存中的日志内容。
///
/// # 参数
///
/// * `content` - 包含日志内容的字符串
/// * `callback` - 处理每个 `Sqllog` 的回调函数
///
/// # 返回
///
/// * `Ok(usize)` - 成功处理的记录数量
/// * `Err(ParseError)` - 解析错误
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::for_each_sqllog_in_string;
///
/// let log = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
/// let count = for_each_sqllog_in_string(log, |sqllog| {
///     println!("会话: {}, 用户: {}", sqllog.meta.sess_id, sqllog.meta.username);
/// }).unwrap();
/// assert_eq!(count, 1);
/// ```
pub fn for_each_sqllog_in_string<F>(content: &str, callback: F) -> Result<usize, ParseError>
where
    F: FnMut(&Sqllog),
{
    let cursor = std::io::Cursor::new(content.as_bytes());
    for_each_sqllog(cursor, callback)
}

/// 从文件路径流式解析 Sqllog
///
/// 这是一个便捷函数，直接从文件路径读取并解析日志。
///
/// # 参数
///
/// * `path` - 日志文件路径
/// * `callback` - 处理每个 `Sqllog` 的回调函数
///
/// # 返回
///
/// * `Ok(usize)` - 成功处理的记录数量
/// * `Err(ParseError)` - 解析或读取错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::for_each_sqllog_from_file;
///
/// let count = for_each_sqllog_from_file("sqllog.txt", |sqllog| {
///     println!("用户: {}, SQL: {}", sqllog.meta.username, sqllog.body);
/// })?;
///
/// println!("处理了 {} 条记录", count);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn for_each_sqllog_from_file<P, F>(path: P, callback: F) -> Result<usize, ParseError>
where
    P: AsRef<std::path::Path>,
    F: FnMut(&Sqllog),
{
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path).map_err(|e| ParseError::FileNotFound(e.to_string()))?;
    let reader = BufReader::new(file);
    for_each_sqllog(reader, callback)
}

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
    let file = File::open(path).map_err(|e| ParseError::FileNotFound(e.to_string()))?;
    let reader = BufReader::new(file);
    Ok(RecordParser::new(reader))
}

/// 从文件读取并返回 Sqllog 迭代器（流式处理）
///
/// 这是一个便捷函数，从文件读取日志并返回 `SqllogParser` 迭代器。
/// 使用迭代器可以避免一次性加载所有数据到内存，适合处理大文件。
///
/// # 参数
///
/// * `path` - 日志文件路径
///
/// # 返回
///
/// * `Ok(SqllogParser<BufReader<File>>)` - Sqllog 迭代器
/// * `Err(ParseError)` - 文件打开错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::iter_sqllogs_from_file;
///
/// let parser = iter_sqllogs_from_file("sqllog.txt")?;
///
/// let mut success_count = 0;
/// let mut error_count = 0;
///
/// for result in parser {
///     match result {
///         Ok(sqllog) => {
///             success_count += 1;
///             println!("用户: {}, SQL: {}", sqllog.meta.username, sqllog.body);
///         }
///         Err(err) => {
///             error_count += 1;
///             eprintln!("解析错误: {}", err);
///         }
///     }
/// }
///
/// println!("成功: {} 条, 错误: {} 个", success_count, error_count);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn iter_sqllogs_from_file<P>(path: P) -> Result<SqllogParser<BufReader<File>>, ParseError>
where
    P: AsRef<Path>,
{
    let file = File::open(path).map_err(|e| ParseError::FileNotFound(e.to_string()))?;
    let reader = BufReader::new(file);
    Ok(SqllogParser::new(reader))
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

/// 从文件读取并收集所有 Sqllogs 和错误（内存模式）
///
/// ⚠️ **警告**：此函数会将所有结果加载到内存中，不适合处理大文件。
/// 对于大文件，请使用 `iter_sqllogs_from_file()` 返回的迭代器。
///
/// # 参数
///
/// * `path` - 日志文件路径
///
/// # 返回
///
/// * `Ok((Vec<Sqllog>, Vec<ParseError>))` - 成功解析的 sqllogs 和遇到的错误
/// * `Err(ParseError)` - 文件打开错误
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::parse_sqllogs_from_file;
///
/// // 仅适用于小文件
/// let (sqllogs, errors) = parse_sqllogs_from_file("small_log.txt")?;
///
/// println!("成功解析 {} 条 SQL 日志", sqllogs.len());
/// println!("遇到 {} 个解析错误", errors.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_sqllogs_from_file<P>(path: P) -> Result<(Vec<Sqllog>, Vec<ParseError>), ParseError>
where
    P: AsRef<Path>,
{
    let mut sqllogs = Vec::new();
    let mut errors = Vec::new();

    for result in iter_sqllogs_from_file(path)? {
        match result {
            Ok(sqllog) => sqllogs.push(sqllog),
            Err(err) => errors.push(err),
        }
    }

    Ok((sqllogs, errors))
}

// ============================================================================
// 向后兼容的 deprecated 别名
// ============================================================================

/// 已废弃：请使用 `iter_records_from_file` 代替
///
/// 此函数已重命名为 `iter_records_from_file` 以更清晰地表达其返回迭代器的语义。
#[deprecated(since = "0.1.3", note = "请使用 `iter_records_from_file` 代替")]
pub fn records_from_file<P>(path: P) -> Result<RecordParser<BufReader<File>>, ParseError>
where
    P: AsRef<Path>,
{
    iter_records_from_file(path)
}

/// 已废弃：请使用 `iter_sqllogs_from_file` 代替
///
/// 此函数已重命名为 `iter_sqllogs_from_file` 以更清晰地表达其返回迭代器的语义。
#[deprecated(since = "0.1.3", note = "请使用 `iter_sqllogs_from_file` 代替")]
pub fn sqllogs_from_file<P>(path: P) -> Result<SqllogParser<BufReader<File>>, ParseError>
where
    P: AsRef<Path>,
{
    iter_sqllogs_from_file(path)
}
