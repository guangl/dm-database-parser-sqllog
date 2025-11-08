use crate::error::ParseError;
use crate::sqllog::{IndicatorsParts, MetaParts, Sqllog};
use crate::tools::is_record_start_line;
use once_cell::sync::Lazy;
use std::io::{BufRead, BufReader, Read};

// 常量定义
const TIMESTAMP_LENGTH: usize = 23;
const MIN_RECORD_LENGTH: usize = 25;
const META_START_INDEX: usize = 25;
const BODY_OFFSET: usize = 2; // ") " 两个字符

// 使用 Lazy 静态初始化 indicator 模式集合，避免重复创建
static INDICATOR_PATTERNS: Lazy<[&'static str; 3]> =
    Lazy::new(|| ["EXECTIME:", "ROWCOUNT:", "EXEC_ID:"]);

// Meta 字段前缀常量
static SESS_PREFIX: &str = "sess:";
static THRD_PREFIX: &str = "thrd:";
static USER_PREFIX: &str = "user:";
static TRXID_PREFIX: &str = "trxid:";
static STMT_PREFIX: &str = "stmt:";
static APPNAME_PREFIX: &str = "appname:";
static IP_PREFIX: &str = "ip:::ffff:";

// Indicator 相关的静态常量
static EXECTIME_PREFIX: &str = "EXECTIME: ";
static EXECTIME_SUFFIX: &str = "(ms)";
static ROWCOUNT_PREFIX: &str = "ROWCOUNT: ";
static ROWCOUNT_SUFFIX: &str = "(rows)";
static EXEC_ID_PREFIX: &str = "EXEC_ID: ";
static EXEC_ID_SUFFIX: &str = ".";

/// 表示一条完整的日志记录（可能包含多行）
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// 记录的所有行（第一行是起始行，后续行是继续行）
    pub lines: Vec<String>,
}

impl Record {
    /// 创建新的记录
    pub fn new(start_line: String) -> Self {
        Self {
            lines: vec![start_line],
        }
    }

    /// 添加继续行
    pub fn add_line(&mut self, line: String) {
        self.lines.push(line);
    }

    /// 获取起始行
    pub fn start_line(&self) -> &str {
        &self.lines[0]
    }

    /// 获取所有行
    pub fn all_lines(&self) -> &[String] {
        &self.lines
    }

    /// 获取完整的记录内容（所有行拼接）
    pub fn full_content(&self) -> String {
        self.lines.join("\n")
    }

    /// 判断是否有继续行
    pub fn has_continuation_lines(&self) -> bool {
        self.lines.len() > 1
    }

    /// 将 Record 解析为 Sqllog
    pub fn parse_to_sqllog(&self) -> Result<Sqllog, ParseError> {
        let lines: Vec<&str> = self.lines.iter().map(|s| s.as_str()).collect();
        parse_record(&lines)
    }
}

/// 从 Reader 中按行读取并解析成 Record 的迭代器
pub struct RecordParser<R: Read> {
    reader: BufReader<R>,
    buffer: String,
    next_line: Option<String>,
    finished: bool,
}

impl<R: Read> RecordParser<R> {
    /// 创建新的 RecordParser
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: String::new(),
            next_line: None,
            finished: false,
        }
    }

    /// 读取下一行
    fn read_line(&mut self) -> std::io::Result<Option<String>> {
        self.buffer.clear();
        let bytes_read = self.reader.read_line(&mut self.buffer)?;

        if bytes_read == 0 {
            Ok(None)
        } else {
            // 移除行尾的换行符
            let line = self.buffer.trim_end_matches(&['\r', '\n'][..]).to_string();
            Ok(Some(line))
        }
    }
}

impl<R: Read> Iterator for RecordParser<R> {
    type Item = std::io::Result<Record>;

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

impl<R: Read> RecordParser<R> {
    /// 获取下一个记录的起始行
    fn get_start_line(&mut self) -> std::io::Result<Option<String>> {
        // 如果有缓存的下一行（上次读取时遇到的新起始行）
        if let Some(line) = self.next_line.take() {
            return Ok(Some(line));
        }

        // 读取并跳过非起始行，直到找到第一个有效起始行
        loop {
            match self.read_line()? {
                Some(line) if is_record_start_line(&line) => return Ok(Some(line)),
                Some(_) => continue, // 跳过非起始行
                None => {
                    self.finished = true;
                    return Ok(None);
                }
            }
        }
    }

    /// 读取当前记录的所有继续行
    fn read_continuation_lines(&mut self, record: &mut Record) -> std::io::Result<()> {
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

/// 便捷函数：从字符串解析记录
pub fn parse_records_from_string(content: &str) -> Vec<Record> {
    let cursor = std::io::Cursor::new(content.as_bytes());
    RecordParser::new(cursor).filter_map(|r| r.ok()).collect()
}

/// 将 RecordParser 转换为 Sqllog 迭代器的适配器
pub struct SqllogParser<R: Read> {
    record_parser: RecordParser<R>,
}

impl<R: Read> SqllogParser<R> {
    /// 创建新的 SqllogParser
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

/// 便捷函数：从字符串直接解析为 Sqllog 列表
pub fn parse_sqllogs_from_string(content: &str) -> Vec<Result<Sqllog, ParseError>> {
    let cursor = std::io::Cursor::new(content.as_bytes());
    SqllogParser::new(cursor).collect()
}

/// 从行数组解析成 Sqllog 结构
///
/// # 参数
/// - `lines`: 包含日志记录的行（第一行必须是有效的起始行）
///
/// # 返回
/// - `Ok(Sqllog)`: 解析成功
/// - `Err(ParseError)`: 解析失败
pub fn parse_record(lines: &[&str]) -> Result<Sqllog, ParseError> {
    if lines.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let first_line = lines[0];

    // 验证第一行格式
    if !is_record_start_line(first_line) {
        return Err(ParseError::InvalidRecordStartLine);
    }

    // 验证行长度
    if first_line.len() < MIN_RECORD_LENGTH {
        return Err(ParseError::LineTooShort(first_line.len()));
    }

    // 解析时间戳
    let ts = &first_line[0..TIMESTAMP_LENGTH];

    // 查找 meta 部分的右括号
    let closing_paren = first_line
        .find(')')
        .ok_or(ParseError::MissingClosingParen)?;

    if closing_paren <= META_START_INDEX {
        return Err(ParseError::InsufficientMetaFields(0));
    }

    // 解析 meta 部分
    let meta_str = &first_line[META_START_INDEX..closing_paren];
    let meta = parse_meta(meta_str)?;

    // 构建 body（包含继续行）
    let body_start = closing_paren + BODY_OFFSET;
    let full_body = build_body(first_line, body_start, &lines[1..]);

    // 尝试解析 indicators（可选）
    let indicators = parse_indicators(&full_body).ok();

    // 提取纯 SQL body（移除 indicators）
    let body = if indicators.is_some() {
        extract_sql_body(&full_body)
    } else {
        full_body
    };

    Ok(Sqllog {
        ts: ts.to_string(),
        meta,
        body,
        indicators,
    })
}

/// 构建完整的 body（包含所有继续行）
#[inline]
fn build_body(first_line: &str, body_start: usize, continuation_lines: &[&str]) -> String {
    if continuation_lines.is_empty() {
        // 只有单行
        if body_start < first_line.len() {
            first_line[body_start..].to_string()
        } else {
            String::new()
        }
    } else {
        // 有多行，计算总容量并预分配
        let has_first_part = body_start < first_line.len();
        let first_part_len = if has_first_part {
            first_line.len() - body_start
        } else {
            0
        };

        let newline_count = if has_first_part {
            continuation_lines.len()
        } else {
            continuation_lines.len() - 1
        };

        let total_len = first_part_len
            + continuation_lines.iter().map(|s| s.len()).sum::<usize>()
            + newline_count;

        let mut result = String::with_capacity(total_len);

        if has_first_part {
            result.push_str(&first_line[body_start..]);
            for line in continuation_lines {
                result.push('\n');
                result.push_str(line);
            }
        } else {
            // 第一行为空，从第一个 continuation_line 开始
            result.push_str(continuation_lines[0]);
            for line in &continuation_lines[1..] {
                result.push('\n');
                result.push_str(line);
            }
        }

        result
    }
}

/// 从 full_body 中提取 SQL 部分（移除 indicators）
#[inline]
fn extract_sql_body(full_body: &str) -> String {
    // 使用预定义的 INDICATOR_PATTERNS 避免每次创建数组
    INDICATOR_PATTERNS
        .iter()
        .filter_map(|pattern| full_body.find(pattern))
        .min()
        .map(|pos| full_body[..pos].trim_end().to_string())
        .unwrap_or_else(|| full_body.to_string())
}

/// 解析 meta 字符串
fn parse_meta(meta_str: &str) -> Result<MetaParts, ParseError> {
    let fields: Vec<&str> = meta_str.split(' ').collect();

    if fields.len() < 7 {
        return Err(ParseError::InsufficientMetaFields(fields.len()));
    }

    // 解析 EP
    let ep = parse_ep_field(fields[0])?;

    // 解析必需字段 - 使用静态常量避免字符串字面量重复
    let sess_id = extract_field_value(fields[1], SESS_PREFIX)?;
    let thrd_id = extract_field_value(fields[2], THRD_PREFIX)?;
    let username = extract_field_value(fields[3], USER_PREFIX)?;
    let trxid = extract_field_value(fields[4], TRXID_PREFIX)?;
    let statement = extract_field_value(fields[5], STMT_PREFIX)?;
    let appname = extract_field_value(fields[6], APPNAME_PREFIX)?;

    // 可选的 client_ip
    let client_ip = fields
        .get(7)
        .map(|field| extract_field_value(field, IP_PREFIX))
        .transpose()?
        .unwrap_or_default();

    Ok(MetaParts {
        ep,
        sess_id,
        thrd_id,
        username,
        trxid,
        statement,
        appname,
        client_ip,
    })
}

/// 解析 EP 字段
#[inline]
fn parse_ep_field(ep_str: &str) -> Result<u8, ParseError> {
    if !ep_str.starts_with("EP[") || !ep_str.ends_with(']') {
        return Err(ParseError::InvalidEpFormat(ep_str.to_string()));
    }

    let ep_num = &ep_str[3..ep_str.len() - 1];
    ep_num
        .parse::<u8>()
        .map_err(|_| ParseError::EpParseError(ep_num.to_string()))
}

/// 从字段中提取值
#[inline]
fn extract_field_value(field: &str, prefix: &str) -> Result<String, ParseError> {
    field
        .strip_prefix(prefix)
        .map(|s| s.to_string())
        .ok_or_else(|| ParseError::InvalidFieldFormat {
            expected: prefix.to_string(),
            actual: field.to_string(),
        })
}

/// 解析 indicators 部分
fn parse_indicators(body: &str) -> Result<IndicatorsParts, ParseError> {
    // 使用预定义的静态常量，避免每次创建字符串
    let exec_time_str = extract_indicator(body, EXECTIME_PREFIX, EXECTIME_SUFFIX)?;
    let row_count_str = extract_indicator(body, ROWCOUNT_PREFIX, ROWCOUNT_SUFFIX)?;
    let exec_id_str = extract_indicator(body, EXEC_ID_PREFIX, EXEC_ID_SUFFIX)?;

    let execute_time = exec_time_str.parse::<f32>().map_err(|_| {
        ParseError::IndicatorsParseError(format!("执行时间解析失败: {}", exec_time_str))
    })?;

    let row_count = row_count_str.parse::<u32>().map_err(|_| {
        ParseError::IndicatorsParseError(format!("行数解析失败: {}", row_count_str))
    })?;

    let execute_id = exec_id_str.parse::<i64>().map_err(|_| {
        ParseError::IndicatorsParseError(format!("执行 ID 解析失败: {}", exec_id_str))
    })?;

    Ok(IndicatorsParts {
        execute_time,
        row_count,
        execute_id,
    })
}

/// 提取 indicator 值
#[inline]
fn extract_indicator<'a>(text: &'a str, prefix: &str, suffix: &str) -> Result<&'a str, ParseError> {
    let start_pos = text
        .find(prefix)
        .ok_or_else(|| ParseError::IndicatorsParseError(format!("未找到 {}", prefix)))?
        + prefix.len();

    let remaining = &text[start_pos..];
    let end_offset = remaining
        .find(suffix)
        .ok_or_else(|| ParseError::IndicatorsParseError(format!("未找到 {}", suffix)))?;

    Ok(remaining[..end_offset].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_line_record() {
        let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
        let records = parse_records_from_string(input);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].lines.len(), 1);
        assert_eq!(records[0].start_line(), input);
        assert!(!records[0].has_continuation_lines());
    }

    #[test]
    fn test_multi_line_record() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
WHERE id = 1"#;

        let records = parse_records_from_string(input);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].lines.len(), 3);
        assert!(records[0].has_continuation_lines());
        assert!(records[0].start_line().starts_with("2025-08-12"));
        assert_eq!(records[0].all_lines()[1], "FROM users");
        assert_eq!(records[0].all_lines()[2], "WHERE id = 1");
    }

    #[test]
    fn test_multiple_records() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) INSERT INTO table"#;

        let records = parse_records_from_string(input);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].lines.len(), 1);
        assert_eq!(records[1].lines.len(), 1);
    }

    #[test]
    fn test_multiple_records_with_continuation() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM table1
WHERE id = 1
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE table2
SET name = 'test'
WHERE id = 2"#;

        let records = parse_records_from_string(input);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].lines.len(), 3);
        assert_eq!(records[1].lines.len(), 3);
        assert!(records[0].has_continuation_lines());
        assert!(records[1].has_continuation_lines());
    }

    #[test]
    fn test_skip_invalid_lines_at_start() {
        let input = r#"Some garbage line
Another invalid line
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"#;

        let records = parse_records_from_string(input);

        assert_eq!(records.len(), 1);
        assert!(records[0].start_line().starts_with("2025-08-12"));
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let records = parse_records_from_string(input);
        assert_eq!(records.len(), 0);
    }

    #[test]
    fn test_full_content() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
WHERE id = 1"#;

        let records = parse_records_from_string(input);
        assert_eq!(records[0].full_content(), input);
    }

    #[test]
    fn test_parse_record_single_line() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:0x999 appname:app ip:::ffff:10.0.0.1) SELECT 1",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.ts, "2025-08-12 10:57:09.548");
        assert_eq!(sqllog.meta.ep, 0);
        assert_eq!(sqllog.meta.sess_id, "0x123");
        assert_eq!(sqllog.meta.thrd_id, "456");
        assert_eq!(sqllog.meta.username, "alice");
        assert_eq!(sqllog.meta.trxid, "789");
        assert_eq!(sqllog.meta.statement, "0x999");
        assert_eq!(sqllog.meta.appname, "app");
        assert_eq!(sqllog.meta.client_ip, "10.0.0.1");
        assert_eq!(sqllog.body, "SELECT 1");
        assert!(sqllog.indicators.is_none());
    }

    #[test]
    fn test_parse_record_with_indicators() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.body, "SELECT 1");

        assert!(sqllog.indicators.is_some());
        let indicators = sqllog.indicators.unwrap();
        assert_eq!(indicators.execute_time, 10.0);
        assert_eq!(indicators.row_count, 5);
        assert_eq!(indicators.execute_id, 12345);
    }

    #[test]
    fn test_parse_record_multiline() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *",
            "FROM users",
            "WHERE id = 1",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.body, "SELECT *\nFROM users\nWHERE id = 1");
    }

    #[test]
    fn test_parse_record_empty_input() {
        let lines: Vec<&str> = vec![];
        let result = parse_record(&lines);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParseError::EmptyInput));
    }

    #[test]
    fn test_parse_record_invalid_format() {
        let lines = vec!["not a valid log line"];
        let result = parse_record(&lines);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InvalidRecordStartLine
        ));
    }

    #[test]
    fn test_parse_record_without_ip() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.meta.client_ip, "");
    }

    #[test]
    fn test_record_parse_to_sqllog() {
        let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
        let records = parse_records_from_string(input);

        assert_eq!(records.len(), 1);
        let sqllog = records[0].parse_to_sqllog().unwrap();
        assert_eq!(sqllog.meta.username, "alice");
        assert_eq!(sqllog.body, "SELECT 1");
    }

    #[test]
    fn test_sqllog_parser() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) INSERT INTO table"#;

        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = SqllogParser::new(cursor);
        let sqllogs: Vec<_> = parser.collect();

        assert_eq!(sqllogs.len(), 2);
        assert!(sqllogs[0].is_ok());
        assert!(sqllogs[1].is_ok());

        let sqllog1 = sqllogs[0].as_ref().unwrap();
        let sqllog2 = sqllogs[1].as_ref().unwrap();

        assert_eq!(sqllog1.meta.username, "alice");
        assert_eq!(sqllog2.meta.username, "bob");
    }

    #[test]
    fn test_parse_sqllogs_from_string() {
        let input = r#"2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *
FROM users
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) UPDATE table"#;

        let results = parse_sqllogs_from_string(input);
        assert_eq!(results.len(), 2);

        let sqllog1 = results[0].as_ref().unwrap();
        assert_eq!(sqllog1.body, "SELECT *\nFROM users");
        assert_eq!(sqllog1.meta.username, "alice");
    }

    // ==================== 辅助函数测试 ====================

    #[test]
    fn test_build_body_single_line() {
        let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
        let closing_paren = first_line.find(')').unwrap();
        let body_start = closing_paren + BODY_OFFSET;
        let continuation: &[&str] = &[];

        let body = build_body(first_line, body_start, continuation);
        assert_eq!(body, "SELECT 1");
    }

    #[test]
    fn test_build_body_multi_line() {
        let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *";
        let closing_paren = first_line.find(')').unwrap();
        let body_start = closing_paren + BODY_OFFSET;
        let continuation = &["FROM users", "WHERE id = 1"];

        let body = build_body(first_line, body_start, continuation);
        assert_eq!(body, "SELECT *\nFROM users\nWHERE id = 1");
    }

    #[test]
    fn test_build_body_empty_first_line() {
        let first_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app)";
        let body_start = first_line.len();
        let continuation = &["SELECT 1"];

        let body = build_body(first_line, body_start, continuation);
        assert_eq!(body, "SELECT 1");
    }

    #[test]
    fn test_extract_sql_body_with_exectime() {
        let full_body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.";
        let sql_body = extract_sql_body(full_body);
        assert_eq!(sql_body, "SELECT 1");
    }

    #[test]
    fn test_extract_sql_body_with_rowcount_first() {
        let full_body = "SELECT 1 ROWCOUNT: 5(rows) EXECTIME: 10(ms) EXEC_ID: 12345.";
        let sql_body = extract_sql_body(full_body);
        assert_eq!(sql_body, "SELECT 1");
    }

    #[test]
    fn test_extract_sql_body_without_indicators() {
        let full_body = "SELECT 1 FROM users";
        let sql_body = extract_sql_body(full_body);
        assert_eq!(sql_body, "SELECT 1 FROM users");
    }

    #[test]
    fn test_parse_ep_field_valid() {
        let result = parse_ep_field("EP[0]");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        let result = parse_ep_field("EP[15]");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 15);
    }

    #[test]
    fn test_parse_ep_field_invalid_format() {
        let result = parse_ep_field("EP0");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InvalidEpFormat(_)
        ));

        let result = parse_ep_field("[0]");
        assert!(result.is_err());

        let result = parse_ep_field("EP[");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ep_field_invalid_number() {
        let result = parse_ep_field("EP[abc]");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParseError::EpParseError(_)));

        let result = parse_ep_field("EP[256]"); // 超过 u8 范围
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_field_value_valid() {
        let result = extract_field_value("sess:123", "sess:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "123");

        let result = extract_field_value("user:alice", "user:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "alice");
    }

    #[test]
    fn test_extract_field_value_invalid_prefix() {
        let result = extract_field_value("sess:123", "user:");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InvalidFieldFormat { .. }
        ));
    }

    #[test]
    fn test_extract_field_value_empty_value() {
        let result = extract_field_value("sess:", "sess:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_parse_meta_valid() {
        let meta_str = "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app";
        let result = parse_meta(meta_str);
        assert!(result.is_ok());

        let meta = result.unwrap();
        assert_eq!(meta.ep, 0);
        assert_eq!(meta.sess_id, "123");
        assert_eq!(meta.thrd_id, "456");
        assert_eq!(meta.username, "alice");
        assert_eq!(meta.trxid, "789");
        assert_eq!(meta.statement, "999");
        assert_eq!(meta.appname, "app");
        assert_eq!(meta.client_ip, "");
    }

    #[test]
    fn test_parse_meta_with_ip() {
        let meta_str =
            "EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:10.0.0.1";
        let result = parse_meta(meta_str);
        assert!(result.is_ok());

        let meta = result.unwrap();
        assert_eq!(meta.client_ip, "10.0.0.1");
    }

    #[test]
    fn test_parse_meta_insufficient_fields() {
        let meta_str = "EP[0] sess:123 thrd:456";
        let result = parse_meta(meta_str);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InsufficientMetaFields(3)
        ));
    }

    #[test]
    fn test_parse_meta_invalid_ep() {
        let meta_str = "EP0 sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app";
        let result = parse_meta(meta_str);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InvalidEpFormat(_)
        ));
    }

    #[test]
    fn test_parse_indicators_valid() {
        let body = "SELECT 1 EXECTIME: 10.5(ms) ROWCOUNT: 100(rows) EXEC_ID: 12345.";
        let result = parse_indicators(body);
        assert!(result.is_ok());

        let indicators = result.unwrap();
        assert_eq!(indicators.execute_time, 10.5);
        assert_eq!(indicators.row_count, 100);
        assert_eq!(indicators.execute_id, 12345);
    }

    #[test]
    fn test_parse_indicators_integer_exectime() {
        let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.";
        let result = parse_indicators(body);
        assert!(result.is_ok());

        let indicators = result.unwrap();
        assert_eq!(indicators.execute_time, 10.0);
    }

    #[test]
    fn test_parse_indicators_missing_exectime() {
        let body = "SELECT 1 ROWCOUNT: 5(rows) EXEC_ID: 12345.";
        let result = parse_indicators(body);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::IndicatorsParseError(_)
        ));
    }

    #[test]
    fn test_parse_indicators_missing_rowcount() {
        let body = "SELECT 1 EXECTIME: 10(ms) EXEC_ID: 12345.";
        let result = parse_indicators(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_indicators_missing_exec_id() {
        let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows)";
        let result = parse_indicators(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_indicators_invalid_exectime_format() {
        let body = "SELECT 1 EXECTIME: abc(ms) ROWCOUNT: 5(rows) EXEC_ID: 12345.";
        let result = parse_indicators(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_indicators_invalid_rowcount_format() {
        let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: xyz(rows) EXEC_ID: 12345.";
        let result = parse_indicators(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_indicators_invalid_exec_id_format() {
        let body = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows) EXEC_ID: abc.";
        let result = parse_indicators(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_indicator_valid() {
        let text = "SELECT 1 EXECTIME: 10(ms) ROWCOUNT: 5(rows)";
        let result = extract_indicator(text, "EXECTIME: ", "(ms)");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "10");

        let result = extract_indicator(text, "ROWCOUNT: ", "(rows)");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "5");
    }

    #[test]
    fn test_extract_indicator_missing_prefix() {
        let text = "SELECT 1 ROWCOUNT: 5(rows)";
        let result = extract_indicator(text, "EXECTIME: ", "(ms)");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_indicator_missing_suffix() {
        let text = "SELECT 1 EXECTIME: 10 ROWCOUNT: 5(rows)";
        let result = extract_indicator(text, "EXECTIME: ", "(ms)");
        assert!(result.is_err());
    }

    // ==================== RecordParser 边界测试 ====================

    #[test]
    fn test_record_parser_empty_input() {
        let input = "";
        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = RecordParser::new(cursor);
        let records: Vec<_> = parser.collect();
        assert_eq!(records.len(), 0);
    }

    #[test]
    fn test_record_parser_only_invalid_lines() {
        let input = r#"garbage line 1
garbage line 2
not a valid record"#;
        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = RecordParser::new(cursor);
        let records: Vec<_> = parser.collect();
        assert_eq!(records.len(), 0);
    }

    #[test]
    fn test_record_parser_mixed_valid_invalid() {
        let input = r#"garbage line
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
more garbage
2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2
invalid line again"#;

        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = RecordParser::new(cursor);
        let records: Vec<_> = parser.collect();

        assert_eq!(records.len(), 2);
        assert!(records[0].is_ok());
        assert!(records[1].is_ok());
    }

    #[test]
    fn test_record_parser_windows_line_endings() {
        let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\r\n2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2\r\n";

        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = RecordParser::new(cursor);
        let records: Vec<_> = parser.collect();

        assert_eq!(records.len(), 2);
        let record1 = records[0].as_ref().unwrap();
        let record2 = records[1].as_ref().unwrap();

        // 验证换行符已被正确移除
        assert!(!record1.start_line().contains('\r'));
        assert!(!record2.start_line().contains('\r'));
    }

    #[test]
    fn test_record_parser_unix_line_endings() {
        let input = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1\n2025-08-12 10:57:10.000 (EP[0] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2\n";

        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = RecordParser::new(cursor);
        let records: Vec<_> = parser.collect();

        assert_eq!(records.len(), 2);
    }

    // ==================== SqllogParser 边界测试 ====================

    #[test]
    fn test_sqllog_parser_empty_input() {
        let input = "";
        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = SqllogParser::new(cursor);
        let sqllogs: Vec<_> = parser.collect();
        assert_eq!(sqllogs.len(), 0);
    }

    #[test]
    fn test_sqllog_parser_mixed_valid_invalid() {
        let input = r#"garbage
2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1
2025-08-12 10:57:10.000 (EP[999] sess:124 thrd:457 user:bob trxid:790 stmt:1000 appname:app) SELECT 2"#;

        let cursor = std::io::Cursor::new(input.as_bytes());
        let parser = SqllogParser::new(cursor);
        let sqllogs: Vec<_> = parser.collect();

        assert_eq!(sqllogs.len(), 2);
        assert!(sqllogs[0].is_ok());
        // EP[999] 超过 u8 范围，应该解析失败
        assert!(sqllogs[1].is_err());
    }

    // ==================== 边界情况和错误处理 ====================

    #[test]
    fn test_parse_record_line_too_short() {
        // 这行太短，is_record_start_line 会拒绝
        let lines = vec!["2025-08-12 10:57:09"];
        let result = parse_record(&lines);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InvalidRecordStartLine
        ));
    }

    #[test]
    fn test_parse_record_missing_closing_paren() {
        // 缺少右括号，is_record_start_line 会拒绝
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app SELECT 1",
        ];
        let result = parse_record(&lines);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InvalidRecordStartLine
        ));
    }

    #[test]
    fn test_parse_record_insufficient_meta_fields() {
        // meta 字段不足，is_record_start_line 会拒绝
        let lines = vec!["2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456) SELECT 1"];
        let result = parse_record(&lines);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::InvalidRecordStartLine
        ));
    }

    #[test]
    fn test_parse_record_with_hex_values() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:0xABCD thrd:0x1234 user:alice trxid:0x789 stmt:0xFFFF appname:app) SELECT 1",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.meta.sess_id, "0xABCD");
        assert_eq!(sqllog.meta.thrd_id, "0x1234");
        assert_eq!(sqllog.meta.trxid, "0x789");
        assert_eq!(sqllog.meta.statement, "0xFFFF");
    }

    #[test]
    fn test_parse_record_multiline_with_indicators() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT *",
            "FROM users",
            "WHERE id = 1 EXECTIME: 15.5(ms) ROWCOUNT: 10(rows) EXEC_ID: 99999.",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.body, "SELECT *\nFROM users\nWHERE id = 1");

        assert!(sqllog.indicators.is_some());
        let indicators = sqllog.indicators.unwrap();
        assert_eq!(indicators.execute_time, 15.5);
        assert_eq!(indicators.row_count, 10);
        assert_eq!(indicators.execute_id, 99999);
    }

    #[test]
    fn test_parse_record_empty_body() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app)",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.body, "");
    }

    #[test]
    fn test_parse_record_special_characters_in_fields() {
        let lines = vec![
            "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:user@domain.com trxid:789 stmt:999 appname:my-app-v1.0) SELECT 1",
        ];

        let result = parse_record(&lines);
        assert!(result.is_ok());

        let sqllog = result.unwrap();
        assert_eq!(sqllog.meta.username, "user@domain.com");
        assert_eq!(sqllog.meta.appname, "my-app-v1.0");
    }

    #[test]
    fn test_record_equality() {
        let record1 = Record::new("line1".to_string());
        let mut record2 = Record::new("line1".to_string());

        assert_eq!(record1, record2);

        record2.add_line("line2".to_string());
        assert_ne!(record1, record2);
    }

    #[test]
    fn test_record_clone() {
        let mut record1 = Record::new("line1".to_string());
        record1.add_line("line2".to_string());

        let record2 = record1.clone();

        assert_eq!(record1, record2);
        assert_eq!(record1.lines, record2.lines);
    }
}
