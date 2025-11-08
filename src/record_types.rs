//! Record 数据结构模块
//! 
//! 定义了表示 sqllog record 的各种数据结构，包括四部分结构和解析结果

use std::collections::HashMap;

/// Record 的四个组成部分（原始字符串切片）
/// 
/// 这是 record 的第一层解析结果，将原始文本分割为四个逻辑部分：
/// 1. **ts** - 时间戳（首行，固定 23 字符）
/// 2. **meta** - 元信息（首行，括号内）
/// 3. **body** - SQL 主体（可能多行）
/// 4. **end** - 指标信息（最后一行，可选）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordParts<'a> {
    /// 时间戳字符串，格式：YYYY-MM-DD HH:MM:SS.mmm
    pub ts: &'a str,
    
    /// 元信息原始字符串（括号内的全部内容）
    pub meta: &'a str,
    
    /// SQL 主体（可能为空，可能跨多行）
    pub body: &'a str,
    
    /// End 指标原始字符串（可能不存在）
    pub end: Option<&'a str>,
}

/// 解析后的 Meta 信息
/// 
/// 使用 HashMap 存储字段值，支持动态字段
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMeta<'a> {
    /// 字段名 -> 字段值的映射
    fields: HashMap<&'static str, &'a str>,
}

impl<'a> ParsedMeta<'a> {
    /// 创建空的 ParsedMeta
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }
    
    /// 插入字段
    pub fn insert(&mut self, name: &'static str, value: &'a str) {
        self.fields.insert(name, value);
    }
    
    /// 获取字段值
    pub fn get(&self, name: &str) -> Option<&'a str> {
        self.fields.get(name).copied()
    }
    
    /// 获取字段值（如果不存在返回空字符串）
    pub fn get_or_empty(&self, name: &str) -> &'a str {
        self.fields.get(name).copied().unwrap_or("")
    }
    
    /// 检查字段是否存在
    pub fn contains(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }
    
    /// 获取所有字段名
    pub fn field_names(&self) -> Vec<&'static str> {
        self.fields.keys().copied().collect()
    }
    
    /// 字段数量
    pub fn len(&self) -> usize {
        self.fields.len()
    }
    
    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl<'a> Default for ParsedMeta<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析后的 End 指标
/// 
/// 使用 HashMap 存储指标值，支持动态指标
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEnd {
    /// 指标名 -> 指标值的映射
    metrics: HashMap<&'static str, u64>,
}

impl ParsedEnd {
    /// 创建空的 ParsedEnd
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
        }
    }
    
    /// 插入指标
    pub fn insert(&mut self, name: &'static str, value: u64) {
        self.metrics.insert(name, value);
    }
    
    /// 获取指标值
    pub fn get(&self, name: &str) -> Option<u64> {
        self.metrics.get(name).copied()
    }
    
    /// 检查指标是否存在
    pub fn contains(&self, name: &str) -> bool {
        self.metrics.contains_key(name)
    }
    
    /// 获取所有指标名
    pub fn metric_names(&self) -> Vec<&'static str> {
        self.metrics.keys().copied().collect()
    }
    
    /// 指标数量
    pub fn len(&self) -> usize {
        self.metrics.len()
    }
    
    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }
}

impl Default for ParsedEnd {
    fn default() -> Self {
        Self::new()
    }
}

/// 完整的解析结果
/// 
/// 这是最终的解析结果，包含了所有结构化的信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRecord<'a> {
    /// 时间戳
    pub ts: &'a str,
    
    /// 解析后的元信息
    pub meta: ParsedMeta<'a>,
    
    /// SQL 主体
    pub body: &'a str,
    
    /// 解析后的指标（可选）
    pub end: Option<ParsedEnd>,
}

impl<'a> ParsedRecord<'a> {
    /// 从 RecordParts 创建（需要进一步解析 meta 和 end）
    pub fn from_parts(parts: RecordParts<'a>, meta: ParsedMeta<'a>, end: Option<ParsedEnd>) -> Self {
        Self {
            ts: parts.ts,
            meta,
            body: parts.body,
            end,
        }
    }
    
    /// 获取 meta 字段值（便捷方法）
    pub fn get_meta(&self, name: &str) -> Option<&'a str> {
        self.meta.get(name)
    }
    
    /// 获取 end 指标值（便捷方法）
    pub fn get_metric(&self, name: &str) -> Option<u64> {
        self.end.as_ref().and_then(|e| e.get(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_meta() {
        let mut meta = ParsedMeta::new();
        meta.insert("user", "admin");
        meta.insert("sess", "12345");
        
        assert_eq!(meta.get("user"), Some("admin"));
        assert_eq!(meta.get("sess"), Some("12345"));
        assert_eq!(meta.get("missing"), None);
        assert_eq!(meta.get_or_empty("missing"), "");
        assert_eq!(meta.len(), 2);
    }

    #[test]
    fn test_parsed_end() {
        let mut end = ParsedEnd::new();
        end.insert("EXECTIME", 100);
        end.insert("ROWCOUNT", 5);
        
        assert_eq!(end.get("EXECTIME"), Some(100));
        assert_eq!(end.get("ROWCOUNT"), Some(5));
        assert_eq!(end.get("missing"), None);
        assert_eq!(end.len(), 2);
    }

    #[test]
    fn test_parsed_record() {
        let parts = RecordParts {
            ts: "2025-08-12 10:57:09.562",
            meta: "EP[0] sess:1 user:admin",
            body: "SELECT 1",
            end: Some("EXECTIME: 10ms"),
        };
        
        let mut meta = ParsedMeta::new();
        meta.insert("EP", "0");
        meta.insert("sess", "1");
        meta.insert("user", "admin");
        
        let mut end = ParsedEnd::new();
        end.insert("EXECTIME", 10);
        
        let record = ParsedRecord::from_parts(parts, meta, Some(end));
        
        assert_eq!(record.ts, "2025-08-12 10:57:09.562");
        assert_eq!(record.get_meta("user"), Some("admin"));
        assert_eq!(record.get_metric("EXECTIME"), Some(10));
        assert_eq!(record.body, "SELECT 1");
    }
}
