---
phase: 08-documentation
plan: 03
subsystem: documentation
tags: [examples, csv, filter, slow-query]
requires: []
provides:
  - Two working standalone examples demonstrating Phase 7 API
  - filter_slow_queries: demonstrates filter_by_exec_time() usage
  - batch_export: demonstrates all-record CSV export via parse_meta()
affects: [08-documentation]

tech-stack:
  added: []
  patterns:
    - "Standalone binary examples in examples/ using LogParserBuilder + iter()"
    - "Box<dyn Error> error handling in examples (no extra dev-deps)"

key-files:
  created:
    - examples/filter_slow_queries.rs
    - examples/batch_export.rs
  modified: []

key-decisions:
  - "Use Box<dyn std::error::Error> instead of anyhow to avoid adding dev-dependencies"
  - "CSV quoting via simple string escaping rather than csv crate (no extra deps)"
  - "Both examples take single CLI argument for file path (D-06 compliance)"

patterns-established:
  - "Example files use doc comment header with Chinese description"
  - "English variable names and inline comments (D-02)"

requirements-completed: [DOC-03]

duration: 6min
completed: 2026-05-19
---

# Phase 08: Plan 03 — Examples Summary

**Two standalone runnable examples (filter_slow_queries + batch_export) using Phase 7 API to demonstrate common usage patterns**

## Performance

- **Duration:** 6 min
- **Started:** 2026-05-19T09:05:00Z
- **Completed:** 2026-05-19T09:11:00Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Created `examples/filter_slow_queries.rs` — filters queries with exec_time >= 100ms using `filter_by_exec_time()` API; prints timestamp, exec time, and SQL body per match; 36 lines
- Created `examples/batch_export.rs` — exports all records as CSV to stdout using `parse_meta().username`, `body()`, and `exec_time()`; properly quotes fields and escapes embedded double-quotes; 53 lines
- Both examples compile without warnings and pass `cargo test --doc`

## Task Commits

Each task was committed atomically:

1. **Task 1: Create examples/filter_slow_queries.rs** - `539f02f` (feat)
2. **Task 2: Create examples/batch_export.rs** - `847056a` (feat)
3. **Task 3: Verify both examples compile** — verified inline during task execution; no separate commit needed as both examples were already committed in prior tasks

**Plan metadata:** _committed below_

## Files Created/Modified

- `examples/filter_slow_queries.rs` — Standalone example filtering slow queries (exec_time >= 100ms) via `filter_by_exec_time()` and `exec_time()` API
- `examples/batch_export.rs` — Standalone example exporting all records as CSV (timestamp, username, sql, exec_time_ms) with field quoting

## Decisions Made

- Used `Box<dyn std::error::Error>` instead of anyhow/eyre to avoid adding dev-dependencies to Cargo.toml
- CSV quoting implemented with simple string escaping (`"` -> `""`) rather than pulling in the csv crate
- Both examples take a single CLI argument for the log file path, with a usage message printed when missing (D-06 compliance)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Compilation error on `batch_export.rs` initial write: partial move of `record.ts` (type `Cow<str>`, not `Copy`) caused "borrow after partial move" error. Fixed by taking a reference `&record.ts` instead of moving the field. Zero-iteration fix, no impact on timeline.

## Threat Flags

None — both examples are read-only CLI tools using the library's public API. No new network endpoints, auth paths, or schema changes introduced.

## Known Stubs

None — both examples are fully functional.

## Next Phase Readiness

- DOC-03 requirement satisfied: `examples/` directory contains 2 standalone runnable examples
- Ready for continuation of Phase 08 documentation (CHANGELOG, README updates, etc.)

---

*Phase: 08-documentation*
*Completed: 2026-05-19*
