---
plan: 03-02
phase: 03-hotpath
status: complete
requirements:
  - HOT-03
  - HOT-04
started: 2026-04-24
completed: 2026-04-24
key-files:
  modified:
    - src/sqllog.rs
    - src/parser.rs
---

## Summary

Plan 03-02 successfully implemented two compiler/OS-level hints for the hot path and I/O initialization layer.

## Changes Made

### Task 1: HOT-03 — Inline and Cold Annotations

**src/sqllog.rs:**
- Added `#[inline(always)]` to `parse_performance_metrics` to force inlining into callers, eliminating function call overhead on the hot path.

**src/parser.rs:**
- Extracted `ParseError::InvalidFormat` construction into a new `#[cold]` function `make_invalid_format_error(raw_bytes: &[u8]) -> ParseError`.
- Replaced all 3 inline `return Err(ParseError::InvalidFormat { raw: ... })` sites in `parse_record_with_hint` with `return Err(make_invalid_format_error(first_line))`.
- The `#[cold]` hint tells the compiler this is an error path, enabling better code layout optimization for the normal (non-error) execution path.

### Task 2: HOT-04 — mmap Sequential Advise

**src/parser.rs:**
- Changed `use memmap2::Mmap;` to `use memmap2::{Advice, Mmap};`
- Added `#[cfg(unix)] let _ = mmap.advise(Advice::Sequential);` after mmap construction in `LogParser::from_path`.
- The advise call hints the OS kernel to use sequential read-ahead for mmap pages, reducing page fault overhead during sequential log parsing.
- Gated with `#[cfg(unix)]` — Windows does not have the `advise()` method on `Mmap`.
- Result is ignored with `let _ = ...` — failure (e.g., unsupported kernel) is silently ignored and does not affect correctness.

## Verification

```
cargo test: 19 passed, 0 failed
cargo build --release: no errors, no warnings
```

### Artifacts Confirmed Present

- `src/sqllog.rs`: `#[inline(always)]` before `parse_performance_metrics`
- `src/parser.rs`: `#[cold]` + `make_invalid_format_error` function
- `src/parser.rs`: `use memmap2::{Advice, Mmap}`
- `src/parser.rs`: `#[cfg(unix)]` + `mmap.advise(Advice::Sequential)`

## Self-Check: PASSED

All success criteria met:
- `cargo test` exits 0
- `cargo build --release` no errors
- HOT-03 inline annotation applied
- HOT-03 cold annotation applied and inline InvalidFormat construction removed
- HOT-04 mmap advise with unix cfg gate applied
