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
            Err(e) => Some(Err(ParseError::FileNotFound {
                path: e.to_string(),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_sqllog_parser_basic() {
        let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n";
        let cursor = Cursor::new(input);
        let parser = SqllogParser::new(cursor);

        let results: Vec<_> = parser.collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        let sqllog = results[0].as_ref().unwrap();
        assert_eq!(sqllog.ts, "2025-08-12 10:57:09.548");
        assert_eq!(sqllog.meta.username, "alice");
        assert_eq!(sqllog.body, "SELECT 1");
    }

    #[test]
    fn test_sqllog_parser_with_indicators() {
        let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.\n";
        let cursor = Cursor::new(input);
        let parser = SqllogParser::new(cursor);

        let results: Vec<_> = parser.collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        let sqllog = results[0].as_ref().unwrap();
        assert!(sqllog.has_indicators());
        assert_eq!(sqllog.execute_time(), Some(10.5));
        assert_eq!(sqllog.row_count(), Some(100));
        assert_eq!(sqllog.execute_id(), Some(12345));
    }

    #[test]
    fn test_sqllog_parser_multiline() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
WHERE id = 1
"#;
        let cursor = Cursor::new(input);
        let parser = SqllogParser::new(cursor);

        let results: Vec<_> = parser.collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        let sqllog = results[0].as_ref().unwrap();
        assert!(sqllog.body.contains("SELECT *"));
        assert!(sqllog.body.contains("FROM users"));
        assert!(sqllog.body.contains("WHERE id = 1"));
    }

    #[test]
    fn test_sqllog_parser_empty() {
        let input = "";
        let cursor = Cursor::new(input);
        let parser = SqllogParser::new(cursor);

        let results: Vec<_> = parser.collect();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_sqllog_parser_multiple_records() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2
"#;
        let cursor = Cursor::new(input);
        let parser = SqllogParser::new(cursor);

        let results: Vec<_> = parser.collect();
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());

        assert_eq!(results[0].as_ref().unwrap().meta.username, "alice");
        assert_eq!(results[1].as_ref().unwrap().meta.username, "bob");
    }
}
