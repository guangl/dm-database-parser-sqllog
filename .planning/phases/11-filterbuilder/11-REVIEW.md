---
phase: 11-filterbuilder
reviewed: 2026-05-22T00:00:00Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - examples/filter_builder.rs
  - src/filter/adapter.rs
  - src/filter/builder.rs
  - src/filter/mod.rs
  - src/lib.rs
  - src/parser/iterator.rs
  - tests/filter_builder.rs
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: findings
---

# Phase 11: Code Review Report

**Reviewed:** 2026-05-22
**Depth:** standard
**Files Reviewed:** 7
**Status:** findings

## Summary

Phase 11 adds a `FilterBuilder` fluent API composing AND-chained predicates over `Sqllog` records, adapter functions wiring filters into `LogIterator`, and `apply_filter` / `apply_filter_keep_errors` delegation methods. The overall design is sound: predicates are boxed with `Send + Sync` for future async use, `Filter::matches` short-circuits via `Iterator::all`, and the builder pattern is consistent.

Three behavioral issues were found: a semantic inconsistency between the pre-existing `filter_by_exec_time` (`>=`) and the new `exec_time_gt` (`>`); silent no-op behaviour when `_between` helpers receive an inverted range; and a lossy `u64 as f32` cast in the legacy adapter. Three informational items cover a spurious lifetime constraint, missing error-handling documentation, and a confusing module visibility pattern.

No critical issues (panics, data corruption, security vulnerabilities) were found.

---

## Warnings

### WR-01: `filter_by_exec_time` and `exec_time_gt` have opposite boundary semantics

**File:** `src/filter/adapter.rs:14` and `src/filter/builder.rs:356`

**Issue:** `filter_by_exec_time` uses `exectime >= threshold` (inclusive), while the new `FilterBuilder::exec_time_gt` uses `r.exectime > min_ms` (exclusive). A caller migrating from `iter.filter_by_exec_time(100)` to `FilterBuilder::new().exec_time_gt(100.0)` silently drops records where `exectime == 100.0`. Both methods accept a millisecond threshold, so users will naturally expect them to be equivalent.

**Recommendation:** Align semantics. Either rename `exec_time_gt` to `exec_time_gte` and add a true `exec_time_gt`, or change `filter_by_exec_time` to use strict `>`. If `>=` is the intended contract for both, change `builder.rs:356` from `>` to `>=`:

```rust
// src/filter/builder.rs
pub fn exec_time_gt(self, min_ms: f32) -> Self {
    self.add(move |r| r.exectime > min_ms)   // currently exclusive
}

// Fix option A: rename to exec_time_gte and keep both variants
pub fn exec_time_gte(self, min_ms: f32) -> Self {
    self.add(move |r| r.exectime >= min_ms)
}

// Fix option B: make adapter consistent by switching adapter to > too
//   src/filter/adapter.rs line 14:
Ok(sqllog) => sqllog.exectime > threshold,
```

---

### WR-02: Inverted `_between` ranges silently match nothing

**File:** `src/filter/builder.rs:369-415`

**Issue:** `exec_time_between`, `rowcount_between`, `exec_id_between`, and `ep_between` do not validate that `min <= max`. Calling `exec_time_between(300.0, 100.0)` compiles, runs, and silently returns zero results because `r.exectime >= 300.0 && r.exectime <= 100.0` is always false. This is a latent footgun — the mistake is invisible at call sites and produces no error or warning.

**Recommendation:** Add a `debug_assert!` at minimum; for a library API a release-mode panic is defensible:

```rust
// src/filter/builder.rs
pub fn exec_time_between(self, min_ms: f32, max_ms: f32) -> Self {
    assert!(min_ms <= max_ms, "exec_time_between: min_ms ({min_ms}) must be <= max_ms ({max_ms})");
    self.add(move |r| r.exectime >= min_ms && r.exectime <= max_ms)
}

pub fn rowcount_between(self, min: u32, max: u32) -> Self {
    assert!(min <= max, "rowcount_between: min ({min}) must be <= max ({max})");
    self.add(move |r| r.rowcount >= min && r.rowcount <= max)
}

pub fn exec_id_between(self, min: i64, max: i64) -> Self {
    assert!(min <= max, "exec_id_between: min ({min}) must be <= max ({max})");
    self.add(move |r| r.exec_id >= min && r.exec_id <= max)
}

pub fn ep_between(self, min: u8, max: u8) -> Self {
    assert!(min <= max, "ep_between: min ({min}) must be <= max ({max})");
    self.add(move |r| r.ep >= min && r.ep <= max)
}
```

---

### WR-03: Lossy `u64 as f32` cast in `filter_by_exec_time`

**File:** `src/filter/adapter.rs:12`

**Issue:** `let threshold = min_ms as f32;` converts a `u64` millisecond value to `f32`. f32 has a 24-bit mantissa, so any value above 2^24 (16,777,216 ms ≈ 4.6 hours) can lose precision. For example, `16_777_217_u64 as f32` rounds to `16_777_216.0`, making `filter_by_exec_time(16_777_217)` behave identically to `filter_by_exec_time(16_777_216)`. While such thresholds are unlikely in practice, the cast is silently lossy and the discrepancy between the `u64` parameter type (implying full integer precision) and the `f32` storage of `exectime` is unaddressed.

**Recommendation:** Either document the precision limit in the method doc comment, or match the parameter type to `f32` (consistent with `FilterBuilder::exec_time_gt`):

```rust
// Option A: change parameter type to f32 for consistency with FilterBuilder
pub fn filter_by_exec_time(
    self,
    min_ms: f32,
) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
    adapter::filter_by_exec_time(self, min_ms)
}

// Option B: document the precision limit
/// Filters records with execution time >= `min_ms` milliseconds.
///
/// **Note:** the threshold is compared against `exectime` (f32). Values above
/// 2^24 (≈16.7M ms) may be rounded due to f32 precision limits.
```

---

## Info

### IN-01: Spurious `'a` lifetime on `filter_by_sql_contains` pattern argument

**File:** `src/parser/iterator.rs:58`

**Issue:** `filter_by_sql_contains` is declared as `pattern: &'a str`, tying the pattern's lifetime to the iterator's data lifetime (`'a`). The adapter immediately clones the pattern (`pattern.to_string()`), so the `'a` bound is unnecessary and overly restrictive — callers cannot pass a short-lived `&str` even though ownership is immediately transferred.

**Recommendation:** Remove the `'a` lifetime from the parameter:

```rust
pub fn filter_by_sql_contains(
    self,
    pattern: &str,       // was: &'a str
) -> impl Iterator<Item = Result<Sqllog, ParseError>> + 'a {
    adapter::filter_by_sql_contains(self, pattern)
}
```

---

### IN-02: `filter_by_exec_time` and `filter_by_sql_contains` doc comments omit error-dropping behaviour

**File:** `src/parser/iterator.rs:47-61`

**Issue:** The public doc comments for `filter_by_exec_time` and `filter_by_sql_contains` do not mention that `Err` records are silently discarded. By contrast, `apply_filter` explicitly documents this in `adapter.rs`. A caller who cares about parse errors will not discover the silent drop unless they inspect the adapter source.

**Recommendation:** Add a note to both doc comments:

```rust
/// Filters records with execution time >= `min_ms` milliseconds.
///
/// Parse errors encountered during iteration are **silently dropped**.
/// Use [`apply_filter_keep_errors`] if error propagation is required.
pub fn filter_by_exec_time(...)

/// Filters records whose SQL body contains `pattern`.
///
/// Parse errors encountered during iteration are **silently dropped**.
/// Use [`apply_filter_keep_errors`] if error propagation is required.
pub fn filter_by_sql_contains(...)
```

---

### IN-03: Mixed `pub(crate)` / `pub mod` visibility in `filter` module hierarchy

**File:** `src/lib.rs:83` and `src/filter/mod.rs:2`

**Issue:** `lib.rs` declares `pub(crate) mod filter`, making the entire `filter` module crate-internal. Inside `filter/mod.rs`, `builder` is declared as `pub mod builder` (fully public). These two declarations do not conflict at compile time, but the inner `pub mod` is misleading — `builder` is not actually reachable from outside the crate through `filter::builder` because the parent module is `pub(crate)`. `Filter` and `FilterBuilder` are accessible only through the `lib.rs` re-exports. This is correct but confusing to future contributors.

**Recommendation:** Align the inner declaration with the actual visibility intent:

```rust
// src/filter/mod.rs
pub(crate) mod adapter;
pub(crate) mod builder;   // was: pub mod builder

pub use builder::{Filter, FilterBuilder};
```

Public types `Filter` and `FilterBuilder` remain accessible through `lib.rs` re-exports without change.

---

_Reviewed: 2026-05-22_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
