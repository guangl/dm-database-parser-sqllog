use atoi::atoi;
use encoding::DecoderTrap;
use encoding::Encoding;
use encoding::all::GB18030;
use memchr::{memchr, memrchr};
use simdutf8::basic::from_utf8 as simd_from_utf8;
use std::borrow::Cow;

use crate::parser::FileEncodingHint;

/// SQL 日志记录，包含时间戳、元数据、SQL 语句体和可选的性能指标。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sqllog<'a> {
    /// 时间戳，格式为 "YYYY-MM-DD HH:MM:SS.mmm"
    pub ts: Cow<'a, str>,
    /// 原始元数据（延迟解析）
    pub meta_raw: Cow<'a, str>,
    /// 原始内容（Body + Indicators），延迟分割和解析
    pub content_raw: Cow<'a, [u8]>,
    /// 提取出的方括号标签（例如 [SEL]、[ORA]），若无则为 None
    pub tag: Option<Cow<'a, str>>,
    /// 文件级编码 hint，用于正确解码 content
    pub encoding: FileEncodingHint,
}

impl<'a> Sqllog<'a> {
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

    /// 解析性能指标（sql 字段为空）
    pub fn parse_indicators(&self) -> Option<PerformanceMetrics<'static>> {
        let ind_bytes = &self.content_raw[self.find_indicators_split()..];
        if ind_bytes.is_empty() {
            return None;
        }
        parse_indicators_from_bytes(ind_bytes)
    }

    /// 解析性能指标和 SQL 语句（热路径：仅调用一次 find_indicators_split）
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

        // SAFETY: meta_raw 是有效 UTF-8（由 parser 保证）。
        // Borrowed 路径通过指针重建将生命周期延长到 'a。
        let to_cow = |bytes: &[u8]| -> Cow<'a, str> {
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
        };

        let mut idx = 0;
        while idx < len {
            while idx < len && meta_bytes[idx] == b' ' {
                idx += 1;
            }
            if idx >= len {
                break;
            }

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
                            // Peek ahead: take next token as appname only if it's not an ip field
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
                                if !next.starts_with(b"ip:") && !next.starts_with(b"ip::") {
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

    fn find_indicators_split(&self) -> usize {
        let data = &self.content_raw;
        let len = data.len();
        let start = len.saturating_sub(256);
        let window = &data[start..len];
        let mut tail = window.len();

        for keyword in [b"EXEC_ID".as_ref(), b"ROWCOUNT".as_ref(), b"EXECTIME".as_ref()] {
            tail = find_keyword_end_backward(window, tail, keyword).unwrap_or(tail);
        }

        start + tail
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Decode a sub-slice of `content_raw` bytes into a `Cow<'a, str>`.
///
/// # Safety
/// `bytes` must be a sub-slice of a `'a`-lived allocation. The caller guarantees
/// this by passing `is_borrowed = true` only when the source `Cow` is `Borrowed`.
#[inline]
unsafe fn decode_content_bytes<'a>(
    bytes: &[u8],
    is_borrowed: bool,
    encoding: FileEncodingHint,
) -> Cow<'a, str> {
    match encoding {
        FileEncodingHint::Utf8 | FileEncodingHint::Auto => match simd_from_utf8(bytes) {
            Ok(s) => {
                if is_borrowed {
                    // SAFETY: bytes is a sub-slice of a 'a-lived mmap
                    unsafe {
                        Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                            bytes.as_ptr(),
                            bytes.len(),
                        )))
                    }
                } else {
                    Cow::Owned(s.to_string())
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

/// Scan `window[..within]` backwards for `keyword: ` (keyword + colon + space).
/// Returns the split boundary (offset just before the keyword) if found.
#[inline]
fn find_keyword_end_backward(window: &[u8], within: usize, keyword: &[u8]) -> Option<usize> {
    let klen = keyword.len();
    let mut search_end = within;
    while let Some(idx) = memrchr(b':', &window[..search_end]) {
        if idx >= klen
            && &window[idx - klen..idx] == keyword
            && idx + 1 < window.len()
            && window[idx + 1] == b' '
        {
            return Some(idx - klen);
        }
        if idx == 0 {
            break;
        }
        search_end = idx;
    }
    None
}

/// Parse `EXECTIME`, `ROWCOUNT`, `EXEC_ID` from a raw indicators byte slice.
/// The `sql` field is left as the default empty string.
fn parse_indicators_from_bytes(ind: &[u8]) -> Option<PerformanceMetrics<'static>> {
    if ind.is_empty() {
        return None;
    }

    let mut out = PerformanceMetrics::default();
    let mut found = false;

    if let Some(idx) = memchr::memmem::find(ind, b"EXECTIME:") {
        let ss = idx + 9;
        if let Some(pi) = memchr(b'(', &ind[ss..]) {
            let val = ind[ss..ss + pi].trim_ascii();
            // SAFETY: val is ASCII digits and '.', a valid UTF-8 subset
            if let Ok(t) = unsafe { std::str::from_utf8_unchecked(val) }.parse::<f32>() {
                out.exectime = t;
                found = true;
            }
        }
    }

    if let Some(idx) = memchr::memmem::find(ind, b"ROWCOUNT:") {
        let ss = idx + 9;
        if let Some(pi) = memchr(b'(', &ind[ss..])
            && let Some(c) = atoi::<u32>(ind[ss..ss + pi].trim_ascii())
        {
            out.rowcount = c;
            found = true;
        }
    }

    if let Some(idx) = memchr::memmem::find(ind, b"EXEC_ID:") {
        let ss = idx + 8;
        let end = memchr(b'.', &ind[ss..]).map(|i| ss + i).unwrap_or(ind.len());
        if let Some(id) = atoi::<i64>(ind[ss..end].trim_ascii()) {
            out.exec_id = id;
            found = true;
        }
    }

    found.then_some(out)
}

/// Strip a leading `": "` prefix (zero-alloc for the `Borrowed` path).
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

/// 日志记录的元数据字段
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetaParts<'a> {
    /// EP（Execution Point）编号
    pub ep: u8,
    pub sess_id: Cow<'a, str>,
    pub thrd_id: Cow<'a, str>,
    pub username: Cow<'a, str>,
    pub trxid: Cow<'a, str>,
    pub statement: Cow<'a, str>,
    pub appname: Cow<'a, str>,
    pub client_ip: Cow<'a, str>,
}

/// SQL 执行的性能指标和 SQL 语句
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
