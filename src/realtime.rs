//! # 实时/增量解析模块
//!
//! 本模块提供实时解析 sqllog 日志文件的功能，支持：
//! - 增量读取文件内容（只处理新增部分）
//! - 维护解析状态（跟踪文件读取位置）
//! - 文件监听模式（持续监控文件变化）
//! - 零拷贝或低拷贝的增量解析
//!
//! ## 使用场景
//!
//! - 实时监控数据库日志
//! - 增量解析大型日志文件
//! - 流式处理持续增长的日志
//!
//! ## 快速开始
//!
//! ```no_run
//! use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};
//! use std::time::Duration;
//!
//! let config = ParserConfig {
//!     file_path: "sqllog.log".into(),
//!     poll_interval: Duration::from_secs(1),
//!     buffer_size: 8192,
//! };
//!
//! let mut parser = RealtimeParser::new(config)?;
//!
//! // 处理新增记录
//! parser.parse_new_records(|parsed| {
//!     println!("用户: {}, SQL: {}", parsed.user, parsed.body);
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::error::ParseError;
use crate::parser::{ParsedRecord, RecordSplitter, parse_record};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

/// 实时解析器配置
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// 日志文件路径
    pub file_path: PathBuf,
    /// 轮询间隔（用于监听模式）
    pub poll_interval: Duration,
    /// 读取缓冲区大小（字节）
    pub buffer_size: usize,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            file_path: PathBuf::from("sqllog.log"),
            poll_interval: Duration::from_secs(1),
            buffer_size: 8192, // 8KB
        }
    }
}

/// 实时解析器
///
/// 维护文件读取状态，支持增量解析和持续监听。
///
/// # 字段说明
///
/// - `config`: 解析器配置
/// - `file_position`: 当前文件读取位置（字节偏移）
/// - `incomplete_buffer`: 缓存不完整的记录片段（跨读取边界的记录）
pub struct RealtimeParser {
    config: ParserConfig,
    file_position: u64,
    incomplete_buffer: String,
}

impl RealtimeParser {
    /// 创建新的实时解析器
    ///
    /// # 参数
    ///
    /// * `config` - 解析器配置
    ///
    /// # 返回值
    ///
    /// 返回 `Result<RealtimeParser, ParseError>`
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};
    ///
    /// let config = ParserConfig::default();
    /// let parser = RealtimeParser::new(config)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(config: ParserConfig) -> Result<Self, ParseError> {
        // 验证文件存在
        if !config.file_path.exists() {
            return Err(ParseError::FileNotFound(
                config.file_path.display().to_string(),
            ));
        }

        Ok(Self {
            config,
            file_position: 0,
            incomplete_buffer: String::new(),
        })
    }

    /// 从指定位置开始解析
    ///
    /// 允许从文件的特定位置开始增量解析，适用于恢复中断的解析任务。
    ///
    /// # 参数
    ///
    /// * `position` - 文件字节偏移量
    pub fn seek_to(&mut self, position: u64) {
        self.file_position = position;
        self.incomplete_buffer.clear();
    }

    /// 获取当前文件读取位置
    pub fn position(&self) -> u64 {
        self.file_position
    }

    /// 解析文件中的新增记录
    ///
    /// 从上次读取的位置继续，解析所有新增的完整记录。
    /// 不完整的记录（跨越读取边界）会被缓存到下次调用时处理。
    ///
    /// # 参数
    ///
    /// * `callback` - 处理每条解析后记录的回调函数
    ///
    /// # 返回值
    ///
    /// 返回本次解析的记录数量
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};
    ///
    /// let mut parser = RealtimeParser::new(ParserConfig::default())?;
    /// let count = parser.parse_new_records(|parsed| {
    ///     println!("时间: {}, 用户: {}", parsed.ts, parsed.user);
    /// })?;
    /// println!("解析了 {} 条新记录", count);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse_new_records<F>(&mut self, mut callback: F) -> Result<usize, ParseError>
    where
        F: FnMut(ParsedRecord),
    {
        let file = File::open(&self.config.file_path).map_err(|e| {
            ParseError::FileNotFound(format!("{}: {}", self.config.file_path.display(), e))
        })?;

        let mut reader = BufReader::new(file);

        // 定位到上次读取的位置
        reader
            .seek(SeekFrom::Start(self.file_position))
            .map_err(|e| ParseError::FileNotFound(format!("Seek error: {}", e)))?;

        // 读取新内容
        let mut buffer = vec![0u8; self.config.buffer_size];
        let mut new_content = String::new();

        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .map_err(|e| ParseError::FileNotFound(format!("Read error: {}", e)))?;

            if bytes_read == 0 {
                break; // EOF
            }

            // 转换为字符串并追加
            let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
            new_content.push_str(&chunk);
            self.file_position += bytes_read as u64;
        }

        if new_content.is_empty() && self.incomplete_buffer.is_empty() {
            return Ok(0); // 没有新内容
        }

        // 检查新内容是否以换行符结束
        let has_trailing_newline = new_content.ends_with('\n') || new_content.ends_with('\r');

        // 合并上次的不完整缓冲区和新内容
        let full_text = if self.incomplete_buffer.is_empty() {
            new_content
        } else {
            format!("{}{}", self.incomplete_buffer, new_content)
        };

        // 解析记录
        let mut count = 0;
        let mut last_complete_end = 0;

        for record in RecordSplitter::new(&full_text) {
            // 检查是否是完整记录（以换行符结束或是最后一条）
            let record_end_in_full =
                record.as_ptr() as usize - full_text.as_ptr() as usize + record.len();

            // 如果记录末尾不是换行符且不是文件末尾，可能是不完整的记录
            let is_complete = record.ends_with('\n')
                || record.ends_with('\r')
                || (record_end_in_full == full_text.len() && has_trailing_newline);

            if is_complete {
                let parsed = parse_record(record);
                callback(parsed);
                count += 1;
                last_complete_end = record_end_in_full;
            } else {
                // 不完整记录，保存到缓冲区
                break;
            }
        }

        // 更新不完整缓冲区
        if last_complete_end < full_text.len() {
            self.incomplete_buffer = full_text[last_complete_end..].to_string();
        } else {
            self.incomplete_buffer.clear();
        }

        Ok(count)
    }

    /// 持续监听文件并解析新记录
    ///
    /// 在一个循环中持续监控文件变化，解析新增记录。
    /// 注意：此函数会阻塞当前线程。
    ///
    /// # 参数
    ///
    /// * `callback` - 处理每条解析后记录的回调函数
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use dm_database_parser_sqllog::realtime::{RealtimeParser, ParserConfig};
    /// use std::time::Duration;
    ///
    /// let config = ParserConfig {
    ///     poll_interval: Duration::from_secs(2),
    ///     ..Default::default()
    /// };
    ///
    /// let mut parser = RealtimeParser::new(config)?;
    /// parser.watch(|parsed| {
    ///     println!("新记录: 用户={}, SQL={}", parsed.user, parsed.body);
    /// })?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn watch<F>(&mut self, mut callback: F) -> Result<(), ParseError>
    where
        F: FnMut(ParsedRecord),
    {
        loop {
            let count = self.parse_new_records(&mut callback)?;
            if count > 0 {
                // 有新记录时立即检查下一批
                continue;
            }
            // 没有新记录时等待一段时间
            std::thread::sleep(self.config.poll_interval);
        }
    }

    /// 解析文件中的所有记录（从头开始）
    ///
    /// 重置文件位置并解析整个文件。
    ///
    /// # 参数
    ///
    /// * `callback` - 处理每条解析后记录的回调函数
    ///
    /// # 返回值
    ///
    /// 返回解析的总记录数
    pub fn parse_all<F>(&mut self, callback: F) -> Result<usize, ParseError>
    where
        F: FnMut(ParsedRecord),
    {
        self.seek_to(0);
        self.parse_new_records(callback)
    }

    /// 重置解析器状态
    ///
    /// 清空缓冲区并重置文件位置到起始处。
    pub fn reset(&mut self) {
        self.file_position = 0;
        self.incomplete_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_incremental_parsing() -> Result<(), Box<dyn std::error::Error>> {
        // 创建临时文件
        let mut temp_file = NamedTempFile::new()?;

        // 写入第一批记录
        writeln!(
            temp_file,
            "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1"
        )?;
        temp_file.flush()?;

        let config = ParserConfig {
            file_path: temp_file.path().to_path_buf(),
            poll_interval: Duration::from_millis(100),
            buffer_size: 1024,
        };

        let mut parser = RealtimeParser::new(config)?;

        // 解析第一批
        let mut count = 0;
        parser.parse_new_records(|_| count += 1)?;
        assert_eq!(count, 1);

        // 写入第二批记录
        writeln!(
            temp_file,
            "2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2"
        )?;
        temp_file.flush()?;

        // 解析第二批（增量）
        count = 0;
        parser.parse_new_records(|_| count += 1)?;
        assert_eq!(count, 1);

        Ok(())
    }
}
