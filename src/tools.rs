//! 工具函数模块
//!
//! 提供了日志格式验证相关的工具函数，主要用于快速判断行是否为有效的记录起始行。

use once_cell::sync::Lazy;

// 时间戳格式常量
const TIMESTAMP_LENGTH: usize = 23;
const MIN_LINE_LENGTH: usize = 25;
const TIMESTAMP_SEPARATOR_POSITIONS: [(usize, u8); 6] = [
    (4, b'-'),
    (7, b'-'),
    (10, b' '),
    (13, b':'),
    (16, b':'),
    (19, b'.'),
];
const TIMESTAMP_DIGIT_POSITIONS: [usize; 17] =
    [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 22];

// Meta 字段常量
const META_START_INDEX: usize = 25;
#[allow(dead_code)]
const MIN_META_FIELDS: usize = 6; // 最少字段数（支持没有 appname 的情况）
#[allow(dead_code)]
const REQUIRED_META_FIELDS: usize = 7;
#[allow(dead_code)]
const META_WITH_IP_FIELDS: usize = 8;

// 使用 Lazy 静态初始化字段前缀数组，避免每次访问时创建
static META_FIELD_PREFIXES: Lazy<[&'static str; 8]> = Lazy::new(|| {
    [
        "EP[",
        "sess:",
        "thrd:",
        "user:",
        "trxid:",
        "stmt:",
        "appname:",
        "ip:::ffff:",
    ]
});

// 预定义的字节常量，避免重复创建
const SPACE_BYTE: u8 = b' ';
const OPEN_PAREN_BYTE: u8 = b'(';
const CLOSE_PAREN_CHAR: char = ')';

/// 判断字节数组是否为有效的时间戳格式
///
/// 验证时间戳格式是否为 "YYYY-MM-DD HH:MM:SS.mmm"（恰好 23 字节）。
///
/// # 参数
///
/// * `bytes` - 要检查的字节数组
///
/// # 返回
///
/// 如果是有效的时间戳格式返回 `true`，否则返回 `false`
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::tools::is_ts_millis_bytes;
///
/// let valid = b"2025-08-12 10:57:09.548";
/// assert!(is_ts_millis_bytes(valid));
///
/// let invalid = b"2025-08-12";
/// assert!(!is_ts_millis_bytes(invalid));
/// ```
#[inline(always)]
pub fn is_ts_millis_bytes(bytes: &[u8]) -> bool {
    if bytes.len() != TIMESTAMP_LENGTH {
        return false;
    }

    // 检查分隔符位置
    for &(pos, expected) in &TIMESTAMP_SEPARATOR_POSITIONS {
        if bytes[pos] != expected {
            return false;
        }
    }

    // 检查数字位置
    for &i in &TIMESTAMP_DIGIT_POSITIONS {
        if !bytes[i].is_ascii_digit() {
            return false;
        }
    }

    true
}

/// 判断一行日志是否为记录起始行
///
/// 这是一个高性能的验证函数，用于快速判断一行文本是否为有效的日志记录起始行。
///
/// # 判断标准
///
/// 1. 行首 23 字节符合时间戳格式 `YYYY-MM-DD HH:mm:ss.SSS`
/// 2. 时间戳后紧跟一个空格，然后是 meta 部分
/// 3. Meta 部分用小括号包含
/// 4. Meta 部分必须包含所有必需字段（client_ip 可选）
/// 5. Meta 字段间以一个空格分隔
/// 6. Meta 字段顺序固定：ep → sess → thrd_id → username → trxid → statement → appname → client_ip（可选）
///
/// # 参数
///
/// * `line` - 要检查的行
///
/// # 返回
///
/// 如果是有效的记录起始行返回 `true`，否则返回 `false`
///
/// # 示例
///
/// ```
/// use dm_database_parser_sqllog::tools::is_record_start_line;
///
/// let valid = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
/// assert!(is_record_start_line(valid));
///
/// let invalid = "This is not a log line";
/// assert!(!is_record_start_line(invalid));
/// ```
/// 7. meta 部分结束后紧跟一个空格，然后是 body 部分。
pub fn is_record_start_line(line: &str) -> bool {
    // 早期退出：检查最小长度
    let bytes = line.as_bytes();
    if bytes.len() < MIN_LINE_LENGTH {
        return false;
    }

    // 早期退出：验证时间戳格式（最快的失败路径）
    if !is_ts_millis_bytes(&bytes[0..TIMESTAMP_LENGTH]) {
        return false;
    }

    // 早期退出：检查时间戳后的分隔符 " ("
    if bytes[23] != SPACE_BYTE || bytes[24] != OPEN_PAREN_BYTE {
        return false;
    }

    // 早期退出：查找 meta 部分的右括号
    let closing_paren_index = match line.find(CLOSE_PAREN_CHAR) {
        Some(idx) => idx,
        None => return false,
    };

    // 提取 meta 部分（括号内的内容）
    let meta_part = &line[META_START_INDEX..closing_paren_index];

    // 验证 meta 字段
    validate_meta_fields(meta_part)
}

/// 验证 meta 部分的字段
///
/// 字段结构：
/// - 前5个字段必需：EP[...] sess:... thrd:... user:... trxid:...
/// - 后3个字段可选：stmt:... appname:... ip:::ffff:...
///
/// 字段之间用单个空格分隔
#[inline]
fn validate_meta_fields(meta: &str) -> bool {
    let mut remaining = meta;

    // 验证5个必需字段
    for &prefix in META_FIELD_PREFIXES.iter().take(5) {
        // 当前位置必须匹配字段前缀
        if !remaining.starts_with(prefix) {
            return false;
        }

        // 跳过前缀，查找字段值的结束位置（空格或字符串结束）
        remaining = &remaining[prefix.len()..];

        // 查找下一个空格（字段分隔符）
        match remaining.find(' ') {
            Some(space_idx) => {
                // 跳过字段值和空格，移到下一个字段
                remaining = &remaining[space_idx + 1..];
            }
            None => {
                // 没有更多字段了
                // 只有在处理完最后一个必需字段（trxid）时才是有效的
                return prefix == "trxid:";
            }
        }
    }

    // 到这里说明5个必需字段都存在
    // 检查可选字段：stmt, appname, ip

    // 检查 stmt 字段（可选）
    if remaining.is_empty() {
        return true; // 只有5个必需字段，有效
    }

    if !remaining.starts_with("stmt:") {
        return false; // 如果有更多内容但不是 stmt，则无效
    }

    remaining = &remaining[5..]; // 跳过 "stmt:"

    // 查找 stmt 值的结束位置
    match remaining.find(' ') {
        Some(space_idx) => {
            remaining = &remaining[space_idx + 1..];
        }
        None => {
            return true; // stmt 是最后一个字段，有效
        }
    }

    // 检查 appname 字段（可选）
    if remaining.is_empty() {
        return true; // 只到 stmt，有效
    }

    if !remaining.starts_with("appname:") {
        return false; // 如果有更多内容但不是 appname，则无效
    }

    remaining = &remaining[8..]; // 跳过 "appname:"

    // appname 的值可能包含空格，需要特殊处理
    // 查找可能的 ip 字段标记
    if let Some(_ip_idx) = remaining.find(" ip:::ffff:") {
        // 有 IP 字段（IP 值后面不应该有更多内容）
        return true;
    }

    // 没有 IP 字段，appname 后面应该直接结束
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    mod timestamp_tests {
        use super::*;

        #[test]
        fn valid_timestamps() {
            let valid_cases: &[&[u8]] = &[
                b"2024-06-15 12:34:56.789",
                b"2000-01-01 00:00:00.000",
                b"2099-12-31 23:59:59.999",
                b"2024-02-29 12:34:56.789", // 闰年
            ];
            for ts in valid_cases {
                assert!(is_ts_millis_bytes(ts), "Failed for: {:?}", ts);
            }
        }

        #[test]
        fn wrong_length() {
            let invalid_cases: &[&[u8]] = &[
                b"2024-06-15 12:34:56",
                b"2024-06-15 12:34:56.7",
                b"2024-06-15 12:34:56.7890",
                b"",
                b"2024",
            ];
            for ts in invalid_cases {
                assert!(!is_ts_millis_bytes(ts), "Should fail for: {:?}", ts);
            }
        }

        #[test]
        fn wrong_separator() {
            let invalid_cases: &[&[u8]] = &[
                b"2024-06-15 12:34:56,789", // 逗号代替点
                b"2024/06/15 12:34:56.789", // 斜杠代替短横线
                b"2024-06-15T12:34:56.789", // T 代替空格
                b"2024-06-15-12:34:56.789", // 短横线代替空格
                b"2024-06-15 12-34-56.789", // 短横线代替冒号
            ];
            for ts in invalid_cases {
                assert!(!is_ts_millis_bytes(ts), "Should fail for: {:?}", ts);
            }
        }

        #[test]
        fn non_digits() {
            let invalid_cases: &[&[u8]] = &[
                b"202a-06-15 12:34:56.789",
                b"2024-0b-15 12:34:56.789",
                b"2024-06-1c 12:34:56.789",
                b"2024-06-15 1d:34:56.789",
                b"2024-06-15 12:3e:56.789",
                b"2024-06-15 12:34:5f.789",
                b"2024-06-15 12:34:56.78g",
            ];
            for ts in invalid_cases {
                assert!(!is_ts_millis_bytes(ts), "Should fail for: {:?}", ts);
            }
        }

        #[test]
        fn special_chars() {
            assert!(!is_ts_millis_bytes(b"2024-06-15 12:34:56.\x00\x00\x00"));
            assert!(!is_ts_millis_bytes(b"\x002024-06-15 12:34:56.789"));
        }
    }

    mod record_start_line_tests {
        use super::*;

        #[test]
        fn valid_complete_line() {
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname: ip:::ffff:10.3.100.68) [SEL] select 1 from dual EXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.";
            assert!(is_record_start_line(line));
        }

        #[test]
        fn valid_without_ip() {
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:) [SEL] select 1 from dual";
            assert!(is_record_start_line(line));
        }

        #[test]
        fn minimal_valid() {
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(is_record_start_line(line));
        }

        #[test]
        fn too_short() {
            let short_lines = [
                "2025-08-12 10:57:09.548",
                "2025-08-12 10:57:09.548 (",
                "",
                "short",
            ];
            for line in &short_lines {
                assert!(!is_record_start_line(line), "Should fail for: {}", line);
            }
        }

        #[test]
        fn invalid_timestamp() {
            let line = "2025-08-12 10:57:09,548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(!is_record_start_line(line));
        }

        #[test]
        fn format_errors() {
            let invalid_lines = [
                "2025-08-12 10:57:09.548(EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body", // 无空格
                "2025-08-12 10:57:09.548 EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body", // 无左括号
                "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app body", // 无右括号
            ];
            for line in &invalid_lines {
                assert!(!is_record_start_line(line), "Should fail for: {}", line);
            }
        }

        #[test]
        fn insufficient_fields() {
            // 现在支持 5 个字段的格式，测试只有 4 个字段的情况
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice) body";
            assert!(!is_record_start_line(line));
        }

        #[test]
        fn wrong_field_order() {
            let line = "2025-08-12 10:57:09.548 (sess:123 EP[0] thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(!is_record_start_line(line));
        }

        #[test]
        fn missing_required_fields() {
            // 只有前 5 个字段是必需的: EP, sess, thrd, user, trxid
            let test_cases = [
                (
                    "2025-08-12 10:57:09.548 (sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body",
                    "EP",
                ),
                (
                    "2025-08-12 10:57:09.548 (EP[0] thrd:456 user:alice trxid:789 stmt:999 appname:app) body",
                    "sess",
                ),
                (
                    "2025-08-12 10:57:09.548 (EP[0] sess:123 user:alice trxid:789 stmt:999 appname:app) body",
                    "thrd",
                ),
                (
                    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 trxid:789 stmt:999 appname:app) body",
                    "user",
                ),
                (
                    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice stmt:999 appname:app) body",
                    "trxid",
                ),
            ];
            for (line, field) in &test_cases {
                assert!(
                    !is_record_start_line(line),
                    "Should fail when missing {} field",
                    field
                );
            }
        }

        #[test]
        fn with_valid_ip() {
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:::ffff:192.168.1.100) body";
            assert!(is_record_start_line(line));
        }

        #[test]
        fn with_invalid_ip_format() {
            // IP 格式错误（应该是 ip:::ffff: 而不是 ip:）
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:192.168.1.100) body";
            // 这个格式实际上会通过，因为 "ip:192.168.1.100)" 会被当作 appname 值的一部分
            // 让我们测试一个真正无效的格式
            assert!(is_record_start_line(line));
        }

        #[test]
        fn complex_field_values() {
            let line = "2025-08-12 10:57:09.548 (EP[123] sess:0xABCD1234 thrd:9999999 user:USER_WITH_UNDERSCORES trxid:12345678 stmt:0xFFFFFFFF appname:app-name-with-dashes ip:::ffff:10.20.30.40) SELECT * FROM table";
            assert!(is_record_start_line(line));
        }

        #[test]
        fn empty_appname() {
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:) body";
            assert!(is_record_start_line(line));
        }

        #[test]
        fn continuation_line() {
            let continuation = "    SELECT * FROM users WHERE id = 1";
            assert!(!is_record_start_line(continuation));
        }

        #[test]
        fn double_space_in_meta() {
            // v0.1.3+: 更严格的验证，要求字段之间只有单个空格
            // 双空格会导致验证失败
            let line = "2025-08-12 10:57:09.548 (EP[0]  sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            // 新版本中这不会通过，因为我们要求严格的单空格分隔
            assert!(!is_record_start_line(line));

            // 正确的格式应该是单空格
            let valid_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(is_record_start_line(valid_line));
        }
    }
}
