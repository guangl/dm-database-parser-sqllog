//! 异步 API：基于 tokio 的非阻塞日志解析器。
//!
//! 通过 [`AsyncLogParser`] 在 tokio blocking 线程池中执行同步解析，
//! 暴露 `async fn parse()` 接口，不阻塞异步运行时。

use std::path::{Path, PathBuf};

use crate::error::ParseError;
use crate::filter::builder::Filter;
use crate::parser::builder::LogParserBuilder;
use crate::parser::encoding::FileEncodingHint;
use crate::record::Sqllog;

/// 异步日志解析器。
///
/// 通过 `tokio::task::spawn_blocking` 在阻塞线程池中调用同步 [`LogParserBuilder`]，
/// 对调用方暴露 `async fn parse()` 接口，不阻塞异步运行时线程。
///
/// # 示例
///
/// ```rust,no_run
/// # #[cfg(feature = "async")]
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use dm_database_parser_sqllog::AsyncLogParser;
///
/// let records = AsyncLogParser::new("sqllog.txt").parse().await?;
/// println!("解析到 {} 条记录", records.len());
/// # Ok(())
/// # }
/// ```
pub struct AsyncLogParser {
    path: PathBuf,
    encoding_hint: FileEncodingHint,
    filter: Option<Filter>,
}

impl AsyncLogParser {
    /// 创建一个新的 `AsyncLogParser`，使用默认编码（自动探测）。
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            encoding_hint: FileEncodingHint::Auto,
            filter: None,
        }
    }

    /// 设置文件编码提示，消费 self 并返回新实例（链式调用）。
    pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
        self.encoding_hint = hint;
        self
    }

    /// 设置组合过滤器，消费 self 并返回新实例（链式调用）。
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// 在阻塞线程池中解析日志文件，返回所有匹配的记录。
    ///
    /// 行为与同步 [`LogIterator`](crate::parser::mod::LogIterator) 一致：
    /// 任意一条记录解析失败即返回 `Err`，不会静默丢弃错误记录。
    ///
    /// # 错误
    ///
    /// - [`AsyncError::Parse`]：文件不存在、格式错误、或任意记录解析失败
    /// - [`AsyncError::Panic`]：阻塞任务内部 panic
    ///
    /// # Panics
    ///
    /// 若调用方不在 tokio 运行时上下文中（如在裸 `std::thread` 中直接调用），
    /// `spawn_blocking` 会 panic，此 panic 会被捕获并以 [`AsyncError::Panic`] 返回，
    /// 而非直接 unwind 调用方。
    pub async fn parse(self) -> Result<Vec<Sqllog>, AsyncError> {
        let path = self.path;
        let encoding_hint = self.encoding_hint;
        let filter = self.filter;

        tokio::task::spawn_blocking(move || -> Result<Vec<Sqllog>, ParseError> {
            let parser = LogParserBuilder::new(&path)
                .encoding_hint(encoding_hint)
                .build()?;
            let iter = parser.iter()?;
            match filter {
                Some(f) => iter.apply_filter_keep_errors(f).collect(),
                None => iter.collect(),
            }
        })
        .await
        .map_err(|e| AsyncError::Panic(e.to_string()))?
        .map_err(AsyncError::Parse)
    }
}

/// 异步解析错误。
#[derive(Debug, thiserror::Error)]
pub enum AsyncError {
    /// 底层同步解析失败。
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    /// 阻塞任务内部发生 panic。
    #[error("blocking task panicked: {0}")]
    Panic(String),
}

// ── 测试 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::builder::FilterBuilder;

    /// 构造一条最小有效日志记录字符串（单行，含性能指标）。
    fn make_record_line(exectime: f32) -> String {
        format!(
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:SYSDBA trxid:3 stmt:4 appname:app) \
            SELECT 1\nEXECTIME:{exectime}(ms) ROWCOUNT:1(rows) EXEC_ID:1."
        )
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn test_parse_returns_records() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("创建临时文件失败");
        write!(
            tmp,
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:SYSDBA trxid:3 stmt:4 appname:app) SELECT 1\nEXECTIME:0.100(ms) ROWCOUNT:1(rows) EXEC_ID:1."
        )
        .unwrap();
        tmp.as_file().sync_all().unwrap();

        let records = AsyncLogParser::new(tmp.path())
            .parse()
            .await
            .expect("parse 不应失败");
        assert_eq!(records.len(), 1);
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn test_parse_file_not_found_returns_error() {
        let result = AsyncLogParser::new("/nonexistent/no_such_file.log")
            .parse()
            .await;
        assert!(result.is_err());
        assert!(matches!(result, Err(AsyncError::Parse(_))));
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn test_parse_with_filter() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("创建临时文件失败");
        // 第一条 exectime=50，第二条 exectime=200
        write!(
            tmp,
            "{}\n{}",
            make_record_line(50.0),
            make_record_line(200.0),
        )
        .unwrap();
        tmp.as_file().sync_all().unwrap();

        let filter = FilterBuilder::new().exec_time_gt(100.0).build();
        let records = AsyncLogParser::new(tmp.path())
            .with_filter(filter)
            .parse()
            .await
            .expect("parse 不应失败");
        assert_eq!(records.len(), 1);
        assert!(records[0].exectime > 100.0);
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn test_encoding_hint_propagated() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("创建临时文件失败");
        write!(
            tmp,
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:SYSDBA trxid:3 stmt:4 appname:app) SELECT 1"
        )
        .unwrap();
        tmp.as_file().sync_all().unwrap();

        let result = AsyncLogParser::new(tmp.path())
            .encoding_hint(FileEncodingHint::Utf8)
            .parse()
            .await;
        assert!(result.is_ok(), "encoding_hint(Utf8) 解析应成功");
        let records = result.unwrap();
        assert!(!records.is_empty());
    }

    #[test]
    fn test_async_error_is_error() {
        let err = AsyncError::Panic("test panic".to_string());
        let display = format!("{err}");
        assert!(display.contains("test panic"), "Display 应包含 panic 消息");
    }

    #[test]
    fn test_async_error_from_parse_error() {
        let parse_err = ParseError::FileNotFound {
            path: "test.log".to_string(),
        };
        let async_err: AsyncError = parse_err.into();
        assert!(
            matches!(async_err, AsyncError::Parse(_)),
            "ParseError 应转换为 AsyncError::Parse"
        );
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn test_parse_error_propagates() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // 写入一条格式正确的记录，再追加一条缺少性能指标的无效记录
        let mut tmp = NamedTempFile::new().expect("创建临时文件失败");
        write!(
            tmp,
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:SYSDBA trxid:3 stmt:4 appname:app) SELECT 1\nEXECTIME:0.100(ms) ROWCOUNT:1(rows) EXEC_ID:1.\n\
             2025-11-17 16:09:42.456 INVALID RECORD WITHOUT METRICS"
        )
        .unwrap();
        tmp.as_file().sync_all().unwrap();

        let result = AsyncLogParser::new(tmp.path()).parse().await;
        assert!(
            result.is_err(),
            "含解析错误的文件应返回 Err，而非静默丢弃"
        );
        assert!(matches!(result, Err(AsyncError::Parse(_))));
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn test_parse_panic_becomes_async_error() {
        let result: Result<Vec<Sqllog>, AsyncError> = {
            let blocking =
                tokio::task::spawn_blocking(|| -> Result<Vec<Sqllog>, crate::error::ParseError> {
                    panic!("intentional test panic");
                })
                .await;
            blocking
                .map_err(|e| AsyncError::Panic(e.to_string()))
                .and_then(|r| r.map_err(AsyncError::Parse))
        };
        assert!(matches!(result, Err(AsyncError::Panic(_))));
        if let Err(AsyncError::Panic(msg)) = result {
            assert!(!msg.is_empty());
        }
    }
}
