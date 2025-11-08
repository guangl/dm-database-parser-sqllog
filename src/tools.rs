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
const REQUIRED_META_FIELDS: usize = 7;
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

/// 期望输入恰好为 23 字节。
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

///
/// 判断一行日志是否为记录起始行。
///
/// 判断标准
/// 1. 行首 23 字节符合时间戳格式 `YYYY-MM-DD HH:mm:ss.SSS` -> ts
/// 2. ts 后面紧跟一个空格，然后就是 meta 部分。
/// 3. meta 是小括号包含起来的。
/// 4. meta 部分必须包含所有字段（client_ip 可能没有）。
/// 5. meta 字段间以一个空格分隔。
/// 6. meta 字段间顺序是固定的。 顺序为 ep -> sess -> thrd_id -> username -> trxid -> statement -> appname -> client_ip (可选)。
/// 7. meta 部分结束后紧跟一个空格，然后是 body 部分。
pub fn is_record_start_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.len() < MIN_LINE_LENGTH {
        return false;
    }

    // 检查时间戳部分
    if !is_ts_millis_bytes(&bytes[0..TIMESTAMP_LENGTH]) {
        return false;
    }

    // 检查时间戳后面的空格和括号
    if bytes[23] != SPACE_BYTE || bytes[24] != OPEN_PAREN_BYTE {
        return false;
    }

    // 查找并检查 meta 部分的右括号
    let closing_paren_index = match line.find(CLOSE_PAREN_CHAR) {
        Some(index) => index,
        None => return false,
    };

    // 解析并验证 meta 字段 - 使用单次迭代验证所有字段
    let meta_part = &line[META_START_INDEX..closing_paren_index];

    // 创建迭代器并验证字段数量和内容
    let mut split_iter = meta_part.split(' ');
    let mut field_count = 0;

    // 验证前 7 个必需字段
    for prefix in META_FIELD_PREFIXES.iter().take(REQUIRED_META_FIELDS) {
        match split_iter.next() {
            Some(field) if field.contains(prefix) => {
                field_count += 1;
            }
            _ => return false,
        }
    }

    // 检查可选的 IP 字段
    if let Some(ip_field) = split_iter.next() {
        if !ip_field.contains(META_FIELD_PREFIXES[REQUIRED_META_FIELDS]) {
            return false;
        }
        field_count += 1;

        // 不应该有更多字段
        if split_iter.next().is_some() {
            return false;
        }
    }

    // 字段数量必须是 7 或 8
    field_count == REQUIRED_META_FIELDS || field_count == META_WITH_IP_FIELDS
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
            let line =
                "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789) body";
            assert!(!is_record_start_line(line));
        }

        #[test]
        fn wrong_field_order() {
            let line = "2025-08-12 10:57:09.548 (sess:123 EP[0] thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(!is_record_start_line(line));
        }

        #[test]
        fn missing_required_fields() {
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
                (
                    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 appname:app) body",
                    "stmt",
                ),
                (
                    "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999) body",
                    "appname",
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
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:192.168.1.100) body";
            assert!(!is_record_start_line(line));
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
            let line = "2025-08-12 10:57:09.548 (EP[0]  sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(!is_record_start_line(line));
        }
    }
}
