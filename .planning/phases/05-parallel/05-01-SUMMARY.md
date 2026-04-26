---
phase: 05-parallel
plan: 01
subsystem: parser
tags: [rust, parallel, index, mmap]

requires:
  - phase: 04-corealgo
    provides: find_next_record_start primitive and FINDER_RECORD_START memmem searcher

provides:
  - pub struct RecordIndex with len()/is_empty() public API
  - pub fn LogParser::index() -> RecordIndex single-threaded index builder
  - RecordIndex exported from dm_database_parser_sqllog crate root

affects: [05-02, 05-03]

tech-stack:
  added: []
  patterns:
    - "Two-phase parallel scan: index() builds offset list, par_iter() consumes it for record-count-balanced partitioning"
    - "saturating_add(1) on pos to prevent infinite loop when find_next_record_start returns same position"
    - "pub(crate) offsets field prevents external access to raw offsets while keeping pub len()/is_empty()"

key-files:
  created: []
  modified:
    - src/parser.rs
    - src/lib.rs
    - tests/parser_parallel.rs

key-decisions:
  - "RecordIndex is newtype wrapping Vec<usize> with pub(crate) offsets — external API only exposes len()/is_empty()"
  - "index() handles file-starts-with-timestamp edge case by pre-checking data[0..23] before the main loop"
  - "pos = next.saturating_add(1) is the T-05-01 DoS mitigation — mandatory for correctness"

patterns-established:
  - "TDD RED/GREEN sequence: failing test commit before implementation commit"
  - "Integration tests use only public API (len/is_empty), not pub(crate) offsets"

requirements-completed: [PAR-01]

duration: inline
completed: 2026-04-26
---

# Phase 05 Plan 01: RecordIndex Type and index() Method Summary

**RecordIndex newtype + LogParser::index() single-thread file scanner exposing record byte-offset list as public PAR-01 API foundation for two-phase parallel scan**

## Performance

- **Tasks:** 2 (RED test commit + GREEN implementation commit)
- **Files modified:** 3

## Accomplishments

- Added `pub struct RecordIndex` with `pub(crate) offsets: Vec<usize>` and `pub fn len()`/`pub fn is_empty()`
- Added `pub fn LogParser::index() -> RecordIndex` that single-thread scans entire mmap, returning all record start byte offsets
- Applied T-05-01 DoS mitigation: `pos = next.saturating_add(1)` prevents infinite loop
- Exported `RecordIndex` from crate root via `src/lib.rs`
- Added 3 integration tests covering count-matches-iter, offsets-valid-timestamps, and empty-file edge cases

## Task Commits

1. **RED**: `test(05-01): add failing tests for RecordIndex and index()` — `02a2dfb`
2. **GREEN**: `feat(05-01): add RecordIndex type and LogParser::index() method` — `031348a`

## Files Created/Modified

- `src/parser.rs` — Added `RecordIndex` struct + `impl RecordIndex` + `LogParser::index()` method
- `src/lib.rs` — Added `RecordIndex` to `pub use parser::{...}` export line
- `tests/parser_parallel.rs` — 3 new integration tests for PAR-01

## Decisions Made

- Used `pub(crate)` for `offsets` field: Plan 02 `par_iter()` rewrite needs direct access within the crate for partition construction
- Placed `RecordIndex` struct definition between `LogParser` struct and `impl LogParser` block
- Did not add `IntoIterator`/`Index`/`as_slice` — PAR-01 only requires `len()`/`is_empty()`

## Deviations from Plan

None.

## Threat Mitigation Verified

| Threat ID | Mitigation | Verified |
|-----------|-----------|---------|
| T-05-01 (DoS - infinite loop) | `pos = next.saturating_add(1)` | confirmed in src/parser.rs |
| T-05-02 (Memory safety) | `next < data.len()` gate before push | loop break condition |
| T-05-03 (Integer overflow) | `saturating_add` prevents usize overflow | same line |
| T-05-04 (Info disclosure) | `offsets` is `pub(crate)` | External tests use only `len()`/`is_empty()` |

## Self-Check: PASSED

- `cargo build` ✓
- `cargo test --test parser_parallel test_index` — 3/3 passed ✓
- `cargo test` — all tests passed ✓
- `cargo clippy --lib -- -D warnings` ✓

## Next Phase Readiness

- `RecordIndex` and `index()` are fully available for Plan 02 (`par_iter()` rewrite)
- Plan 02 can access `idx.offsets` directly (pub(crate)) for partition construction
