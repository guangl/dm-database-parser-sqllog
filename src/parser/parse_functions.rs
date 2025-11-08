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
        return Err(ParseError::InvalidRecordStartLine {
            raw: first_line.to_string(),
        });
    }

    // 验证行长度
    if first_line.len() < MIN_RECORD_LENGTH {
        return Err(ParseError::LineTooShort {
            length: first_line.len(),
            raw: first_line.to_string(),
        });
    }

    // 解析时间戳
    let ts = &first_line[0..TIMESTAMP_LENGTH];

    // 查找 meta 部分的右括号
    let closing_paren = first_line
        .find(')')
        .ok_or_else(|| ParseError::MissingClosingParen {
            raw: first_line.to_string(),
        })?;

    if closing_paren <= META_START_INDEX {
        return Err(ParseError::InsufficientMetaFields {
            count: 0,
            raw: first_line[META_START_INDEX..].to_string(),
        });
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
    // 使用前缀定位法而非简单分割，以正确处理 appname 中的空格

    // 解析 EP - 从头开始
    let ep_end = meta_str
        .find(' ')
        .ok_or(ParseError::InsufficientMetaFields {
            count: 0,
            raw: meta_str.to_string(),
        })?;
    let ep = parse_ep_field(&meta_str[..ep_end], meta_str)?;

    // 解析 sess
    let sess_start = ep_end + 1;
    let sess_end = meta_str[sess_start..]
        .find(' ')
        .ok_or(ParseError::InsufficientMetaFields {
            count: 1,
            raw: meta_str.to_string(),
        })?
        + sess_start;
    let sess_id = extract_field_value(&meta_str[sess_start..sess_end], SESS_PREFIX, meta_str)?;

    // 解析 thrd
    let thrd_start = sess_end + 1;
    let thrd_end = meta_str[thrd_start..]
        .find(' ')
        .ok_or(ParseError::InsufficientMetaFields {
            count: 2,
            raw: meta_str.to_string(),
        })?
        + thrd_start;
    let thrd_id = extract_field_value(&meta_str[thrd_start..thrd_end], THRD_PREFIX, meta_str)?;

    // 解析 user
    let user_start = thrd_end + 1;
    let user_end = meta_str[user_start..]
        .find(' ')
        .ok_or(ParseError::InsufficientMetaFields {
            count: 3,
            raw: meta_str.to_string(),
        })?
        + user_start;
    let username = extract_field_value(&meta_str[user_start..user_end], USER_PREFIX, meta_str)?;

    // 解析 trxid
    let trxid_start = user_end + 1;
    let trxid_end_result = meta_str[trxid_start..].find(' ');
    let (trxid, after_trxid) = if let Some(trxid_end_offset) = trxid_end_result {
        let trxid_end = trxid_start + trxid_end_offset;
        (
            extract_field_value(&meta_str[trxid_start..trxid_end], TRXID_PREFIX, meta_str)?,
            trxid_end + 1,
        )
    } else {
        // 没有更多字段，trxid 是最后一个字段（只有 5 个字段）
        (
            extract_field_value(&meta_str[trxid_start..], TRXID_PREFIX, meta_str)?,
            meta_str.len(),
        )
    };

    // 如果只有 5 个字段，返回默认值
    if after_trxid >= meta_str.len() {
        return Ok(MetaParts {
            ep,
            sess_id,
            thrd_id,
            username,
            trxid,
            statement: String::new(),
            appname: String::new(),
            client_ip: String::new(),
        });
    }

    // 解析 stmt（可能不存在）
    let stmt_start = after_trxid;
    let stmt_end_result = meta_str[stmt_start..].find(' ');
    let (statement, after_stmt) = if let Some(stmt_end_offset) = stmt_end_result {
        let stmt_end = stmt_start + stmt_end_offset;
        (
            extract_field_value(&meta_str[stmt_start..stmt_end], STMT_PREFIX, meta_str)?,
            stmt_end + 1,
        )
    } else {
        // 没有更多字段，stmt 是最后一个字段（只有 6 个字段）
        (
            extract_field_value(&meta_str[stmt_start..], STMT_PREFIX, meta_str)?,
            meta_str.len(),
        )
    };

    // 如果只有 6 个字段，返回默认 appname 和 client_ip
    if after_stmt >= meta_str.len() {
        return Ok(MetaParts {
            ep,
            sess_id,
            thrd_id,
            username,
            trxid,
            statement,
            appname: String::new(),
            client_ip: String::new(),
        });
    }

    // 解析 appname（可选，且值可能包含空格）
    let appname_start = after_stmt;
    let (appname, client_ip) = if appname_start < meta_str.len() {
        // 检查是否有 appname 字段
        if meta_str[appname_start..].starts_with(APPNAME_PREFIX) {
            // 找到 appname，需要确定其结束位置
            // appname 后面可能跟着 " ip:::ffff:" 或者直接结束
            let appname_value_start = appname_start + APPNAME_PREFIX.len();
            if let Some(ip_pos) = meta_str[appname_value_start..].find(" ip:::ffff:") {
                // 有 IP 字段
                let appname_value = &meta_str[appname_value_start..appname_value_start + ip_pos];
                let ip_start = appname_value_start + ip_pos + 1;
                let client_ip = extract_field_value(&meta_str[ip_start..], IP_PREFIX, meta_str)?;
                (appname_value.to_string(), client_ip)
            } else {
                // 没有 IP 字段，appname 到末尾
                let appname_value = &meta_str[appname_value_start..];
                (appname_value.to_string(), String::new())
            }
        } else {
            // 没有 appname 字段
            (String::new(), String::new())
        }
    } else {
        (String::new(), String::new())
    };

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
pub(crate) fn parse_ep_field(ep_str: &str, raw: &str) -> Result<u8, ParseError> {
    if !ep_str.starts_with("EP[") || !ep_str.ends_with(']') {
        return Err(ParseError::InvalidEpFormat {
            value: ep_str.to_string(),
            raw: raw.to_string(),
        });
    }

    let ep_num = &ep_str[3..ep_str.len() - 1];
    ep_num.parse::<u8>().map_err(|_| ParseError::EpParseError {
        value: ep_num.to_string(),
        raw: raw.to_string(),
    })
}

/// 从字段中提取值
#[inline]
pub(crate) fn extract_field_value(
    field: &str,
    prefix: &str,
    raw: &str,
) -> Result<String, ParseError> {
    field
        .strip_prefix(prefix)
        .map(|s| s.to_string())
        .ok_or_else(|| ParseError::InvalidFieldFormat {
            expected: prefix.to_string(),
            actual: field.to_string(),
            raw: raw.to_string(),
        })
}

/// 解析 indicators 部分
pub(crate) fn parse_indicators(body: &str) -> Result<IndicatorsParts, ParseError> {
    // 使用预定义的静态常量，避免每次创建字符串
    let exec_time_str = extract_indicator(body, EXECTIME_PREFIX, EXECTIME_SUFFIX)?;
    let row_count_str = extract_indicator(body, ROWCOUNT_PREFIX, ROWCOUNT_SUFFIX)?;
    let exec_id_str = extract_indicator(body, EXEC_ID_PREFIX, EXEC_ID_SUFFIX)?;

    let execute_time =
        exec_time_str
            .parse::<f32>()
            .map_err(|_| ParseError::IndicatorsParseError {
                reason: format!("执行时间解析失败: {}", exec_time_str),
                raw: body.to_string(),
            })?;

    let row_count = row_count_str
        .parse::<u32>()
        .map_err(|_| ParseError::IndicatorsParseError {
            reason: format!("行数解析失败: {}", row_count_str),
            raw: body.to_string(),
        })?;

    let execute_id = exec_id_str
        .parse::<i64>()
        .map_err(|_| ParseError::IndicatorsParseError {
            reason: format!("执行 ID 解析失败: {}", exec_id_str),
            raw: body.to_string(),
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
        .ok_or_else(|| ParseError::IndicatorsParseError {
            reason: format!("未找到 {}", prefix),
            raw: text.to_string(),
        })?
        + prefix.len();

    let remaining = &text[start_pos..];
    let end_offset = remaining
        .find(suffix)
        .ok_or_else(|| ParseError::IndicatorsParseError {
            reason: format!("未找到 {}", suffix),
            raw: text.to_string(),
        })?;

    Ok(remaining[..end_offset].trim())
}
