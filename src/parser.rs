use memchr::{memchr, memrchr};
use simdutf8::basic::from_utf8 as simd_from_utf8;
use smartstring::alias::String as SmartString;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::error::ParseError;
use crate::sqllog::{IndicatorsParts, MetaParts, Sqllog};
use crate::tools::is_ts_millis_bytes;

/// 从文件迭代解析 SQL 日志记录
///
/// # 参数
///
/// * `path` - 日志文件路径
///
/// # 返回
///
/// 返回一个迭代器，每次迭代返回 `Result<Sqllog, ParseError>`
pub fn iter_records_from_file<P: AsRef<Path>>(
    path: P,
) -> impl Iterator<Item = Result<Sqllog, ParseError>> {
    let path_buf = path.as_ref().to_path_buf();

    match File::open(&path_buf) {
        Ok(file) => {
            // Use a larger buffer for BufReader to reduce syscalls
            let reader = BufReader::with_capacity(256 * 1024, file);
            LogIterator {
                reader: Some(reader),
                current_buf: Vec::with_capacity(4096), // Pre-allocate for typical record size
                started: false,
                initial_error: None,
                eof: false,
                pending_verification: Vec::new(),
            }
        }
        Err(e) => LogIterator {
            reader: None,
            current_buf: Vec::new(),
            started: false,
            initial_error: Some(ParseError::IoError(e.to_string())),
            eof: true,
            pending_verification: Vec::new(),
        },
    }
}

pub struct LogIterator {
    reader: Option<BufReader<File>>,
    current_buf: Vec<u8>,
    started: bool,
    initial_error: Option<ParseError>,
    eof: bool,
    pending_verification: Vec<u8>,
}

impl Iterator for LogIterator {
    type Item = Result<Sqllog, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.initial_error.take() {
            return Some(Err(err));
        }

        if self.eof && self.current_buf.is_empty() && self.pending_verification.is_empty() {
            return None;
        }

        let reader = match self.reader.as_mut() {
            Some(r) => r,
            None => return None,
        };

        loop {
            // Handle pending verification from previous iteration
            if !self.pending_verification.is_empty() {
                let buf = match reader.fill_buf() {
                    Ok(b) => b,
                    Err(e) => return Some(Err(ParseError::IoError(e.to_string()))),
                };

                if buf.is_empty() {
                    // EOF with pending bytes
                    self.eof = true;
                    self.current_buf
                        .extend_from_slice(&self.pending_verification);
                    self.pending_verification.clear();
                    if !self.current_buf.is_empty() {
                        let res = parse_record_from_bytes(&self.current_buf);
                        self.current_buf.clear();
                        return Some(res);
                    }
                    return None;
                }

                // We need 24 bytes total to verify (1 byte '\n' is in pending[0])
                let needed_in_buf = 24usize.saturating_sub(self.pending_verification.len());

                if buf.len() >= needed_in_buf {
                    let mut ts_check_buf = [0u8; 23];
                    let pending_ts_part = &self.pending_verification[1..];
                    let pending_len = pending_ts_part.len();

                    ts_check_buf[0..pending_len].copy_from_slice(pending_ts_part);
                    ts_check_buf[pending_len..23].copy_from_slice(&buf[0..23 - pending_len]);

                    if is_ts_millis_bytes(&ts_check_buf) {
                        // Found boundary!
                        if !self.started {
                            self.started = true;
                            self.current_buf.clear();
                            self.current_buf
                                .extend_from_slice(&self.pending_verification[1..]);
                            self.pending_verification.clear();
                            // Continue to main loop to process buf
                        } else {
                            self.current_buf.push(self.pending_verification[0]);
                            let res = parse_record_from_bytes(&self.current_buf);
                            self.current_buf.clear();

                            self.current_buf
                                .extend_from_slice(&self.pending_verification[1..]);
                            self.pending_verification.clear();

                            return Some(res);
                        }
                    } else {
                        // Not a boundary
                        self.current_buf
                            .extend_from_slice(&self.pending_verification);
                        self.pending_verification.clear();
                    }
                } else {
                    // Not enough bytes in buf to verify.
                    self.pending_verification.extend_from_slice(buf);
                    let len = buf.len();
                    reader.consume(len);
                    continue;
                }
            }

            let mut consume_amount: Option<usize> = None;
            let mut result_to_return = None;

            {
                let buf = match reader.fill_buf() {
                    Ok(b) => b,
                    Err(e) => return Some(Err(ParseError::IoError(e.to_string()))),
                };

                if buf.is_empty() {
                    // EOF
                    self.eof = true;
                    if !self.current_buf.is_empty() {
                        let res = parse_record_from_bytes(&self.current_buf);
                        self.current_buf.clear();
                        return Some(res);
                    }
                    return None;
                }

                let buf_len = buf.len();
                let mut search_start = 0;
                let mut consumed_in_buf = 0;

                // If we haven't started, we need to find the first timestamp
                if !self.started && self.current_buf.is_empty() {
                    if buf_len >= 23 && is_ts_millis_bytes(&buf[0..23]) {
                        self.started = true;
                    }
                }

                while let Some(idx) = memchr(b'\n', &buf[search_start..]) {
                    let newline_pos = search_start + idx;

                    // Check if we have enough bytes to verify timestamp
                    if newline_pos + 23 < buf_len {
                        if is_ts_millis_bytes(&buf[newline_pos + 1..newline_pos + 24]) {
                            // Found boundary!

                            if !self.started {
                                self.started = true;
                                consumed_in_buf = newline_pos + 1;
                                search_start = consumed_in_buf;
                                self.current_buf.clear();
                                continue;
                            }

                            let chunk = &buf[consumed_in_buf..newline_pos + 1];

                            if self.current_buf.is_empty() {
                                result_to_return = Some(parse_record_from_bytes(chunk));
                            } else {
                                self.current_buf.extend_from_slice(chunk);
                                result_to_return = Some(parse_record_from_bytes(&self.current_buf));
                                self.current_buf.clear();
                            }

                            consumed_in_buf = newline_pos + 1;
                            consume_amount = Some(consumed_in_buf);
                            break;
                        } else {
                            // Not a boundary
                            search_start = newline_pos + 1;
                        }
                    } else {
                        // Not enough bytes to verify timestamp.
                        // We stop here.
                        let chunk = &buf[consumed_in_buf..newline_pos];
                        self.current_buf.extend_from_slice(chunk);

                        // Move ambiguous bytes to pending
                        self.pending_verification
                            .extend_from_slice(&buf[newline_pos..]);

                        consumed_in_buf = buf_len; // We consumed everything (some to current_buf, some to pending)
                        consume_amount = Some(buf_len);
                        break;
                    }
                }

                if consume_amount.is_none() {
                    // Loop finished naturally.
                    if self.started {
                        self.current_buf.extend_from_slice(&buf[consumed_in_buf..]);
                    }
                    consume_amount = Some(buf_len);
                }
            }

            let amount = consume_amount.unwrap();
            reader.consume(amount);

            if let Some(res) = result_to_return {
                return Some(res);
            }
        }
    }
}

fn parse_record_from_bytes(bytes: &[u8]) -> Result<Sqllog, ParseError> {
    parse_record(bytes)
}

pub fn parse_record(record_bytes: &[u8]) -> Result<Sqllog, ParseError> {
    // Find end of first line
    let (first_line, rest) = match memchr(b'\n', record_bytes) {
        Some(idx) => (&record_bytes[..idx], &record_bytes[idx + 1..]),
        None => (record_bytes, &[] as &[u8]),
    };

    // 1. Timestamp
    if first_line.len() < 23 {
        return Err(ParseError::InvalidFormat {
            raw: String::from_utf8_lossy(first_line).to_string(),
        });
    }
    // We assume ASCII/UTF-8 for timestamp
    let ts = match simd_from_utf8(&first_line[0..23]) {
        Ok(s) => SmartString::from(s),
        Err(_) => SmartString::from(String::from_utf8_lossy(&first_line[0..23])),
    };

    // 2. Meta
    // Format: TS (META) BODY
    // Find first '(' after TS
    let meta_start = match memchr(b'(', &first_line[23..]) {
        Some(idx) => 23 + idx,
        None => {
            return Err(ParseError::InvalidFormat {
                raw: String::from_utf8_lossy(first_line).to_string(),
            });
        }
    };

    // Find closing ')' for meta.
    // We search for ") " starting from meta_start.
    let meta_end = first_line[meta_start..]
        .windows(2)
        .position(|w| w == b") ")
        .map(|idx| meta_start + idx)
        .or_else(|| {
            // Fallback: find last ')'
            memrchr(b')', &first_line[meta_start..]).map(|idx| meta_start + idx)
        });

    let meta_end = match meta_end {
        Some(idx) => idx,
        None => {
            return Err(ParseError::InvalidFormat {
                raw: String::from_utf8_lossy(first_line).to_string(),
            });
        }
    };

    let meta_bytes = &first_line[meta_start + 1..meta_end];
    let meta = parse_meta(meta_bytes);

    // 3. Body & 4. Indicators
    let body_start_in_first_line = meta_end + 1;

    let mut first_line_body = if body_start_in_first_line < first_line.len() {
        &first_line[body_start_in_first_line..]
    } else {
        &[]
    };

    let start_idx = first_line_body
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(first_line_body.len());
    first_line_body = &first_line_body[start_idx..];

    let (clean_body, indicators) = if !rest.is_empty() {
        let (len, indicators) = parse_indicators_bytes(rest);
        let rest_body = &rest[..len];

        let mut s = String::with_capacity(first_line_body.len() + 1 + rest_body.len());

        match simd_from_utf8(first_line_body) {
            Ok(sub) => s.push_str(sub),
            Err(_) => s.push_str(&String::from_utf8_lossy(first_line_body)),
        }

        if !first_line_body.is_empty() {
            s.push('\n');
        }

        match simd_from_utf8(rest_body) {
            Ok(sub) => s.push_str(sub),
            Err(_) => s.push_str(&String::from_utf8_lossy(rest_body)),
        }

        (s, indicators)
    } else {
        let (len, indicators) = parse_indicators_bytes(first_line_body);
        let final_body = &first_line_body[..len];

        let s = match simd_from_utf8(final_body) {
            Ok(s) => s.to_string(),
            Err(_) => String::from_utf8_lossy(final_body).into_owned(),
        };
        (s, indicators)
    };

    Ok(Sqllog {
        ts,
        meta,
        body: clean_body,
        indicators,
    })
}

fn parse_meta(meta_bytes: &[u8]) -> MetaParts {
    let mut meta = MetaParts::default();

    let mut parts = meta_bytes
        .split(|b| b.is_ascii_whitespace())
        .filter(|p| !p.is_empty())
        .peekable();

    while let Some(part) = parts.next() {
        if part.starts_with(b"EP[") && part.ends_with(b"]") {
            // EP[0]
            let num_bytes = &part[3..part.len() - 1];
            if let Ok(s) = simd_from_utf8(num_bytes) {
                if let Ok(ep) = s.parse::<u8>() {
                    meta.ep = ep;
                }
            }
            continue;
        }

        if let Some(idx) = memchr(b':', part) {
            let key = &part[0..idx];
            let val = &part[idx + 1..];

            // Helper to convert bytes to SmartString using simdutf8
            let to_smart = |bytes: &[u8]| -> SmartString {
                match simd_from_utf8(bytes) {
                    Ok(s) => SmartString::from(s),
                    Err(_) => SmartString::from(String::from_utf8_lossy(bytes)),
                }
            };

            match key {
                b"sess" => meta.sess_id = to_smart(val),
                b"thrd" => meta.thrd_id = to_smart(val),
                b"user" => meta.username = to_smart(val),
                b"trxid" => meta.trxid = to_smart(val),
                b"stmt" => meta.statement = to_smart(val),
                b"appname" => {
                    // appname might be empty or take next token
                    if val.is_empty() {
                        // Check next token
                        if let Some(next_part) = parts.peek() {
                            if next_part.starts_with(b"ip:") && !next_part.starts_with(b"ip::") {
                                // Next part is ip key
                            } else {
                                // Next part is value for appname
                                meta.appname = to_smart(next_part);
                                parts.next(); // Consume it
                            }
                        }
                    } else {
                        meta.appname = to_smart(val);
                    }
                }
                b"ip" => {
                    meta.client_ip = to_smart(val);
                }
                _ => {
                    // Unknown key
                }
            }
        }
    }
    meta
}

fn trim_bytes(b: &[u8]) -> &[u8] {
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

fn parse_indicators_bytes(body: &[u8]) -> (usize, Option<IndicatorsParts>) {
    let mut indicators = IndicatorsParts::default();
    let mut has_indicators = false;
    let current_len = body.len();

    let search_limit = 256;
    let start_search = current_len.saturating_sub(search_limit);
    let search_slice = &body[start_search..current_len];

    let mut tail_len = search_slice.len();

    // Search for "EXEC_ID: "
    // We search backwards for ':' and check prefix
    let mut search_end = tail_len;
    while let Some(idx) = memrchr(b':', &search_slice[..search_end]) {
        if idx >= 7 && &search_slice[idx - 7..idx] == b"EXEC_ID" {
            // Found "EXEC_ID:"
            if idx + 1 < search_slice.len() && search_slice[idx + 1] == b' ' {
                let suffix = &search_slice[idx + 2..];
                // Find end of number.
                if let Some(dot_idx) = memchr(b'.', suffix) {
                    let val_bytes = &suffix[..dot_idx];
                    let val_trimmed = trim_bytes(val_bytes);
                    if let Ok(s) = simd_from_utf8(val_trimmed) {
                        if let Ok(id) = s.parse::<i64>() {
                            indicators.execute_id = id;
                            has_indicators = true;
                            tail_len = idx - 7; // Start of "EXEC_ID"
                        }
                    }
                }
                break;
            }
        }
        if idx == 0 {
            break;
        }
        search_end = idx;
    }

    let slice_view = &search_slice[..tail_len];

    // ROWCOUNT:
    search_end = slice_view.len();
    while let Some(idx) = memrchr(b':', &slice_view[..search_end]) {
        if idx >= 8 && &slice_view[idx - 8..idx] == b"ROWCOUNT" {
            if idx + 1 < slice_view.len() && slice_view[idx + 1] == b' ' {
                let suffix = &slice_view[idx + 2..];
                // Find "(rows)"
                if let Some(open_paren) = memchr(b'(', suffix) {
                    if suffix[open_paren..].starts_with(b"(rows)") {
                        let val_bytes = &suffix[..open_paren];
                        let val_trimmed = trim_bytes(val_bytes);
                        if let Ok(s) = simd_from_utf8(val_trimmed) {
                            if let Ok(count) = s.parse::<u32>() {
                                indicators.row_count = count;
                                has_indicators = true;
                                tail_len = idx - 8;
                            }
                        }
                    }
                }
                break;
            }
        }
        if idx == 0 {
            break;
        }
        search_end = idx;
    }

    let slice_view = &search_slice[..tail_len];

    // EXECTIME:
    search_end = slice_view.len();
    while let Some(idx) = memrchr(b':', &slice_view[..search_end]) {
        if idx >= 8 && &slice_view[idx - 8..idx] == b"EXECTIME" {
            if idx + 1 < slice_view.len() && slice_view[idx + 1] == b' ' {
                let suffix = &slice_view[idx + 2..];
                // Find "(ms)"
                if let Some(open_paren) = memchr(b'(', suffix) {
                    if suffix[open_paren..].starts_with(b"(ms)") {
                        let val_bytes = &suffix[..open_paren];
                        let val_trimmed = trim_bytes(val_bytes);
                        if let Ok(s) = simd_from_utf8(val_trimmed) {
                            if let Ok(time) = s.parse::<f32>() {
                                indicators.execute_time = time;
                                has_indicators = true;
                                tail_len = idx - 8;
                            }
                        }
                    }
                }
                break;
            }
        }
        if idx == 0 {
            break;
        }
        search_end = idx;
    }

    if has_indicators {
        let body_part = &search_slice[..tail_len];
        let trimmed_len = body_part
            .iter()
            .rposition(|&x| !x.is_ascii_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        (start_search + trimmed_len, Some(indicators))
    } else {
        (current_len, None)
    }
}
