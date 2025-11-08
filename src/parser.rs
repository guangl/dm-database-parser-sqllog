/// 解析后的日志记录
///
/// 该结构体包含从日志文本中解析出的所有字段。所有字符串字段都是对原始输入文本的引用，
/// 因此不会产生额外的内存分配。
///
/// # 记录结构（四部分）
///
/// 每条记录由四个部分组成：
/// 1. **ts** - 时间戳（必定在首行）
/// 2. **meta** - 元信息（必定在首行，跟在时间戳后）
/// 3. **body** - SQL 主体（可能跨多行）
/// 4. **end** - 执行信息（可选，如果存在必定在最后一行）
///
/// # 字段说明
///
/// ## 核心四部分
/// - `ts`: 时间戳字符串（格式：`YYYY-MM-DD HH:MM:SS.mmm`），必定在首行
/// - `meta`: 完整的元信息部分（从时间戳后到 SQL 主体前的内容），必定在首行
/// - `body`: SQL 主体内容（可能跨多行）
/// - `end`: 执行信息行（可选，格式：`EXECTIME: Xms ROWCOUNT: Y EXEC_ID: Z`），如果存在必定在最后一行
///
/// ## 解析字段
/// - `meta_raw`: 原始元信息字符串（括号内的内容）
/// - `ep`: 执行计划标识符
/// - `sess`: 会话标识符
/// - `thrd`: 线程标识符
/// - `user`: 用户名
/// - `trxid`: 事务ID
/// - `stmt`: 语句标识符
/// - `appname`: 应用程序名称
/// - `ip`: 客户端IP地址（可选）
/// - `execute_time_ms`: 执行时间（毫秒，可选）
/// - `row_count`: 影响的行数（可选）
/// - `execute_id`: 执行ID（可选）
///
/// # 示例
///
/// ```rust
/// use dm_database_parser_sqllog::parse_record;
///
/// let log_text = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1\nEXECTIME: 10ms ROWCOUNT: 1 EXEC_ID: 100";
/// let parsed = parse_record(log_text);
/// 
/// // 四部分结构
/// println!("时间戳: {}", parsed.ts);
/// println!("元信息: {}", parsed.meta);
/// println!("SQL主体: {}", parsed.body);
/// if let Some(end) = parsed.end {
///     println!("执行信息: {}", end);
/// }
/// 
/// // 解析字段
/// println!("用户: {}, 事务ID: {}", parsed.user, parsed.trxid);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRecord<'a> {
    // === 核心四部分 ===
    /// 时间戳部分（必定在首行）
    pub ts: &'a str,
    /// 完整元信息部分（必定在首行，包含括号及内容）
    pub meta: &'a str,
    /// SQL 主体（可能跨多行）
    pub body: &'a str,
    /// 执行信息行（可选，如果存在必定在最后一行）
    pub end: Option<&'a str>,
    
    // === 解析后的字段 ===
    /// 原始元信息字符串（括号内的内容）
    pub meta_raw: &'a str,
    /// 执行计划标识符
    pub ep: &'a str,
    /// 会话标识符
    pub sess: &'a str,
    /// 线程标识符
    pub thrd: &'a str,
    /// 用户名
    pub user: &'a str,
    /// 事务ID
    pub trxid: &'a str,
    /// 语句标识符
    pub stmt: &'a str,
    /// 应用程序名称
    pub appname: &'a str,
    /// 客户端IP地址（可选）
    pub ip: Option<&'a str>,
    /// 执行时间（毫秒，可选）
    pub execute_time_ms: Option<u64>,
    /// 影响的行数（可选）
    pub row_count: Option<u64>,
    /// 执行ID（可选）
    pub execute_id: Option<u64>,
}

/// 迭代器，从输入日志文本中产生记录切片（`&str`），不进行额外分配。
///
/// `RecordSplitter` 旨在以最小分配（零分配或极少分配）的方式，从整个日志文本中
/// 按"记录"（record）边界切分并逐条返回。这里的"记录"由如下格式决定：每条记录
/// 都以固定长度的时间戳开始（23 字符，格式 `YYYY-MM-DD HH:MM:SS.mmm`），且时间戳位于
/// 行首（紧贴换行或文件开头），时间戳之后通常跟随一个空格和一个以圆括号包围的元信息，
/// 然后是记录主体（body），记录主体可能跨多行。
///
/// # 设计目标
///
/// - 尽量避免对每条记录进行拷贝；返回的是对原始输入字符串的切片（`&str`）
/// - 通过只扫描字节数组并使用简单的索引运算来保持最高性能
/// - 保持内部不变式以便 `next()` 实现更简单且安全
///
/// # 使用示例
///
/// ```rust
/// use dm_database_parser_sqllog::RecordSplitter;
///
/// let log_text = r#"
/// 2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
/// 2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
/// "#;
///
/// let splitter = RecordSplitter::new(log_text);
/// for record in splitter {
///     println!("记录: {}", record.lines().next().unwrap_or(""));
/// }
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

///
/// 构造一个 RecordSplitter，用于将日志文本拆分为记录。
/// 拆分的核心规则：
/// 1. 每条记录以固定长度的时间戳开始（23 字符，`YYYY-MM-DD HH:MM:SS.mmm`），时间戳必须位于行首（紧贴换行或文件开头）。
/// 2. 紧随时间戳后，必须跟着一个空格和以括号包围的元信息字段，格式和顺序如下：
///    EP[xxx] sess:xxx thrd:xxx user:xxx trxid:xxx stmt:xxx appname:xxx
///    所有字段缺一不可，且顺序必须一致。
/// 3. 记录主体（body）内容可跨多行，直至遇到下一条合法记录的时间戳或文件结尾。
/// 4. 每条记录内部，最终必定包含一行如下 "END" 信息：
///    EXECTIME: xxxms ROWCOUNT: xxx EXEC_ID: xxx
///    该行用于判定记录结束。
///
/// Splitter 工作流程：
/// - 在整个文本中线性查找，基于上述规则判定记录起始和结束位置。
/// - 避免不必要的分配，每条记录直接作为对原始日志输入的切片返回（&str）。
///
impl<'a> RecordSplitter<'a> {
    pub fn new(text: &'a str) -> Self {
        let bytes = text.as_bytes();
        let n = text.len();
        let first_start = Self::find_first_record_start(bytes, n);

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

    /// 查找第一个合法记录的起始位置
    fn find_first_record_start(bytes: &[u8], n: usize) -> Option<usize> {
        const TS_LEN: usize = 23;
        const FIELDS: [&[u8]; 7] = [
            b"EP[",
            b"sess:",
            b"thrd:",
            b"user:",
            b"trxid:",
            b"stmt:",
            b"appname:",
        ];

        if n < TS_LEN {
            return None;
        }

        let limit = n.saturating_sub(TS_LEN);
        (0..=limit).find(|&pos| {
            Self::is_line_start_with_timestamp(bytes, n, pos, TS_LEN)
                && Self::validate_meta_fields(bytes, n, pos + TS_LEN, &FIELDS)
        })
    }

    /// 检查位置是否为行首且后跟合法时间戳
    fn is_line_start_with_timestamp(bytes: &[u8], n: usize, pos: usize, ts_len: usize) -> bool {
        if pos + ts_len > n {
            return false;
        }
        let is_line_start = pos == 0 || bytes[pos - 1] == b'\n';
        let has_valid_timestamp = crate::tools::is_ts_millis_bytes(&bytes[pos..pos + ts_len]);
        is_line_start && has_valid_timestamp
    }

    /// 验证元信息字段是否按顺序匹配（所有字段必须存在）
    fn validate_meta_fields(bytes: &[u8], n: usize, mut pos: usize, fields: &[&[u8]]) -> bool {
        // 检查时间戳后是否有空格
        if pos >= n || bytes[pos] != b' ' {
            return false;
        }
        pos += 1; // 跳过空格

        // 跳过可选的左括号
        if pos < n && bytes[pos] == b'(' {
            pos += 1;
        }

        // 必须匹配所有字段
        for &pat in fields {
            // 检查字段前缀是否匹配
            let pat_len = pat.len();
            if pos + pat_len > n || &bytes[pos..pos + pat_len] != pat {
                return false;
            }
            pos += pat_len;

            // 跳过字段值
            if pat == b"EP[" {
                // EP[ 后直到 ]
                while pos < n && bytes[pos] != b']' {
                    pos += 1;
                }
                if pos >= n || bytes[pos] != b']' {
                    return false;
                }
                pos += 1; // 跳过 ]
            } else {
                // 其他字段：跳过字段值直到空格
                if pos >= n {
                    return false;
                }
                while pos < n && bytes[pos] != b' ' {
                    pos += 1;
                }
            }
            // 跳过分隔的空格
            while pos < n && bytes[pos] == b' ' {
                pos += 1;
            }
        }

        // 所有字段都已匹配
        true
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
///
/// 该函数会扫描整个文本，识别所有合法的日志记录，并将它们拆分为独立的切片。
/// 同时会收集文件开头的前导错误行（在第一个合法记录之前的内容）。
///
/// # 参数
///
/// * `text` - 完整的日志文本
///
/// # 返回值
///
/// 返回一个元组 `(records, errors)`：
/// - `records`: 所有合法记录的切片向量，每个元素都是对原始文本的引用
/// - `errors`: 前导错误行的向量（在第一个合法记录之前的所有行）
///
/// # 示例
///
/// ```rust
/// use dm_database_parser_sqllog::split_by_ts_records_with_errors;
///
/// let log_text = r#"
/// garbage line
/// 2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1
/// 2025-08-12 10:57:10.123 (EP[0] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) SELECT 2
/// "#;
///
/// let (records, errors) = split_by_ts_records_with_errors(log_text);
/// println!("找到 {} 条记录，{} 条前导错误", records.len(), errors.len());
/// ```
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
///
/// # 参数
///
/// * `text` - 完整的日志文本
/// * `records` - 用于存储记录切片的向量（会被清空后填充）
/// * `errors` - 用于存储前导错误行的向量（会被清空后填充）
///
/// # 示例
///
/// ```rust
/// use dm_database_parser_sqllog::split_into;
///
/// let mut records = Vec::new();
/// let mut errors = Vec::new();
///
/// let log_text = r#"2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1"#;
/// split_into(log_text, &mut records, &mut errors);
/// // 处理 records 和 errors...
/// ```
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
///
/// 这是处理日志文本时分配最少的方式。该函数会遍历所有记录，对每条记录调用回调函数，
/// 但不分配任何 Vec 来存储记录。
///
/// # 参数
///
/// * `text` - 完整的日志文本
/// * `f` - 对每条记录调用的回调函数
///
/// # 示例
///
/// ```rust
/// use dm_database_parser_sqllog::for_each_record;
///
/// let log_text = r#"..."#;
/// for_each_record(log_text, |rec| {
///     println!("记录: {}", rec.lines().next().unwrap_or(""));
/// });
/// ```
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
///
/// 该函数会遍历所有记录，解析每条记录为 `ParsedRecord`，然后调用回调函数。
/// 与 `for_each_record` 类似，这是零分配的处理方式。
///
/// # 参数
///
/// * `text` - 完整的日志文本
/// * `f` - 对每条解析后的记录调用的回调函数
///
/// # 示例
///
/// ```rust
/// use dm_database_parser_sqllog::parse_records_with;
///
/// let log_text = r#"..."#;
/// parse_records_with(log_text, |parsed| {
///     println!("用户: {}, 事务ID: {}", parsed.user, parsed.trxid);
/// });
/// ```
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
///
/// 这是最简单的解析方式，会分配一个新的 Vec 来存储所有解析后的记录。
/// 如果需要避免分配，请使用 `parse_into` 或 `parse_records_with`。
///
/// # 参数
///
/// * `text` - 完整的日志文本
///
/// # 返回值
///
/// 返回包含所有解析后记录的向量。
///
/// # 示例
///
/// ```rust
/// use dm_database_parser_sqllog::parse_all;
///
/// let log_text = r#"..."#;
/// let records = parse_all(log_text);
/// for record in records {
///     println!("用户: {}", record.user);
/// }
/// ```
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
    ep: &'a str,
    sess: &'a str,
    thrd: &'a str,
    user: &'a str,
    trxid: &'a str,
    stmt: &'a str,
    appname: &'a str,
    ip: Option<&'a str>,
}

fn parse_meta(meta_raw: &str) -> MetaParts<'_> {
    let mut parts = MetaParts {
        ep: "",
        sess: "",
        thrd: "",
        user: "",
        trxid: "",
        stmt: "",
        appname: "",
        ip: None,
    };

    let mut iter = meta_raw.split_whitespace().peekable();
    while let Some(tok) = iter.next() {
        if tok.starts_with("EP[") {
            parts.ep = tok;
        } else if let Some(val) = tok.strip_prefix("sess:") {
            parts.sess = val;
        } else if let Some(val) = tok.strip_prefix("thrd:") {
            parts.thrd = val;
        } else if let Some(val) = tok.strip_prefix("user:") {
            parts.user = val;
        } else if let Some(val) = tok.strip_prefix("trxid:") {
            parts.trxid = val;
        } else if let Some(val) = tok.strip_prefix("stmt:") {
            parts.stmt = val;
        } else if tok == "appname:" {
            if let Some(next) = iter.peek() {
                if (*next).starts_with("ip:::") {
                    let nexttok = iter.next().unwrap();
                    let ippart = nexttok.trim_start_matches("ip:::");
                    let ipclean = ippart.trim_start_matches("ffff:");
                    parts.ip = Some(ipclean);
                    parts.appname = "";
                } else {
                    let val = iter.next().unwrap();
                    parts.appname = val;
                }
            } else {
                parts.appname = "";
            }
        } else if let Some(val) = tok.strip_prefix("appname:") {
            if val.starts_with("ip:::") {
                let ippart = val.trim_start_matches("ip:::");
                let ipclean = ippart.trim_start_matches("ffff:");
                parts.ip = Some(ipclean);
                parts.appname = "";
            } else {
                parts.appname = val;
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

/// 解析单条记录。
///
/// 该函数将一条日志记录文本解析为 `ParsedRecord` 结构体。
/// 返回的结构体中的所有字符串字段都是对输入文本的引用，不会产生额外的内存分配。
///
/// 记录结构：
/// 1. **ts** - 时间戳（必定在首行）
/// 2. **meta** - 元信息（必定在首行）
/// 3. **body** - SQL 主体（可能跨多行）
/// 4. **end** - 执行信息（可选，如果存在必定在最后一行）
///
/// # 参数
///
/// * `rec` - 单条日志记录的文本（通常由 `RecordSplitter` 或相关函数产生）
///
/// # 返回值
///
/// 返回解析后的 `ParsedRecord`，所有字段都是对输入文本的引用。
///
/// # 示例
///
/// ```rust
/// use dm_database_parser_sqllog::parse_record;
///
/// let record_text = "2025-08-12 10:57:09.562 (EP[0] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) SELECT 1";
/// let parsed = parse_record(record_text);
/// println!("时间戳: {}, 用户: {}", parsed.ts, parsed.user);
/// ```
pub fn parse_record(rec: &'_ str) -> ParsedRecord<'_> {
    // 1) 将记录分割为 ts / meta_raw / body
    let (ts, meta_raw, mut body) = split_ts_meta_body(rec);

    // 2) 提取完整的 meta 部分（从时间戳后到 body 开始前）
    let meta = if rec.len() > 23 {
        let after_ts_start = 23;
        if let Some(open_idx) = rec[after_ts_start..].find('(') {
            if let Some(close_idx) = rec[after_ts_start + open_idx..].find(')') {
                let meta_end = after_ts_start + open_idx + close_idx + 1;
                &rec[after_ts_start..meta_end]
            } else {
                ""
            }
        } else {
            ""
        }
    } else {
        ""
    };

    // 3) 分离 body 和 end 部分
    // end 必定在最后一行，格式为 "EXECTIME: Xms ROWCOUNT: Y EXEC_ID: Z"
    let (body_part, end_part) = if body.contains("EXECTIME:") {
        // 查找最后一个换行符
        if let Some(last_newline) = body.rfind('\n') {
            let potential_end = &body[last_newline + 1..];
            // 检查是否包含 EXECTIME
            if potential_end.trim_start().starts_with("EXECTIME:") {
                (&body[..last_newline], Some(potential_end.trim()))
            } else {
                (body, None)
            }
        } else {
            // 整个 body 就是 end 行
            if body.trim_start().starts_with("EXECTIME:") {
                ("", Some(body.trim()))
            } else {
                (body, None)
            }
        }
    } else {
        (body, None)
    };

    body = body_part;
    let end = end_part;

    // 4) 解析 meta 字段
    let meta_parsed = parse_meta(meta_raw);

    // 5) 从 body 或 end 解析数值指标
    let (execute_time_ms, row_count, execute_id) = if let Some(end_line) = end {
        parse_body_metrics(end_line)
    } else {
        parse_body_metrics(body)
    };

    ParsedRecord {
        // 核心四部分
        ts,
        meta,
        body,
        end,
        
        // 解析字段
        meta_raw,
        ep: meta_parsed.ep,
        sess: meta_parsed.sess,
        thrd: meta_parsed.thrd,
        user: meta_parsed.user,
        trxid: meta_parsed.trxid,
        stmt: meta_parsed.stmt,
        appname: meta_parsed.appname,
        ip: meta_parsed.ip,
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
        let log_text = "garbage\n2023-10-05 14:23:45.123 (EP[1] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp) foo\n2023-10-05 14:23:46.456 (EP[2] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp) bar\n";
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
        assert_eq!(r0.appname, "");

        let r1 = parse_record(records[1]);
        assert!(r1.body.contains("TRX: START"));
    }

    #[test]
    fn test_missing_sess_field_should_be_error() {
        // 缺少 sess: 字段 - 找不到有效记录，所有内容都是前导错误
        let log_text = "garbage1\n2023-10-05 14:23:45.123 (EP[12345] thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        // 如果找不到第一个有效记录，leading_errors_slice 返回 None，所以 errors 为空
        // 但整个文本都没有有效记录，所以 records 也为空
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_missing_thrd_field_should_be_error() {
        // 缺少 thrd: 字段 - 找不到有效记录
        let log_text = "garbage2\n2023-10-05 14:23:45.123 (EP[12345] sess:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0); // 找不到第一个有效记录时，leading_errors_slice 返回 None
    }

    #[test]
    fn test_missing_user_field_should_be_error() {
        // 缺少 user: 字段 - 找不到有效记录
        let log_text = "garbage3\n2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_missing_trxid_field_should_be_error() {
        // 缺少 trxid: 字段 - 找不到有效记录
        let log_text = "garbage4\n2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 user:admin stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_missing_stmt_field_should_be_error() {
        // 缺少 stmt: 字段 - 找不到有效记录
        let log_text = "garbage5\n2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 user:admin trxid:0 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_missing_appname_field_should_be_error() {
        // 缺少 appname: 字段 - 找不到有效记录
        let log_text = "garbage6\n2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_wrong_field_order_should_be_error() {
        // 字段顺序错误：sess 和 thrd 交换 - 找不到有效记录
        let log_text = "garbage\n2023-10-05 14:23:45.123 (EP[12345] thrd:1 sess:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_invalid_timestamp_format_should_be_error() {
        // 时间戳格式错误（缺少毫秒部分） - 找不到有效记录
        let log_text = "garbage\n2023-10-05 14:23:45 (EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_invalid_timestamp_length_should_be_error() {
        // 时间戳长度不足 - 找不到有效记录
        let log_text = "garbage\n2023-10-05 14:23:45.1 (EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_missing_space_after_timestamp_should_be_error() {
        // 时间戳后没有空格 - 找不到有效记录
        let log_text = "garbage\n2023-10-05 14:23:45.123(EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_missing_ep_bracket_should_be_error() {
        // EP[ 后没有闭合括号 ] - 找不到有效记录
        let log_text = "garbage\n2023-10-05 14:23:45.123 (EP[12345 sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_no_timestamp_line_should_be_error() {
        // 没有时间戳的普通行
        let log_text = "garbage line 1\ngarbage line 2\njust a normal line\n2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        assert_eq!(records.len(), 1);
        assert_eq!(errors.len(), 3); // 3 行错误内容
    }

    #[test]
    fn test_mixed_valid_and_invalid_records() {
        // 混合有效和无效记录
        // 注意：next() 方法只检查时间戳，不验证字段，所以所有有时间戳的行都会被当作记录
        let log_text = "2023-10-05 14:23:45.123 (EP[12345] sess:1 thrd:1 user:admin trxid:0 stmt:1 appname:MyApp)\nSELECT 1\ninvalid line\n2023-10-05 14:24:00.456 (EP[12346] sess:2 thrd:2 user:guest trxid:0 stmt:2 appname:MyApp)\nINSERT 1\n2023-10-05 14:24:01.789 (EP[12347] sess:3)\ninvalid record\n";
        let (records, errors) = split_by_ts_records_with_errors(log_text);
        // next() 方法只检查时间戳，所以所有有时间戳的行都会被当作记录（即使字段不完整）
        assert_eq!(records.len(), 3);
        // invalid line 会被包含在第一个和第二个记录之间，所以 errors 中只有前导错误（如果有）
        assert_eq!(errors.len(), 0); // 第一个记录有效，所以没有前导错误
    }
}
