/// 文件编码提示，用于指示日志文件的字符编码。
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
