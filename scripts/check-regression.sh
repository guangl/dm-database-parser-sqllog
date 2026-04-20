#!/usr/bin/env bash
# scripts/check-regression.sh
# CI 回归门禁脚本：对比 criterion estimates.json 与 baseline.json
# 用法：bash scripts/check-regression.sh
# 退化超过阈值时 exit 1，并输出具体数字（D-10）
set -euo pipefail

BASELINE="benchmarks/baseline.json"
THRESHOLD=5  # 退化百分比阈值（D-11：5%）
FAILED=0

if [[ ! -f "$BASELINE" ]]; then
    echo "ERROR: baseline file not found: $BASELINE"
    exit 1
fi

# 遍历 baseline.json 中所有 benchmark key（格式：{group}/{bench_name}）
while IFS= read -r bench_key; do
    baseline_ns=$(jq -r --arg k "$bench_key" '.[$k]' "$BASELINE")

    # 解析 key 为路径：parser_group/parse_sqllog_file_5mb
    # → target/criterion/parser_group/parse_sqllog_file_5mb/new/estimates.json
    estimates_file="target/criterion/${bench_key}/new/estimates.json"

    if [[ ! -f "$estimates_file" ]]; then
        echo "WARNING: estimates file not found: $estimates_file (skipping)"
        continue
    fi

    current_ns=$(jq -r '.mean.point_estimate' "$estimates_file")

    # 防护：任一值为 null 时跳过（字段缺失或结构变化）
    if [[ "$current_ns" == "null" || "$baseline_ns" == "null" ]]; then
        echo "WARNING: could not read values for $bench_key (null), skipping"
        continue
    fi

    # 计算退化百分比：(current - baseline) / baseline * 100
    regression=$(awk "BEGIN { printf \"%.1f\", ($current_ns - $baseline_ns) / $baseline_ns * 100 }")

    # 超过阈值则报告失败（D-10：输出格式）
    if awk "BEGIN { exit ($regression > $THRESHOLD) ? 0 : 1 }"; then
        echo "FAIL: $bench_key"
        echo "  baseline: $(printf '%.0f' $baseline_ns) ns"
        echo "  current:  $(printf '%.0f' $current_ns) ns"
        echo "  regression: ${regression}% (threshold: ${THRESHOLD}%)"
        FAILED=1
    else
        echo "PASS: $bench_key (${regression}%)"
    fi
done < <(jq -r 'keys[]' "$BASELINE")

if [[ $FAILED -eq 0 ]]; then
    echo "All benchmarks within threshold."
fi

exit $FAILED
