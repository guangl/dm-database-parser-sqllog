use atoi::atoi;
use memchr::memchr;
use memchr::memchr2;
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

    /// 事务锁事件（LOCK_TID 日志条目），非锁行为 None
    pub lock_event: Option<LockEvent>,
}

/// 事务锁事件
///
/// 对应达梦 SQL 日志中的 `LOCK_TID` 锁等待条目。三种格式：
/// - 单阻塞: `trx[N] LOCK_TID (mode:M, table id:T, tid[K]) wait used time:X(ms|us)`
/// - 多阻塞: `trx[N] LOCK_TID (mode:M, table id:T) wait for K trxs, trx[a, b, ...more] used time:X(ms|us)`
/// - legacy: `trx[N] wait for LOCK_TID (mode:M, table id:T, tid[a, b]) used time:X(ms|us)`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LockEvent {
    /// 等待者事务 ID（`trx[N]` 中的 N）
    pub waiting_trx_id: u64,
    /// 阻塞者事务 ID 列表（`tid[...]` 或 `trx[...]` 中的数字）
    pub blocking_trx_ids: Vec<u64>,
    /// 锁模式（`mode:M`，如 S / X）
    pub lock_mode: String,
    /// 锁定的表 ID（`table id:T`）
    pub table_id: u64,
    /// 等待时长，统一为微秒（`(ms)` 时 ×1000，`(us)` 时原样）
    pub wait_time_us: u64,
    /// 阻塞者列表是否被 `...more` 截断
    pub has_more_blocking: bool,
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

// ── 事务锁事件解析（memchr 定位 + 定界符解析，零正则）──────────────────────────

/// 跳过前导 ASCII 空格。
#[inline]
fn skip_ascii_spaces(mut s: &[u8]) -> &[u8] {
    while s.first() == Some(&b' ') {
        s = &s[1..];
    }
    s
}

/// 取到第一个 `,` 或 `)` 为止；返回 `(字段, 定界符及之后)`。
#[inline]
fn split_at_comma_or_paren(s: &[u8]) -> (&[u8], &[u8]) {
    match memchr2(b',', b')', s) {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, &[]),
    }
}

/// 解析方括号内逗号分隔的数字列表（如 `tid[456, 789]` 或 `trx[1, 2]`）。
///
/// 支持 `...more` 截断标记。返回是否包含 `...more`。
fn parse_id_list(bytes: &[u8], out: &mut Vec<u64>) -> Option<bool> {
    let mut has_more = false;
    for part in bytes.split(|&b| b == b',') {
        let part = part.trim_ascii();
        if part == b"...more" {
            has_more = true;
        } else if !part.is_empty() {
            out.push(atoi::<u64>(part)?);
        }
    }
    Some(has_more)
}

/// 从 SQL body 字节中解析事务锁事件。
///
/// 输入为 `trx[...]` 开头的锁行 body（不含时间戳与元数据）。支持三种格式：
/// - 单阻塞:  `trx[N] LOCK_TID (mode:M, table id:T, tid[K]) wait used time:X(ms|us)`
/// - 多阻塞:  `trx[N] LOCK_TID (mode:M, table id:T) wait for K trxs, trx[a, b, ...more] used time:X(ms|us)`
/// - legacy:  `trx[N] wait for LOCK_TID (mode:M, table id:T, tid[a, b]) used time:X(ms|us)`
///
/// 返回 `None` 表示不是可识别的锁行。
pub(crate) fn parse_lock_event_from_bytes(body: &[u8]) -> Option<LockEvent> {
    let mut event = LockEvent::default();
    let mut rest = body;

    // ── S0: `trx[N]` 等待者 ──
    rest = rest.strip_prefix(b"trx[")?;
    let close = memchr(b']', rest)?;
    event.waiting_trx_id = atoi::<u64>(&rest[..close])?;
    rest = skip_ascii_spaces(&rest[close + 1..]);

    // ── S1: `LOCK_TID` 定位（legacy 前有 `wait for `）──
    let is_legacy = rest.starts_with(b"wait for ");
    if is_legacy {
        rest = rest.strip_prefix(b"wait for ")?;
    }
    rest = rest.strip_prefix(b"LOCK_TID")?;
    rest = skip_ascii_spaces(rest);

    // ── S2: `(mode:M, table id:T[, tid[...]])` ──
    rest = rest.strip_prefix(b"(")?;
    rest = rest.strip_prefix(b"mode:")?;
    let (mode, r) = split_at_comma_or_paren(rest);
    event.lock_mode = String::from_utf8_lossy(mode.trim_ascii()).into_owned();
    rest = skip_ascii_spaces(r);
    rest = rest.strip_prefix(b",")?;
    rest = skip_ascii_spaces(rest);
    rest = rest.strip_prefix(b"table id:")?;
    let (table_id, r) = split_at_comma_or_paren(rest);
    event.table_id = atoi::<u64>(table_id.trim_ascii())?;
    rest = skip_ascii_spaces(r);

    // 可选的括号内 `tid[...]`（单阻塞 / legacy 的阻塞者列表）
    if let Some(r) = rest.strip_prefix(b",") {
        rest = skip_ascii_spaces(r);
        rest = rest.strip_prefix(b"tid[")?;
        let close = memchr(b']', rest)?;
        if let Some(has_more) = parse_id_list(&rest[..close], &mut event.blocking_trx_ids) {
            event.has_more_blocking = has_more;
        }
        rest = skip_ascii_spaces(&rest[close + 1..]);
    }
    rest = rest.strip_prefix(b")")?;
    rest = skip_ascii_spaces(rest);

    // ── S3: 括号后分支 ──
    if is_legacy {
        // legacy: `used time:X(ms|us)`
        rest = rest.strip_prefix(b"used time:")?;
    } else if let Some(r) = rest.strip_prefix(b"wait for ") {
        // 多阻塞: `wait for K trxs, trx[a, b, ...more] used time:X(ms|us)`
        let digits = r.iter().take_while(|&&b| b.is_ascii_digit()).count();
        let r = &r[digits..];
        rest = r.strip_prefix(b" trxs, trx[")?;
        let close = memchr(b']', rest)?;
        if let Some(has_more) = parse_id_list(&rest[..close], &mut event.blocking_trx_ids) {
            event.has_more_blocking = has_more;
        }
        rest = skip_ascii_spaces(&rest[close + 1..]);
        rest = rest.strip_prefix(b"used time:")?;
    } else {
        // 单阻塞: `wait used time:X(ms|us)`
        rest = rest.strip_prefix(b"wait used time:")?;
    }

    // ── S4: 等待时长（统一微秒，`(ms)` ×1000，`(us)` 原样）──
    let digits = rest.iter().take_while(|&&b| b.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let time_val = atoi::<u64>(&rest[..digits])?;
    let unit = &rest[digits..];
    if unit.starts_with(b"(ms)") {
        event.wait_time_us = time_val * 1000;
    } else if unit.starts_with(b"(us)") {
        event.wait_time_us = time_val;
    } else {
        return None;
    }

    Some(event)
}

// ── 测试 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_event(body: &str, expected: LockEvent) {
        let event = parse_lock_event_from_bytes(body.as_bytes());
        assert_eq!(event, Some(expected), "body: {body}");
    }

    #[test]
    fn single_block_format() {
        // 单阻塞：括号内 tid[K]，时间前缀 `wait used time:`
        assert_event(
            "trx[123] LOCK_TID (mode:S, table id:42, tid[456]) wait used time:500(ms)",
            LockEvent {
                waiting_trx_id: 123,
                blocking_trx_ids: vec![456],
                lock_mode: "S".into(),
                table_id: 42,
                wait_time_us: 500_000,
                has_more_blocking: false,
            },
        );
    }

    #[test]
    fn single_block_format_us_unit() {
        // 微秒单位原样保留
        assert_event(
            "trx[123] LOCK_TID (mode:S, table id:42, tid[456]) wait used time:800(us)",
            LockEvent {
                waiting_trx_id: 123,
                blocking_trx_ids: vec![456],
                lock_mode: "S".into(),
                table_id: 42,
                wait_time_us: 800,
                has_more_blocking: false,
            },
        );
    }

    #[test]
    fn multi_block_format() {
        // 多阻塞：括号内无 tid，阻塞者在括号后 trx[...] 中，含 ...more
        assert_event(
            "trx[123] LOCK_TID (mode:S, table id:42) wait for 3 trxs, trx[456, 789, ...more] used time:500(ms)",
            LockEvent {
                waiting_trx_id: 123,
                blocking_trx_ids: vec![456, 789],
                lock_mode: "S".into(),
                table_id: 42,
                wait_time_us: 500_000,
                has_more_blocking: true,
            },
        );
    }

    #[test]
    fn multi_block_format_no_more() {
        // 多阻塞无截断
        assert_event(
            "trx[123] LOCK_TID (mode:X, table id:7) wait for 2 trxs, trx[456, 789] used time:300(ms)",
            LockEvent {
                waiting_trx_id: 123,
                blocking_trx_ids: vec![456, 789],
                lock_mode: "X".into(),
                table_id: 7,
                wait_time_us: 300_000,
                has_more_blocking: false,
            },
        );
    }

    #[test]
    fn legacy_format() {
        // legacy：`wait for LOCK_TID`，括号内 tid[a, b]，时间前缀 `used time:`
        assert_event(
            "trx[123] wait for LOCK_TID (mode:S, table id:42, tid[456, 789]) used time:900(ms)",
            LockEvent {
                waiting_trx_id: 123,
                blocking_trx_ids: vec![456, 789],
                lock_mode: "S".into(),
                table_id: 42,
                wait_time_us: 900_000,
                has_more_blocking: false,
            },
        );
    }

    #[test]
    fn not_a_lock_line() {
        // 非锁行：不以 trx[ 开头 → None
        assert_eq!(parse_lock_event_from_bytes(b"SELECT 1"), None);
        // 以 trx[ 开头但缺 LOCK_TID → None
        assert_eq!(parse_lock_event_from_bytes(b"trx[1] some other content"), None);
        // LOCK_TID 但无括号结构 → None
        assert_eq!(parse_lock_event_from_bytes(b"trx[1] LOCK_TID garbage"), None);
        // 空输入
        assert_eq!(parse_lock_event_from_bytes(b""), None);
    }

    #[test]
    fn wait_used_time_without_tid_in_paren() {
        // 单阻塞但括号内缺少 tid 字段（部分格式变体）→ 仍可解析，阻塞列表为空
        assert_event(
            "trx[123] LOCK_TID (mode:S, table id:42) wait used time:100(ms)",
            LockEvent {
                waiting_trx_id: 123,
                blocking_trx_ids: vec![],
                lock_mode: "S".into(),
                table_id: 42,
                wait_time_us: 100_000,
                has_more_blocking: false,
            },
        );
    }
}
