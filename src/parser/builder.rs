use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::str;

use crate::error::ParseError;
use crate::parser::LogParser;
use crate::parser::encoding::FileEncodingHint;

/// 配置并构建 [`LogParser`] 的构建器模式 API。
pub struct LogParserBuilder {
    path: PathBuf,
    encoding_hint: Option<FileEncodingHint>,
}

impl LogParserBuilder {
    /// 创建一个新的 `LogParserBuilder`。
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            encoding_hint: None,
        }
    }

    /// 设置文件编码提示。
    pub fn encoding_hint(mut self, hint: FileEncodingHint) -> Self {
        self.encoding_hint = Some(hint);
        self
    }

    /// 构建并返回 [`LogParser`] 实例。
    ///
    /// 仅读取文件头尾各最多 64 KB / 4 KB 用于编码探测，不加载整个文件。
    pub fn build(self) -> Result<LogParser, ParseError> {
        let encoding = match self.encoding_hint {
            Some(hint) => hint,
            None => detect_encoding(&self.path)?,
        };
        Ok(LogParser { path: self.path, encoding })
    }
}

fn detect_encoding(path: &Path) -> Result<FileEncodingHint, ParseError> {
    let mut file = File::open(path).map_err(|e| ParseError::IoError(e.to_string()))?;

    let mut head_buf = Vec::with_capacity(64 * 1024);
    file.by_ref()
        .take(64 * 1024)
        .read_to_end(&mut head_buf)
        .map_err(|e| ParseError::IoError(e.to_string()))?;

    let head_ok = str::from_utf8(&head_buf).is_ok();

    let file_size = file
        .seek(SeekFrom::End(0))
        .map_err(|e| ParseError::IoError(e.to_string()))?;

    let tail_ok = if file_size > head_buf.len() as u64 {
        let tail_start = file_size.saturating_sub(4 * 1024).max(head_buf.len() as u64);
        file.seek(SeekFrom::Start(tail_start))
            .map_err(|e| ParseError::IoError(e.to_string()))?;
        let mut tail_buf = Vec::with_capacity(4 * 1024);
        file.read_to_end(&mut tail_buf)
            .map_err(|e| ParseError::IoError(e.to_string()))?;
        str::from_utf8(&tail_buf).is_ok()
    } else {
        true
    };

    Ok(if head_ok && tail_ok {
        FileEncodingHint::Utf8
    } else {
        FileEncodingHint::Gb18030
    })
}
