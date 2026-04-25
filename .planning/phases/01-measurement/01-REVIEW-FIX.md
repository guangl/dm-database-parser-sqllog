---
phase: 01-measurement
fixed_at: 2026-04-20T00:00:00Z
fix_scope: critical_warning
findings_in_scope: 5
fixed: 5
skipped: 0
iteration: 1
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed:** 2026-04-20
**Scope:** Critical + Warning (5 findings)
**Status:** all_fixed

## Fixes Applied

### WR-01: `actions/upload-artifact@v7` → `@v4`

**File:** `.github/workflows/benchmark.yml:55`
**Commit:** fix(01): use correct upload-artifact@v4 action version
**Action:** Changed `upload-artifact@v7` to `upload-artifact@v4` (v7 does not exist).

---

### WR-02: `actions/github-script@v9` → `@v7`

**File:** `.github/workflows/benchmark.yml:64`
**Commit:** fix(01): use correct github-script@v7 action version
**Action:** Changed `github-script@v9` to `github-script@v7` (v9 does not exist).

---

### WR-03: PR 评论脚本改为使用 `check-regression.sh` 输出

**File:** `.github/workflows/benchmark.yml:66-83`
**Commit:** fix(01): fix PR comment script to use regression check output
**Action:** 删除读取不存在的 `parser_bench.html` 的逻辑，改为执行 `check-regression.sh` 并将其输出作为 PR 评论内容。

---

### WR-04: `check-regression.sh` 添加 `null` 防护

**File:** `scripts/check-regression.sh:30`
**Commit:** fix(01): guard against null jq values in check-regression.sh
**Action:** 在 `current_ns` 和 `baseline_ns` 读取后添加 null 检查，遇到 `"null"` 时输出警告并跳过该 benchmark，避免 awk 算术异常。

---

### WR-05: `update-baseline.yml` 添加 `contents: write` 权限

**File:** `.github/workflows/update-baseline.yml:12`
**Commit:** fix(01): add contents: write permission to update-baseline workflow
**Action:** 在 job 级别添加 `permissions: contents: write`，确保 `git push` 在受限默认权限仓库中不会以 403 失败。

---

## Info Findings (not in scope)

IN-01, IN-02, IN-03 were out of scope (critical_warning fix scope). No action taken.

_Fixed by: Claude (gsd-code-fixer)_
