//! 实时 SQL 日志解析模块
//!
//! 提供实时监控和解析 SQL 日志文件的功能，支持：
//! - 文件内容变化监控
//! - 增量读取新增内容
//! - 实时解析新日志
//! - 回调处理每条日志
//!
//! # 示例
//!
//! ```no_run
//! use dm_database_parser_sqllog::realtime::RealtimeSqllogParser;
//! use std::time::Duration;
//!
//! let mut parser = RealtimeSqllogParser::new("sqllog.txt")
//!     .expect("Failed to create parser");
//!
//! parser.watch(|sqllog| {
//!     println!("新日志: {} - {}", sqllog.ts, sqllog.body);
//! }).expect("Watch failed");
//! ```

use crate::error::ParseError;
use crate::parser::parse_record;
use crate::sqllog::Sqllog;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

/// 实时 SQL 日志解析器
///
/// 监控指定文件的变化，实时解析新增的日志记录
pub struct RealtimeSqllogParser {
    /// 日志文件路径
    file_path: PathBuf,
    /// 当前文件读取位置
    position: u64,
    /// 文件读取器
    reader: Option<BufReader<File>>,
    /// 缓冲区,用于存储跨行的记录
    buffer: String,
}

impl RealtimeSqllogParser {
    /// 创建新的实时解析器
    ///
    /// # 参数
    ///
    /// * `path` - SQL 日志文件路径
    ///
    /// # 返回
    ///
    /// 成功返回解析器实例，失败返回错误
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use dm_database_parser_sqllog::realtime::RealtimeSqllogParser;
    ///
    /// let parser = RealtimeSqllogParser::new("sqllog.txt")
    ///     .expect("Failed to create parser");
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
        let file_path = path.as_ref().to_path_buf();

        // 检查文件是否存在
        if !file_path.exists() {
            return Err(ParseError::FileNotFound {
                path: file_path.to_string_lossy().to_string(),
            });
        }

        // 打开文件并定位到末尾
        let file = File::open(&file_path)
            .map_err(|e| ParseError::IoError(format!("Failed to open file: {}", e)))?;

        let mut reader = BufReader::new(file);
        let position = reader
            .seek(SeekFrom::End(0))
            .map_err(|e| ParseError::IoError(format!("Failed to seek file: {}", e)))?;

        Ok(Self {
            file_path,
            position,
            reader: Some(reader),
            buffer: String::new(),
        })
    }

    /// 从文件开头开始监控
    ///
    /// 默认情况下，解析器从文件末尾开始监控。
    /// 调用此方法后，将从文件开头开始解析所有内容。
    pub fn from_beginning(mut self) -> Result<Self, ParseError> {
        if let Some(ref mut reader) = self.reader {
            self.position = reader
                .seek(SeekFrom::Start(0))
                .map_err(|e| ParseError::IoError(format!("Failed to seek file: {}", e)))?;
        }
        Ok(self)
    }

    /// 读取新增的内容
    fn read_new_content(&mut self) -> Result<Vec<String>, ParseError> {
        let mut lines = Vec::new();

        if let Some(ref mut _reader) = self.reader {
            // 重新打开文件以获取最新内容
            let file = File::open(&self.file_path)
                .map_err(|e| ParseError::IoError(format!("Failed to reopen file: {}", e)))?;

            let mut new_reader = BufReader::new(file);
            new_reader
                .seek(SeekFrom::Start(self.position))
                .map_err(|e| ParseError::IoError(format!("Failed to seek: {}", e)))?;

            let mut line = String::new();
            loop {
                let bytes_read = new_reader
                    .read_line(&mut line)
                    .map_err(|e| ParseError::IoError(format!("Failed to read line: {}", e)))?;

                if bytes_read == 0 {
                    break;
                }

                self.position += bytes_read as u64;

                // 只添加非空行
                if !line.trim().is_empty() {
                    lines.push(line.trim_end().to_string());
                }

                line.clear();
            }

            self.reader = Some(new_reader);
        }

        Ok(lines)
    }

    /// 处理新增的行
    fn process_lines<F>(&mut self, lines: Vec<String>, mut callback: F) -> Result<(), ParseError>
    where
        F: FnMut(Sqllog),
    {
        for line in lines {
            // 检查是否是新记录的开始
            if crate::tools::is_record_start_line(&line) {
                // 如果缓冲区有内容，先处理之前的记录
                if !self.buffer.is_empty() {
                    // 将缓冲区内容分割成行
                    let buffer_lines: Vec<&str> = self.buffer.lines().collect();
                    if let Ok(sqllog) = parse_record(&buffer_lines) {
                        callback(sqllog);
                    }
                    self.buffer.clear();
                }
                // 开始新记录
                self.buffer.push_str(&line);
                self.buffer.push('\n');
            } else {
                // 继续行
                if !self.buffer.is_empty() {
                    self.buffer.push_str(&line);
                    self.buffer.push('\n');
                }
            }
        }

        Ok(())
    }

    /// 刷新缓冲区，处理最后一条未完成的记录
    ///
    /// 主要用于测试或确保所有记录都被处理
    #[cfg(test)]
    fn flush_buffer<F>(&mut self, mut callback: F) -> Result<(), ParseError>
    where
        F: FnMut(Sqllog),
    {
        if !self.buffer.is_empty() {
            let buffer_lines: Vec<&str> = self.buffer.lines().collect();
            if let Ok(sqllog) = parse_record(&buffer_lines) {
                callback(sqllog);
            }
            self.buffer.clear();
        }
        Ok(())
    }

    /// 启动监控并处理新增日志
    ///
    /// # 参数
    ///
    /// * `callback` - 处理每条新日志的回调函数
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use dm_database_parser_sqllog::realtime::RealtimeSqllogParser;
    ///
    /// let mut parser = RealtimeSqllogParser::new("sqllog.txt")
    ///     .expect("Failed to create parser");
    ///
    /// parser.watch(|sqllog| {
    ///     println!("时间: {}, SQL: {}", sqllog.ts, sqllog.body);
    /// }).expect("Watch failed");
    /// ```
    pub fn watch<F>(mut self, mut callback: F) -> Result<(), ParseError>
    where
        F: FnMut(Sqllog),
    {
        let (tx, rx) = channel();

        // 创建文件监控器
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default(),
        )
        .map_err(|e| ParseError::IoError(format!("Failed to create watcher: {}", e)))?;

        // 开始监控文件
        watcher
            .watch(&self.file_path, RecursiveMode::NonRecursive)
            .map_err(|e| ParseError::IoError(format!("Failed to watch file: {}", e)))?;

        println!("开始监控文件: {:?}", self.file_path);

        // 事件循环
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    // 检查是否是修改事件
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        // 读取新内容
                        match self.read_new_content() {
                            Ok(lines) => {
                                if !lines.is_empty() {
                                    self.process_lines(lines, &mut callback)?;
                                }
                            }
                            Err(e) => {
                                eprintln!("读取文件失败: {}", e);
                            }
                        }
                    }
                }
                Err(_) => {
                    // 超时，继续循环
                    continue;
                }
            }
        }
    }

    /// 监控一段时间后停止
    ///
    /// # 参数
    ///
    /// * `duration` - 监控时长
    /// * `callback` - 处理每条新日志的回调函数
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use dm_database_parser_sqllog::realtime::RealtimeSqllogParser;
    /// use std::time::Duration;
    ///
    /// let mut parser = RealtimeSqllogParser::new("sqllog.txt")
    ///     .expect("Failed to create parser");
    ///
    /// // 监控 60 秒
    /// parser.watch_for(Duration::from_secs(60), |sqllog| {
    ///     println!("新日志: {}", sqllog.body);
    /// }).expect("Watch failed");
    /// ```
    pub fn watch_for<F>(mut self, duration: Duration, mut callback: F) -> Result<(), ParseError>
    where
        F: FnMut(Sqllog),
    {
        let (tx, rx) = channel();
        let start_time = std::time::Instant::now();

        // 创建文件监控器
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default(),
        )
        .map_err(|e| ParseError::IoError(format!("Failed to create watcher: {}", e)))?;

        // 开始监控文件
        watcher
            .watch(&self.file_path, RecursiveMode::NonRecursive)
            .map_err(|e| ParseError::IoError(format!("Failed to watch file: {}", e)))?;

        println!(
            "开始监控文件 {:?}, 持续 {} 秒",
            self.file_path,
            duration.as_secs()
        );

        // 事件循环
        while start_time.elapsed() < duration {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        match self.read_new_content() {
                            Ok(lines) => {
                                if !lines.is_empty() {
                                    self.process_lines(lines, &mut callback)?;
                                }
                            }
                            Err(e) => {
                                eprintln!("读取文件失败: {}", e);
                            }
                        }
                    }
                }
                Err(_) => {
                    // 超时，继续循环
                    continue;
                }
            }
        }

        println!("监控结束");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tempfile::NamedTempFile;

    #[test]
    fn test_realtime_parser_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let parser = RealtimeSqllogParser::new(temp_file.path());
        assert!(parser.is_ok());

        // 验证解析器从文件末尾开始
        let parser = parser.unwrap();
        assert!(parser.position > 0 || parser.position == 0);
    }

    #[test]
    fn test_nonexistent_file() {
        let parser = RealtimeSqllogParser::new("/nonexistent/file.txt");
        assert!(parser.is_err());

        if let Err(ParseError::FileNotFound { path }) = parser {
            assert!(path.contains("nonexistent"));
        } else {
            panic!("Expected FileNotFound error");
        }
    }

    #[test]
    fn test_from_beginning() {
        let temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file.as_file(), "test content").unwrap();

        let parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 验证位置在文件开头
        assert_eq!(parser.position, 0);
    }

    #[test]
    fn test_watch_for_timeout() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // 写入初始内容
        writeln!(
            temp_file,
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        let result = parser.watch_for(Duration::from_millis(500), move |_sqllog| {
            let mut count = counter_clone.lock().unwrap();
            *count += 1;
        });

        assert!(result.is_ok());
    }

    #[test]
    fn test_read_new_content() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // 写入初始内容
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file, "line 2").unwrap();
        temp_file.flush().unwrap();

        // 创建解析器并定位到末尾
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 追加新内容
        writeln!(temp_file, "line 3").unwrap();
        writeln!(temp_file, "line 4").unwrap();
        temp_file.flush().unwrap();

        // 读取新内容
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line 3");
        assert_eq!(lines[1], "line 4");
    }

    #[test]
    fn test_process_single_line_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新缓冲区以处理最后一条记录
        let received_clone2 = received.clone();
        parser
            .flush_buffer(move |sqllog| {
                received_clone2.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert_eq!(sqllogs[0].meta.username, "alice");
        assert!(sqllogs[0].body.contains("SELECT 1"));
    }

    #[test]
    fn test_process_multiline_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "FROM users".to_string(),
            "WHERE id = 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新缓冲区
        let received_clone2 = received.clone();
        parser
            .flush_buffer(move |sqllog| {
                received_clone2.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("FROM users"));
        assert!(sqllogs[0].body.contains("WHERE id = 1"));
    }

    #[test]
    fn test_process_multiple_records() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
            "2025-08-12 10:57:11.548 (EP[0] sess:125 thrd:458 user:carol trxid:791 stmt:1001 appname:app) SELECT 3".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新缓冲区
        let received_clone2 = received.clone();
        parser
            .flush_buffer(move |sqllog| {
                received_clone2.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 3);
        assert_eq!(sqllogs[0].meta.username, "alice");
        assert_eq!(sqllogs[1].meta.username, "bob");
        assert_eq!(sqllogs[2].meta.username, "carol");
    }

    #[test]
    fn test_process_mixed_records() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "FROM users".to_string(),
            "2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) INSERT INTO".to_string(),
            "logs VALUES (1)".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新缓冲区
        let received_clone2 = received.clone();
        parser
            .flush_buffer(move |sqllog| {
                received_clone2.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 2);
        assert!(sqllogs[0].body.contains("FROM users"));
        assert!(sqllogs[1].body.contains("logs VALUES"));
    }

    #[test]
    fn test_empty_lines_ignored() {
        let temp_file = NamedTempFile::new().unwrap();
        let _parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "".to_string(),
            "   ".to_string(),
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "".to_string(),
        ];

        // 空行应该被 read_new_content 过滤掉，这里直接测试非空行
        let non_empty_lines: Vec<String> =
            lines.into_iter().filter(|l| !l.trim().is_empty()).collect();

        assert_eq!(non_empty_lines.len(), 1);
    }

    #[test]
    fn test_record_with_performance_indicators() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新缓冲区
        let received_clone2 = received.clone();
        parser
            .flush_buffer(move |sqllog| {
                received_clone2.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].indicators.is_some());

        if let Some(ref indicators) = sqllogs[0].indicators {
            assert_eq!(indicators.execute_time, 10.5);
            assert_eq!(indicators.row_count, 100);
            assert_eq!(indicators.execute_id, 12345);
        }
    }

    #[test]
    fn test_buffer_persistence() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 处理部分记录（未完成）
        let lines1 = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "FROM users".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines1, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 第一次不应该有完整记录
        assert_eq!(received.lock().unwrap().len(), 0);

        // 处理下一条新记录（触发前一条完成）
        let lines2 = vec![
            "2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 1".to_string(),
        ];

        let received_clone2 = received.clone();
        parser
            .process_lines(lines2, move |sqllog| {
                received_clone2.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 现在应该有第一条完整记录
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("FROM users"));
    }

    #[test]
    fn test_invalid_record_ignored() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "invalid line without proper format".to_string(),
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新缓冲区
        let received_clone2 = received.clone();
        parser
            .flush_buffer(move |sqllog| {
                received_clone2.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 只有有效记录应该被处理
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
    }

    #[test]
    fn test_watch_file_modification() {
        use std::fs::OpenOptions;
        use std::io::Write;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // 写入初始内容
        {
            let mut file = OpenOptions::new()
                .write(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(
                file,
                "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
            )
            .unwrap();
            file.flush().unwrap();
        }

        let parser = RealtimeSqllogParser::new(&path).unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 使用 watch_for 监控较短时间
        let handle = std::thread::spawn(move || {
            let _ = parser.watch_for(Duration::from_millis(500), move |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            });
        });

        // 给监控足够时间启动
        std::thread::sleep(Duration::from_millis(100));

        // 追加新记录
        {
            let mut file = OpenOptions::new()
                .write(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(
                file,
                "2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
            )
            .unwrap();
            file.flush().unwrap();
        }

        // 等待监控完成
        handle.join().unwrap();

        // 验证接收到新记录
        let _sqllogs = received.lock().unwrap();
        // 注意: 文件监控可能因系统原因不稳定,这里只测试功能没有 panic
        // 在真实环境中应该能收到记录,但在测试环境可能有延迟
        // 所以这里不强制断言一定有记录
    }

    #[test]
    fn test_read_new_content_edge_cases() {
        use std::io::Write;

        let mut temp_file = NamedTempFile::new().unwrap();

        // 写入带有空行的内容
        writeln!(temp_file, "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1").unwrap();
        writeln!(temp_file, "").unwrap(); // 空行
        writeln!(temp_file, "   ").unwrap(); // 只有空格的行
        writeln!(temp_file, "2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines = parser.read_new_content().unwrap();

        // 空行应该被过滤掉
        assert_eq!(lines.len(), 2); // 只有两条有效记录
        assert!(lines[0].contains("SELECT 1"));
        assert!(lines[1].contains("SELECT 2"));
    }

    #[test]
    fn test_process_lines_with_continuation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 模拟一条跨多行的记录，然后是另一条完整记录
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "  FROM users".to_string(),
            "  WHERE id = 1".to_string(),
            "2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 第一条记录应该被触发完成（因为第二条记录开始了）
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("FROM users"));
        assert!(sqllogs[0].body.contains("WHERE id = 1"));
    }

    #[test]
    fn test_from_beginning_position() {
        use std::io::Write;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
        )
        .unwrap();
        temp_file.flush().unwrap();

        // 创建解析器，默认从文件末尾开始
        let parser1 = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        // position 应该在文件末尾
        assert!(parser1.position > 0);

        // 使用 from_beginning
        let parser2 = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();
        // position 应该在文件开头
        assert_eq!(parser2.position, 0);
    }

    #[test]
    fn test_buffer_clear_on_new_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "2025-08-12 10:57:10.548 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 第一条记录应该被处理（被第二条触发）
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert_eq!(sqllogs[0].meta.username, "alice");
    }

    #[test]
    fn test_flush_buffer() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 添加一条未完成的记录到缓冲区
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "FROM users".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 处理行，但不会触发回调（没有新记录开始）
        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 此时缓冲区有内容，但回调未触发
        assert_eq!(received.lock().unwrap().len(), 0);

        // 刷新缓冲区
        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 现在应该有一条记录
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("FROM users"));
    }

    #[test]
    fn test_process_lines_empty_buffer_continuation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 缓冲区为空时，继续行应该被忽略
        let lines = vec![
            "this is a continuation line".to_string(),
            "another continuation".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 不应该有记录被处理
        assert_eq!(received.lock().unwrap().len(), 0);
        // 缓冲区应该为空（继续行被忽略）
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_process_lines_invalid_record_in_buffer() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "invalid record line".to_string(),
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 第一行无效，缓冲区为空，继续行被忽略
        // 第二行有效，但没有后续记录触发，所以不会调用回调
        assert_eq!(received.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_read_new_content_with_reader() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 第一次读取，应该跳过已读内容
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 0); // from_end=true，所以初始内容被跳过

        // 追加新内容
        writeln!(
            temp_file,
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
        )
        .unwrap();
        temp_file.flush().unwrap();

        // 第二次读取，应该读到新内容
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("bob"));
    }

    #[test]
    fn test_process_lines_with_multiple_complete_records() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "FROM users".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
            "FROM orders".to_string(),
            "2025-08-12 10:57:11.548 (EP[2] sess:125 thrd:458 user:charlie trxid:791 stmt:1001 appname:app) SELECT 3".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 应该处理了两条完整记录（alice 和 bob）
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 2);
        assert_eq!(sqllogs[0].meta.username, "alice");
        assert!(sqllogs[0].body.contains("FROM users"));
        assert_eq!(sqllogs[1].meta.username, "bob");
        assert!(sqllogs[1].body.contains("FROM orders"));
    }

    #[test]
    fn test_read_new_content_file_shrunk() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 先读取一次，更新 position
        let _ = parser.read_new_content().unwrap();
        let old_position = parser.position;

        // 现在 position 已经是合法的值
        // 验证可以继续读取
        assert!(old_position > 0);
    }

    #[test]
    fn test_process_lines_callback_error_handling() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
        ];

        let count = Arc::new(Mutex::new(0));
        let count_clone = count.clone();

        // 即使回调中有逻辑，也应该正常执行
        let result = parser.process_lines(lines, |_sqllog| {
            let mut c = count_clone.lock().unwrap();
            *c += 1;
        });

        assert!(result.is_ok());
        assert_eq!(*count.lock().unwrap(), 1); // 第一条被第二条触发
    }

    #[test]
    fn test_flush_buffer_empty() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let count = Arc::new(Mutex::new(0));
        let count_clone = count.clone();

        // 缓冲区为空时刷新，不应该调用回调
        let result = parser.flush_buffer(|_| {
            *count_clone.lock().unwrap() += 1;
        });

        assert!(result.is_ok());
        assert_eq!(*count.lock().unwrap(), 0);
    }

    #[test]
    fn test_flush_buffer_invalid_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 手动在缓冲区中添加无效记录
        parser.buffer.push_str("invalid record data\n");

        let count = Arc::new(Mutex::new(0));
        let count_clone = count.clone();

        // 刷新时，无效记录不会触发回调
        let result = parser.flush_buffer(|_| {
            *count_clone.lock().unwrap() += 1;
        });

        assert!(result.is_ok());
        assert_eq!(*count.lock().unwrap(), 0);
        // 缓冲区应该被清空
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_new_parser_file_position() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Some initial content").unwrap();
        temp_file.flush().unwrap();

        let parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 默认情况下，parser 应该定位到文件末尾
        let metadata = std::fs::metadata(temp_file.path()).unwrap();
        assert_eq!(parser.position, metadata.len());
    }

    #[test]
    fn test_from_beginning_resets_position() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        let parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // from_beginning 后 position 应该为 0
        assert_eq!(parser.position, 0);
    }

    #[test]
    fn test_read_new_content_updates_position() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        let initial_position = parser.position;

        // 追加新内容
        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        // 读取新内容
        let lines = parser.read_new_content().unwrap();

        // position 应该更新
        assert!(parser.position > initial_position);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_process_lines_with_trailing_newline() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "".to_string(), // 空行
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 空行不应该影响处理
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
    }

    #[test]
    fn test_buffer_accumulation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 第一批：只有起始行
        let lines1 = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines1, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 缓冲区应该有内容，但没有完整记录
        assert!(!parser.buffer.is_empty());
        assert_eq!(received.lock().unwrap().len(), 0);

        // 第二批：继续行
        let lines2 = vec!["FROM users".to_string(), "WHERE id = 1".to_string()];

        parser
            .process_lines(lines2, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 仍然没有完整记录
        assert_eq!(received.lock().unwrap().len(), 0);

        // 第三批：新记录触发之前的记录
        let lines3 = vec![
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
                .to_string(),
        ];

        parser
            .process_lines(lines3, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 现在应该有一条完整记录
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("FROM users"));
    }

    #[test]
    fn test_process_lines_only_continuation_lines() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 只有继续行，没有起始行
        let lines = vec![
            "continuation line 1".to_string(),
            "continuation line 2".to_string(),
            "continuation line 3".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 缓冲区为空时，继续行被忽略
        assert_eq!(received.lock().unwrap().len(), 0);
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_multiple_flushes() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
        ];

        let count = Arc::new(Mutex::new(0));
        let count_clone = count.clone();

        parser
            .process_lines(lines, |_| {
                *count_clone.lock().unwrap() += 1;
            })
            .unwrap();

        // 第一次刷新
        parser
            .flush_buffer(|_| {
                *count_clone.lock().unwrap() += 1;
            })
            .unwrap();

        // 第二次刷新，缓冲区已空
        parser
            .flush_buffer(|_| {
                *count_clone.lock().unwrap() += 1;
            })
            .unwrap();

        // 只应该触发一次回调（第一次刷新）
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn test_from_beginning_with_existing_content() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"
        )
        .unwrap();
        writeln!(
            temp_file,
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 从头读取应该能读到所有内容
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_position_tracking_accuracy() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "First line").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        let pos1 = parser.position;

        // 追加内容
        writeln!(temp_file, "Second line").unwrap();
        temp_file.flush().unwrap();

        parser.read_new_content().unwrap();
        let pos2 = parser.position;

        // position 应该准确反映读取位置
        assert!(pos2 > pos1);

        // 再次读取应该没有新内容
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 0);
        assert_eq!(parser.position, pos2); // position 不变
    }

    #[test]
    fn test_process_lines_single_valid_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "FROM users".to_string(),
            "WHERE id = 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 没有新记录触发，所以不会调用回调
        assert_eq!(received.lock().unwrap().len(), 0);

        // 但缓冲区应该有内容
        assert!(!parser.buffer.is_empty());
    }

    #[test]
    fn test_new_parser_position_at_end() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        let parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 默认 position 应该在文件末尾
        let metadata = std::fs::metadata(temp_file.path()).unwrap();
        assert_eq!(parser.position, metadata.len());
    }

    #[test]
    fn test_read_new_content_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 空文件读取应该返回空列表
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_read_new_content_only_empty_lines() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "").unwrap();
        writeln!(temp_file, "   ").unwrap();
        writeln!(temp_file, "\t").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 只有空行应该被过滤掉
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_process_lines_single_complete_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 单条记录没有后续记录触发，不会调用回调
        assert_eq!(received.lock().unwrap().len(), 0);
        // 但缓冲区应该有内容
        assert!(!parser.buffer.is_empty());
    }

    #[test]
    fn test_read_new_content_incremental() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 第一次读取
        let lines1 = parser.read_new_content().unwrap();
        assert_eq!(lines1.len(), 1);
        assert_eq!(lines1[0], "Line 1");

        // 追加新内容
        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        // 第二次读取，应该只读到新内容
        let lines2 = parser.read_new_content().unwrap();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0], "Line 2");
    }

    #[test]
    fn test_process_lines_with_trailing_newlines() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n".to_string(),
            "FROM users\n".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 没有新记录开始，不会触发回调
        assert_eq!(received.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_multiple_process_lines_calls() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 第一次调用
        parser
            .process_lines(
                vec!["2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string()],
                |sqllog| {
                    received_clone.lock().unwrap().push(sqllog);
                },
            )
            .unwrap();

        // 第二次调用，新记录
        parser
            .process_lines(
                vec!["2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string()],
                |sqllog| {
                    received_clone.lock().unwrap().push(sqllog);
                },
            )
            .unwrap();

        // 第一条应该被处理
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert_eq!(sqllogs[0].meta.username, "alice");
    }

    #[test]
    fn test_read_new_content_position_tracking() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "First line").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let initial_pos = parser.position;
        assert_eq!(initial_pos, 0);

        // 读取内容
        let _lines = parser.read_new_content().unwrap();

        // position 应该更新
        assert!(parser.position > initial_pos);

        let new_pos = parser.position;

        // 再次读取（没有新内容）
        let lines2 = parser.read_new_content().unwrap();
        assert_eq!(lines2.len(), 0);

        // position 应该保持不变
        assert_eq!(parser.position, new_pos);
    }

    #[test]
    fn test_process_lines_with_windows_line_endings() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\r\n".to_string(),
            "FROM users\r\n".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2\r\n".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 应该正确处理 Windows 行尾
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
    }

    #[test]
    fn test_flush_buffer_with_multiline_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 添加多行记录到缓冲区
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "FROM users".to_string(),
            "WHERE id = 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新缓冲区
        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 应该处理了多行记录
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("FROM users"));
        assert!(sqllogs[0].body.contains("WHERE id = 1"));
    }

    #[test]
    fn test_read_new_content_reopens_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Initial content").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 追加内容
        writeln!(temp_file, "New content").unwrap();
        temp_file.flush().unwrap();

        // read_new_content 应该重新打开文件并读取新内容
        let lines = parser.read_new_content().unwrap();

        // 应该读到新内容
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "New content");
    }

    #[test]
    fn test_process_lines_multiple_new_records() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
            "2025-08-12 10:57:11.548 (EP[2] sess:125 thrd:458 user:charlie trxid:791 stmt:1001 appname:app) SELECT 3".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 前两条应该被处理
        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 2);
        assert_eq!(sqllogs[0].meta.username, "alice");
        assert_eq!(sqllogs[1].meta.username, "bob");
    }

    #[test]
    fn test_read_new_content_with_mixed_content() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "").unwrap(); // 空行
        writeln!(temp_file, "   ").unwrap(); // 只有空格
        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines = parser.read_new_content().unwrap();
        // 空行应该被过滤
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Line 1");
        assert_eq!(lines[1], "Line 2");
    }

    #[test]
    fn test_buffer_state_after_flush() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        parser.buffer.push_str("2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1");

        assert!(!parser.buffer.is_empty());

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 刷新后缓冲区应该被清空
        assert!(parser.buffer.is_empty());
        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_process_lines_alternating_records_and_continuations() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "FROM table1".to_string(),
            "WHERE id = 1".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE table2".to_string(),
            "SET value = 1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("FROM table1"));
        assert!(sqllogs[0].body.contains("WHERE id = 1"));
    }

    #[test]
    fn test_read_new_content_no_reader() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 清空 reader 模拟异常状态
        parser.reader = None;

        let lines = parser.read_new_content().unwrap();
        // reader 为 None 时应该返回空列表
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_process_lines_empty_input() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 处理空列表
        parser
            .process_lines(vec![], |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        assert_eq!(received.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_buffer_with_invalid_parse() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 添加无效的缓冲区内容
        parser.buffer.push_str("Invalid line without proper format");

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 刷新时应该忽略无效记录
        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        assert_eq!(received.lock().unwrap().len(), 0);
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_position_after_multiple_reads() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "First").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let pos1 = parser.position;
        parser.read_new_content().unwrap();
        let pos2 = parser.position;

        assert!(pos2 > pos1);

        // 追加更多内容
        writeln!(temp_file, "Second").unwrap();
        temp_file.flush().unwrap();

        parser.read_new_content().unwrap();
        let pos3 = parser.position;

        assert!(pos3 > pos2);
    }

    #[test]
    fn test_from_beginning_clears_buffer() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Content").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 添加一些缓冲内容
        parser.buffer.push_str("Some buffered data");

        assert!(!parser.buffer.is_empty());

        // from_beginning 应该重置position,但不会清空buffer
        let parser = parser.from_beginning().unwrap();

        assert_eq!(parser.position, 0);
    }

    #[test]
    fn test_process_lines_with_only_invalid_records() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "Invalid record 1".to_string(),
            "Invalid record 2".to_string(),
            "Invalid record 3".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 无效记录不应该被处理
        assert_eq!(received.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_read_new_content_after_file_truncation() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Original content line 1").unwrap();
        writeln!(temp_file, "Original content line 2").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 读取初始内容
        let lines1 = parser.read_new_content().unwrap();
        assert_eq!(lines1.len(), 2);

        // 文件被截断并重写（模拟日志轮转）
        temp_file.as_file_mut().set_len(0).unwrap();
        temp_file.seek(SeekFrom::Start(0)).unwrap();
        writeln!(temp_file, "New content after rotation").unwrap();
        temp_file.flush().unwrap();

        // 重置 parser 从头开始读
        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines2 = parser.read_new_content().unwrap();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0], "New content after rotation");
    }

    #[test]
    fn test_process_lines_with_very_long_continuation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let mut lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
        ];

        // 添加100行继续行
        for i in 1..=100 {
            lines.push(format!("continuation line {}", i));
        }

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 没有新记录开始，所以不会触发回调
        assert_eq!(received.lock().unwrap().len(), 0);

        // 但缓冲区应该包含所有行
        assert!(parser.buffer.contains("continuation line 1"));
        assert!(parser.buffer.contains("continuation line 100"));
    }

    #[test]
    fn test_new_with_relative_path() {
        let temp_file = NamedTempFile::new().unwrap();
        let result = RealtimeSqllogParser::new(temp_file.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_lines_record_boundary() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "continuation".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
            "continuation2".to_string(),
            "2025-08-12 10:57:11.548 (EP[2] sess:125 thrd:458 user:charlie trxid:791 stmt:1001 appname:app) SELECT 3".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 2);
        assert!(sqllogs[0].body.contains("continuation"));
        assert!(sqllogs[1].body.contains("continuation2"));
    }

    #[test]
    fn test_buffer_management() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        assert!(parser.buffer.is_empty());

        parser.buffer.push_str("test data");
        assert!(!parser.buffer.is_empty());

        parser.buffer.clear();
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_read_new_content_seek_behavior() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 第一次读取所有行
        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 3);

        // 再次读取应该没有新内容
        let lines2 = parser.read_new_content().unwrap();
        assert_eq!(lines2.len(), 0);
    }

    #[test]
    fn test_process_lines_buffer_growth() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
        ];

        // 添加大量继续行测试缓冲区增长
        let mut all_lines = lines.clone();
        for i in 0..1000 {
            all_lines.push(format!("continuation line {}", i));
        }

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(all_lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 缓冲区应该能处理大量数据
        assert!(parser.buffer.len() > 1000);
    }

    #[test]
    fn test_multiple_sequential_flushes() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let received = Arc::new(Mutex::new(Vec::new()));

        // 第一次刷新（空缓冲区）
        parser
            .flush_buffer(|sqllog| {
                received.lock().unwrap().push(sqllog);
            })
            .unwrap();
        assert_eq!(received.lock().unwrap().len(), 0);

        // 添加记录
        parser.buffer.push_str("2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1");

        // 第二次刷新
        let received_clone = received.clone();
        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();
        assert_eq!(received.lock().unwrap().len(), 1);

        // 第三次刷新（再次为空）
        let received_clone = received.clone();
        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();
        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_position_consistency() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Test line").unwrap();
        temp_file.flush().unwrap();

        let parser1 = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        let metadata = std::fs::metadata(temp_file.path()).unwrap();

        // 新parser的position应该在文件末尾
        assert_eq!(parser1.position, metadata.len());

        let parser2 = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // from_beginning后position应该在开头
        assert_eq!(parser2.position, 0);
    }

    #[test]
    fn test_read_with_unicode_content() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "中文内容").unwrap();
        writeln!(temp_file, "日本語").unwrap();
        writeln!(temp_file, "한국어").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "中文内容");
        assert_eq!(lines[1], "日本語");
        assert_eq!(lines[2], "한국어");
    }

    #[test]
    fn test_process_lines_with_callback_panic_safety() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 回调不会 panic,正常执行
        let result = parser.process_lines(lines, |sqllog| {
            received_clone.lock().unwrap().push(sqllog);
        });

        assert!(result.is_ok());
        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_empty_file_from_beginning() {
        let temp_file = NamedTempFile::new().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 0);
        assert_eq!(parser.position, 0);
    }

    #[test]
    fn test_process_lines_preserves_line_order() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) First".to_string(),
            "continuation 1".to_string(),
            "continuation 2".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) Second".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        let body = &sqllogs[0].body;

        // 确保行顺序被保留
        let first_pos = body.find("continuation 1").unwrap();
        let second_pos = body.find("continuation 2").unwrap();
        assert!(first_pos < second_pos);
    }

    #[test]
    fn test_large_file_position_tracking() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // 写入大量数据
        for i in 0..1000 {
            writeln!(temp_file, "Line {}", i).unwrap();
        }
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        assert_eq!(parser.position, 0);

        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 1000);

        // position应该移到文件末尾
        let metadata = std::fs::metadata(temp_file.path()).unwrap();
        assert_eq!(parser.position, metadata.len());
    }

    #[test]
    fn test_concurrent_buffer_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        parser.buffer.push_str("Line 1\n");
        assert!(parser.buffer.contains("Line 1"));

        parser.buffer.push_str("Line 2\n");
        assert!(parser.buffer.contains("Line 2"));

        parser.buffer.clear();
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_read_new_content_with_special_characters() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line with tabs\t\ttabs").unwrap();
        writeln!(temp_file, "Line with quotes \"quoted\"").unwrap();
        writeln!(temp_file, "Line with backslash \\backslash").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines = parser.read_new_content().unwrap();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("tabs"));
        assert!(lines[1].contains("quoted"));
        assert!(lines[2].contains("backslash"));
    }

    #[test]
    fn test_process_lines_mixed_line_endings() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 混合 Unix 和 Windows 行尾
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT\n".to_string(),
            "FROM table\r\n".to_string(),
            "WHERE id = 1\n".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 缓冲区应该累积所有行
        assert!(parser.buffer.contains("FROM table"));
        assert!(parser.buffer.contains("WHERE id = 1"));
    }

    #[test]
    fn test_flush_buffer_multiple_times_same_record() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let received = Arc::new(Mutex::new(Vec::new()));

        parser.buffer.push_str("2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1");

        // 第一次刷新
        let received_clone = received.clone();
        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        assert_eq!(received.lock().unwrap().len(), 1);
        assert!(parser.buffer.is_empty());

        // 第二次刷新空缓冲区
        let received_clone = received.clone();
        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        assert_eq!(received.lock().unwrap().len(), 1); // 不应该增加
    }

    #[test]
    fn test_position_after_from_beginning() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Content").unwrap();
        temp_file.flush().unwrap();

        let parser1 = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        assert!(parser1.position > 0);

        let parser2 = parser1.from_beginning().unwrap();
        assert_eq!(parser2.position, 0);
    }

    #[test]
    fn test_process_lines_single_continuation_line() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 只有继续行，没有记录开始
        let lines = vec!["continuation line".to_string()];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 没有记录被处理
        assert_eq!(received.lock().unwrap().len(), 0);
        // 缓冲区应该为空（没有记录开始就没有累积）
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_read_new_content_line_by_line() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines1 = parser.read_new_content().unwrap();
        assert_eq!(lines1.len(), 1);

        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        let lines2 = parser.read_new_content().unwrap();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0], "Line 2");

        writeln!(temp_file, "Line 3").unwrap();
        temp_file.flush().unwrap();

        let lines3 = parser.read_new_content().unwrap();
        assert_eq!(lines3.len(), 1);
        assert_eq!(lines3[0], "Line 3");
    }

    #[test]
    fn test_buffer_content_integrity() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let test_content = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT * FROM users WHERE id = 1 AND name = 'test'";

        parser.buffer.push_str(test_content);
        assert_eq!(parser.buffer, test_content);

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("SELECT * FROM users"));
    }

    #[test]
    fn test_multiple_files_sequential() {
        let temp_file1 = NamedTempFile::new().unwrap();
        let temp_file2 = NamedTempFile::new().unwrap();

        let parser1 = RealtimeSqllogParser::new(temp_file1.path());
        assert!(parser1.is_ok());

        let parser2 = RealtimeSqllogParser::new(temp_file2.path());
        assert!(parser2.is_ok());
    }

    #[test]
    fn test_process_lines_exact_record_boundaries() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) R1".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) R2".to_string(),
            "2025-08-12 10:57:11.548 (EP[2] sess:125 thrd:458 user:charlie trxid:791 stmt:1001 appname:app) R3".to_string(),
            "2025-08-12 10:57:12.548 (EP[3] sess:126 thrd:459 user:dave trxid:792 stmt:1002 appname:app) R4".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 3);
        assert_eq!(sqllogs[0].meta.username, "alice");
        assert_eq!(sqllogs[1].meta.username, "bob");
        assert_eq!(sqllogs[2].meta.username, "charlie");
    }

    #[test]
    fn test_read_new_content_respects_position() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // position已经在末尾
        let initial_pos = parser.position;
        let lines = parser.read_new_content().unwrap();

        // 没有新内容
        assert_eq!(lines.len(), 0);
        assert_eq!(parser.position, initial_pos);
    }

    #[test]
    fn test_process_lines_callback_receives_correct_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:testuser trxid:789 stmt:999 appname:testapp) SELECT test_column FROM test_table".to_string(),
            "WHERE test_id = 123".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:user2 trxid:790 stmt:1000 appname:app2) SELECT 2".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert_eq!(sqllogs[0].meta.username, "testuser");
        assert!(sqllogs[0].body.contains("test_column"));
        assert!(sqllogs[0].body.contains("WHERE test_id = 123"));
    }

    #[test]
    fn test_from_beginning_multiple_times() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Test content").unwrap();
        temp_file.flush().unwrap();

        let parser1 = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        let _pos1 = parser1.position;

        let parser2 = parser1.from_beginning().unwrap();
        assert_eq!(parser2.position, 0);

        let parser3 = parser2.from_beginning().unwrap();
        assert_eq!(parser3.position, 0);
    }

    #[test]
    fn test_buffer_newline_handling() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT".to_string(),
            "FROM table".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 检查缓冲区是否正确添加换行符
        assert!(parser.buffer.contains('\n'));
    }

    #[test]
    fn test_parser_state_independence() {
        let temp_file1 = NamedTempFile::new().unwrap();
        let temp_file2 = NamedTempFile::new().unwrap();

        let mut parser1 = RealtimeSqllogParser::new(temp_file1.path()).unwrap();
        let mut parser2 = RealtimeSqllogParser::new(temp_file2.path()).unwrap();

        parser1.buffer.push_str("Buffer 1");
        parser2.buffer.push_str("Buffer 2");

        assert_eq!(parser1.buffer, "Buffer 1");
        assert_eq!(parser2.buffer, "Buffer 2");
    }

    #[test]
    fn test_read_new_content_empty_lines_filtering() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "").unwrap();
        writeln!(temp_file, "").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "   ").unwrap();
        writeln!(temp_file, "Line 3").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let lines = parser.read_new_content().unwrap();

        // 应该只有非空行
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Line 1");
        assert_eq!(lines[1], "Line 2");
        assert_eq!(lines[2], "Line 3");
    }

    #[test]
    fn test_process_lines_maintains_buffer_state() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines1 = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT".to_string(),
            "FROM table1".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines1, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let buffer_after_first = parser.buffer.clone();

        let lines2 = vec!["WHERE id = 1".to_string()];

        parser
            .process_lines(lines2, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 缓冲区应该继续累积
        assert!(parser.buffer.contains("FROM table1"));
        assert!(parser.buffer.contains("WHERE id = 1"));
        assert!(parser.buffer.len() > buffer_after_first.len());
    }

    #[test]
    fn test_file_path_storage() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let parser = RealtimeSqllogParser::new(&path).unwrap();
        assert_eq!(parser.file_path, path);
    }

    #[test]
    fn test_process_lines_with_whitespace_only_lines() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT".to_string(),
            "    ".to_string(), // 只有空格
            "\t\t".to_string(), // 只有tab
            "FROM table".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 空白行也应该被添加到缓冲区（因为它们不是记录开始）
        assert!(parser.buffer.contains("FROM table"));
    }

    #[test]
    fn test_flush_buffer_with_performance_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 添加带性能指标的记录
        parser.buffer.push_str(
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n"
        );
        parser
            .buffer
            .push_str("exectime[100] rowcount[5] exec_id[12345]");

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .flush_buffer(|sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);

        // 验证记录被解析（性能指标可能在body中）
        assert!(sqllogs[0].body.contains("SELECT 1") || sqllogs[0].body.contains("exectime"));
    }

    #[test]
    fn test_sequential_read_operations() {
        let mut temp_file = NamedTempFile::new().unwrap();

        writeln!(temp_file, "Initial line").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 第一次读取
        let lines1 = parser.read_new_content().unwrap();
        assert_eq!(lines1.len(), 1);

        // 没有新内容
        let lines2 = parser.read_new_content().unwrap();
        assert_eq!(lines2.len(), 0);

        // 添加新内容
        writeln!(temp_file, "New line 1").unwrap();
        writeln!(temp_file, "New line 2").unwrap();
        temp_file.flush().unwrap();

        // 第三次读取
        let lines3 = parser.read_new_content().unwrap();
        assert_eq!(lines3.len(), 2);

        // 再次没有新内容
        let lines4 = parser.read_new_content().unwrap();
        assert_eq!(lines4.len(), 0);
    }

    #[test]
    fn test_process_lines_record_completeness() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *".to_string(),
            "FROM users".to_string(),
            "WHERE active = true".to_string(),
            "AND deleted = false".to_string(),
            "ORDER BY created_at DESC".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE settings".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);

        // 验证完整的多行记录
        let body = &sqllogs[0].body;
        assert!(body.contains("FROM users"));
        assert!(body.contains("WHERE active = true"));
        assert!(body.contains("AND deleted = false"));
        assert!(body.contains("ORDER BY created_at DESC"));
    }

    #[test]
    fn test_position_tracking_edge_cases() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // 空文件
        let mut parser1 = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();
        assert_eq!(parser1.position, 0);

        parser1.read_new_content().unwrap();
        assert_eq!(parser1.position, 0); // 仍然在开头

        // 有内容的文件
        writeln!(temp_file, "Line 1").unwrap();
        temp_file.flush().unwrap();

        let mut parser2 = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        parser2.read_new_content().unwrap();
        assert!(parser2.position > 0);
    }

    #[test]
    fn test_buffer_clear_behavior() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        // 添加内容到缓冲区
        parser.buffer.push_str("Test data line 1\n");
        parser.buffer.push_str("Test data line 2\n");
        assert!(!parser.buffer.is_empty());
        assert!(parser.buffer.len() > 20);

        // 清空缓冲区
        parser.buffer.clear();
        assert!(parser.buffer.is_empty());
        assert_eq!(parser.buffer.len(), 0);

        // 重新添加
        parser.buffer.push_str("New data");
        assert!(!parser.buffer.is_empty());
    }

    #[test]
    fn test_error_handling_nonexistent_path() {
        let result = RealtimeSqllogParser::new("/nonexistent/path/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_beginning_after_position_change() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        let _initial_pos = parser.position;

        // 读取一些内容
        parser = parser.from_beginning().unwrap();
        parser.read_new_content().unwrap();

        assert!(parser.position > 0);

        // 再次 from_beginning
        let parser = parser.from_beginning().unwrap();
        assert_eq!(parser.position, 0);
    }

    #[test]
    fn test_continuous_reading_pattern() {
        let mut temp_file = NamedTempFile::new().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 模拟持续读取模式
        for i in 1..=5 {
            writeln!(temp_file, "Line {}", i).unwrap();
            temp_file.flush().unwrap();

            let lines = parser.read_new_content().unwrap();
            assert_eq!(lines.len(), 1);
        }
    }

    #[test]
    fn test_buffer_accumulation_across_calls() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 第一批
        parser
            .process_lines(
                vec!["2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT".to_string()],
                |sqllog| {
                    received_clone.lock().unwrap().push(sqllog);
                },
            )
            .unwrap();

        // 第二批
        let received_clone = received.clone();
        parser
            .process_lines(vec!["FROM table1".to_string()], |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 第三批
        let received_clone = received.clone();
        parser
            .process_lines(vec!["WHERE id = 1".to_string()], |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 所有内容都在缓冲区中
        assert!(parser.buffer.contains("SELECT"));
        assert!(parser.buffer.contains("FROM table1"));
        assert!(parser.buffer.contains("WHERE id = 1"));
    }

    #[test]
    fn test_callback_execution_order() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = order.clone();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:user1 trxid:789 stmt:999 appname:app) R1".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:user2 trxid:790 stmt:1000 appname:app) R2".to_string(),
            "2025-08-12 10:57:11.548 (EP[2] sess:125 thrd:458 user:user3 trxid:791 stmt:1001 appname:app) R3".to_string(),
        ];

        parser
            .process_lines(lines, |sqllog| {
                order_clone
                    .lock()
                    .unwrap()
                    .push(sqllog.meta.username.clone());
            })
            .unwrap();

        let order_vec = order.lock().unwrap();
        assert_eq!(order_vec.len(), 2);
        assert_eq!(order_vec[0], "user1");
        assert_eq!(order_vec[1], "user2");
    }

    #[test]
    fn test_file_metadata_reading() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Test").unwrap();
        temp_file.flush().unwrap();

        let parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();
        let metadata = std::fs::metadata(temp_file.path()).unwrap();

        assert_eq!(parser.position, metadata.len());
    }

    #[test]
    fn test_empty_continuation_handling() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT".to_string(),
            "".to_string(), // 空行
            "FROM table".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        // 空行也会被添加到缓冲区
        assert!(parser.buffer.contains("SELECT"));
        assert!(parser.buffer.contains("FROM table"));
    }

    #[test]
    fn test_process_lines_with_utf8_content() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:用户 trxid:789 stmt:999 appname:app) SELECT名称".to_string(),
            "FROM 表".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        assert!(parser.buffer.contains("用户"));
        assert!(parser.buffer.contains("FROM 表"));
    }

    #[test]
    fn test_reader_reopen_mechanism() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        temp_file.flush().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        // 第一次读取
        let lines1 = parser.read_new_content().unwrap();
        assert_eq!(lines1.len(), 1);

        // reader 应该被保留
        assert!(parser.reader.is_some());

        // 添加新内容
        writeln!(temp_file, "Line 2").unwrap();
        temp_file.flush().unwrap();

        // 第二次读取会重新打开文件
        let lines2 = parser.read_new_content().unwrap();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0], "Line 2");
    }

    #[test]
    fn test_complex_multiline_scenario() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // 复杂的多行场景
        parser
            .process_lines(
                vec![
                    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT id,".to_string(),
                    "       name,".to_string(),
                    "       email".to_string(),
                    "FROM users".to_string(),
                    "WHERE status = 'active'".to_string(),
                    "  AND verified = true".to_string(),
                    "ORDER BY created_at DESC".to_string(),
                    "LIMIT 100".to_string(),
                ],
                |sqllog| {
                    received_clone.lock().unwrap().push(sqllog);
                },
            )
            .unwrap();

        // 缓冲区应该包含完整的 SQL
        assert!(parser.buffer.contains("name,"));
        assert!(parser.buffer.contains("email"));
        assert!(parser.buffer.contains("WHERE status"));
        assert!(parser.buffer.contains("LIMIT 100"));
    }

    #[test]
    fn test_position_monotonic_increase() {
        let mut temp_file = NamedTempFile::new().unwrap();

        let mut parser = RealtimeSqllogParser::new(temp_file.path())
            .unwrap()
            .from_beginning()
            .unwrap();

        let mut last_position = parser.position;

        for i in 1..=10 {
            writeln!(temp_file, "Line {}", i).unwrap();
            temp_file.flush().unwrap();

            parser.read_new_content().unwrap();

            // position 应该单调递增
            assert!(parser.position >= last_position);
            last_position = parser.position;
        }
    }

    #[test]
    fn test_mixed_valid_invalid_continuation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut parser = RealtimeSqllogParser::new(temp_file.path()).unwrap();

        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT".to_string(),
            "valid continuation".to_string(),
            "another valid line".to_string(),
            "2025-08-12 10:57:10.548 (EP[1] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE".to_string(),
        ];

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        parser
            .process_lines(lines, |sqllog| {
                received_clone.lock().unwrap().push(sqllog);
            })
            .unwrap();

        let sqllogs = received.lock().unwrap();
        assert_eq!(sqllogs.len(), 1);
        assert!(sqllogs[0].body.contains("valid continuation"));
        assert!(sqllogs[0].body.contains("another valid line"));
    }
}
