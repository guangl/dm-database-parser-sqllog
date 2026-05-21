---
phase: 07-apiergonomics
plan: 01
status: complete
tags:
  - rust
  - builder
---

# 07-01 SUMMARY: LogParserBuilder

## What was built

- **LogParserBuilder** struct with `new()`, `threads()`, `parallel_threshold()`, `encoding_hint()`, `build()` methods
- **LogParser** now has `parallel_threshold: usize` field, replacing the hardcoded `const PAR_THRESHOLD`
- **FileEncodingHint** visibility changed from `pub(crate)` to `pub`
- **LogParser::from_path** fully removed
- All 10 test files and 1 benchmark file migrated to `LogParserBuilder::new(path).build().unwrap()`

## Key decisions

- `build()` calls `rayon::ThreadPoolBuilder::build_global()` when `threads()` is set; if already initialized, silently ignores
- `encoding_hint(None)` performs auto-detection (same as old `from_path` behavior); `Some(hint)` skips detection
- `parallel_threshold` defaults to 32 MiB (same as the old constant)
- `exec_time()` and `row_count()` return `Result<Option<u64>, ParseError>` — Err variant reserved for future

## Files modified

| File | Changes |
|------|---------|
| src/parser.rs | +LogParserBuilder struct + impl, +parallel_threshold field, -from_path, pub FileEncodingHint |
| src/lib.rs | +LogParserBuilder + FileEncodingHint exports, updated doc examples |
| tests/ | 9 test files: from_path → LogParserBuilder::new().build() |
| benches/parser_benchmark.rs | 8 from_path → LogParserBuilder::new().build() |

## Verification

- `cargo build` — passed
- `cargo test` — 19 tests + 4 doctests passed
- `cargo clippy --all-targets -- -D warnings` — clean
- `grep -rn "LogParser::from_path" src/ tests/ benches/` — no results
- `grep -c "pub struct LogParserBuilder" src/parser.rs` — 1

## Self-Check: PASSED
