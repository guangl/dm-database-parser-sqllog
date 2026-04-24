use atoi::atoi;
use encoding::DecoderTrap;
use encoding::Encoding;
use encoding::all::GB18030;
use memchr::memchr;
use memchr::memmem::{Finder, FinderRev};
use simdutf8::basic::from_utf8 as simd_from_utf8;
use std::borrow::Cow;
use std::sync::LazyLock;

use crate::parser::FileEncodingHint;

/// Pre-built SIMD finders for performance indicators — avoids per-call initialization.
static FINDER_EXECTIME: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"EXECTIME:"));
static FINDER_ROWCOUNT: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"ROWCOUNT:"));
static FINDER_EXEC_ID: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"EXEC_ID:"));

/// Maximum byte length of an indicators section.
/// Typical indicators ("EXECTIME: x(ms) ROWCOUNT: y(rows) EXEC_ID: z.") are ≤ 80 bytes.
/// 256 is a conservative upper bound that covers unusual padding or long EXEC_ID values.
const INDICATORS_WINDOW: usize = 256;

/// Pre-built reverse SIMD finders for split detection (include trailing space to avoid false positives).
static FINDER_REV_EXECTIME: LazyLock<FinderRev<'static>> =
    LazyLock::new(|| FinderRev::new(b"EXECTIME: "));
static FINDER_REV_ROWCOUNT: LazyLock<FinderRev<'static>> =
    LazyLock::new(|| FinderRev::new(b"ROWCOUNT: "));
static FINDER_REV_EXEC_ID: LazyLock<FinderRev<'static>> =
    LazyLock::new(|| FinderRev::new(b"EXEC_ID: "));

/// SQL 日志记录
///
/// 表示一条完整的 SQL 日志记录，包含时间戳、元数据、SQL 语句体和可选的性能指标。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sqllog<'a> {
    /// 时间戳，格式为 "YYYY-MM-DD HH:MM:SS.mmm"
    pub ts: Cow<'a, str>,

    /// 原始元数据字节（延迟解析）
    pub meta_raw: Cow<'a, str>,

    /// 原始内容（包含 Body 和 Indicators），延迟分割和解析
    pub content_raw: Cow<'a, [u8]>,

    /// 提取出的方括号标签（例如 [SEL]、[ORA]），若无则为 None
    pub tag: Option<Cow<'a, str>>,

    /// 文件级编码 hint（由 parser 探测），用于正确解码 content
    pub(crate) encoding: FileEncodingHint,
}

impl<'a> Sqllog<'a> {
    // ── 公开 API ─────────────────────────────────────────────────────────────

    /// 获取 SQL 语句体（延迟分割）
    pub fn body(&self) -> Cow<'a, str> {
        let split = self.find_indicators_split();
        let is_borrowed = matches!(&self.content_raw, Cow::Borrowed(_));
        // SAFETY: body_bytes 是 content_raw 的子切片，与 content_raw 共享 'a 生命周期
        unsafe { decode_content_bytes(&self.content_raw[..split], is_borrowed, self.encoding) }
    }

    /// 获取 SQL 语句体的长度（不做 UTF-8 校验，不分配）
    #[inline]
    pub fn body_len(&self) -> usize {
        self.find_indicators_split()
    }

    /// 获取 SQL 语句体的原始字节切片（不分配）
    #[inline]
    pub fn body_bytes(&self) -> &[u8] {
        &self.content_raw[..self.find_indicators_split()]
    }

    /// 获取原始性能指标字符串（延迟分割）
    pub fn indicators_raw(&self) -> Option<Cow<'a, str>> {
        let split = self.find_indicators_split();
        let ind_bytes = &self.content_raw[split..];
        if ind_bytes.is_empty() {
            return None;
        }
        let is_borrowed = matches!(&self.content_raw, Cow::Borrowed(_));
        // SAFETY: ind_bytes 是 content_raw 的子切片，与 content_raw 共享 'a 生命周期
        Some(unsafe { decode_content_bytes(ind_bytes, is_borrowed, self.encoding) })
    }

    /// 解析性能指标（sql 字段为空字符串）
    pub fn parse_indicators(&self) -> Option<PerformanceMetrics<'static>> {
        let ind_bytes = &self.content_raw[self.find_indicators_split()..];
        if ind_bytes.is_empty() {
            return None;
        }
        parse_indicators_from_bytes(ind_bytes)
    }

    /// 解析性能指标和 SQL 语句
    ///
    /// 返回包含 EXECTIME、ROWCOUNT、EXEC_ID 和 SQL 语句的 [`PerformanceMetrics`]。
    ///
    /// 当 tag 为 `"ORA"` 时，SQL 语句开头可能带有 `": "`，本方法会自动去除。
    ///
    /// # 实现说明
    /// 仅调用一次 `find_indicators_split()`，body 解码与 indicators 解析均在同一
    /// 次遍历中完成，`Cow::Borrowed` 路径全程零分配。
    pub fn parse_performance_metrics(&self) -> PerformanceMetrics<'a> {
        let split = self.find_indicators_split();
        let is_borrowed = matches!(&self.content_raw, Cow::Borrowed(_));

        // SAFETY: 子切片与 content_raw 共享 'a 生命周期
        let sql_raw =
            unsafe { decode_content_bytes(&self.content_raw[..split], is_borrowed, self.encoding) };

        let sql = if self.tag.as_deref() == Some("ORA") {
            strip_ora_prefix(sql_raw)
        } else {
            sql_raw
        };

        let mut pm = parse_indicators_from_bytes(&self.content_raw[split..]).unwrap_or_default();
        pm.sql = sql;
        pm
    }

    /// 解析元数据
    pub fn parse_meta(&self) -> MetaParts<'a> {
        let meta_bytes = self.meta_raw.as_bytes();
        let mut meta = MetaParts::default();
        let len = meta_bytes.len();
        let is_borrowed = matches!(&self.meta_raw, Cow::Borrowed(_));

        let to_cow = |bytes: &[u8]| -> Cow<'a, str> {
            if is_borrowed {
                // For Utf8 / Auto encoding: meta_raw is Cow::Borrowed — bytes is a sub-slice
                // of the memory-mapped buffer that lives for 'a.  The file was validated as
                // UTF-8 during `from_path`, so the unchecked conversion is sound.
                unsafe {
                    Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        bytes.as_ptr(),
                        bytes.len(),
                    )))
                }
            } else {
                // For Gb18030 / Auto-fallback encoding: meta_raw is Cow::Owned (already decoded
                // to a valid UTF-8 String).  We must NOT transmute the lifetime to 'a because
                // the Owned String lives only as long as `self`, not 'a.  Return an owned copy.
                Cow::Owned(
                    std::str::from_utf8(bytes)
                        .expect("meta_raw is always valid UTF-8")
                        .to_string(),
                )
            }
        };

        let mut idx = 0;
        while idx < len {
            // Skip whitespace
            while idx < len && meta_bytes[idx] == b' ' {
                idx += 1;
            }
            if idx >= len {
                break;
            }

            // Find token end
            let start = idx;
            while idx < len && meta_bytes[idx] != b' ' {
                idx += 1;
            }
            let part = &meta_bytes[start..idx];

            // Parse EP[n]
            if part.len() > 4
                && part[0] == b'E'
                && part[1] == b'P'
                && part[2] == b'['
                && part[part.len() - 1] == b']'
            {
                if let Some(ep) = atoi::<u8>(&part[3..part.len() - 1]) {
                    meta.ep = ep;
                }
                continue;
            }

            // Find ':'
            if let Some(sep) = memchr(b':', part) {
                let key = &part[..sep];
                let val = &part[sep + 1..];

                match key {
                    b"sess" => meta.sess_id = to_cow(val),
                    b"thrd" => meta.thrd_id = to_cow(val),
                    b"user" => meta.username = to_cow(val),
                    b"trxid" => meta.trxid = to_cow(val),
                    b"stmt" => meta.statement = to_cow(val),
                    b"ip" => meta.client_ip = to_cow(val),
                    b"appname" => {
                        if !val.is_empty() {
                            meta.appname = to_cow(val);
                        } else {
                            // Peek next token; treat it as appname only if it is not an ip field
                            let mut peek = idx;
                            while peek < len && meta_bytes[peek] == b' ' {
                                peek += 1;
                            }
                            if peek < len {
                                let peek_start = peek;
                                while peek < len && meta_bytes[peek] != b' ' {
                                    peek += 1;
                                }
                                let next = &meta_bytes[peek_start..peek];
                                if !(next.starts_with(b"ip:") || next.starts_with(b"ip::")) {
                                    meta.appname = to_cow(next);
                                    idx = peek;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        meta
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn find_indicators_split(&self) -> usize {
        let data = &self.content_raw;
        let len = data.len();

        // HOT-01: O(1) 早退 — DM 格式中有指标的记录以 '.' 结尾（EXEC_ID: N.）
        // 或以 ')' 结尾（仅 EXECTIME/ROWCOUNT，格式为 N(ms)/N(rows)）。
        // 跳过末尾 \n/\r，取最后一个有效字节；既非 '.' 也非 ')' 则无指标，直接返回。
        let last_meaningful = data
            .iter()
            .rev()
            .find(|&&b| b != b'\n' && b != b'\r')
            .copied();
        if last_meaningful != Some(b'.') && last_meaningful != Some(b')') {
            return len;
        }

        let start = len.saturating_sub(INDICATORS_WINDOW);
        let window = &data[start..];

        // Use SIMD reverse finders; take the leftmost (minimum) match position.
        let mut earliest = window.len();
        for finder in [
            &*FINDER_REV_EXECTIME,
            &*FINDER_REV_ROWCOUNT,
            &*FINDER_REV_EXEC_ID,
        ] {
            if let Some(idx) = finder.rfind(window) {
                earliest = earliest.min(idx);
            }
        }
        let split = start + earliest;
        // Only accept the split if the trailing slice contains parseable indicators.
        // If parse_indicators_from_bytes returns None, the "indicator-looking" text
        // is part of the SQL body — return len (no split).
        if split < len && parse_indicators_from_bytes(&data[split..]).is_none() {
            return len;
        }
        split
    }
}

// ── Module-level helpers ──────────────────────────────────────────────────────

/// Decode a sub-slice of `content_raw` bytes into a `Cow<'a, str>`.
///
/// # Safety
/// `bytes` must be a sub-slice of a `'a`-lived allocation (i.e., the original
/// `Cow::Borrowed(&'a [u8])`). The caller guarantees this by passing `is_borrowed = true`
/// only when the source `Cow` is `Borrowed`.
#[inline]
unsafe fn decode_content_bytes<'a>(
    bytes: &[u8],
    is_borrowed: bool,
    encoding: FileEncodingHint,
) -> Cow<'a, str> {
    match encoding {
        FileEncodingHint::Utf8 => {
            // File was already validated as UTF-8 during `from_path`; skip per-slice re-validation.
            if is_borrowed {
                unsafe {
                    Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        bytes.as_ptr(),
                        bytes.len(),
                    )))
                }
            } else {
                unsafe { Cow::Owned(std::str::from_utf8_unchecked(bytes).to_string()) }
            }
        }
        FileEncodingHint::Auto => match simd_from_utf8(bytes) {
            Ok(_) => {
                if is_borrowed {
                    unsafe {
                        Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                            bytes.as_ptr(),
                            bytes.len(),
                        )))
                    }
                } else {
                    unsafe { Cow::Owned(std::str::from_utf8_unchecked(bytes).to_string()) }
                }
            }
            Err(_) => Cow::Owned(String::from_utf8_lossy(bytes).into_owned()),
        },
        FileEncodingHint::Gb18030 => match GB18030.decode(bytes, DecoderTrap::Strict) {
            Ok(s) => Cow::Owned(s),
            Err(_) => Cow::Owned(String::from_utf8_lossy(bytes).into_owned()),
        },
    }
}

/// Parse `EXECTIME`, `ROWCOUNT`, `EXEC_ID` from a raw indicators byte slice.
/// The `sql` field of the returned struct is left as the default empty string.
/// Returns `None` if none of the three fields are present.
fn parse_indicators_from_bytes(ind: &[u8]) -> Option<PerformanceMetrics<'static>> {
    if ind.is_empty() {
        return None;
    }

    let mut out = PerformanceMetrics::default();
    let mut found = false;

    if let Some(idx) = FINDER_EXECTIME.find(ind) {
        let ss = idx + 9;
        if let Some(pi) = memchr(b'(', &ind[ss..]) {
            let val = ind[ss..ss + pi].trim_ascii();
            if let Ok(t) = fast_float::parse::<f32, _>(val) {
                out.exectime = t;
                found = true;
            }
        }
    }

    if let Some(idx) = FINDER_ROWCOUNT.find(ind) {
        let ss = idx + 9;
        if let Some(pi) = memchr(b'(', &ind[ss..])
            && let Some(c) = atoi::<u32>(ind[ss..ss + pi].trim_ascii())
        {
            out.rowcount = c;
            found = true;
        }
    }

    if let Some(idx) = FINDER_EXEC_ID.find(ind) {
        let ss = idx + 8;
        let end = memchr(b'.', &ind[ss..])
            .map(|i| ss + i)
            .unwrap_or(ind.len());
        if let Some(id) = atoi::<i64>(ind[ss..end].trim_ascii()) {
            out.exec_id = id;
            found = true;
        }
    }

    found.then_some(out)
}

/// Strip a leading `": "` prefix from a `Cow<str>` (zero-alloc for both paths).
#[inline]
fn strip_ora_prefix(s: Cow<'_, str>) -> Cow<'_, str> {
    match s {
        Cow::Borrowed(inner) => Cow::Borrowed(inner.strip_prefix(": ").unwrap_or(inner)),
        Cow::Owned(mut inner) => {
            if inner.starts_with(": ") {
                inner.drain(..2);
            }
            Cow::Owned(inner)
        }
    }
}

// ── Public types ──────────────────────────────────────────────────────────────

/// 元数据部分
///
/// 包含日志记录的所有元数据字段，如会话 ID、用户名等。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetaParts<'a> {
    /// EP（Execution Point）编号，范围 0-255
    pub ep: u8,

    /// 会话 ID
    pub sess_id: Cow<'a, str>,

    /// 线程 ID
    pub thrd_id: Cow<'a, str>,

    /// 用户名
    pub username: Cow<'a, str>,

    /// 事务 ID
    pub trxid: Cow<'a, str>,

    /// 语句 ID
    pub statement: Cow<'a, str>,

    /// 应用程序名称
    pub appname: Cow<'a, str>,

    /// 客户端 IP 地址（可选）
    pub client_ip: Cow<'a, str>,
}

/// SQL 记录的性能指标和 SQL 语句
///
/// 包含 SQL 执行的性能指标，如执行时间、影响行数、执行 ID 和完整的 SQL 语句。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PerformanceMetrics<'a> {
    /// 执行时间（毫秒）
    pub exectime: f32,

    /// 影响的行数
    pub rowcount: u32,

    /// 执行 ID
    pub exec_id: i64,

    /// 完整的 SQL 语句
    pub sql: Cow<'a, str>,
}
