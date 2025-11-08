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
        let file = File::open(&file_path).map_err(|e| {
            ParseError::IoError(format!("Failed to open file: {}", e))
        })?;

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
            let file = File::open(&self.file_path).map_err(|e| {
                ParseError::IoError(format!("Failed to reopen file: {}", e))
            })?;

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
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    ) {
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
    pub fn watch_for<F>(
        mut self,
        duration: Duration,
        mut callback: F,
    ) -> Result<(), ParseError>
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
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    ) {
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
    }

    #[test]
    fn test_nonexistent_file() {
        let parser = RealtimeSqllogParser::new("/nonexistent/file.txt");
        assert!(parser.is_err());
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
}
