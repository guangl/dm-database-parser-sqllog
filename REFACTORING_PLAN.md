# Parser 重构计划

## 当前问题

1. **字段硬编码**：所有 meta 字段（EP、sess、thrd等）都在代码中硬编码
2. **扩展性差**：如果 sqllog 格式添加新字段，需要修改多处代码
3. **结构不清晰**：record 的四个部分（ts、meta、body、end）没有明确的数据结构表示
4. **end 位置理解**：当前实现中 end 在第一行，但需求是"end 必定在最后一行"

## 新设计方案

### 1. 清晰的四部分结构

```rust
/// Record 的四个组成部分
pub struct RecordParts<'a> {
    pub ts: &'a str,        // 时间戳（首行，23字符）
    pub meta: &'a str,      // 元信息（首行，括号内）
    pub body: &'a str,      // SQL主体（可能多行）
    pub end: Option<&'a str>, // 指标信息（最后一行，可选）
}
```

### 2. 可配置的字段定义

```rust
/// Meta 字段定义（可扩展）
pub struct MetaFieldDef {
    pub name: &'static str,     // 字段名，如 "EP", "sess"
    pub required: bool,         // 是否必需
    pub has_brackets: bool,     // 是否带方括号，如 EP[xxx]
}

/// End 指标定义（可扩展）
pub struct EndMetricDef {
    pub keyword: &'static str,  // 关键字，如 "EXECTIME:"
    pub unit: Option<&'static str>, // 单位，如 "ms"
}
```

### 3. 动态字段解析

使用 `HashMap` 存储解析结果，而不是固定的结构体字段：

```rust
pub struct ParsedMeta<'a> {
    fields: HashMap<&'static str, &'a str>,
}

pub struct ParsedEnd {
    metrics: HashMap<&'static str, u64>,
}
```

### 4. 配置驱动的解析器

```rust
pub struct ParserConfig {
    meta_fields: Vec<MetaFieldDef>,
    end_metrics: Vec<EndMetricDef>,
    strict_mode: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self::dmdb_default() // DM 数据库默认配置
    }
}
```

## 迁移步骤

1. ✅ 创建新的数据结构定义
2. ⏳ 实现新的解析逻辑
3. ⏳ 保持向后兼容（旧 API 调用新实现）
4. ⏳ 更新所有测试
5. ⏳ 文档更新

## 关于 end 位置的说明

根据实际测试用例分析：
- **实际格式**：指标（EXECTIME等）在第一行 meta 之后
- **理想格式**：指标应该在最后一行
- **当前策略**：先在第一行查找指标（兼容现有测试），未来支持两种格式
