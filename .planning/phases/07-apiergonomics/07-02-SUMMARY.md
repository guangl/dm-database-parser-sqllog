---
phase: 07-apiergonomics
plan: 02
status: complete
tags:
  - rust
  - filters
---

# 07-02 SUMMARY: LogIterator Filter Methods

## What was built

- **`filter_by_exec_time(min_ms: u64)`** — filters records to those with EXECTIME >= min_ms. Uses `parse_performance_metrics()` internally. Records without EXECTIME (exectime == 0.0) are automatically filtered when min_ms > 0.
- **`filter_by_sql_contains(pattern: &str)`** — filters records whose SQL body contains the given pattern (case-sensitive). Uses `body()` with `contains()`. Parse errors are skipped.
- Both methods return `impl Iterator<Item = Result<Sqllog<'a>, ParseError>>` and are added to the `impl<'a> LogIterator<'a>` block.

## Key decisions

- Both methods use `Iterator::filter()` internally, wrapping the existing LogIterator
- `filter_by_exec_time` compares `f32` exectime against `u64` min_ms via `as f32` cast
- `filter_by_sql_contains` calls `pattern.to_string()` once outside the closure for a single allocation
- No new types or traits introduced — both methods stay on `LogIterator`

## Files modified

| File | Changes |
|------|---------|
| src/parser.rs | +filter_by_exec_time (with doc example), +filter_by_sql_contains (with doc example) |
| tests/parser_filters.rs | 6 tests total (3 exec_time + 3 sql_contains) |

## Test coverage

| Test | Scenario |
|------|----------|
| test_filter_by_exec_time_filters_low_exec_time | 5ms filtered, 200ms kept, no-EXECTIME filtered |
| test_filter_by_exec_time_keeps_high_exec_time | Both 150ms/300ms above 100ms threshold kept |
| test_filter_by_exec_time_empty_when_all_below | Both 5ms/50ms below 100ms threshold |
| test_filter_by_sql_contains_matches | Case-sensitive SELECT match |
| test_filter_by_sql_contains_empty_when_no_match | No match on INSERT/UPDATE |
| test_filter_by_sql_contains_skips_parse_errors | Invalid record skipped, valid kept |

## Verification

- `cargo build` — passed
- `cargo test` — all tests passed
- `cargo test --test parser_filters` — 6/6 passed
- `cargo clippy --all-targets -- -D warnings` — clean

## Self-Check: PASSED
