---
phase: 04-corealgo
fixed_at: 2026-04-25T10:43:53Z
review_path: .planning/phases/04-corealgo/04-REVIEW.md
iteration: 1
fix_scope: critical_warning
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 04: Code Review Fix Report

**Fixed at:** 2026-04-25T10:43:53Z
**Source review:** `.planning/phases/04-corealgo/04-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 4 (1 Critical + 3 Warning)
- Fixed: 4
- Skipped: 0

## Fixed Issues

### CR-01: `from_raw_parts` lifetime extension is unsound for `FileEncodingHint::Utf8` path

**Files modified:** `src/parser.rs`
**Commit:** `0781548`
**Applied fix:** Removed all four occurrences of the redundant `std::slice::from_raw_parts(ptr, len)` wrapping in `parse_record_with_hint`. The slices (`meta_bytes`, `inner`) are already `&[u8]` values carrying the correct `'a` lifetime — passing them through `from_raw_parts` with their own pointer and length is a no-op semantically but created a confusing lifetime-laundering appearance. Replaced with direct `std::str::from_utf8_unchecked(meta_bytes)` / `std::str::from_utf8_unchecked(inner)` in both the `Utf8` and `Auto` branches, with updated SAFETY comments. Also updated the `Auto` branch to drop the now-unused `s` binding from `simd_from_utf8` (changed `Ok(s) =>` to `Ok(_) =>`).

### WR-01: Encoding detection gap — middle of large files never sampled

**Files modified:** `src/parser.rs`
**Commit:** `9198a1b`
**Applied fix:** Added an explanatory comment block to the encoding detection section (lines 49-61) documenting the known limitation: for files larger than ~68 KB the middle of the file is not sampled, so GB18030 sequences appearing only in the middle could cause misclassification as UTF-8. The comment also explains why this is acceptable in practice (DM log files use GB18030 throughout or are entirely ASCII-safe) and notes where a middle-window sample could be added if needed.

### WR-02: `dedup()` is load-bearing but has no explanatory comment

**Files modified:** `src/parser.rs`
**Commit:** `583cd0e`
**Applied fix:** Added a four-line comment immediately before `starts.dedup()` explaining that `dedup` is load-bearing for correctness: when the file has fewer records than threads, `find_next_record_start` returns `data.len()` multiple times, producing duplicate entries that would generate zero-length chunks without `dedup`.

### WR-03: `parse_record` public API uses wrong `is_multiline` default

**Files modified:** `src/parser.rs`
**Commit:** `29784da`
**Applied fix:** Replaced the hardcoded `is_multiline: true` in `parse_record` with an auto-detection via `memchr(b'\n', record_bytes).is_some()`. This correctly sets `is_multiline` based on whether the input slice actually contains a newline, making the public API correct for both single-line callers (no redundant branch) and multi-line callers (explicit multiline path). All 61 tests pass after this change.

## Skipped Issues

None — all in-scope findings were successfully fixed.

---

_Fixed: 2026-04-25T10:43:53Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
