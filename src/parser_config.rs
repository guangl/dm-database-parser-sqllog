//! Parser 配置模块
//! 
//! 提供可扩展的字段定义和解析器配置，使得 sqllog 格式变化时只需更新配置，
//! 而不需要修改核心解析逻辑。

use std::collections::HashMap;

/// Meta 字段定义
/// 
/// 定义元信息中的一个字段，包括字段名、是否必需、特殊格式等
#[derive(Debug, Clone)]
pub struct MetaFieldDef {
    /// 字段名（用于解析时匹配），如 "EP", "sess", "user"
    pub name: &'static str,
    
    /// 是否为必需字段（如果缺失会导致解析错误）
    pub required: bool,
    
    /// 是否使用方括号格式，如 EP[xxx]
    pub has_brackets: bool,
    
    /// 是否使用冒号分隔符，如 sess:xxx
    pub has_colon: bool,
    
    /// 字段在元信息中的期望顺序（用于验证）
    pub order: usize,
}

/// End 指标定义
/// 
/// 定义 end 部分的一个指标，包括关键字、单位等
#[derive(Debug, Clone)]
pub struct EndMetricDef {
    /// 关键字，如 "EXECTIME", "ROWCOUNT"
    pub keyword: &'static str,
    
    /// 单位（可选），如 "ms"
    pub unit: Option<&'static str>,
    
    /// 数据类型
    pub value_type: MetricValueType,
}

/// 指标值类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricValueType {
    /// 无符号整数
    UnsignedInt,
    /// 浮点数
    Float,
    /// 字符串
    String,
}

/// Parser 配置
/// 
/// 定义解析器的行为和支持的字段
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Meta 字段定义列表
    pub meta_fields: Vec<MetaFieldDef>,
    
    /// End 指标定义列表
    pub end_metrics: Vec<EndMetricDef>,
    
    /// 严格模式：是否严格要求字段顺序
    pub strict_field_order: bool,
    
    /// 是否允许未知字段
    pub allow_unknown_fields: bool,
    
    /// 时间戳长度（字符数）
    pub timestamp_length: usize,
}

impl ParserConfig {
    /// DM 数据库的默认配置
    pub fn dmdb_default() -> Self {
        Self {
            meta_fields: vec![
                MetaFieldDef {
                    name: "EP",
                    required: true,
                    has_brackets: true,
                    has_colon: false,
                    order: 0,
                },
                MetaFieldDef {
                    name: "sess",
                    required: true,
                    has_brackets: false,
                    has_colon: true,
                    order: 1,
                },
                MetaFieldDef {
                    name: "thrd",
                    required: true,
                    has_brackets: false,
                    has_colon: true,
                    order: 2,
                },
                MetaFieldDef {
                    name: "user",
                    required: true,
                    has_brackets: false,
                    has_colon: true,
                    order: 3,
                },
                MetaFieldDef {
                    name: "trxid",
                    required: true,
                    has_brackets: false,
                    has_colon: true,
                    order: 4,
                },
                MetaFieldDef {
                    name: "stmt",
                    required: true,
                    has_brackets: false,
                    has_colon: true,
                    order: 5,
                },
                MetaFieldDef {
                    name: "appname",
                    required: true,
                    has_brackets: false,
                    has_colon: true,
                    order: 6,
                },
                MetaFieldDef {
                    name: "ip",
                    required: false,
                    has_brackets: false,
                    has_colon: true,
                    order: 7,
                },
            ],
            end_metrics: vec![
                EndMetricDef {
                    keyword: "EXECTIME",
                    unit: Some("ms"),
                    value_type: MetricValueType::UnsignedInt,
                },
                EndMetricDef {
                    keyword: "ROWCOUNT",
                    unit: None,
                    value_type: MetricValueType::UnsignedInt,
                },
                EndMetricDef {
                    keyword: "EXEC_ID",
                    unit: None,
                    value_type: MetricValueType::UnsignedInt,
                },
            ],
            strict_field_order: true,
            allow_unknown_fields: false,
            timestamp_length: 23,
        }
    }
    
    /// 创建字段名到定义的映射（用于快速查找）
    pub fn meta_field_map(&self) -> HashMap<&'static str, &MetaFieldDef> {
        self.meta_fields
            .iter()
            .map(|def| (def.name, def))
            .collect()
    }
    
    /// 创建指标关键字到定义的映射
    pub fn end_metric_map(&self) -> HashMap<&'static str, &EndMetricDef> {
        self.end_metrics
            .iter()
            .map(|def| (def.keyword, def))
            .collect()
    }
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self::dmdb_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ParserConfig::default();
        assert_eq!(config.meta_fields.len(), 8);
        assert_eq!(config.end_metrics.len(), 3);
        assert_eq!(config.timestamp_length, 23);
    }

    #[test]
    fn test_meta_field_map() {
        let config = ParserConfig::default();
        let map = config.meta_field_map();
        
        assert!(map.contains_key("EP"));
        assert!(map.contains_key("sess"));
        assert!(map.contains_key("user"));
        
        let ep_def = map.get("EP").unwrap();
        assert!(ep_def.has_brackets);
        assert!(!ep_def.has_colon);
    }

    #[test]
    fn test_end_metric_map() {
        let config = ParserConfig::default();
        let map = config.end_metric_map();
        
        assert!(map.contains_key("EXECTIME"));
        assert!(map.contains_key("ROWCOUNT"));
        
        let exectime_def = map.get("EXECTIME").unwrap();
        assert_eq!(exectime_def.unit, Some("ms"));
    }
}
