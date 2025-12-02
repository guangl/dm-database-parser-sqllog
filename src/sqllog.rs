use atoi::atoi;
use memchr::{memchr, memrchr};
use simdutf8::basic::from_utf8 as simd_from_utf8;
use std::borrow::Cow;

/// SQL 日志记录
///
/// 表示一条完整的 SQL 日志记录，包含时间戳、元数据、SQL 语句体和可选的性能指标。
///

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sqllog<'a> {
    /// 时间戳，格式为 "YYYY-MM-DD HH:MM:SS.mmm"
    pub ts: Cow<'a, str>,

    /// 原始元数据字节（延迟解析）
    pub meta_raw: Cow<'a, str>,

    /// 原始内容（包含 Body 和 Indicators），延迟分割和解析
    pub content_raw: Cow<'a, [u8]>,
}

impl<'a> Sqllog<'a> {
    /// 获取 SQL 语句体（延迟分割）
    pub fn body(&self) -> Cow<'a, str> {
        let split = self.find_indicators_split();
        let body_bytes = &self.content_raw[..split];
        match simd_from_utf8(body_bytes) {
            Ok(s) => match &self.content_raw {
                Cow::Borrowed(_) => unsafe {
                    let ptr = body_bytes.as_ptr();
                    let len = body_bytes.len();
                    let slice = std::slice::from_raw_parts(ptr, len);
                    Cow::Borrowed(std::str::from_utf8_unchecked(slice))
                },
                Cow::Owned(_) => Cow::Owned(s.to_string()),
            },
            Err(_) => Cow::Owned(String::from_utf8_lossy(body_bytes).into_owned()),
        }
    }

    /// 获取原始性能指标字符串（延迟分割）
    pub fn indicators_raw(&self) -> Option<Cow<'a, str>> {
        let split = self.find_indicators_split();
        let indicators_bytes = &self.content_raw[split..];
        if indicators_bytes.is_empty() {
            return None;
        }
        match &self.content_raw {
            Cow::Borrowed(_) => unsafe {
                let ptr = indicators_bytes.as_ptr();
                let len = indicators_bytes.len();
                let slice = std::slice::from_raw_parts(ptr, len);
                Some(Cow::Borrowed(std::str::from_utf8_unchecked(slice)))
            },
            Cow::Owned(_) => unsafe {
                Some(Cow::Owned(
                    std::str::from_utf8_unchecked(indicators_bytes).to_string(),
                ))
            },
        }
    }

    fn find_indicators_split(&self) -> usize {
        let body = &self.content_raw;
        let current_len = body.len();
        let search_limit = 256;
        let start_search = current_len.saturating_sub(search_limit);
        let search_slice = &body[start_search..current_len];

        let mut tail_len = search_slice.len();

        // 1. EXEC_ID
        let mut search_end = tail_len;
        while let Some(idx) = memrchr(b':', &search_slice[..search_end]) {
            if idx >= 7
                && &search_slice[idx - 7..idx] == b"EXEC_ID"
                && idx + 1 < search_slice.len()
                && search_slice[idx + 1] == b' '
            {
                tail_len = idx - 7;
                break;
            }
            if idx == 0 {
                break;
            }
            search_end = idx;
        }

        // 2. ROWCOUNT
        let slice_view = &search_slice[..tail_len];
        search_end = slice_view.len();
        while let Some(idx) = memrchr(b':', &slice_view[..search_end]) {
            if idx >= 8
                && &search_slice[idx - 8..idx] == b"ROWCOUNT"
                && idx + 1 < search_slice.len()
                && search_slice[idx + 1] == b' '
            {
                tail_len = idx - 8;
                break;
            }
            if idx == 0 {
                break;
            }
            search_end = idx;
        }

        // 3. EXECTIME
        let slice_view = &search_slice[..tail_len];
        search_end = slice_view.len();
        while let Some(idx) = memrchr(b':', &slice_view[..search_end]) {
            if idx >= 8
                && &search_slice[idx - 8..idx] == b"EXECTIME"
                && idx + 1 < search_slice.len()
                && search_slice[idx + 1] == b' '
            {
                tail_len = idx - 8;
                break;
            }
            if idx == 0 {
                break;
            }
            search_end = idx;
        }

        start_search + tail_len
    }

    /// 解析性能指标
    pub fn parse_indicators(&self) -> Option<IndicatorsParts> {
        let raw_cow = self.indicators_raw()?;
        let raw = raw_cow.as_ref();
        let bytes = raw.as_bytes();

        // We need to parse the indicators from the raw string.
        // The format is "EXECTIME: ... ROWCOUNT: ... EXEC_ID: ..."
        // But the order might vary or some might be missing?
        // The parser logic in parser.rs handled this by searching backwards.
        // We should duplicate that logic here or move it here.
        // Since we want to keep parser.rs focused on splitting, let's implement parsing here.

        let mut indicators = IndicatorsParts::default();
        let mut has_indicators = false;

        // Helper to trim
        fn trim(b: &[u8]) -> &[u8] {
            let start = b
                .iter()
                .position(|&x| !x.is_ascii_whitespace())
                .unwrap_or(0);
            let end = b
                .iter()
                .rposition(|&x| !x.is_ascii_whitespace())
                .map(|i| i + 1)
                .unwrap_or(start);
            &b[start..end]
        }

        // We can use a simple forward scan or regex-like search since we have the isolated string.
        // "EXECTIME: 1.0(ms) ROWCOUNT: 1(rows) EXEC_ID: 100."

        // Parse EXECTIME
        if let Some(idx) = memchr::memmem::find(bytes, b"EXECTIME:")
            && let Some(end) = memchr(b'(', &bytes[idx..])
        {
            let val_bytes = &bytes[idx + 9..idx + end]; // 9 is len of "EXECTIME:"
            let val_trimmed = trim(val_bytes);
            // unsafe is fine as we trust the source from parser
            let s = unsafe { std::str::from_utf8_unchecked(val_trimmed) };
            if let Ok(time) = s.parse::<f32>() {
                indicators.execute_time = time;
                has_indicators = true;
            }
        }

        // Parse ROWCOUNT
        if let Some(idx) = memchr::memmem::find(bytes, b"ROWCOUNT:")
            && let Some(end) = memchr(b'(', &bytes[idx..])
        {
            let val_bytes = &bytes[idx + 9..idx + end];
            let val_trimmed = trim(val_bytes);
            if let Some(count) = atoi::<u32>(val_trimmed) {
                indicators.row_count = count;
                has_indicators = true;
            }
        }

        // Parse EXEC_ID
        if let Some(idx) = memchr::memmem::find(bytes, b"EXEC_ID:") {
            // Ends with . or end of string
            let suffix = &bytes[idx + 8..];
            let end = memchr(b'.', suffix).unwrap_or(suffix.len());
            let val_bytes = &suffix[..end];
            let val_trimmed = trim(val_bytes);
            if let Some(id) = atoi::<i64>(val_trimmed) {
                indicators.execute_id = id;
                has_indicators = true;
            }
        }

        if has_indicators {
            Some(indicators)
        } else {
            None
        }
    }

    /// 解析元数据
    pub fn parse_meta(&self) -> MetaParts<'a> {
        let meta_bytes = self.meta_raw.as_bytes();
        let mut meta = MetaParts::default();
        let mut idx = 0;
        let len = meta_bytes.len();

        while idx < len {
            // Skip whitespace
            while idx < len && meta_bytes[idx].is_ascii_whitespace() {
                idx += 1;
            }
            if idx >= len {
                break;
            }

            let start = idx;
            // Find end of token using memchr for space (optimization)
            let end = match memchr(b' ', &meta_bytes[idx..]) {
                Some(i) => idx + i,
                None => len,
            };

            let part = &meta_bytes[start..end];
            idx = end;

            if part.starts_with(b"EP[") && part.ends_with(b"]") {
                // EP[0]
                let num_bytes = &part[3..part.len() - 1];
                if let Some(ep) = atoi::<u8>(num_bytes) {
                    meta.ep = ep;
                }
                continue;
            }

            if let Some(sep_idx) = memchr(b':', part) {
                let key = &part[0..sep_idx];
                let val = &part[sep_idx + 1..];

                // Helper to convert bytes to Cow using unsafe for known ASCII keys
                let to_cow_trusted = |bytes: &[u8]| -> Cow<'a, str> {
                    // We need to extend the lifetime of bytes to 'a.
                    // Since meta_raw is Cow<'a, str>, if it's Borrowed, the bytes are &'a [u8].
                    // If it's Owned, we can't return Cow::Borrowed referencing it easily without unsafe.
                    // But wait, self.meta_raw is Cow<'a, str>.
                    // If self.meta_raw is Borrowed(&'a str), then bytes are from that slice, so they are &'a [u8].
                    // If self.meta_raw is Owned(String), then bytes are from that String. We can't return Cow::Borrowed(&'a str) pointing to it.

                    // This is the tricky part of lazy parsing with Cow.
                    // If we have Owned data, we must return Owned data or clone.
                    // But parse_meta returns MetaParts<'a>.

                    // If self.meta_raw is Borrowed, we can return Borrowed.
                    // If self.meta_raw is Owned, we MUST return Owned.

                    match &self.meta_raw {
                        Cow::Borrowed(_) => unsafe {
                            // Reconstruct the lifetime 'a
                            // We know bytes points into self.meta_raw which is 'a
                            let ptr = bytes.as_ptr();
                            let len = bytes.len();
                            let slice = std::slice::from_raw_parts(ptr, len);
                            Cow::Borrowed(std::str::from_utf8_unchecked(slice))
                        },
                        Cow::Owned(_) => {
                            // We must allocate
                            unsafe { Cow::Owned(std::str::from_utf8_unchecked(bytes).to_string()) }
                        }
                    }
                };

                let to_cow = |bytes: &[u8]| -> Cow<'a, str> {
                    match &self.meta_raw {
                        Cow::Borrowed(_) => match simd_from_utf8(bytes) {
                            Ok(_) => unsafe {
                                let ptr = bytes.as_ptr();
                                let len = bytes.len();
                                let slice = std::slice::from_raw_parts(ptr, len);
                                Cow::Borrowed(std::str::from_utf8_unchecked(slice))
                            },
                            Err(_) => Cow::Owned(String::from_utf8_lossy(bytes).into_owned()),
                        },
                        Cow::Owned(_) => match simd_from_utf8(bytes) {
                            Ok(s) => Cow::Owned(s.to_string()),
                            Err(_) => Cow::Owned(String::from_utf8_lossy(bytes).into_owned()),
                        },
                    }
                };

                match key {
                    b"sess" => meta.sess_id = to_cow_trusted(val),
                    b"thrd" => meta.thrd_id = to_cow_trusted(val),
                    b"user" => meta.username = to_cow(val),
                    b"trxid" => meta.trxid = to_cow_trusted(val),
                    b"stmt" => meta.statement = to_cow_trusted(val),
                    b"appname" => {
                        if val.is_empty() {
                            let mut next_idx = idx;
                            while next_idx < len && meta_bytes[next_idx].is_ascii_whitespace() {
                                next_idx += 1;
                            }
                            if next_idx < len {
                                let next_start = next_idx;
                                let next_end = match memchr(b' ', &meta_bytes[next_idx..]) {
                                    Some(i) => next_idx + i,
                                    None => len,
                                };
                                let next_part = &meta_bytes[next_start..next_end];

                                if next_part.starts_with(b"ip:") && !next_part.starts_with(b"ip::")
                                {
                                    // Next part is ip key
                                } else {
                                    meta.appname = to_cow(next_part);
                                    idx = next_end;
                                }
                            }
                        } else {
                            meta.appname = to_cow(val);
                        }
                    }
                    b"ip" => {
                        meta.client_ip = to_cow_trusted(val);
                    }
                    _ => {}
                }
            }
        }
        meta
    }
}

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

/// 性能指标部分
///
/// 包含 SQL 执行的性能指标，如执行时间、影响行数等。
///

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IndicatorsParts {
    /// 执行时间（毫秒）
    pub execute_time: f32,

    /// 影响的行数
    pub row_count: u32,

    /// 执行 ID
    pub execute_id: i64,
}
