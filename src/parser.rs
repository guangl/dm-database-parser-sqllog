use memchr::memmem::Finder;
use memchr::{memchr, memrchr};
#[cfg(unix)]
use memmap2::Advice;
use memmap2::Mmap;
use simdutf8::basic::from_utf8 as simd_from_utf8;
use std::borrow::Cow;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::error::ParseError;
use crate::sqllog::Sqllog;
use encoding::all::GB18030;
use encoding::{DecoderTrap, Encoding};

/// Pre-built SIMD searcher for the `") "` meta-close pattern.
/// Avoids rebuilding the Finder on every record parse.
static FINDER_CLOSE_META: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b") "));

/// Pre-built SIMD searcher for the `"\n20"` record-start pattern.
/// Shared across threads via LazyLock; constructed once on first use.
static FINDER_RECORD_START: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"\n20"));

/// 文件编码提示，用于指示日志文件的字符编码。
///
/// 传递给 [`LogParserBuilder::encoding_hint`] 以跳过自动编码探测。
/// 如果未指定，构建器会自动对文件首尾采样以确定编码。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FileEncodingHint {
    /// 自动探测编码（默认行为）
    #[default]
    Auto,
    /// 文件使用 UTF-8 编码
    Utf8,
    /// 文件使用 GB18030 编码
    Gb18030,
}

/// SQL 日志文件解析器，提供对达梦数据库 SQL 日志的内存映射和迭代解析。
///
/// 通过 [`LogParserBuilder`] 构建实例。内部使用 `memmap2` 进行内存映射，
/// 自动检测文件编码（UTF-8 或 GB18030）。
///
/// 解析操作是惰性的——记录在调用 [`iter`](LogParser::iter()) 或
/// [`par_iter`](LogParser::par_iter()) 时逐条解析，不会预先加载所有数据到内存。
pub struct LogParser {
    mmap: Mmap,
    encoding: FileEncodingHint,
    parallel_threshold: usize,
}

/// 配置并构建 [`LogParser`] 的构建器模式 API。
///
/// 提供链式调用的方式设置解析器参数，然后通过
/// [`build`](LogParserBuilder::build()) 方法创建最终的 `LogParser` 实例。
///
/// # Example
/// ```rust,no_run
/// # use dm_database_parser_sqllog::LogParserBuilder;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let parser = LogParserBuilder::new("sqllog.txt")
///     .threads(4)
///     .parallel_threshold(64 * 1024 * 1024)
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct LogParserBuilder {
    path: PathBuf,
    threads: Option<usize>,
    parallel_threshold: Option<usize>,
    encoding_hint: Option<FileEncodingHint>,
}

impl LogParserBuilder {
    /// 创建一个新的 `LogParserBuilder`。
    ///
    /// `path` 是要解析的 SQL 日志文件的路径。
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            threads: None,
            parallel_threshold: None,
            encoding_hint: None,
        }
    }

    /// 设置并行扫描的线程数。
    ///
    /// 会尝试设置 Rayon 全局线程池。由于全局线程池只能设置一次，
    /// 多次调用此方法时只有第一次实际生效。
    pub fn threads(mut self, n: usize) -> Self {
        self.threads = Some(n);
        self
    }

    /// 设置并行扫描的阈值（字节数）。
    ///
    /// 文件小于此阈值时将自动退化到单线程扫描，避免并行调度开销。
    /// 默认值为 32 MB。
    pub fn parallel_threshold(mut self, threshold: usize) -> Self {
        self.parallel_threshold = Some(threshold);
        self
    }

    /// 设置文件编码提示。
    ///
    /// 如果指定了编码，构建器将跳过自动编码探测直接使用指定编码。
    /// 如果未指定（默认），构建器会对文件首尾进行采样以自动检测编码。
    pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
        self.encoding_hint = Some(hint);
        self
    }

    /// 构建并返回 [`LogParser`] 实例。
    ///
    /// 执行内存映射和编码探测。如果发生 I/O 错误则返回 [`ParseError::IoError`]。
    pub fn build(self) -> Result<LogParser, ParseError> {
        // 如果并行线程数已配置，尝试设置 rayon 全局线程池
        // build_global 只能调用一次；如已配置则静默忽略。
        if let Some(n) = self.threads {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build_global();
        }

        let file = File::open(&self.path).map_err(|e| ParseError::IoError(e.to_string()))?;
        let mmap = unsafe { Mmap::map(&file).map_err(|e| ParseError::IoError(e.to_string()))? };

        #[cfg(unix)]
        let _ = mmap.advise(Advice::Sequential);

        // 根据 encoding_hint 确定编码
        // Some(hint) 时跳过自动探测直接使用指定编码
        // None 时执行与 from_path 相同的 head+tail 采样
        let encoding = match self.encoding_hint {
            Some(hint) => hint,
            None => {
                let head_size = mmap.len().min(64 * 1024);
                let tail_start = mmap.len().saturating_sub(4 * 1024).max(head_size);
                let head_ok = simd_from_utf8(&mmap[..head_size]).is_ok();
                let tail_ok =
                    tail_start >= mmap.len() || simd_from_utf8(&mmap[tail_start..]).is_ok();
                if head_ok && tail_ok {
                    FileEncodingHint::Utf8
                } else {
                    FileEncodingHint::Gb18030
                }
            }
        };

        let parallel_threshold = self.parallel_threshold.unwrap_or(32 * 1024 * 1024);

        Ok(LogParser {
            mmap,
            encoding,
            parallel_threshold,
        })
    }
}
/// 每个元素是某条记录在内存映射缓冲区内的绝对字节偏移。
/// 用于两阶段并行扫描：先建索引，再按记录数均匀分区给多线程。
pub struct RecordIndex {
    pub(crate) offsets: Vec<usize>,
}

impl RecordIndex {
    /// 记录总数
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// 是否为空（文件不含任何完整记录）
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }
}

impl LogParser {
    /// 返回顺序迭代器，逐条解析 SQL 日志记录。
    ///
    /// 每次调用返回新的 [`LogIterator`]，其 Item 为
    /// `Result<Sqllog<'a>, ParseError>`。迭代器内部使用 `memchr` 快速定位
    /// 记录边界，支持单行和多行记录。
    ///
    /// 如果需要跳过格式错误的记录，可链式调用
    /// [`skip_errors`](LogIterator::skip_errors)。
    pub fn iter(&self) -> LogIterator<'_> {
        LogIterator {
            data: &self.mmap,
            pos: 0,
            encoding: self.encoding,
            line_number: 1,
        }
    }

    /// 两阶段扫描第一阶段：构建记录起始字节偏移索引。
    /// 单线程扫描整个文件，返回的 `RecordIndex` 可直接用于并行处理阶段。
    pub fn index(&self) -> RecordIndex {
        let data: &[u8] = &self.mmap;
        let mut offsets: Vec<usize> = Vec::new();

        // 第 0 条记录：仅当文件首字节即是时间戳时才单独 push
        // （find_next_record_start 会先跳过首行，所以首行就是时间戳的情况需要单独处理）
        if data.len() >= 23 && is_timestamp_start(&data[0..23]) {
            offsets.push(0);
        }

        let mut pos: usize = 0;
        loop {
            let next = find_next_record_start(data, pos);
            if next >= data.len() {
                break;
            }
            // 防止与首条记录重复 push（首字节即是时间戳的边界情况）
            if offsets.last() != Some(&next) {
                offsets.push(next);
            }
            // Pitfall 1: pos 必须前进至少 1，否则 find_next_record_start
            // 在首行就是时间戳时会返回同一个 next，无限循环
            pos = next.saturating_add(1);
        }
        RecordIndex { offsets }
    }

    /// 返回 Rayon 并行迭代器，遍历所有 SQL 日志记录。
    ///
    /// 大文件（文件大小 ≥ 并行阈值，默认 32 MB）按记录边界分成 N 个字节对齐块，
    /// 产生 O(threads) 开销而非 O(records)。小文件仅使用单个分区，
    /// 因此 Rayon 以单线程执行，避免调度开销（PAR-03 语义）。
    ///
    /// 此方法内部不调用 `index()`：在内存映射的 I/O 密集型工作负载上，
    /// 完整顺序预扫描会使 I/O 加倍。
    pub fn par_iter(
        &self,
    ) -> impl rayon::iter::ParallelIterator<Item = Result<Sqllog<'_>, ParseError>> + '_ {
        use rayon::prelude::*;

        let par_threshold = self.parallel_threshold;

        let data: &[u8] = &self.mmap;
        let encoding = self.encoding;

        let bounds: Vec<(usize, usize)> = if data.is_empty() {
            Vec::new()
        } else if data.len() < par_threshold {
            vec![(0, data.len())]
        } else {
            let num_threads = rayon::current_num_threads().max(1);
            let chunk_size = (data.len() / num_threads).max(1);
            let mut starts: Vec<usize> = vec![0];
            for i in 1..num_threads {
                let boundary = find_next_record_start(data, i * chunk_size);
                if boundary < data.len() {
                    starts.push(boundary);
                }
            }
            starts.push(data.len());
            starts.dedup();
            starts.windows(2).map(|w| (w[0], w[1])).collect()
        };

        bounds
            .into_par_iter()
            .flat_map_iter(move |(start, end)| LogIterator {
                data: &data[start..end],
                pos: 0,
                encoding,
                line_number: 0,
            })
    }
}

/// SQL 日志记录的顺序迭代器。
///
/// 由 [`LogParser::iter()`] 返回。每次调用 `next()` 解析一条记录，
/// 返回 `Option<Result<Sqllog<'a>, ParseError>>`。
///
/// 支持通过以下方法链式处理：
/// - [`skip_errors`](LogIterator::skip_errors) — 跳过解析错误的记录
/// - [`filter_by_exec_time`](LogIterator::filter_by_exec_time) — 按执行时间过滤
/// - [`filter_by_sql_contains`](LogIterator::filter_by_sql_contains) — 按 SQL 内容过滤
pub struct LogIterator<'a> {
    data: &'a [u8],
    pos: usize,
    encoding: FileEncodingHint,
    /// 当前文件的绝对行号。迭代器追踪遇到的每一个 '\n' 字节并累加。
    /// 从 1 开始计数（文件首行为第 1 行）。
    /// 注意：par_iter() 分区的 LogIterator 中此值为 0（分区扫描无法维护全局行号）。
    line_number: u64,
}

impl<'a> LogIterator<'a> {
    /// 返回一个跳过解析错误的迭代器。
    ///
    /// 调用 [`iter()`](LogParser::iter()) 返回的迭代器会产生 `Result<Sqllog, ParseError>`。
    /// 如果只关心成功解析的记录而希望忽略格式错误的行，可以使用 `skip_errors()`。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use dm_database_parser_sqllog::LogParserBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let parser = LogParserBuilder::new("sqllog.txt").build()?;
    /// for sqllog in parser.iter().skip_errors() {
    ///     println!("SQL: {}", sqllog.body());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # 注意
    ///
    /// - `par_iter()` 不支持此方法（它返回 Rayon 的 `ParallelIterator`，不是 `LogIterator`）。
    /// - `skip_errors` 不改变内部行号行为。如果需要在调试时查看错误上下文，请使用原生的
    ///   [`iter()`](LogParser::iter()) 遍历 `Result` 并检查错误信息。
    pub fn skip_errors(self) -> impl Iterator<Item = Sqllog<'a>> + 'a {
        self.filter_map(Result::ok)
    }

    /// 过滤出执行时间大于等于 `min_ms` 毫秒的记录。
    ///
    /// 使用 `parse_performance_metrics()` 内部解析逻辑，不重复实现。
    /// 不包含 EXECTIME 字段的记录和解析错误的记录将被过滤掉。
    ///
    /// # Example
    /// ```rust,no_run
    /// # use dm_database_parser_sqllog::LogParserBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let parser = LogParserBuilder::new("sqllog.txt").build()?;
    /// for record in parser.iter().filter_by_exec_time(100) {
    ///     // record: Result<Sqllog, ParseError>
    ///     println!("{}", record?.body());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn filter_by_exec_time(
        self,
        min_ms: u64,
    ) -> impl Iterator<Item = Result<Sqllog<'a>, ParseError>> + 'a {
        self.filter(move |item| match item {
            Ok(sqllog) => {
                let metrics = sqllog.parse_performance_metrics();
                metrics.exectime >= min_ms as f32
            }
            Err(_) => false,
        })
    }

    /// 过滤出 SQL 语句体包含指定 `pattern` 的记录。
    ///
    /// 使用 `body()` 方法获取 SQL 体并进行字符串包含检查。
    /// 解析错误的记录将被过滤掉。pattern 匹配区分大小写。
    ///
    /// # Example
    /// ```rust,no_run
    /// # use dm_database_parser_sqllog::LogParserBuilder;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let parser = LogParserBuilder::new("sqllog.txt").build()?;
    /// for record in parser.iter().filter_by_sql_contains("SELECT") {
    ///     println!("{}", record?.body());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn filter_by_sql_contains(
        self,
        pattern: &str,
    ) -> impl Iterator<Item = Result<Sqllog<'a>, ParseError>> + 'a {
        let pattern = pattern.to_string();
        self.filter(move |item| match item {
            Ok(sqllog) => sqllog.body().contains(&pattern),
            Err(_) => false,
        })
    }
}

impl<'a> Iterator for LogIterator<'a> {
    type Item = Result<Sqllog<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.pos >= self.data.len() {
                return None;
            }

            let data = &self.data[self.pos..];
            let current_line = self.line_number;

            // 快速路径：先用 memchr 找第一个 '\n'，若下一行即是时间戳则为单行记录
            // 慢速路径（多行）：用 FINDER_RECORD_START.find_iter 跳过嵌入换行
            let (record_end, next_start, is_multiline) = match memchr(b'\n', data) {
                None => (data.len(), data.len(), false),
                Some(first_nl) => {
                    let ts_start = first_nl + 1;
                    if ts_start + 23 <= data.len()
                        && is_timestamp_start(&data[ts_start..ts_start + 23])
                    {
                        // 单行记录：边界就是第一个 '\n'
                        (first_nl, ts_start, false)
                    } else {
                        // 多行记录：用 memmem 跳过嵌入换行继续搜索
                        // ALGO-01: find_iter 替代逐行 while-memchr 循环
                        let mut found_boundary: Option<usize> = None;
                        for candidate in FINDER_RECORD_START.find_iter(&data[ts_start..]) {
                            let abs_ts = ts_start + candidate + 1;
                            if abs_ts + 23 <= data.len()
                                && is_timestamp_start(&data[abs_ts..abs_ts + 23])
                            {
                                found_boundary = Some(ts_start + candidate);
                                break;
                            }
                        }
                        match found_boundary {
                            Some(idx) => (idx, idx + 1, true),
                            None => (data.len(), data.len(), true),
                        }
                    }
                }
            };

            let record_slice = &data[..record_end];
            self.pos += next_start;

            // 更新行号：统计本次迭代消耗的字节中所有 '\n' 的数量
            // 必须在空记录 continue 之前执行，这样空记录跳过时 line_number 也被正确更新
            self.line_number += data[..next_start].iter().filter(|&&b| b == b'\n').count() as u64;

            // Trim trailing CR if present
            let record_slice = if record_slice.ends_with(b"\r") {
                &record_slice[..record_slice.len() - 1]
            } else {
                record_slice
            };

            // Skip empty slices iteratively instead of recursing to avoid stack overflow
            // when the file contains many consecutive blank lines.
            if record_slice.is_empty() {
                continue;
            }

            return Some(parse_record_with_hint(
                record_slice,
                is_multiline,
                self.encoding,
                current_line,
            ));
        }
    }
}

/// Find the position of the next record start at or after `from`.
/// A record start is a line beginning with a timestamp pattern.
fn find_next_record_start(data: &[u8], from: usize) -> usize {
    let mut pos = from;
    // Skip to start of next line
    if let Some(nl) = memchr(b'\n', &data[pos..]) {
        pos += nl + 1;
    } else {
        return data.len();
    }
    // 先检查 pos 本身是否是时间戳行（Finder 不会命中无前置 '\n' 的行首）
    if pos + 23 <= data.len() && is_timestamp_start(&data[pos..pos + 23]) {
        return pos;
    }

    // ALGO-01: memmem 单次扫描替代逐行 memchr loop
    for candidate in FINDER_RECORD_START.find_iter(&data[pos..]) {
        let ts_start = pos + candidate + 1;
        if ts_start + 23 <= data.len() && is_timestamp_start(&data[ts_start..ts_start + 23]) {
            return ts_start;
        }
    }
    data.len()
}

/// 从原始字节解析单条 SQL 日志记录。
///
/// 自动检测多行模式（包含嵌入换行的记录）。此函数是公开的独立解析入口，
/// 适合已从文件中读出完整记录的调用方。
///
/// 内部调用 `parse_record_with_hint` 进行实际解析。
/// [`LogIterator::next()`] 内部也调用此函数。
///
/// 对于完整的内存映射文件解析，推荐使用 [`LogParserBuilder`] 和
/// [`LogParser::iter()`] 获得更好的性能和行号追踪。
pub fn parse_record<'a>(record_bytes: &'a [u8]) -> Result<Sqllog<'a>, ParseError> {
    // Auto-detect multiline: inspect whether the bytes actually contain a newline
    // rather than hardcoding true, which caused a redundant memchr scan for
    // single-line records and was semantically misleading.
    let is_multiline = memchr(b'\n', record_bytes).is_some();
    parse_record_with_hint(record_bytes, is_multiline, FileEncodingHint::Auto, 0)
}

fn parse_record_with_hint<'a>(
    record_bytes: &'a [u8],
    is_multiline: bool,
    encoding_hint: FileEncodingHint,
    line_number: u64,
) -> Result<Sqllog<'a>, ParseError> {
    // Find end of first line
    let (first_line, _rest) = if is_multiline {
        match memchr(b'\n', record_bytes) {
            Some(idx) => {
                let mut line = &record_bytes[..idx];
                if line.ends_with(b"\r") {
                    line = &line[..line.len() - 1];
                }
                (line, &record_bytes[idx + 1..])
            }
            None => {
                let mut line = record_bytes;
                if line.ends_with(b"\r") {
                    line = &line[..line.len() - 1];
                }
                (line, &[] as &[u8])
            }
        }
    } else {
        let mut line = record_bytes;
        if line.ends_with(b"\r") {
            line = &line[..line.len() - 1];
        }
        (line, &[] as &[u8])
    };

    // 1. Timestamp
    if first_line.len() < 23 {
        return Err(make_invalid_format_error(first_line, line_number));
    }
    // Use safe conversion — validates all 23 bytes are valid UTF-8.
    // The performance cost of from_utf8 on a 23-byte string is negligible
    // and removes the soundness dependency on upstream validation.
    let ts = match std::str::from_utf8(&first_line[0..23]) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => return Err(make_invalid_format_error(first_line, line_number)),
    };

    // 2. Meta
    // Format: TS (META) BODY
    // Find first '(' after TS
    let meta_start = match memchr(b'(', &first_line[23..]) {
        Some(idx) => 23 + idx,
        None => {
            return Err(make_invalid_format_error(first_line, line_number));
        }
    };

    // Find closing ')' for meta using pre-built SIMD Finder.
    let meta_end = match FINDER_CLOSE_META.find(&first_line[meta_start..]) {
        Some(idx) => Some(meta_start + idx),
        None => memrchr(b')', &first_line[meta_start..]).map(|idx| meta_start + idx),
    };

    let meta_end = match meta_end {
        Some(idx) => idx,
        None => {
            return Err(make_invalid_format_error(first_line, line_number));
        }
    };

    let meta_bytes = &first_line[meta_start + 1..meta_end];
    // Lazy parsing: store raw bytes as a Cow<'a, str>.
    // For Utf8 / Auto-UTF8 encoding: meta_bytes is a sub-slice of the memory-mapped buffer
    // (raw UTF-8 bytes) that lives for 'a — borrowing is sound.
    // For Gb18030 / Auto-GB18030 encoding: GB18030.decode() produces a new owned String, so
    // meta_raw becomes Cow::Owned; the 'a lifetime is NOT extended to that allocation.
    let meta_raw = match encoding_hint {
        FileEncodingHint::Utf8 => {
            // File already validated as UTF-8 during `from_path`; skip per-slice re-validation.
            // SAFETY: meta_bytes is a sub-slice of record_bytes which lives for 'a.
            // No lifetime extension via from_raw_parts needed — meta_bytes already carries 'a.
            unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(meta_bytes)) }
        }
        FileEncodingHint::Gb18030 => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
            Ok(s) => Cow::Owned(s),
            Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
        },
        FileEncodingHint::Auto => match simd_from_utf8(meta_bytes) {
            Ok(_) => {
                // SAFETY: meta_bytes is a sub-slice of record_bytes which lives for 'a;
                // simd_from_utf8 confirmed it is valid UTF-8.
                unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(meta_bytes)) }
            }
            Err(_) => match GB18030.decode(meta_bytes, DecoderTrap::Strict) {
                Ok(s) => Cow::Owned(s),
                Err(_) => Cow::Owned(String::from_utf8_lossy(meta_bytes).into_owned()),
            },
        },
    };

    // 3. Body & 4. Indicators
    let body_start_in_first_line = meta_end + 1;

    // The ") " pattern guarantees one space; skip it directly.
    let content_start = if body_start_in_first_line < first_line.len()
        && first_line[body_start_in_first_line] == b' '
    {
        body_start_in_first_line + 1
    } else {
        body_start_in_first_line
    };

    // Extract optional leading tag like [SEL] or [ORA]
    let mut tag: Option<Cow<'a, str>> = None;
    let content_slice = if content_start < record_bytes.len() {
        let mut s = &record_bytes[content_start..];
        // If it starts with '[', try to find matching ']' and treat inner token as tag
        if !s.is_empty()
            && s[0] == b'['
            && let Some(end_idx) = memchr(b']', s)
            && end_idx >= 1
        {
            let inner = &s[1..end_idx];
            // Accept token without spaces and reasonable length
            if !inner.contains(&b' ') && inner.len() <= 32 {
                tag = match encoding_hint {
                    FileEncodingHint::Utf8 => {
                        // File already validated as UTF-8; skip re-validation.
                        // SAFETY: inner is a sub-slice of record_bytes which lives for 'a.
                        // No from_raw_parts needed — inner already carries 'a lifetime.
                        Some(unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(inner)) })
                    }
                    _ => match simd_from_utf8(inner) {
                        Ok(_) => Some(unsafe {
                            // SAFETY: inner is a sub-slice of record_bytes which lives for 'a;
                            // simd_from_utf8 confirmed it is valid UTF-8.
                            Cow::Borrowed(std::str::from_utf8_unchecked(inner))
                        }),
                        Err(_) => match encoding_hint {
                            FileEncodingHint::Gb18030 => {
                                match GB18030.decode(inner, DecoderTrap::Strict) {
                                    Ok(s) => Some(Cow::Owned(s)),
                                    Err(_) => Some(Cow::Owned(
                                        String::from_utf8_lossy(inner).into_owned(),
                                    )),
                                }
                            }
                            _ => Some(Cow::Owned(String::from_utf8_lossy(inner).into_owned())),
                        },
                    },
                };
                // Move past the closing ']' and any following ASCII whitespace
                s = &s[end_idx + 1..];
                let mut skip = 0usize;
                while skip < s.len() && s[skip].is_ascii_whitespace() {
                    skip += 1;
                }
                s = &s[skip..];
            }
        }
        s
    } else {
        &[] as &[u8]
    };

    let content_raw = Cow::Borrowed(content_slice);

    Ok(Sqllog {
        ts,
        meta_raw,
        content_raw,
        tag,
        encoding: encoding_hint,
    })
}

// u64 掩码常量：验证时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"
// 字节位置：0('2'), 1('0'), 4('-'), 7('-'), 10(' '), 13(':'), 16(':'), 19('.')
const LO_MASK: u64 = 0xFF0000FF0000FFFF; // data[0..8]：位置 0,1,4,7
const LO_EXPECTED: u64 = 0x2D00002D00003032; // LE: '2'=0x32,'0'=0x30,'-'=0x2D,'-'=0x2D
const HI_MASK: u64 = 0x0000FF0000FF0000; // data[8..16]：位置 10,13（偏移 2,5）
const HI_EXPECTED: u64 = 0x00003A0000200000; // LE: ' '=0x20,':'=0x3A

/// 检查 bytes[0..23] 是否符合时间戳格式 "20YY-MM-DD HH:MM:SS.mmm"。
/// 调用前需确保 bytes.len() >= 23（由调用方做长度检查）。
#[inline(always)]
fn is_timestamp_start(bytes: &[u8]) -> bool {
    debug_assert!(bytes.len() >= 23);
    let lo = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let hi = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    // 位置 16(':') 和 19('.') 用两次单字节比较（比第三次 u64 load 更清晰）
    (lo & LO_MASK == LO_EXPECTED)
        && (hi & HI_MASK == HI_EXPECTED)
        && bytes[16] == b':'
        && bytes[19] == b'.'
}

/// 将原始字节转换为 InvalidFormat 错误（错误路径，标注 cold 避免影响热路径代码布局）
#[cold]
fn make_invalid_format_error(raw_bytes: &[u8], line_number: u64) -> ParseError {
    ParseError::InvalidFormat {
        raw: String::from_utf8_lossy(raw_bytes).to_string(),
        line_number,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_timestamp_start_valid() {
        let ts = b"2025-11-17 16:09:41.123";
        assert!(is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_year_prefix() {
        let ts = b"1025-11-17 16:09:41.123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_month_separator() {
        let ts = b"2025X11-17 16:09:41.123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_second_separator() {
        let ts = b"2025-11-17 16:09X41.123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_wrong_millis_separator() {
        let ts = b"2025-11-17 16:09:41X123";
        assert!(!is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_exactly_23_bytes() {
        let ts = b"2025-11-17 16:09:41.123";
        assert_eq!(ts.len(), 23);
        assert!(is_timestamp_start(ts));
    }

    #[test]
    fn test_is_timestamp_start_trailing_garbage() {
        let ts = b"2025-11-17 16:09:41.123extra_garbage_here";
        assert!(is_timestamp_start(ts));
    }

    #[cfg(not(miri))]
    #[test]
    fn test_builder_encoding_hint_utf8() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(
            tmp,
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1"
        )
        .unwrap();
        tmp.as_file().sync_all().unwrap();

        // 显式指定 UTF-8 编码
        let parser = LogParserBuilder::new(tmp.path())
            .encoding_hint(FileEncodingHint::Utf8)
            .build()
            .expect("build");
        let record = parser.iter().next().unwrap().unwrap();
        assert_eq!(record.ts, "2025-11-17 16:09:41.123");
        assert!(record.body().contains("SELECT 1"));
    }

    #[cfg(not(miri))]
    #[test]
    fn test_builder_threads_and_threshold() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("tmp");
        writeln!(
            tmp,
            "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:u trxid:3 stmt:4 appname:a) SELECT 1"
        )
        .unwrap();
        tmp.as_file().sync_all().unwrap();

        // 配置 threads 和 parallel_threshold
        let parser = LogParserBuilder::new(tmp.path())
            .threads(2)
            .parallel_threshold(1)
            .build()
            .expect("build");
        let count = parser.iter().filter_map(|r| r.ok()).count();
        assert_eq!(count, 1);
    }

    #[cfg(not(miri))]
    #[test]
    fn test_builder_file_not_found() {
        let result = LogParserBuilder::new("/nonexistent/path.log").build();
        assert!(result.is_err());
        match result {
            Err(ParseError::IoError(_)) => {}
            _ => panic!("Expected IoError on nonexistent file"),
        }
    }
}
