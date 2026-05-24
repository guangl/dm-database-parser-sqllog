# Phase 12 Plan 01: AsyncLogParser + tokio Async Interface Summary

## Outcome

Successfully implemented the tokio async interface layer for dm-database-parser-sqllog. Users can now call `AsyncLogParser::new(path).parse().await` in async contexts without manually writing `spawn_blocking`.

## What Was Built

### Task 1: Cargo.toml — tokio Optional Dependency

- Added `tokio = { version = "1", features = ["rt"], optional = true }` to `[dependencies]`
- Added `tokio = { version = "1", features = ["rt", "macros"] }` to `[dev-dependencies]`
- Added `[features]` section with `async = ["tokio/rt"]`
- Non-async users: zero tokio dependency added to their dependency tree
- Async users: `cargo build --features async` brings in tokio with only the runtime feature
- Commit: `c07d193`

### Task 2: AsyncLogParser + AsyncError Implementation

- `src/async_api/mod.rs`: Full implementation with builder pattern
  - `AsyncLogParser::new(path)` — constructs with default Auto encoding hint
  - `.encoding_hint(hint)` — override encoding (consumes self, returns self)
  - `.with_filter(filter)` — set Filter predicate (consumes self, returns self)
  - `.parse().await` — runs sync mmap parse inside `tokio::task::spawn_blocking`
  - `AsyncError` enum: `Parse(#[from] ParseError)` + `Panic(String)` with thiserror
  - 6 unit tests: parse_returns_records, file_not_found, with_filter, encoding_hint, error_is_error, from_parse_error
- `src/lib.rs`: Appended `#[cfg(feature = "async")] pub mod async_api` + `pub use` re-exports
- Commit: `8d10413`

## Verification Results

- `cargo build`: PASS
- `cargo build --features async`: PASS
- `cargo clippy --features async -- -D warnings`: 0 warnings
- `cargo test --features async`: all tests pass
- `cargo test`: all tests pass
- `cargo llvm-cov --workspace --all-features --fail-under-lines 90`: 90.66%

## Key Decisions Applied

- D-01: AsyncLogParser takes ownership via `parse(self)` — no Arc, no Clone needed
- D-03: Filter passed as `Option<Filter>`, None = no filtering
- D-05: Builder pattern with consuming methods (encoding_hint, with_filter return Self)
- D-07: tokio version "1" (semver flexible)
- D-08: async feature activates only `tokio/rt` — no rt-multi-thread, no macros
- D-11: encoding_hint stored directly as FileEncodingHint (Copy, not Option)

## Requirements Satisfied

- ASYNC-01: AsyncLogParser::new(path).parse().await returns Vec<Sqllog>
- ASYNC-02: Internally wraps sync mmap path via spawn_blocking
- ASYNC-03: tokio only in dependency tree when features = ["async"]
- ASYNC-04: with_filter(filter) applies Filter inside spawn_blocking

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

- `.planning/phases/12-asyncapi/12-01-SUMMARY.md`: present
- Task 1 commit `c07d193`: verified in git log
- Task 2 commit `8d10413`: verified in git log
