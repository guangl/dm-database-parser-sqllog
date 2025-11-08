//! 核心解析函数
//!
//! 包含了所有用于解析 SQL 日志的核心函数，如解析记录、元数据、指标等。

use crate::error::ParseError;
use crate::parser::constants::*;
use crate::sqllog::{IndicatorsParts, MetaParts, Sqllog};
use crate::tools::is_record_start_line;

/// 从行数组解析成 Sqllog 结构
///
/// 这是主要的解析函数，将一行或多行文本解析为结构化的 `Sqllog` 对象。
///
/// # 参数
///
/// * `lines` - 包含日志记录的行（第一行必须是有效的起始行，后续行是继续行）
///
/// # 返回
///
/// * `Ok(Sqllog)` - 解析成功
/// * `Err(ParseError)` - 解析失败，包含详细的错误信息
///
/// # 错误
///
/// 可能返回以下错误：
/// - `EmptyInput` - 输入为空
/// - `InvalidRecordStartLine` - 第一行不是有效的记录起始行
/// - `LineTooShort` - 行长度不足
/// - `MissingClosingParen` - 缺少右括号
/// - `InsufficientMetaFields` - Meta 字段数量不足
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::parse_record;
///
/// let lines = vec!["2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1"];
/// let sqllog = parse_record(&lines).unwrap();
///
/// assert_eq!(sqllog.ts, "2025-08-12 10:57:09.548");
/// assert_eq!(sqllog.meta.username, "alice");
/// assert_eq!(sqllog.body, "SELECT 1");
/// ```
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
///
/// 将第一行的 body 部分和所有继续行拼接成完整的 SQL 语句体。
/// 使用预分配内存优化性能。
///
/// # 参数
///
/// * `first_line` - 起始行
/// * `body_start` - body 在起始行中的起始位置
/// * `continuation_lines` - 所有继续行
///
/// # 返回
///
/// 返回拼接后的完整 body 字符串
#[inline]
pub(crate) fn build_body(
    first_line: &str,
    body_start: usize,
    continuation_lines: &[&str],
) -> String {
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
pub(crate) fn extract_sql_body(full_body: &str) -> String {
    // 使用预定义的 INDICATOR_PATTERNS 避免每次创建数组
    INDICATOR_PATTERNS
        .iter()
        .filter_map(|pattern| full_body.find(pattern))
        .min()
        .map(|pos| full_body[..pos].trim_end().to_string())
        .unwrap_or_else(|| full_body.to_string())
}

/// 解析 meta 字符串
pub(crate) fn parse_meta(meta_str: &str) -> Result<MetaParts, ParseError> {
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
pub(crate) fn parse_ep_field(ep_str: &str) -> Result<u8, ParseError> {
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
pub(crate) fn extract_field_value(field: &str, prefix: &str) -> Result<String, ParseError> {
    field
        .strip_prefix(prefix)
        .map(|s| s.to_string())
        .ok_or_else(|| ParseError::InvalidFieldFormat {
            expected: prefix.to_string(),
            actual: field.to_string(),
        })
}

/// 解析 indicators 部分
pub(crate) fn parse_indicators(body: &str) -> Result<IndicatorsParts, ParseError> {
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
pub(crate) fn extract_indicator<'a>(
    text: &'a str,
    prefix: &str,
    suffix: &str,
) -> Result<&'a str, ParseError> {
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
