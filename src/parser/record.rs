//! Record 结构定义和相关方法
//!
//! Record 表示一条原始的日志记录，可能包含多行（起始行 + 继续行）。

use crate::error::ParseError;
use crate::parser::parse_functions;
use crate::sqllog::Sqllog;

/// 表示一条完整的日志记录（可能包含多行）
///
/// 日志记录由一个起始行和零个或多个继续行组成。起始行包含时间戳和元数据，
/// 继续行包含多行 SQL 语句的后续部分。
///

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// 记录的所有行（第一行是起始行，后续行是继续行）
    pub lines: Vec<String>,
}

impl Record {
    /// 创建新的记录
    ///
    /// # 参数
    ///
    /// * `start_line` - 记录的起始行
    pub fn new(start_line: String) -> Self {
        Self {
            lines: vec![start_line],
        }
    }

    /// 添加继续行
    ///
    /// # 参数
    ///
    /// * `line` - 要添加的继续行
    pub fn add_line(&mut self, line: String) {
        self.lines.push(line);
    }

    /// 获取起始行
    ///
    /// # 返回
    ///
    /// 返回记录的第一行（起始行）
    pub fn start_line(&self) -> &str {
        &self.lines[0]
    }

    /// 获取所有行
    ///
    /// # 返回
    ///
    /// 返回包含所有行的切片
    pub fn all_lines(&self) -> &[String] {
        &self.lines
    }

    /// 获取完整的记录内容（所有行拼接）
    ///
    /// # 返回
    ///
    /// 返回所有行用换行符拼接后的字符串
    pub fn full_content(&self) -> String {
        self.lines.join("\n")
    }

    /// 判断是否有继续行
    ///
    /// # 返回
    ///
    /// 如果记录包含多行（有继续行）返回 `true`，否则返回 `false`
    pub fn has_continuation_lines(&self) -> bool {
        self.lines.len() > 1
    }

    /// 将 Record 解析为 Sqllog
    ///
    /// # 返回
    ///
    /// * `Ok(Sqllog)` - 解析成功
    /// * `Err(ParseError)` - 解析失败
    pub fn parse_to_sqllog(&self) -> Result<Sqllog, ParseError> {
        let lines: Vec<&str> = self.lines.iter().map(|s| s.as_str()).collect();
        parse_functions::parse_record(&lines)
    }
}
