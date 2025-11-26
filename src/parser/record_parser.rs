//! RecordParser - 从 Reader 流式读取并解析 Record
//!
//! 提供了一个迭代器，可以从任何实现了 `Read` trait 的源中逐条读取日志记录。

use crate::error::ParseError;
use crate::parser::record::Record;
use crate::sqllog::Sqllog;
use crate::tools::is_record_start_line;
use rayon::prelude::*;
use std::collections::VecDeque;
use std::{
    io::{self, BufRead, BufReader, Read},
    mem,
};

/// 从 Reader 中按行读取并解析成 Record 的迭代器
///
/// `RecordParser` 实现了 `Iterator` trait，可以逐条读取日志记录。
/// 它会自动识别记录的起始行和继续行，并将它们组合成完整的 `Record`。
///
/// # 类型参数
///
/// * `R` - 实现了 `Read` trait 的类型
pub struct RecordParser<R: Read> {
    reader: BufReader<R>,
    buffer: String,
    next_line: Option<String>,
    finished: bool,
}

impl<R: Read> RecordParser<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: String::new(),
            next_line: None,
            finished: false,
        }
    }

    /// 读取下一行
    fn read_line(&mut self) -> io::Result<Option<String>> {
        self.buffer.clear();
        let bytes_read = self.reader.read_line(&mut self.buffer)?;

        if bytes_read == 0 {
            Ok(None)
        } else {
            // 优化：原地移除换行符，避免创建新字符串
            let mut len = self.buffer.len();
            while len > 0 {
                let last_byte = self.buffer.as_bytes()[len - 1];
                if last_byte == b'\n' || last_byte == b'\r' {
                    len -= 1;
                } else {
                    break;
                }
            }

            // 只在需要时才创建新字符串（避免额外的 trim + to_string 开销）
            if len != self.buffer.len() {
                self.buffer.truncate(len);
            }

            // 使用 mem::take 避免额外的克隆，保持缓冲区容量
            Ok(Some(mem::take(&mut self.buffer)))
        }
    }

    /// 获取下一个记录的起始行
    fn get_start_line(&mut self) -> io::Result<Option<String>> {
        // 如果有缓存的下一行（上次读取时遇到的新起始行）
        if let Some(line) = self.next_line.take() {
            return Ok(Some(line));
        }

        // 读取并跳过非起始行，直到找到第一个有效起始行
        loop {
                match self.read_line()? {
                Some(line) if crate::tools::is_probable_record_start_line(&line) => return Ok(Some(line)),
                Some(_) => continue, // 跳过非起始行
                None => {
                    self.finished = true;
                    return Ok(None);
                }
            }
        }
    }

    /// 读取当前记录的所有继续行
    fn read_continuation_lines(&mut self, record: &mut Record) -> io::Result<()> {
        loop {
            match self.read_line()? {
                Some(line) if is_record_start_line(&line) => {
                    // 遇到下一个起始行，保存它并结束当前记录
                    self.next_line = Some(line);
                    break;
                }
                Some(line) => {
                    // 继续行
                    record.add_line(line);
                }
                None => {
                    // 文件结束
                    self.finished = true;
                    break;
                }
            }
        }
        Ok(())
    }
}

impl<R: Read> Iterator for RecordParser<R> {
    type Item = io::Result<Record>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        // 获取记录的起始行
        let start_line = match self.get_start_line() {
            Ok(Some(line)) => line,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };

        let mut record = Record::new(start_line);

        // 读取继续行
        match self.read_continuation_lines(&mut record) {
            Ok(()) => Some(Ok(record)),
            Err(e) => Some(Err(e)),
        }
    }
}

/// Sqllog 迭代器，使用批量缓冲 + 并行处理优化性能
///
/// 该迭代器用于从 `RecordParser` 并行转换为 `Sqllog`，并在 crate 内部使用。
pub(crate) struct SqllogIterator<R: Read> {
    record_parser: RecordParser<R>,
    buffer: VecDeque<Result<Sqllog, ParseError>>,
    batch_size: usize,
}

impl<R: Read> SqllogIterator<R> {
    /// 创建新的 SqllogIterator，使用默认批次大小（10000）
    pub(crate) fn new(record_parser: RecordParser<R>) -> Self {
        Self {
            record_parser,
            buffer: VecDeque::new(),
            batch_size: 10000, // 每次并行处理 1万条
        }
    }

    /// 填充缓冲区：批量读取记录并并行解析
    fn fill_buffer(&mut self) {
        let mut records: Vec<Record> = Vec::with_capacity(self.batch_size);

        // 批量读取记录
        for _ in 0..self.batch_size {
            match self.record_parser.next() {
                Some(Ok(record)) => records.push(record),
                Some(Err(io_err)) => {
                    self.buffer
                        .push_back(Err(ParseError::IoError(io_err.to_string())));
                }
                None => break,
            }
        }

        if records.is_empty() {
            return;
        }

        // 并行解析
        let results: Vec<Result<Sqllog, ParseError>> = records
            .par_iter()
            .map(|record| record.parse_to_sqllog())
            .collect();

        // 将结果放入缓冲区
        for result in results {
            self.buffer.push_back(result);
        }
    }
}

impl<R: Read> Iterator for SqllogIterator<R> {
    type Item = Result<Sqllog, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        // 如果缓冲区为空，尝试填充
        if self.buffer.is_empty() {
            self.fill_buffer();
        }

        // 从缓冲区取出结果
        self.buffer.pop_front()
    }
}
