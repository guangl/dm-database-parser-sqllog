//! SqllogParser - 将 RecordParser 转换为 Sqllog 迭代器
//!
//! 提供了一个适配器，将原始的 `Record` 流转换为解析后的 `Sqllog` 流。

use crate::error::ParseError;
use crate::parser::record_parser::RecordParser;
use crate::sqllog::Sqllog;
use std::io::Read;

/// 将 RecordParser 转换为 Sqllog 迭代器的适配器
///
/// `SqllogParser` 在 `RecordParser` 的基础上，自动将每个 `Record` 解析为 `Sqllog`。
///
/// # 类型参数
///
/// * `R` - 实现了 `Read` trait 的类型
///
/// # 示例
///
/// ```no_run
/// use dm_database_parser_sqllog::SqllogParser;
/// use std::fs::File;
///
/// let file = File::open("sqllog.txt").unwrap();
/// let parser = SqllogParser::new(file);
///
/// for result in parser {
///     match result {
///         Ok(sqllog) => println!("SQL: {}", sqllog.body),
///         Err(e) => eprintln!("解析错误: {}", e),
///     }
/// }
/// ```
pub struct SqllogParser<R: Read> {
    record_parser: RecordParser<R>,
}

impl<R: Read> SqllogParser<R> {
    /// 创建新的 SqllogParser
    ///
    /// # 参数
    ///
    /// * `reader` - 任何实现了 `Read` trait 的类型
    ///
    /// # 返回
    ///
    /// 返回一个新的 `SqllogParser` 实例
    pub fn new(reader: R) -> Self {
        Self {
            record_parser: RecordParser::new(reader),
        }
    }
}

impl<R: Read> Iterator for SqllogParser<R> {
    type Item = Result<Sqllog, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.record_parser.next()? {
            Ok(record) => Some(record.parse_to_sqllog()),
            Err(e) => Some(Err(ParseError::FileNotFound(e.to_string()))),
        }
    }
}
