use memchr::memchr;

const TIMESTAMP_LENGTH: usize = 23;
const MIN_LINE_LENGTH: usize = 25;

/// 判断字节数组是否为有效的时间戳格式 "YYYY-MM-DD HH:MM:SS.mmm"（恰好 23 字节）
///
/// ```
/// use dm_database_parser_sqllog::tools::is_ts_millis_bytes;
/// assert!(is_ts_millis_bytes(b"2025-08-12 10:57:09.548"));
/// assert!(!is_ts_millis_bytes(b"2025-08-12"));
/// ```
#[inline(always)]
pub fn is_ts_millis_bytes(bytes: &[u8]) -> bool {
    bytes.len() == TIMESTAMP_LENGTH
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b' '
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'.'
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[5].is_ascii_digit()
        && bytes[6].is_ascii_digit()
        && bytes[8].is_ascii_digit()
        && bytes[9].is_ascii_digit()
        && bytes[11].is_ascii_digit()
        && bytes[12].is_ascii_digit()
        && bytes[14].is_ascii_digit()
        && bytes[15].is_ascii_digit()
        && bytes[17].is_ascii_digit()
        && bytes[18].is_ascii_digit()
        && bytes[20].is_ascii_digit()
        && bytes[21].is_ascii_digit()
        && bytes[22].is_ascii_digit()
}

/// 判断一行日志是否为记录起始行
///
/// 验证：时间戳格式 + ` (` 前缀 + meta 包含 EP、sess、thrd、user、trxid（按序）
///
/// ```
/// use dm_database_parser_sqllog::tools::is_record_start_line;
/// let valid = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) SELECT 1";
/// assert!(is_record_start_line(valid));
/// assert!(!is_record_start_line("This is not a log line"));
/// ```
pub fn is_record_start_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.len() < MIN_LINE_LENGTH {
        return false;
    }
    if !is_ts_millis_bytes(&bytes[..TIMESTAMP_LENGTH]) {
        return false;
    }
    if bytes[23] != b' ' || bytes[24] != b'(' {
        return false;
    }
    let closing = match line.find(')') {
        Some(idx) => idx,
        None => return false,
    };
    validate_meta_fields_fast(&line[25..closing])
}

/// 验证 meta 字段顺序与前缀（EP → sess → thrd → user → trxid）
#[inline]
fn validate_meta_fields_fast(meta: &str) -> bool {
    let bytes = meta.as_bytes();
    // 最小合法 meta: "EP[0] sess:1 thrd:1 user:a trxid:1"
    if bytes.len() < 38 {
        return false;
    }
    let mut pos = 0;
    for prefix in [b"EP[" as &[u8], b"sess:", b"thrd:", b"user:"] {
        if !bytes[pos..].starts_with(prefix) {
            return false;
        }
        pos += match memchr(b' ', &bytes[pos..]) {
            Some(idx) => idx + 1,
            None => return false,
        };
    }
    bytes[pos..].starts_with(b"trxid:")
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
                "2025-08-12 10:57:09.548(EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body",
                "2025-08-12 10:57:09.548 EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body",
                "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app body",
            ];
            for line in &invalid_lines {
                assert!(!is_record_start_line(line), "Should fail for: {}", line);
            }
        }

        #[test]
        fn insufficient_fields() {
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
            let line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app ip:192.168.1.100) body";
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
            let line = "2025-08-12 10:57:09.548 (EP[0]  sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(!is_record_start_line(line));

            let valid_line = "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:789 stmt:999 appname:app) body";
            assert!(is_record_start_line(valid_line));
        }
    }
}
