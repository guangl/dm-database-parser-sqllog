# Phase 1: Measurement - Context

**Gathered:** 2026-04-19
**Status:** Ready for planning

<domain>
## Phase Boundary

建立可信的基准测量基础设施：让开发者可以用真实语料库衡量任意代码改动对吞吐量的影响，并在 CI 中自动捕获退化。

**不在本 Phase 范围内：** 任何性能优化（优化在 Phase 2-5 进行）。

</domain>

<decisions>
## Implementation Decisions

### 多行 SQL 语料库（MEAS-03）

- **D-01:** 合成语料库中 **20% 为多行 SQL 记录，80% 为单行记录**，接近真实 DM 日志中 DDL/复杂查询的出现频率
- **D-02:** 多行 SQL 记录中，SQL body 包含 **2–5 行换行**，覆盖常见 JOIN/子查询场景
- **D-03:** 语料库总大小 **5 MB**，与现有 `parse_sqllog_file_5mb` benchmark 保持一致，方便对比
- **D-04:** 新增独立 benchmark 变体，命名为 `parse_sqllog_multiline_5mb`，与现有单行 benchmark 并列；现有 `parse_sqllog_file_5mb` 保留不变

### CI Baseline 标定（MEAS-04）

- **D-05:** **在 CI 环境（GitHub Actions ubuntu-latest）重新标定 baseline**，替换现有本地测量值（674,425 ns），避免 CI 无 AVX2 导致的环境不匹配
- **D-06:** Baseline 更新策略：**手动触发**（`workflow_dispatch`），开发者在确认优化有效后手动运行，不自动覆盖
- **D-07:** 单独创建 `update-baseline.yml` workflow 负责重新标定并提交新 `baseline.json`，不与 `benchmark.yml` 合并

### CI 回归门禁（MEAS-04）

- **D-08:** 门禁实现方式：**自定义 shell 脚本**，读取 criterion 输出的 `estimates.json`，与 `baseline.json` 对比，无需额外工具链依赖
- **D-09:** 对比指标：**mean**（criterion estimates.json 中的 `mean.point_estimate`），直接反映平均单次运行时间
- **D-10:** 失败时输出具体数字，格式示例：
  ```
  FAIL: parse_sqllog_file_5mb
    baseline: 674425 ns
    current:  735000 ns
    regression: 9.0% (threshold: 5%)
  ```
- **D-11:** 退化阈值：**5%**，与 REQUIREMENTS.md MEAS-04 一致

### Throughput 指标（MEAS-01）

- **Claude's Discretion:** 使用 `criterion::Throughput::Bytes(file_size)` 报告 GB/s，同时在 benchmark group 中用 `Throughput::Elements(record_count)` 报告 records/sec。实现细节由 planner 决定。

### parse_performance_metrics() 变体（MEAS-02）

- **Claude's Discretion:** 新增 benchmark 变体调用 `parse_performance_metrics()`，具体变体命名和语料库复用方式由 planner 决定。

### Claude's Discretion

- Throughput 单位的 criterion API 用法（`Throughput::Bytes` vs 自定义计算）
- `parse_performance_metrics()` 变体是否复用多行语料库
- Shell 脚本的具体实现细节（jq vs python vs awk）
- criterion estimates.json 的精确路径（根据实际 criterion 输出目录确定）

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 需求规格
- `.planning/REQUIREMENTS.md` — MEAS-01 到 MEAS-04 的完整需求定义
- `.planning/ROADMAP.md` — Phase 1 的 Success Criteria（4 条验收标准）

### 现有代码
- `benches/parser_benchmark.rs` — 现有 benchmark 结构，新代码在此文件扩展
- `.github/workflows/benchmark.yml` — 现有 CI benchmark workflow，门禁脚本在此添加
- `benchmarks/baseline.json` — 当前 baseline 文件，CI 标定后将被替换

### 架构参考
- `.planning/research/PITFALLS.md` — 已知陷阱，包含 CI/benchmark 相关注意事项
- `src/sqllog.rs` — `parse_performance_metrics()` 的实现，benchmark 变体需调用此方法

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `generate_synthetic_log(target_bytes)` 函数（benches/parser_benchmark.rs:8）：已有合成日志生成器，可扩展支持多行 SQL 记录比例参数
- `benchmark_parser` 函数 + `parser_group`：现有 benchmark group 结构，新变体在同一 group 内添加
- `benchmarks/baseline.json`：已有 baseline 文件格式（benchmark name → ns 映射），CI 脚本的输入格式已确定

### Established Patterns
- criterion `group.bench_function` 模式：现有代码展示了如何注册 benchmark 变体
- `NamedTempFile` + `LogParser::from_path` 组合：标准的临时文件 benchmark 模式

### Integration Points
- `benchmark.yml` 中的 `Run parser benchmarks` step 之后添加门禁脚本 step
- `update-baseline.yml` 新 workflow 在运行 benchmark 后提交新 `baseline.json`

</code_context>

<specifics>
## Specific Ideas

- 门禁失败输出必须包含具体数字（baseline 值、当前值、退化百分比），不只是退出状态码
- benchmark 命名约定：`parse_sqllog_{描述}_{大小}`，如 `parse_sqllog_file_5mb`、`parse_sqllog_multiline_5mb`、`parse_sqllog_metrics_5mb`

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-measurement*
*Context gathered: 2026-04-19*
