use once_cell::sync::Lazy;
use super::matcher::Matcher;

// 模式按照要求的顺序列出
#[allow(dead_code)]
static DEFAULT_PATTERNS: &[&str] = &[
    "EP[", "sess:", "thrd:", "user:", "trxid:", "stmt:", "appname:",
];

// 懒惰初始化一个基于 DEFAULT_PATTERNS 的 Matcher，以便现有测试可以提前预热。
#[allow(dead_code)]
static DEFAULT_MATCHER: Lazy<Matcher> = Lazy::new(|| Matcher::from_patterns(DEFAULT_PATTERNS));

#[inline(always)]
pub fn is_ts_millis(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 23 {
        return false;
    }
    // 固定符号位置
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b' '
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
    {
        return false;
    }
    // 其余位置必须为数字
    for &i in &[
        0usize, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 22,
    ] {
        if !bytes[i].is_ascii_digit() {
            return false;
        }
    }
    true
}

/// `is_ts_millis` 的字节切片变体，以避免在扫描大缓冲区时创建临时 `&str` 切片。
/// 期望输入恰好为 23 字节。
#[inline(always)]
pub fn is_ts_millis_bytes(bytes: &[u8]) -> bool {
    if bytes.len() != 23 {
        return false;
    }
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b' '
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
    {
        return false;
    }
    // 数字位置（硬编码索引）
    for &i in &[
        0usize, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 22,
    ] {
        if !bytes[i].is_ascii_digit() {
            return false;
        }
    }
    true
}

/// 判断一行是否为 sqllog 的“记录起始行”。
///
/// 判定规则（严格匹配当前实现）：
/// 1. 要求时间戳严格位于行首（不允许前导空白）；
/// 2. 行首的前 23 个字符必须正好是时间戳，格式为 `YYYY-MM-DD HH:MM:SS.mmm`（由 `is_ts_millis` 验证）；
/// 3. 在时间戳之后必须存在一对圆括号 `(...)`，括号内为元信息（元数据）；
/// 4. 元信息中必须包含以下 7 个关键短语，且它们首次出现的顺序必须严格为：
///    EP[ -> sess: -> thrd: -> user: -> trxid: -> stmt: -> appname:
///
/// 输入/输出：
/// - 输入：单行文本 `line: &str`（可以包含前导空白）；
/// - 输出：bool，若满足上述所有条件则返回 true，否则返回 false。
///
/// 复杂度与性能：
/// - 使用双数组 Aho-Corasick（daachorse）自动机一次扫描元信息（O(n + total_matches)），比多次子串查找更高效；
/// - 该函数在最坏情况下仍然是线性相对输入长度的；
/// - 适合在对大量日志行做快速分组时使用。
///
/// 边界情况与注意事项：
/// - 关键字必须出现在括号内部；若关键字在括号外出现则视为不匹配；
/// - 关键字匹配是基于文本子串（大小写敏感）；如果需要忽略大小写或支持更多变体，应在自动机构建时调整或归一化输入；
/// - 只检查关键字的首次出现位置，以验证顺序；若关键字重复，只看第一次出现的位置；
/// - 时间戳严格按字符位置校验，不尝试解析为日期/时间类型以节省分配与解析开销。
pub fn is_record_start(line: &str) -> bool {
    // 1) 要求时间戳严格从行首开始（不允许前导空白）
    //    因为日志格式保证时间戳占据前 23 个字符的位置
    if line.len() < 23 {
        return false;
    }

    // 2) 校验时间戳格式（前 23 字符）
    if !is_ts_millis(&line[0..23]) {
        return false;
    }

    // 3) 在时间戳之后查找第一对圆括号，括号内为元信息
    // 格式固定：时间戳后紧跟一个空格，然后立刻是 '('，其后为元信息直到匹配的 ')'
    let rest = &line[23..];
    // 检查至少有两个字符（空格 + '('）
    let rest_bytes = rest.as_bytes();
    if rest_bytes.len() < 2 || rest_bytes[0] != b' ' || rest_bytes[1] != b'(' {
        return false;
    }
    // open 相对于 rest 的索引
    let open = 1usize; // 紧接在空格之后
    let close = match rest[open..].find(')') {
        Some(p) => open + p,
        // 没有匹配的 ')' 则不是起始行
        None => return false,
    };
    // 元信息字符串（不包含括号）
    let meta = &rest[open + 1..close];

    // 4) 使用 Matcher 在 meta 中一次扫描所有模式，记录每个模式的首次出现位置
    //    patterns 的定义顺序就是我们要求的出现顺序（见静态 DEFAULT_PATTERNS 定义）
    let matcher = &*DEFAULT_MATCHER;
    let fp = matcher.find_first_positions(meta.as_bytes());
    let mut first_pos: [Option<usize>; 7] = [None, None, None, None, None, None, None];
    for (i, p) in fp.into_iter().enumerate().take(first_pos.len()) {
        first_pos[i] = p;
    }

    // 要求全部 7 个模式均出现
    if first_pos.iter().any(|p| p.is_none()) {
        return false;
    }

    // 验证首次出现位置严格递增，保证顺序为 EP -> sess -> thrd -> user -> trxid -> stmt -> appname
    let mut prev: Option<usize> = None;
    for p in &first_pos {
        let cur = p.unwrap();
        if let Some(prev_pos) = prev {
            // 若当前位置小于等于前一个位置，说明顺序错误或重叠
            if cur <= prev_pos {
                return false;
            }
        }
        prev = Some(cur);
    }

    true
}

/// 预热内部自动机和相关静态结构，以便第一次计时调用不包含延迟初始化分配。
#[allow(dead_code)]
pub fn prewarm() {
    // 强制初始化静态自动机
    let _ = &*DEFAULT_MATCHER;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ts_millis() {
        let valid_ts = "2023-10-05 14:23:45.123";
        let invalid_ts_1 = "2023/10/05 14:23:45.123"; // 错误的分隔符
        let invalid_ts_2 = "2023-10-05 14:23:45"; // 缺少毫秒部分
        let invalid_ts_3 = "2023-10-05T14:23:45.123"; // 日期和时间之间分隔符错误
        let invalid_ts_4 = "2023-10-05 14:23:4a.123"; // 包含非数字字符

        assert!(is_ts_millis(valid_ts));
        assert!(!is_ts_millis(invalid_ts_1));
        assert!(!is_ts_millis(invalid_ts_2));
        assert!(!is_ts_millis(invalid_ts_3));
        assert!(!is_ts_millis(invalid_ts_4));
    }

    #[test]
    fn test_is_record_start_basic() {
        let line = "2025-08-12 10:57:09.561 (EP[0] sess:abc thrd:1 user:joe trxid:123 stmt:0x1 appname:my)";
        assert!(is_record_start(line));
    }

    #[test]
    fn test_is_record_start_different_order() {
        // 相同关键字但顺序错误现在不应被接受
        let line = "2025-08-12 10:57:09.561 (user:joe appname:my trxid:123 thrd:1 sess:abc stmt:0x1 EP[0])";
        assert!(!is_record_start(line));
    }

    #[test]
    fn test_is_record_start_correct_order_complex() {
        // 关键字可能穿插出现，但仍需保持所需顺序 EP -> sess -> thrd -> user -> trxid -> stmt -> appname
        let line = "2025-08-12 10:57:09.561 (EP[0] foobar sess:abc baz thrd:1 qux user:joe trxid:123 stmt:0x1 zz appname:my)";
        assert!(is_record_start(line));
    }

    #[test]
    fn test_is_record_start_leading_whitespace() {
        // 有前导空格的行现在不被接受（时间戳必须在行首）
        let line = "   2025-08-12 10:57:09.561 (EP[0] sess:abc thrd:1 user:joe trxid:123 stmt:0x1 appname:my)";
        assert!(!is_record_start(line));
    }

    #[test]
    fn test_is_record_start_missing_keyword() {
        let line = "2025-08-12 10:57:09.561 (EP[0] sess:abc thrd:1 trxid:123 stmt:0x1 appname:my)"; // 缺少 user
        assert!(!is_record_start(line));
    }

    #[test]
    fn test_is_record_start_keyword_outside_parentheses() {
        let line =
            "2025-08-12 10:57:09.561 EP[0] sess:abc thrd:1 user:joe trxid:123 stmt:0x1 appname:my";
        // 因为我们要求元数据位于括号内，因此应返回 false
        assert!(!is_record_start(line));
    }

    #[test]
    fn test_is_record_start_no_parentheses() {
        let line = "2025-08-12 10:57:09.561 some random text";
        assert!(!is_record_start(line));
    }

    #[test]
    fn test_is_record_start_invalid_timestamp() {
        let line =
            "2025-08-12T10:57:09 (EP[0] sess:abc thrd:1 user:joe trxid:123 stmt:0x1 appname:my)";
        assert!(!is_record_start(line));
    }
}
