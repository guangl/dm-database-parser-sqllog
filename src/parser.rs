#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRecord<'a> {
    pub ts: &'a str,
    pub meta_raw: &'a str,
    pub ep: Option<&'a str>,
    pub sess: Option<&'a str>,
    pub thrd: Option<&'a str>,
    pub user: Option<&'a str>,
    pub trxid: Option<&'a str>,
    pub stmt: Option<&'a str>,
    pub appname: Option<&'a str>,
    pub ip: Option<&'a str>,
    pub body: &'a str,
    pub execute_time_ms: Option<u64>,
    pub row_count: Option<u64>,
    pub execute_id: Option<u64>,
}

/// 迭代器，从输入日志文本中产生记录切片(&str)，不进行额外分配。
///
/// 详细说明：
/// RecordSplitter 旨在以最小分配（零分配或极少分配）的方式，从整个日志文本中
/// 按“记录”（record）边界切分并逐条返回。这里的“记录”由如下格式决定：每条记录
/// 都以固定长度的时间戳开始（23 字符，格式 `YYYY-MM-DD HH:MM:SS.mmm`），且时间戳位于
/// 行首（紧贴换行或文件开头），时间戳之后通常跟随一个空格和一个以圆括号包围的元信息，
/// 然后是记录主体（body），记录主体可能跨多行。
///
/// 设计目标与不变式：
/// - 尽量避免对每条记录进行拷贝；返回的是对原始输入字符串的切片（&str）。
/// - 通过只扫描字节数组并使用简单的索引运算来保持最高性能。
/// - 保持下列内部不变式以便 `next()` 实现更简单且安全：
///   * `0 <= scan_pos <= n`，scan_pos 始终单调不减，用于向前搜索下一条记录的起始位置；
///   * `next_start` 保存下一个要返回记录的起始索引（相对于 `text`），当为 None 时表示迭代结束；
///   * `first_start` 如果存在，表示第一个合法记录起始索引（用于提取前导错误段）；
///   * `finished` 标志一旦为 true，表示迭代终止，不再返回任何记录。
///
/// 字段说明：
/// - `text`：原始输入字符串（借用），返回的记录切片都指向该字符串。
/// - `bytes`：`text.as_bytes()` 的借用，便于按字节高效检测时间戳等。
/// - `n`：`text.len()`，用于边界判断。
/// - `scan_pos`：下一次搜索开始的位置（相对于 `text`），用于寻找下一个时间戳起始；
/// - `next_start`：下一次要返回记录的起始位置（Some(idx)）；
/// - `finished`：迭代是否已结束的标志；
/// - `first_start`：第一个记录的起始位置（用于提取文件起始处的前导错误文本）。
///
/// 算法与复杂度：
/// - 构造时（`new`）只进行一次向前扫描，寻找第一个满足“行首+时间戳”的位置；该扫描为 O(n)。
/// - `next()` 在所有记录上总共扫描一次文本（每个字节最多被检查常数次），总体时间为 O(n)。
/// - 该实现避免在每条记录上分配新的字符串或 Vec，因此适合高吞吐场景。
///
/// 使用示例：
/// ```ignore
/// let splitter = RecordSplitter::new(&log_text);
/// for rec in splitter { process(rec); }
/// ```
pub struct RecordSplitter<'a> {
    text: &'a str,
    bytes: &'a [u8],
    n: usize,
    // 扫描位置：始终单调不减
    scan_pos: usize,
    // 下一个要返回的记录的起始索引
    next_start: Option<usize>,
    // 是否已返回最后一条记录
    finished: bool,
    // 缓存的前缀（前导错误）结束索引
    first_start: Option<usize>,
}

impl<'a> RecordSplitter<'a> {
    pub fn new(text: &'a str) -> Self {
        let bytes = text.as_bytes();
        let n = text.len();
        let mut first_start = None;
        if n >= 23 {
            // 在构造时寻找第一个可能的记录起始位置（减少后续第一轮迭代的开销）
            // 我们按字节线性扫描：对于每个位置 pos，判断该位置是否为行首并且随后 23 字节是合法时间戳。
            // 一旦找到第一个符合条件的位置就停止（first_start 用于提取前导错误）。
            let limit = n.saturating_sub(23);
            let mut pos = 0usize;
            while pos <= limit {
                if (pos == 0 || bytes[pos - 1] == b'\n')
                    && crate::tools::is_ts_millis_bytes(&bytes[pos..pos + 23])
                {
                    first_start = Some(pos);
                    break;
                }
                pos += 1;
            }
        }
        // scan_pos 设置为 first_start+1，保证下一次搜索从第一个起始之后继续（避免重复检测）
        let scan_pos = first_start.unwrap_or(0).saturating_add(1);
        RecordSplitter {
            text,
            bytes,
            n,
            scan_pos,
            next_start: first_start,
            finished: false,
            first_start,
        }
    }

    /// 返回完整的前导错误文本切片（第一条记录之前的所有内容）
    ///
    /// 说明：当日志文件开头存在非记录内容（例如垃圾行或日志碎片）时，`first_start` 会
    /// 指向第一个合法记录的起始位置；`leading_errors_slice()` 返回从文件开始到该位置的全部文本，
    /// 便于调用者单独处理这部分错误/告警信息。
    pub fn leading_errors_slice(&self) -> Option<&'a str> {
        self.first_start.map(|s| &self.text[..s])
    }
}

impl<'a> Iterator for RecordSplitter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let start = match self.next_start {
            Some(s) => s,
            None => {
                self.finished = true;
                return None;
            }
        };

        // 扫描下一个记录的起始位置
        // 逻辑：从当前 scan_pos 向前搜索下一个满足“行首+时间戳”的位置。
        // 如果找到，则当前记录的结束位置为该 timestamp 的起始位置（end = pos），并把 next_start 设置为该 pos，
        // 以便下一次调用返回后续记录；如果搜索到末尾未找到，则把剩余文本作为最后一条记录返回。
        if self.scan_pos > self.n {
            // 没有足够空间容纳另一个时间戳，返回剩余内容
            self.finished = true;
            return Some(&self.text[start..self.n]);
        }
        let limit = self.n.saturating_sub(23);
        let mut pos = self.scan_pos;
        while pos <= limit {
            if (pos == 0 || self.bytes[pos - 1] == b'\n')
                && crate::tools::is_ts_millis_bytes(&self.bytes[pos..pos + 23])
            {
                // 找到下一个起始位置
                let end = pos;
                // 为下一次调用做准备
                self.next_start = Some(pos);
                self.scan_pos = pos + 1;
                return Some(&self.text[start..end]);
            }
            pos += 1;
        }

        // 没有下一个起始位置 => 返回最后一条记录
        self.finished = true;
        Some(&self.text[start..self.n])
    }
}

/// 使用时间戳检测将完整日志文本拆分为记录。
/// 返回 (records, leading_errors)。每条记录都是从 `text` 借用的切片。
pub fn split_by_ts_records_with_errors<'a>(text: &'a str) -> (Vec<&'a str>, Vec<&'a str>) {
    let mut records: Vec<&'a str> = Vec::new();
    let mut errors: Vec<&'a str> = Vec::new();

    let splitter = RecordSplitter::new(text);
    if let Some(prefix) = splitter.leading_errors_slice() {
        for line in prefix.lines() {
            errors.push(line);
        }
    }
    for rec in splitter {
        records.push(rec);
    }
    (records, errors)
}

/// 拆分到调用者提供的容器以避免每次调用分配。
///
/// 该函数会清空并填充 `records` 和 `errors`。如果调用者在重复调用中重用这些
/// 向量（例如在循环中），则可以避免每次调用分配新的 `Vec`。
pub fn split_into<'a>(text: &'a str, records: &mut Vec<&'a str>, errors: &mut Vec<&'a str>) {
    records.clear();
    errors.clear();

    let splitter = RecordSplitter::new(text);
    if let Some(prefix) = splitter.leading_errors_slice() {
        for line in prefix.lines() {
            errors.push(line);
        }
    }
    for rec in splitter {
        records.push(rec);
    }
}

/// 对记录进行流式处理，并对每条记录调用回调而不分配 Vec。
/// 这是处理日志文本时分配最少的方式。
pub fn for_each_record<F>(text: &str, mut f: F)
where
    F: FnMut(&str),
{
    let splitter = RecordSplitter::new(text);
    // 对流式 API 忽略前导错误；如果需要，调用者可以通过 RecordSplitter::leading_errors_slice 检查它们。
    if let Some(_prefix) = splitter.leading_errors_slice() {
        // 在迭代之前释放前缀借用
    }
    for rec in splitter {
        f(rec);
    }
}

/// 解析每条记录并用 ParsedRecord 调用回调；与流式 Splitter 一起使用时实现零分配。
pub fn parse_records_with<F>(text: &str, mut f: F)
where
    F: for<'r> FnMut(ParsedRecord<'r>),
{
    for_each_record(text, |rec| {
        let parsed = parse_record(rec);
        f(parsed);
    });
}

/// 解析到调用方提供的 Vec 中以避免每次调用分配新的 Vec。
pub fn parse_into<'a>(text: &'a str, out: &mut Vec<ParsedRecord<'a>>) {
    out.clear();
    let splitter = RecordSplitter::new(text);
    for rec in splitter {
        out.push(parse_record(rec));
    }
}

/// 顺序解析所有记录并返回 ParsedRecord 的 Vec。
pub fn parse_all(text: &str) -> Vec<ParsedRecord<'_>> {
    let splitter = RecordSplitter::new(text);
    splitter.map(|r| parse_record(r)).collect()
}

fn parse_digits_forward(s: &str, mut i: usize) -> Option<(u64, usize)> {
    let bytes = s.as_bytes();
    let n = bytes.len();
    // 跳过非数字
    while i < n && !bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= n || !bytes[i].is_ascii_digit() {
        return None;
    }
    let mut val: u64 = 0;
    while i < n && bytes[i].is_ascii_digit() {
        val = val
            .saturating_mul(10)
            .saturating_add((bytes[i] - b'0') as u64);
        i += 1;
    }
    Some((val, i))
}

// 辅助：将记录分割成 (ts, meta_raw, body)，均为 &str（借用）
fn split_ts_meta_body<'a>(rec: &'a str) -> (&'a str, &'a str, &'a str) {
    let ts: &'a str = if rec.len() >= 23 { &rec[..23] } else { "" };
    let after_ts: &'a str = if rec.len() > 23 { &rec[23..] } else { "" };
    let mut meta_raw: &'a str = "";
    let mut body: &'a str = "";

    if let Some(open_idx) = after_ts.find('(') {
        if let Some(close_rel) = after_ts[open_idx..].find(')') {
            meta_raw = &after_ts[open_idx + 1..open_idx + close_rel];
            let body_start = 23 + open_idx + close_rel + 1;
            if body_start < rec.len() {
                body = rec[body_start..].trim_start();
            }
        } else {
            // 没有闭合括号：将剩余部分视为 body
            body = after_ts;
        }
    } else {
        // 没有元数据括号：时间戳之后的全部内容都是 body
        body = after_ts;
    }

    (ts, meta_raw, body)
}

// 辅助：解析 meta_raw 中的各个字段，返回一个小结构
#[derive(Debug)]
struct MetaParts<'a> {
    ep: Option<&'a str>,
    sess: Option<&'a str>,
    thrd: Option<&'a str>,
    user: Option<&'a str>,
    trxid: Option<&'a str>,
    stmt: Option<&'a str>,
    appname: Option<&'a str>,
    ip: Option<&'a str>,
}

fn parse_meta(meta_raw: &str) -> MetaParts {
    let mut parts = MetaParts {
        ep: None,
        sess: None,
        thrd: None,
        user: None,
        trxid: None,
        stmt: None,
        appname: None,
        ip: None,
    };

    let mut iter = meta_raw.split_whitespace().peekable();
    while let Some(tok) = iter.next() {
        if tok.starts_with("EP[") {
            parts.ep = Some(tok);
        } else if let Some(val) = tok.strip_prefix("sess:") {
            parts.sess = Some(val);
        } else if let Some(val) = tok.strip_prefix("thrd:") {
            parts.thrd = Some(val);
        } else if let Some(val) = tok.strip_prefix("user:") {
            parts.user = Some(val);
        } else if let Some(val) = tok.strip_prefix("trxid:") {
            parts.trxid = Some(val);
        } else if let Some(val) = tok.strip_prefix("stmt:") {
            parts.stmt = Some(val);
        } else if tok == "appname:" {
            if let Some(next) = iter.peek() {
                if (*next).starts_with("ip:::") {
                    let nexttok = iter.next().unwrap();
                    let ippart = nexttok.trim_start_matches("ip:::");
                    let ipclean = ippart.trim_start_matches("ffff:");
                    parts.ip = Some(ipclean);
                    parts.appname = Some("");
                } else {
                    let val = iter.next().unwrap();
                    parts.appname = Some(val);
                }
            } else {
                parts.appname = Some("");
            }
        } else if let Some(val) = tok.strip_prefix("appname:") {
            if val.starts_with("ip:::") {
                let ippart = val.trim_start_matches("ip:::");
                let ipclean = ippart.trim_start_matches("ffff:");
                parts.ip = Some(ipclean);
                parts.appname = Some("");
            } else {
                parts.appname = Some(val);
            }
        }
    }

    parts
}

// 辅助：从 body 末尾反向提取数值指标（EXEC_ID, ROWCOUNT, EXECTIME）
fn parse_body_metrics(body: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    let mut execute_id: Option<u64> = None;
    let mut row_count: Option<u64> = None;
    let mut execute_time_ms: Option<u64> = None;

    let body_str = body;
    let mut search_end = body_str.len();

    if let Some(pos) = body_str[..search_end].rfind("EXEC_ID:") {
        let start = pos + "EXEC_ID:".len();
        if let Some((v, _)) = parse_digits_forward(body_str, start) {
            execute_id = Some(v);
        }
        search_end = pos;
    }

    if let Some(pos) = body_str[..search_end].rfind("ROWCOUNT:") {
        let start = pos + "ROWCOUNT:".len();
        if let Some((v, _)) = parse_digits_forward(body_str, start) {
            row_count = Some(v);
        }
        search_end = pos;
    }

    if let Some(pos) = body_str[..search_end].rfind("EXECTIME:") {
        let start = pos + "EXECTIME:".len();
        if let Some((v, _)) = parse_digits_forward(body_str, start) {
            execute_time_ms = Some(v);
        }
    }

    (execute_time_ms, row_count, execute_id)
}

/// 解析单条记录（由 split_by_ts_records_with_errors 生成）。
/// 返回一个从输入 `rec` 借用的 ParsedRecord。
pub fn parse_record(rec: &'_ str) -> ParsedRecord<'_> {
    // 1) 将记录分割为 ts / meta_raw / body
    let (ts, meta_raw, body) = split_ts_meta_body(rec);

    // 2) 解析 meta 字段
    let meta = parse_meta(meta_raw);

    // 3) 从 body 解析数值指标
    let (execute_time_ms, row_count, execute_id) = parse_body_metrics(body);

    ParsedRecord {
        ts,
        meta_raw,
        ep: meta.ep,
        sess: meta.sess,
        thrd: meta.thrd,
        user: meta.user,
        trxid: meta.trxid,
        stmt: meta.stmt,
        appname: meta.appname,
        ip: meta.ip,
        body,
        execute_time_ms,
        row_count,
        execute_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_by_ts_records() {
        let log_text = "2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT * FROM users
2023-10-05 14:24:00.456 (EP[12346] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp)\nINSERT INTO orders VALUES (1, 'item');\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);

        assert_eq!(records.len(), 2);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_split_with_leading_errors() {
        let log_text = "garbage line 1\ngarbage line 2\n2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);

        assert_eq!(records.len(), 1);
        assert_eq!(errors.len(), 2);
        assert!(records[0].contains("SELECT 1"));
    }

    #[test]
    fn test_record_splitter_iterator() {
        let log_text =
            "garbage\n2023-10-05 14:23:45.123 (EP[1]) foo\n2023-10-05 14:23:46.456 (EP[2]) bar\n";
        let it = RecordSplitter::new(log_text);
        assert_eq!(it.leading_errors_slice().unwrap().trim(), "garbage");
        let v: Vec<&str> = it.collect();
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_parse_simple_log_sample() {
        let log_text = "2025-08-12 10:57:09.562 (EP[0] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:688489653 stmt:0x7fb236077b70 appname: ip:::ffff:10.3.100.68) EXECTIME: 0ms ROWCOUNT: 1 EXEC_ID: 289655185\n2025-08-12 10:57:09.562 (EP[0] sess:0x7fb24f392a30 thrd:757794 user:HBTCOMS_V3_PROD trxid:0 stmt:NULL appname:) TRX: START\n";

        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 2);

        let r0 = parse_record(records[0]);
        assert_eq!(r0.execute_time_ms, Some(0));
        assert_eq!(r0.row_count, Some(1));
        assert_eq!(r0.execute_id, Some(289655185));
        assert_eq!(r0.ip, Some("10.3.100.68"));
        assert_eq!(r0.appname, Some(""));

        let r1 = parse_record(records[1]);
        assert!(r1.body.contains("TRX: START"));
    }
}

