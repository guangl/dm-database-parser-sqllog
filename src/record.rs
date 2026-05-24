use atoi::atoi;
use memchr::memchr;
use memchr::memrchr;

/// SQL 日志记录
///
/// 表示一条完整的 SQL 日志记录，所有字段在解析时一次性填充。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sqllog {
    /// 时间戳，格式为 "YYYY-MM-DD HH:MM:SS.mmm"
    pub ts: String,

    /// 方括号标签（例如 `[SEL]`、`[ORA]`），若无则为 None
    pub tag: Option<String>,

    // ── 元数据字段 ──
    /// EP（Execution Point）编号，范围 0-255
    pub ep: u8,

    /// 会话 ID
    pub sess_id: String,

    /// 线程 ID
    pub thrd_id: String,

    /// 用户名
    pub username: String,

    /// 事务 ID
    pub trxid: String,

    /// 语句 ID
    pub statement: String,

    /// 应用程序名称
    pub appname: String,

    /// 客户端 IP 地址
    pub client_ip: String,

    // ── SQL 语句体 ──
    /// SQL 语句体
    pub sql: String,

    // ── 性能指标 ──
    /// 执行时间（毫秒），无指标时为 0.0
    pub exectime: f32,

    /// 影响的行数，无指标时为 0
    pub rowcount: u32,

    /// 执行 ID，无指标时为 0
    pub exec_id: i64,
}

/// 解析元数据：从 meta 字节切片中提取所有字段。
///
/// meta_bytes 必须为有效 UTF-8。
pub(crate) fn parse_meta_from_bytes(
    meta_bytes: &[u8],
) -> (u8, String, String, String, String, String, String, String) {
    let mut ep: u8 = 0;
    let mut sess_id = String::new();
    let mut thrd_id = String::new();
    let mut username = String::new();
    let mut trxid = String::new();
    let mut statement = String::new();
    let mut appname = String::new();
    let mut client_ip = String::new();

    let bytes = meta_bytes;
    let len = bytes.len();
    let mut idx = 0;

    while idx < len {
        // Skip whitespace
        while idx < len && bytes[idx] == b' ' {
            idx += 1;
        }
        if idx >= len {
            break;
        }

        // Find token end
        let start = idx;
        while idx < len && bytes[idx] != b' ' {
            idx += 1;
        }
        let part = &bytes[start..idx];

        // Parse EP[n]
        if part.len() > 4
            && part[0] == b'E'
            && part[1] == b'P'
            && part[2] == b'['
            && part[part.len() - 1] == b']'
        {
            if let Some(ep_val) = atoi::<u8>(&part[3..part.len() - 1]) {
                ep = ep_val;
            }
            continue;
        }

        // Find ':'
        if let Some(sep) = memchr(b':', part) {
            let val_bytes = &part[sep + 1..];
            let val = String::from_utf8_lossy(val_bytes).into_owned();

            match &part[..sep] {
                b"sess" => sess_id = val,
                b"thrd" => thrd_id = val,
                b"user" => username = val,
                b"trxid" => trxid = val,
                b"stmt" => statement = val,
                b"ip" => client_ip = val,
                b"appname" => {
                    if !val_bytes.is_empty() {
                        appname = val;
                    } else {
                        // Peek next token; treat it as appname only if it is not an ip field
                        let mut peek = idx;
                        while peek < len && bytes[peek] == b' ' {
                            peek += 1;
                        }
                        if peek < len {
                            let peek_start = peek;
                            while peek < len && bytes[peek] != b' ' {
                                peek += 1;
                            }
                            let next = &bytes[peek_start..peek];
                            if !(next.starts_with(b"ip:") || next.starts_with(b"ip::")) {
                                appname = String::from_utf8_lossy(next).into_owned();
                                idx = peek;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    (
        ep, sess_id, thrd_id, username, trxid, statement, appname, client_ip,
    )
}

/// 解析性能指标：从 indicators 字节切片中提取 EXECTIME, ROWCOUNT, EXEC_ID。
///
/// 使用 memchr 扫描 ':' 和 '(' 定界符。
pub(crate) fn parse_indicators_from_bytes(ind: &[u8]) -> (f32, u32, i64) {
    if ind.is_empty() {
        return (0.0, 0, 0);
    }

    let mut exectime: f32 = 0.0;
    let mut rowcount: u32 = 0;
    let mut exec_id: i64 = 0;

    // Scan for EXECTIME
    let mut search_start = 0;
    while search_start < ind.len() {
        if let Some(colon) = memchr(b':', &ind[search_start..]) {
            let colon_pos = search_start + colon;
            if colon_pos >= 8 && &ind[colon_pos - 8..colon_pos] == b"EXECTIME" {
                let ss = colon_pos + 1;
                if let Some(pi) = memchr(b'(', &ind[ss..]) {
                    let val_bytes = &ind[ss..ss + pi];
                    let val_str = String::from_utf8_lossy(val_bytes).trim_ascii().to_string();
                    if let Ok(t) = val_str.parse::<f32>() {
                        exectime = t;
                    }
                }
                break;
            }
            search_start = colon_pos + 1;
        } else {
            break;
        }
    }

    // Scan for ROWCOUNT
    search_start = 0;
    while search_start < ind.len() {
        if let Some(colon) = memchr(b':', &ind[search_start..]) {
            let colon_pos = search_start + colon;
            if colon_pos >= 8 && &ind[colon_pos - 8..colon_pos] == b"ROWCOUNT" {
                let ss = colon_pos + 1;
                if let Some(pi) = memchr(b'(', &ind[ss..]) {
                    let val_bytes = &ind[ss..ss + pi];
                    let val_str = String::from_utf8_lossy(val_bytes).trim_ascii().to_string();
                    if let Ok(r) = val_str.parse::<u32>() {
                        rowcount = r;
                    }
                }
                break;
            }
            search_start = colon_pos + 1;
        } else {
            break;
        }
    }

    // Scan for EXEC_ID
    search_start = 0;
    while search_start < ind.len() {
        if let Some(colon) = memchr(b':', &ind[search_start..]) {
            let colon_pos = search_start + colon;
            if colon_pos >= 7 && &ind[colon_pos - 7..colon_pos] == b"EXEC_ID" {
                let ss = colon_pos + 1;
                let end = memchr(b'.', &ind[ss..])
                    .map(|i| ss + i)
                    .unwrap_or(ind.len());
                let val_bytes = &ind[ss..end];
                let val_str = String::from_utf8_lossy(val_bytes).trim_ascii().to_string();
                if let Ok(id) = val_str.parse::<i64>() {
                    exec_id = id;
                }
                break;
            }
            search_start = colon_pos + 1;
        } else {
            break;
        }
    }

    (exectime, rowcount, exec_id)
}

/// 在 indicator 字节中查找分割点（body 结束、indicators 开始的位置）。
///
/// 返回 body 的字节长度。
pub(crate) fn find_indicators_split(data: &[u8]) -> usize {
    let len = data.len();

    // 快速早退：末尾不是 '.' 或 ')' 则无指标。
    let last_meaningful = data
        .iter()
        .rev()
        .find(|&&b| b != b'\n' && b != b'\r')
        .copied();
    if last_meaningful != Some(b'.') && last_meaningful != Some(b')') {
        return len;
    }

    // 在末尾 256 字节窗口内反向扫描 ':' 找指标关键字。
    let window_start = len.saturating_sub(256);
    let window = &data[window_start..];

    let mut exectime_pos: Option<usize> = None;
    let mut rowcount_pos: Option<usize> = None;
    let mut exec_id_pos: Option<usize> = None;
    let mut search_end = window.len();
    while search_end > 0 {
        if exectime_pos.is_some() && rowcount_pos.is_some() && exec_id_pos.is_some() {
            break;
        }
        match memrchr(b':', &window[..search_end]) {
            None => break,
            Some(colon) => {
                if exectime_pos.is_none() && colon >= 8 && &window[colon - 8..colon] == b"EXECTIME"
                {
                    exectime_pos = Some(colon - 8);
                } else if rowcount_pos.is_none()
                    && colon >= 8
                    && &window[colon - 8..colon] == b"ROWCOUNT"
                {
                    rowcount_pos = Some(colon - 8);
                } else if exec_id_pos.is_none()
                    && colon >= 7
                    && &window[colon - 7..colon] == b"EXEC_ID"
                {
                    exec_id_pos = Some(colon - 7);
                }
                search_end = colon;
            }
        }
    }

    let earliest = [exectime_pos, rowcount_pos, exec_id_pos]
        .into_iter()
        .flatten()
        .min();
    match earliest {
        Some(pos) => {
            let split = window_start + pos;
            // 验证守卫：假阳性时返回全文
            let (_exectime, _rowcount, exec_id) = parse_indicators_from_bytes(&data[split..]);
            if exec_id != 0 || _exectime != 0.0 || _rowcount != 0 {
                split
            } else {
                len
            }
        }
        None => len,
    }
}
