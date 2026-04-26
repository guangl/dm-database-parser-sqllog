---
phase: 05-parallel
plan: 02
subsystem: parser
tags: [rust, parallel, rayon, threshold, par_iter]

requires:
  - plan: 05-01
    provides: RecordIndex type and LogParser::index() method

provides:
  - Rewritten par_iter() with 32 MB threshold two-phase partitioning
  - Small file (<32 MB): single partition, Rayon executes single-threaded (PAR-03)
  - Large file (>=32 MB): index()-based record-count-balanced partitioning (PAR-02)

affects: [05-03]

tech-stack:
  added: []
  patterns:
    - "PAR_THRESHOLD = 32 MB local const — not exposed as public API"
    - "bounds.len() == 1 for small files: Rayon naturally single-threads, no Either needed"
    - "dedup on partition_starts prevents zero-length chunks when records < num_threads"
    - "Last partition extended to data.len() to cover all bytes"

key-files:
  created: []
  modified:
    - src/parser.rs
    - tests/parser_parallel.rs

key-decisions:
  - "No rayon::iter::Either or par_bridge — unified bounds path avoids type incompatibility"
  - "PAR_THRESHOLD is function-local const to avoid locking 32 MB as public API"
  - "large file total==0 fallback to single partition prevents step_by(0) panic"

requirements-completed: [PAR-02, PAR-03]

duration: inline
completed: 2026-04-26
---

# Phase 05 Plan 02: par_iter() Rewrite — Two-Phase Index Partitioning

**par_iter() rewritten with 32 MB threshold: small files use single-partition (no Rayon overhead), large files use RecordIndex-based record-count-balanced partitioning**

## Accomplishments

- Replaced byte-based chunking with two-path architecture:
  - **Small file (<32 MB)**: `bounds = vec![(0, data.len())]` — Rayon single-threads naturally
  - **Large file (>=32 MB)**: calls `self.index()`, partitions by `total / num_threads` records
  - **Empty file**: `bounds = vec![]` — yields 0 records, no panic
  - **Large file, no valid records**: fallback to single partition
- `PAR_THRESHOLD = 32 * 1024 * 1024` as function-local const
- All 9 integration tests pass (6 existing + 3 new PAR-02/PAR-03 tests)

## Task Commits

1. **Task 1**: `feat(05-02): rewrite par_iter() with 32 MB threshold and two-phase index partitioning`
2. **Task 2**: `test(05-02): add PAR-02/PAR-03 tests for large/small file par_iter paths`

## Performance Observations

- `iter()` 5 MB benchmark: **9.44 GiB/s** — no regression vs Phase 4 baseline
- Small file `par_iter` overhead: negligible (single-partition, no index() call)
- Large file speedup measurement: deferred to Plan 03 (64 MB benchmark pair)

## Self-Check: PASSED

- `cargo build` ✓
- `cargo test --test parser_parallel` — 9/9 passed ✓ (including 33 MB large-file multiline test)
- `cargo test` — all tests passed ✓
- `grep "const PAR_THRESHOLD" src/parser.rs` — confirmed ✓
- `grep "self.index()" src/parser.rs` — confirmed in large-file branch ✓
- No `rayon::iter::Either` or `par_bridge` introduced ✓

## Next Phase Readiness

- Plan 03 can add 64 MB seq/par benchmark pair directly against this implementation
- `par_iter()` on 64 MB file (> 32 MB threshold) will exercise the two-phase index path
