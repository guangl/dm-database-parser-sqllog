# Phase 1: Measurement - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-19
**Phase:** 01-measurement
**Areas discussed:** 多行 SQL 语料库设计, baseline.json 标定策略, CI 门禁实现方式

---

## 多行 SQL 语料库设计

| Option | Description | Selected |
|--------|-------------|----------|
| 20% 多行（推荐） | 80% 单行 + 20% 多行，接近真实 DM 日志频率 | ✓ |
| 50% 多行 | 压力测试场景，偏离真实分布 | |

**User's choice:** 20% 多行

---

| Option | Description | Selected |
|--------|-------------|----------|
| 2–5 行（推荐） | 覆盖常见 JOIN/子查询场景 | ✓ |
| 10–20 行 | 大 SQL 场景（存储过程），较罕见 | |
| 随机 1–10 行 | 分布更真实，但 benchmark 方差更大 | |

**User's choice:** 2–5 行

---

| Option | Description | Selected |
|--------|-------------|----------|
| 5 MB 大小一致（推荐） | 与现有 benchmark 可对比 | ✓ |
| 10 MB 更大样本 | 降低方差，但运行更慢 | |

**User's choice:** 5 MB

---

## baseline.json 标定策略

| Option | Description | Selected |
|--------|-------------|----------|
| CI 环境重新标定（推荐） | 在 GitHub Actions 标定，避免 CI/本地环境不匹配 | ✓ |
| 保留本地基准，加大容差 | 将 5% 改为 20-30%，容忍环境差异 | |
| 双基准：CI + 本地各自维护 | 最准确但维护成本翻倍 | |

**User's choice:** CI 环境重新标定

---

| Option | Description | Selected |
|--------|-------------|----------|
| 手动触发更新 | 开发者确认优化有效后手动运行，可控 | ✓ |
| 合并到 main 时自动更新 | 无需人工操作，但可能被随机波动欺骗 | |

**User's choice:** 手动触发更新

---

## CI 门禁实现方式

| Option | Description | Selected |
|--------|-------------|----------|
| 自定义 shell 脚本（推荐） | 无外部依赖，逻辑透明 | ✓ |
| critcmp 工具 | 输出美观，需安装额外工具链 | |

**User's choice:** 自定义 shell 脚本

---

| Option | Description | Selected |
|--------|-------------|----------|
| mean（推荐） | criterion estimates.json 直接可读 | ✓ |
| median | 对尾部波动更鲁棒，但差异不大 | |

**User's choice:** mean

---

| Option | Description | Selected |
|--------|-------------|----------|
| 具体数字（推荐） | baseline 值、当前值、退化百分比全部输出 | ✓ |
| 仅退出状态码 1 | 简单，但需要查看 criterion HTML 报告 | |

**User's choice:** 具体数字

---

| Option | Description | Selected |
|--------|-------------|----------|
| 单独 update-baseline workflow（推荐） | 职责清晰，不干扰日常 CI | ✓ |
| 在现有 benchmark.yml 中加 step | 集中管理，但逻辑更复杂 | |

**User's choice:** 单独 update-baseline workflow

---

## Claude's Discretion

- criterion::Throughput API 用法（Bytes vs Elements）
- parse_performance_metrics() benchmark 变体的命名和语料库复用
- shell 脚本的具体实现（jq/python/awk）
- criterion estimates.json 的精确路径

## Deferred Ideas

None.
