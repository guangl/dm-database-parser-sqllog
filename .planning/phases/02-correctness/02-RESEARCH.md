# Phase 2: Correctness - Research

**Researched:** 2026-04-20
**Domain:** Rust unsafe code correctness, Miri CI integration, byte-level string splitting logic
**Confidence:** HIGH (all findings verified against live codebase and tool runs)

## Summary

Phase 2 addresses three correctness risks in the parser before any hot-path optimizations land.
All three requirements are small, self-contained fixes with low implementation risk.

CORR-01 is a one-line change: remove the 64 KB sample cap in `LogParser::from_path` so encoding
detection scans the entire file. The existing test infrastructure already covers this; one new
regression test for large files with late-appearing GB18030 bytes is needed.

CORR-02 requires adding a Miri CI job and annotating mmap-using tests with `#[cfg(not(miri))]`.
Miri is already installed on the nightly toolchain and passes all `parse_record`-based tests
(10/10 in `performance_metrics.rs`). mmap is fundamentally unsupported under Miri; the unsafe
`decode_content_bytes` paths are fully reachable via `parse_record` (no mmap required).

CORR-03 is a confirmed bug: `find_indicators_split` uses `rfind` on the last 256 bytes of
`content_raw` and will incorrectly split at any `"EXECTIME: "` / `"ROWCOUNT: "` / `"EXEC_ID: "`
found in SQL body text when no real indicators follow. The fix is a validation step inside
`find_indicators_split` that calls `parse_indicators_from_bytes` on the candidate trailing slice
and falls back to `content_raw.len()` (no split) if parsing yields nothing.

**Primary recommendation:** Implement CORR-01 and CORR-03 as one plan (code fixes + tests), then
CORR-02 as a separate plan (CI infra + `#[cfg(not(miri))]` annotations).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORR-01 | 编码检测采样范围扩展至整个文件（或足够大的样本） | One-line fix in `from_path`; simdutf8 at ~50 GB/s makes full-scan cost acceptable |
| CORR-02 | Miri 加入 CI，覆盖 unsafe 解码路径 | Miri installed and passing; need CI job + `#[cfg(not(miri))]` annotations on mmap tests |
| CORR-03 | `find_indicators_split` 针对 SQL body 内含指标关键字有测试用例 | Bug confirmed; fix via parse-after-find validation; test cases designed |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Encoding detection | Parser (file open) | — | One-time file-level decision at `from_path`; all subsequent decoding uses the stored hint |
| Unsafe UTF-8 decode | `decode_content_bytes` (sqllog.rs) | `parse_record_with_hint` (parser.rs) | Both contain `unsafe` blocks that Miri must cover |
| Indicator/body split | `find_indicators_split` (sqllog.rs) | — | Pure logic function; no I/O dependency |
| CI validation | GitHub Actions workflow | Local `cargo miri test` | Miri job runs on nightly; must be isolated from mmap tests |

---

## CORR-01: Encoding Detection Sampling

### Current Code (parser.rs:37-43)

```rust
// Sample the first 64 KB to determine encoding.
let sample = &mmap[..mmap.len().min(65536)];
let encoding = if simd_from_utf8(sample).is_ok() {
    FileEncodingHint::Utf8
} else {
    FileEncodingHint::Gb18030
};
```

[VERIFIED: src/parser.rs line 37-43]

### The Bug

A DM log file that is valid UTF-8 in its first 64 KB but contains GB18030-encoded bytes later
will be misclassified as `Utf8`. Subsequent `decode_content_bytes` calls with
`FileEncodingHint::Utf8` then call `std::str::from_utf8_unchecked` on those bytes —
**undefined behaviour** (invalid UTF-8 passed to unchecked conversion).

This is not a theoretical risk: real DM log files often start with ASCII/UTF-8 session setup
lines and only contain Chinese characters (GB18030) in SQL bodies or appnames that appear later.

### Fix (1-line change)

```rust
// Sample entire file to determine encoding — eliminates truncation misclassification.
let sample = &mmap[..];
let encoding = if simd_from_utf8(sample).is_ok() {
    FileEncodingHint::Utf8
} else {
    FileEncodingHint::Gb18030
};
```

**Performance impact:** `simdutf8` throughput is ~50 GB/s on modern x86/ARM hardware.
[ASSUMED: exact throughput on CI runner; benchmark on CI environment may differ.]
- 100 MB file: ~2 ms extra overhead at open time (one-time cost)
- 1 GB file: ~20 ms

This is acceptable for a library that reads entire files. The sequential mmap access pattern
means the OS will stream pages in any case.

### Test Strategy for CORR-01

One new integration test is needed. The existing `file_encoding_detection_gb18030` test uses a
single short record, so the GB18030 bytes are in the first 64 KB — it does NOT catch the bug.

**New test:** Write a temp file where the first 65 536+ bytes are valid ASCII, followed by a
record containing GB18030-encoded bytes. Verify `parse_meta().username` decodes correctly.

```rust
#[test]
fn encoding_detection_gb18030_after_64kb_boundary() {
    use encoding::all::GB18030;
    use encoding::{EncoderTrap, Encoding};
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Pad with valid ASCII records to push past 64 KB
    let ascii_record = "2025-11-17 16:09:41.123 (EP[0] sess:1 thrd:2 user:ascii trxid:0 stmt:0 appname:app) SELECT 1;\n";
    let repeat_count = 65536 / ascii_record.len() + 2;

    let username = "用户";
    let user_bytes = GB18030.encode(username, EncoderTrap::Strict).unwrap();
    let mut gb_line: Vec<u8> = b"2025-11-17 16:09:42.000 (EP[0] sess:2 thrd:2 user:".to_vec();
    gb_line.extend_from_slice(&user_bytes);
    gb_line.extend_from_slice(b" trxid:0 stmt:0 appname:app) SELECT 2;\n");

    let mut tmp = NamedTempFile::new().unwrap();
    for _ in 0..repeat_count {
        tmp.write_all(ascii_record.as_bytes()).unwrap();
    }
    tmp.write_all(&gb_line).unwrap();
    tmp.as_file().sync_all().unwrap();

    let parser = LogParser::from_path(tmp.path()).unwrap();
    let records: Vec<_> = parser.iter().collect();
    let last = records.last().unwrap().as_ref().unwrap();
    assert_eq!(last.parse_meta().username, username);
}
```

This test must be annotated `#[cfg(not(miri))]` because it uses `LogParser` (mmap).

---

## CORR-02: Miri CI Integration

### Miri Availability

- **Local:** Miri installed under nightly toolchain (`miri 0.1.0 (e22c616e4e 2026-04-19)`)
  [VERIFIED: `rustup run nightly cargo miri --version`]
- **CI:** nightly toolchain must be installed in the Miri job; `rustup component add miri` is
  sufficient since the component is available for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`.

### memmap2 + Miri Constraint

`memmap2 0.9.10` does NOT support Miri. The `Mmap::map` call errors with:

```
unsupported operation: Miri does not support file-backed memory mappings
```

[VERIFIED: `MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test edge_cases`]

Even `tempfile::NamedTempFile::new()` fails under Miri isolation; with
`-Zmiri-disable-isolation` the file creation succeeds but `Mmap::map` still errors.

**Conclusion:** All tests that use `LogParser::from_path` must be annotated `#[cfg(not(miri))]`.

### Tests That Pass Under Miri (No Changes Needed)

All 10 tests in `tests/performance_metrics.rs` pass under Miri:
[VERIFIED: `cargo miri test --test performance_metrics`]

These tests use `parse_record` (raw bytes, no mmap) and exercise:
- `str::from_utf8_unchecked` for timestamp (parser.rs:253)
- `str::from_utf8_unchecked` for meta via `Auto` path (parser.rs:304)
- `decode_content_bytes` with `is_borrowed=true`, `Auto` encoding (sqllog.rs:264)
- `parse_meta` `to_cow` closure (sqllog.rs:128)
- `parse_performance_metrics` full path (sqllog.rs:106)

The `FileEncodingHint::Utf8` path in `decode_content_bytes` is only reachable via
`LogParser::from_path` (mmap) and cannot be covered by Miri. This is an acceptable gap
because the Utf8 path is only entered after the file-level validation already proved validity.

### Tests Requiring `#[cfg(not(miri))]`

| File | Tests to Annotate |
|------|------------------|
| `tests/edge_cases.rs` | `probable_record_start_line_and_iterator_singleline_detection` |
| `tests/sqllog_additional.rs` | `file_encoding_detection_gb18030`, `file_encoding_detection_utf8` |
| `tests/parser_iterator.rs` | ALL (all use `LogParser` + `tempfile`) |
| `tests/parser_errors.rs` | ALL (all use `LogParser` + `tempfile`) |
| `tests/integration_test.rs` | ALL (all use `LogParser` + `tempfile`) |

The new CORR-01 test (`encoding_detection_gb18030_after_64kb_boundary`) also needs
`#[cfg(not(miri))]`.

### CI Job Structure

New file: `.github/workflows/miri.yml`

```yaml
name: Miri

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

jobs:
  miri:
    name: Miri unsafe check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6

      - name: Install nightly + Miri
        uses: dtolnay/rust-toolchain@nightly
        with:
          components: miri

      - name: Cache cargo registry
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-miri-${{ hashFiles('**/Cargo.lock') }}

      - name: Run Miri (unsafe decode paths)
        run: cargo miri test --test performance_metrics --test sqllog_additional --test edge_cases
        env:
          MIRIFLAGS: "-Zmiri-disable-isolation"
```

**Note:** `--test sqllog_additional` and `--test edge_cases` will have some tests skipped via
`#[cfg(not(miri))]`. Only `performance_metrics.rs` can run entirely without annotations.

### Unsafe Blocks Covered by Miri

| Location | Unsafe Operation | Covered By |
|----------|-----------------|------------|
| `parser.rs:253` | `str::from_utf8_unchecked` (timestamp) | `performance_metrics.rs` (all tests) |
| `parser.rs:290-293` | `str::from_utf8_unchecked` (meta, Utf8 hint) | NOT covered (mmap required) |
| `parser.rs:304-309` | `str::from_utf8_unchecked` (meta, Auto hint) | `performance_metrics.rs` |
| `parser.rs:347-352` | `str::from_utf8_unchecked` (tag, Utf8 hint) | NOT covered (mmap required) |
| `parser.rs:354-359` | `str::from_utf8_unchecked` (tag, Auto hint) | `edge_cases.rs::appname_empty_*` |
| `sqllog.rs:128-135` | `str::from_utf8_unchecked` in `to_cow` | `sqllog_additional.rs::meta_parsing_*` |
| `sqllog.rs:251-258` | `decode_content_bytes` Utf8 path | NOT covered (mmap required) |
| `sqllog.rs:264-271` | `decode_content_bytes` Auto path | `performance_metrics.rs` (all tests) |

---

## CORR-03: find_indicators_split False Split Bug

### Bug Analysis

`find_indicators_split` (sqllog.rs:212-230):

```rust
fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    let len = data.len();
    let start = len.saturating_sub(256);
    let window = &data[start..];

    let mut earliest = window.len();
    for finder in [
        &*FINDER_REV_EXECTIME,  // searches "EXECTIME: "
        &*FINDER_REV_ROWCOUNT,  // searches "ROWCOUNT: "
        &*FINDER_REV_EXEC_ID,   // searches "EXEC_ID: "
    ] {
        if let Some(idx) = finder.rfind(window) {
            earliest = earliest.min(idx);
        }
    }
    start + earliest
}
```

[VERIFIED: src/sqllog.rs lines 212-230]

**Bug confirmed** via live test: SQL body `"SELECT * FROM metrics WHERE col = 'EXECTIME: slow'"` with no real indicators results in `body()` returning `"SELECT * FROM metrics WHERE col = '"` (truncated). [VERIFIED: cargo test with #[ignore] test added temporarily]

The `rfind` finds the last occurrence of `"EXECTIME: "` in the window. When this is in the SQL body (not a real indicator), the split is placed incorrectly. The trailing slice `"EXECTIME: slow'"` is passed to `parse_indicators_from_bytes`, which finds no `'('` after the value and returns `None` — so `parse_indicators()` returns `None` (correct), but `body()` is still truncated (wrong).

### Scenario Matrix

| SQL body contains | Real indicators | rfind result | body() | indicators | Status |
|------------------|----------------|-------------|--------|------------|--------|
| No keyword | None | `window.len()` (no match) | Correct | None | OK |
| No keyword | Present | Real indicator position | Correct | Parsed | OK |
| `EXECTIME: slow` | None | Position in SQL body | **WRONG** | None | **BUG** |
| `EXECTIME: slow` | Real indicator after | Last occurrence = real | Correct | Parsed | OK |
| `EXECTIME: 5(ms)` (valid fmt in SQL) | None | Position in SQL body | **WRONG** | Some (!) | **BUG** |

The last row is the hardest case: if the SQL body contains something that looks exactly like a
real indicator, both `body()` and `indicators()` are wrong.

### Fix: Post-Find Validation

After computing `earliest`, verify the candidate trailing slice actually contains parseable
indicators. If not, return `len` (no split).

```rust
fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    let len = data.len();
    let start = len.saturating_sub(256);
    let window = &data[start..];

    let mut earliest = window.len();
    for finder in [
        &*FINDER_REV_EXECTIME,
        &*FINDER_REV_ROWCOUNT,
        &*FINDER_REV_EXEC_ID,
    ] {
        if let Some(idx) = finder.rfind(window) {
            earliest = earliest.min(idx);
        }
    }

    let split = start + earliest;
    // Validate: only accept the split if the trailing slice parses as real indicators.
    if split < len && parse_indicators_from_bytes(&data[split..]).is_none() {
        return len; // no valid indicator found — entire content is body
    }
    split
}
```

**Note:** `parse_indicators_from_bytes` is a module-level function in `sqllog.rs` (currently not
`pub`). `find_indicators_split` is a method of `Sqllog` in the same file — no visibility change
needed.

**Edge case (SQL body contains valid-looking indicator AND real indicator):**
`"SELECT 1 WHERE EXECTIME: 5(ms) ROWCOUNT: 1(rows) EXEC_ID: 99."` — rfind finds the rightmost
match which is `EXEC_ID: ` (the real one), split is at `EXEC_ID:` position. Validation passes
because parsing succeeds. Body includes `"EXECTIME: 5(ms) ROWCOUNT: 1(rows) "` as part of SQL —
this is a known ambiguity but acceptable; the real-world format is highly unlikely to embed
perfectly-formatted indicator strings inside SQL literals.

### Required Test Cases for CORR-03

All tests use `parse_record` (no mmap, Miri-compatible).

```rust
// Test 1: EXECTIME: in SQL body, no real indicators -> body correct, indicators None
fn find_indicators_split_exectime_keyword_in_sql_body_no_indicators()

// Test 2: ROWCOUNT: in SQL body, no real indicators -> body correct, indicators None
fn find_indicators_split_rowcount_keyword_in_sql_body_no_indicators()

// Test 3: EXEC_ID: in SQL body, no real indicators -> body correct, indicators None
fn find_indicators_split_exec_id_keyword_in_sql_body_no_indicators()

// Test 4: Keyword in SQL body AND real indicators present -> split at real indicator, correct
fn find_indicators_split_keyword_in_body_plus_real_indicators()

// Test 5: Multiple keywords in SQL body, no real indicators -> entire content is body
fn find_indicators_split_multiple_keywords_in_body_no_indicators()
```

---

## Standard Stack

No new dependencies are introduced in Phase 2. All fixes use existing crate capabilities.

| Used Already | Version | Role in Phase 2 |
|-------------|---------|-----------------|
| `simdutf8` | 0.1.5 | Full-file UTF-8 validation in CORR-01 |
| `encoding` (GB18030) | 0.2 | Unchanged; already in decode path |
| `memchr` (FinderRev) | 2.7.6 | Unchanged; rfind in `find_indicators_split` |

**Miri toolchain:** nightly. No `Cargo.toml` changes needed; Miri runs as a cargo command.

---

## Architecture Patterns

### CORR-01: Minimal Diff

Only `src/parser.rs` line 38 changes:

```
Before: let sample = &mmap[..mmap.len().min(65536)];
After:  let sample = &mmap[..];
```

No API changes, no struct changes. The `FileEncodingHint` enum and downstream code remain identical.

### CORR-02: Annotation Pattern

```rust
// In tests that use LogParser / tempfile:
#[test]
#[cfg(not(miri))]
fn test_that_uses_mmap() { ... }
```

No test logic changes. Only attribute additions.

### CORR-03: find_indicators_split Validation

The fix adds ~5 lines to `find_indicators_split`. Function stays under 40 lines (currently ~18
lines, fix adds ~5 lines = 23 total). [VERIFIED: current function is 19 lines]

```
find_indicators_split (after fix):
- Compute window (same as now)
- rfind for all three patterns (same as now)
- NEW: call parse_indicators_from_bytes on candidate slice
- NEW: if None, return len (no split)
- return split (same as now)
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Miri CI job | Custom unsafe checker | `cargo miri test` | Miri is the authoritative UB detector for Rust |
| Indicator format validation | Regex or hand-written parser | Reuse existing `parse_indicators_from_bytes` | Already handles all three indicators correctly |
| GB18030 re-encoding test data | Custom encoder | `encoding::all::GB18030.encode(...)` | Already a dev-dependency |

---

## Common Pitfalls

### Pitfall 1: Miri Requires nightly

**What goes wrong:** Running `cargo miri test` with stable toolchain errors immediately.
**Why it happens:** Miri is only available on nightly.
**How to avoid:** CI job uses `dtolnay/rust-toolchain@nightly` with `components: miri`.
**Warning signs:** `error[E0463]: can't find crate` or `cargo: command not found: miri`.

### Pitfall 2: Forgetting `#[cfg(not(miri))]` on tempfile Tests

**What goes wrong:** Miri job fails with "mmap not supported" even after adding CI job.
**Why it happens:** Tests using `NamedTempFile` + `LogParser` are still compiled and run.
**How to avoid:** Search for all uses of `LogParser` in tests; annotate each test function.
**Warning signs:** `error: unsupported operation: Miri does not support file-backed memory mappings`.

### Pitfall 3: find_indicators_split Fix Breaks Legitimate Indicator Parsing

**What goes wrong:** After the fix, some records with real indicators return `body()` = full
content and `indicators()` = None.
**Why it happens:** `parse_indicators_from_bytes` has a bug or the indicator format is
unexpected (e.g., no space after value, missing parenthetical).
**How to avoid:** Run the full existing test suite after the fix. All existing tests that had
valid indicators must still pass.
**Warning signs:** `performance_metrics_full` or `indicators_*` tests fail after CORR-03 fix.

### Pitfall 4: CORR-01 Fix Breaks Empty File Handling

**What goes wrong:** `simd_from_utf8(&[][..])` is called on an empty mmap.
**Why it happens:** `&mmap[..]` on an empty mmap returns an empty slice.
**How to avoid:** `simd_from_utf8(b"")` returns `Ok("")` — empty slice is valid UTF-8. No
special handling needed. [VERIFIED: simdutf8 treats empty slice as valid UTF-8]

---

## Code Examples

### CORR-01 Fix (parser.rs)

```rust
// Source: src/parser.rs, from_path method
pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
    let file = File::open(path).map_err(|e| ParseError::IoError(e.to_string()))?;
    let mmap = unsafe { Mmap::map(&file).map_err(|e| ParseError::IoError(e.to_string()))? };

    // Scan entire file to eliminate misclassification from early-section sampling.
    let sample = &mmap[..];
    let encoding = if simd_from_utf8(sample).is_ok() {
        FileEncodingHint::Utf8
    } else {
        FileEncodingHint::Gb18030
    };

    Ok(Self { mmap, encoding })
}
```

### CORR-03 Fix (sqllog.rs)

```rust
// Source: src/sqllog.rs, find_indicators_split
fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    let len = data.len();
    let start = len.saturating_sub(256);
    let window = &data[start..];

    let mut earliest = window.len();
    for finder in [
        &*FINDER_REV_EXECTIME,
        &*FINDER_REV_ROWCOUNT,
        &*FINDER_REV_EXEC_ID,
    ] {
        if let Some(idx) = finder.rfind(window) {
            earliest = earliest.min(idx);
        }
    }

    let split = start + earliest;
    // Only accept split if the trailing slice contains parseable indicators.
    if split < len && parse_indicators_from_bytes(&data[split..]).is_none() {
        return len;
    }
    split
}
```

### Miri CI Job Annotation Pattern

```rust
// Tests that use LogParser (mmap): annotate with #[cfg(not(miri))]
#[test]
#[cfg(not(miri))]
fn probable_record_start_line_and_iterator_singleline_detection() {
    // ...
}
```

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust nightly | CORR-02 Miri | Yes | nightly-2026-04-19 | — |
| Miri component | CORR-02 | Yes (installed) | 0.1.0 | — |
| tempfile (dev-dep) | Existing tests | Yes | 3.27.0 | — |
| encoding (dep) | GB18030 tests | Yes | 0.2 | — |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | simdutf8 full-file scan adds <20ms overhead for 1GB files on CI | CORR-01 performance | If CI machine is much slower, from_path latency increases; benchmark regression possible |
| A2 | `parse_indicators_from_bytes` returning `None` reliably signals "not real indicators" | CORR-03 fix | If function has bugs with exotic indicator formats, fix may suppress valid splits |

---

## Open Questions

1. **CORR-03 edge case: perfectly-formed indicator syntax embedded in SQL**
   - What we know: `"WHERE col='EXECTIME: 5.0(ms)'"` will still be misclassified if no real
     indicators follow — `parse_indicators_from_bytes` will return `Some(...)` for this case
   - What's unclear: Is this scenario present in real DM log files?
   - Recommendation: Accept this limitation for now; document it. The fix eliminates the
     common case (non-numeric value after keyword). Phase 3's HOT-01 (early-exit on no `.`
     suffix) may naturally exclude many false positive cases.

2. **CORR-02: Should the Miri job run on schedule or only on PRs?**
   - What we know: Miri runs are slow (~2 min compile + ~2 min test for this codebase)
   - What's unclear: CI budget constraints
   - Recommendation: Run on push to main + pull_request (same as benchmark.yml pattern);
     no separate schedule needed.

---

## Sources

### Primary (HIGH confidence)
- `src/parser.rs` lines 37-43 — encoding detection sampling, verified live [VERIFIED]
- `src/sqllog.rs` lines 212-230 — `find_indicators_split` implementation [VERIFIED]
- `src/sqllog.rs` lines 242-280 — `decode_content_bytes` unsafe blocks [VERIFIED]
- `cargo miri test --test performance_metrics` output — 10/10 tests pass [VERIFIED]
- `MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test edge_cases` — confirms mmap failure [VERIFIED]
- `miri 0.1.0 (e22c616e4e 2026-04-19)` via `rustup run nightly cargo miri --version` [VERIFIED]
- memmap2 0.9.10 `src/stub.rs` — confirms no native Miri support, stub returns `Err(Unsupported)` [VERIFIED]
- Live bug reproduction: temporary `#[ignore]` test confirmed body truncation [VERIFIED]

### Secondary (MEDIUM confidence)
- simdutf8 throughput estimate (~50 GB/s) — from training knowledge; actual CI throughput may vary [ASSUMED → A1]

---

## Metadata

**Confidence breakdown:**
- CORR-01 fix: HIGH — one-line change, behavior fully traced
- CORR-02 Miri setup: HIGH — Miri installed, test runs verified
- CORR-03 fix: HIGH — bug confirmed, fix approach traced through code
- Performance impact estimates: MEDIUM — based on known simdutf8 characteristics

**Research date:** 2026-04-20
**Valid until:** 2026-05-20 (stable Rust ecosystem; Miri API is stable across nightly builds)

---

## RESEARCH COMPLETE
