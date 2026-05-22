use std::fs;
use std::path::{Path, PathBuf};
use std::str;

use crate::error::ParseError;
use crate::parser::encoding::FileEncodingHint;
use crate::parser::LogParser;

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
    pub fn build(self) -> Result<LogParser, ParseError> {
        let data = fs::read(&self.path)
            .map_err(|e| ParseError::IoError(e.to_string()))?;

        let encoding = match self.encoding_hint {
            Some(hint) => hint,
            None => {
                // 自动编码探测：采样头部 64KB 和尾部 4KB
                let head_size = data.len().min(64 * 1024);
                let head_ok = str::from_utf8(&data[..head_size]).is_ok();
                let tail_start = data.len().saturating_sub(4 * 1024).max(head_size);
                let tail_ok = tail_start >= data.len()
                    || str::from_utf8(&data[tail_start..]).is_ok();
                if head_ok && tail_ok {
                    FileEncodingHint::Utf8
                } else {
                    FileEncodingHint::Gb18030
                }
            }
        };

        Ok(LogParser { data, encoding })
    }
}
