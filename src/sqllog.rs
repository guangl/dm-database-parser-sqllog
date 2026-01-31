use atoi::atoi;
use encoding::DecoderTrap;
use encoding::Encoding;
use encoding::all::GB18030;
use memchr::{memchr, memrchr};
use simdutf8::basic::from_utf8 as simd_from_utf8;
use std::borrow::Cow;

/// SQL 日志记录
///
/// 表示一条完整的 SQL 日志记录，包含时间戳、元数据、SQL 语句体和可选的性能指标。
///
///
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
    pub encoding: crate::parser::FileEncodingHint,
}

impl<'a> Sqllog<'a> {
    /// 获取 SQL 语句体（延迟分割）
    pub fn body(&self) -> Cow<'a, str> {
        let split = self.find_indicators_split();
        let body_bytes = &self.content_raw[..split];

        match self.encoding {
            crate::parser::FileEncodingHint::Utf8 | crate::parser::FileEncodingHint::Auto => {
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
            crate::parser::FileEncodingHint::Gb18030 => {
                // Decode using GB18030 and return owned string
                match GB18030.decode(body_bytes, DecoderTrap::Strict) {
                    Ok(s) => Cow::Owned(s),
                    Err(_) => Cow::Owned(String::from_utf8_lossy(body_bytes).into_owned()),
                }
            }
        }
    }

    /// 获取 SQL 语句体的长度（不做 UTF-8 校验，不分配）
    #[inline]
    pub fn body_len(&self) -> usize {
        self.find_indicators_split()
    }

    /// 获取 SQL 语句体的原始字节切片（不分配）
    #[inline]
    pub fn body_bytes(&self) -> &[u8] {
        let split = self.find_indicators_split();
        &self.content_raw[..split]
    }

    /// 获取原始性能指标字符串（延迟分割）
    pub fn indicators_raw(&self) -> Option<Cow<'a, str>> {
        let split = self.find_indicators_split();
        let indicators_bytes = &self.content_raw[split..];
        if indicators_bytes.is_empty() {
            return None;
        }

        match self.encoding {
            crate::parser::FileEncodingHint::Utf8 | crate::parser::FileEncodingHint::Auto => {
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
            crate::parser::FileEncodingHint::Gb18030 => {
                match GB18030.decode(indicators_bytes, DecoderTrap::Strict) {
                    Ok(s) => Some(Cow::Owned(s)),
                    Err(_) => Some(Cow::Owned(
                        String::from_utf8_lossy(indicators_bytes).into_owned(),
                    )),
                }
            }
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
        let split = self.find_indicators_split();
        let indicators_bytes = &self.content_raw[split..];
        if indicators_bytes.is_empty() {
            return None;
        }

        let mut indicators = IndicatorsParts::default();
        let mut has_indicators = false;

        // Parse EXECTIME
        if let Some(idx) = memchr::memmem::find(indicators_bytes, b"EXECTIME:") {
            // Find '(' after EXECTIME:
            let search_start = idx + 9;
            if let Some(paren_idx) = memchr(b'(', &indicators_bytes[search_start..]) {
                let val_bytes = &indicators_bytes[search_start..search_start + paren_idx];
                // Trim manually for speed
                let mut start = 0;
                let mut end = val_bytes.len();
                while start < end && val_bytes[start] == b' ' {
                    start += 1;
                }
                while end > start && val_bytes[end - 1] == b' ' {
                    end -= 1;
                }
                if start < end {
                    let s = unsafe { std::str::from_utf8_unchecked(&val_bytes[start..end]) };
                    if let Ok(time) = s.parse::<f32>() {
                        indicators.execute_time = time;
                        has_indicators = true;
                    }
                }
            }
        }

        // Parse ROWCOUNT
        if let Some(idx) = memchr::memmem::find(indicators_bytes, b"ROWCOUNT:") {
            let search_start = idx + 9;
            if let Some(paren_idx) = memchr(b'(', &indicators_bytes[search_start..]) {
                let val_bytes = &indicators_bytes[search_start..search_start + paren_idx];
                let mut start = 0;
                let mut end = val_bytes.len();
                while start < end && val_bytes[start] == b' ' {
                    start += 1;
                }
                while end > start && val_bytes[end - 1] == b' ' {
                    end -= 1;
                }
                if start < end {
                    if let Some(count) = atoi::<u32>(&val_bytes[start..end]) {
                        indicators.row_count = count;
                        has_indicators = true;
                    }
                }
            }
        }

        // Parse EXEC_ID
        if let Some(idx) = memchr::memmem::find(indicators_bytes, b"EXEC_ID:") {
            let search_start = idx + 8;
            let end = memchr(b'.', &indicators_bytes[search_start..])
                .map(|i| search_start + i)
                .unwrap_or(indicators_bytes.len());
            let val_bytes = &indicators_bytes[search_start..end];
            let mut start = 0;
            let mut trimmed_end = val_bytes.len();
            while start < trimmed_end && val_bytes[start] == b' ' {
                start += 1;
            }
            while trimmed_end > start && val_bytes[trimmed_end - 1] == b' ' {
                trimmed_end -= 1;
            }
            if start < trimmed_end {
                if let Some(id) = atoi::<i64>(&val_bytes[start..trimmed_end]) {
                    indicators.execute_id = id;
                    has_indicators = true;
                }
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
        let len = meta_bytes.len();

        // Determine if we're working with borrowed or owned data once
        let is_borrowed = matches!(&self.meta_raw, Cow::Borrowed(_));

        // Fast path: single pass through meta_bytes with manual tokenization
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

                // Fast conversion: no validation, direct unsafe cast
                let to_cow = |bytes: &[u8]| -> Cow<'a, str> {
                    if is_borrowed {
                        unsafe {
                            Cow::Borrowed(std::str::from_utf8_unchecked(
                                std::slice::from_raw_parts(bytes.as_ptr(), bytes.len()),
                            ))
                        }
                    } else {
                        unsafe { Cow::Owned(std::str::from_utf8_unchecked(bytes).to_string()) }
                    }
                };

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
                            // Peek next token
                            let mut peek_idx = idx;
                            while peek_idx < len && meta_bytes[peek_idx] == b' ' {
                                peek_idx += 1;
                            }
                            if peek_idx < len {
                                let peek_start = peek_idx;
                                while peek_idx < len && meta_bytes[peek_idx] != b' ' {
                                    peek_idx += 1;
                                }
                                let next_part = &meta_bytes[peek_start..peek_idx];
                                // If the next token is an ip (single or double/triple colon forms), do NOT treat it as appname
                                if !(next_part.starts_with(b"ip:")
                                    || next_part.starts_with(b"ip::"))
                                {
                                    meta.appname = to_cow(next_part);
                                    idx = peek_idx;
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
