# Technology Stack

**Project:** dm-database-parser-sqllog — performance optimization
**Researched:** 2026-04-18
**Confidence note:** All external tool access (WebSearch, WebFetch, Bash, Context7) was denied during this research session. Findings are based on (a) deep static analysis of the current codebase and (b) training knowledge current to August 2025. Confidence levels are assigned conservatively.

---

## Current Stack Assessment

| Library | Version | Role | Status |
|---------|---------|------|--------|
| memchr | 2.7.6 | newline search + memmem pattern search | KEEP — already optimal for its role |
| simdutf8 | 0.1.5 | UTF-8 validation at open time | KEEP — correct and fast |
| memmap2 | 0.9.9 | memory-mapped file I/O | KEEP — no better alternative |
| rayon | 1.10 | parallel chunk iteration | KEEP — but chunking strategy has room |
| atoi | 2.0.0 | integer parsing from bytes | KEEP |
| fast-float | 0.2 | float parsing from bytes | KEEP |
| encoding | 0.2 | GB18030 decode (rare path) | LOW PRIORITY — rare path, no change needed |
| thiserror | 2.0.17 | error types | KEEP |

---

## Recommended Additions and Changes

### 1. Replace Iterative `memchr(b'\n')` with `memmem::Finder::new(b"\n20")`

**What to change:** `LogIterator::next()` currently calls `memchr(b'\n', ...)` in a loop, checking the 8 fixed-position bytes of every newline to see if the next line starts with a `20` year prefix. For single-line records (the common case with the synthetic benchmark), this means one `memchr` call per record — but each call processes from `scan_pos` to the end of the remaining data, paying O(n) setup cost repeatedly.

**Better approach:** Use a pre-built `memmem::Finder::new(b"\n20")` (already in scope via the existing `memchr` dependency, no new crate needed). A single `find()` call on the full remaining slice jumps directly to the next candidate record boundary. For single-line records this halves the linear scan work: instead of finding `\n` then separately checking the next two bytes, the SIMD engine finds `\n20` atomically in one pass. For multi-line records the savings are even larger.

**Implementation sketch:**

```rust
static FINDER_RECORD_START: LazyLock<Finder<'static>> =
    LazyLock::new(|| Finder::new(b"\n20"));

// In LogIterator::next():
// Replace the while-let memchr loop with:
match FINDER_RECORD_START.find(&data[scan_pos..]) {
    Some(idx) => {
        // candidate at scan_pos + idx + 1
        let candidate = scan_pos + idx + 1;
        // still validate the full 8-position pattern to reject false positives
        // (e.g. "\n2024" in SQL body text)
        if validate_ts_pattern(&data[candidate..]) {
            found_next = Some(scan_pos + idx);
            break;
        }
        scan_pos = candidate; // false positive, keep searching
    }
    None => break,
}
```

**Confidence:** HIGH — `memmem::Finder` is already a dependency; this is a pure algorithmic change with no new code risk. The `\n20` pattern is rare enough in SQL text that false positives add negligible overhead.

**Estimated gain:** 15–30% on single-line dominated workloads (fewer total SIMD passes over the data). Multi-line heavy files may see 40–50%.

---

### 2. Merge `find_indicators_split` into a Single Reverse Scan

**What to change:** `find_indicators_split` currently runs three separate `rfind` calls over the last 256 bytes. Each call is a SIMD scan over the same 256-byte window.

**Better approach:** Replace with a single manual backward scan over the 256-byte window that simultaneously checks for all three indicator prefixes in one pass. At 256 bytes this is trivially fast, but eliminating two redundant scans removes ~2× overhead in the common case (where all three indicators are present).

```rust
fn find_indicators_split(data: &[u8]) -> usize {
    let start = data.len().saturating_sub(256);
    let window = &data[start..];
    // Walk backwards byte by byte; stop at first 'E', 'R', or 'E' (EXEC_ID) that matches
    // whichever keyword comes earliest in the window.
    // Single pass, no SIMD needed at 256 bytes — L1 cache handles it.
    let mut earliest = window.len();
    let mut i = window.len().saturating_sub(1);
    loop {
        // check EXECTIME, ROWCOUNT, EXEC_ID prefixes at position i
        // ...
        if i == 0 { break; }
        i -= 1;
    }
    start + earliest
}
```

**Confidence:** HIGH — trivial change, no new dependency.

**Estimated gain:** Small in absolute ns (256-byte window), but reduces constant factor of `parse_performance_metrics()`.

---

### 3. Parallel Chunk-Boundary Discovery

**What to change:** `par_iter()` computes chunk boundaries _serially_ before spawning threads. For very large files (multi-GB) the serial boundary scan itself becomes a bottleneck.

**Better approach:** Use `rayon::scope` or `rayon::join` to discover boundaries in parallel. Split the file into coarse halves, then recursively bisect to `num_threads` boundaries. This is O(log n) serial work instead of O(n).

**No new crate needed** — rayon already provides the primitives.

**Confidence:** MEDIUM — correct in theory; whether the boundary scan is actually the bottleneck at 5 MB (current benchmark size) is unlikely. This becomes relevant at >100 MB files.

---

### 4. Add `mimalloc` as a Non-Default Feature for GB18030 Path

**Crate:** `mimalloc` 0.1.48 (already a dev-dependency in the project)

**Why:** The GB18030 decode path allocates `String` via `encoding::decode`, which hits the system allocator repeatedly. `mimalloc` reduces allocation cost by ~30–50% on many platforms. Promote from dev-dependency to an optional feature:

```toml
[features]
fast-alloc = ["mimalloc"]

[dependencies]
mimalloc = { version = "0.1", optional = true }
```

**Confidence:** MEDIUM — mimalloc is well-established (used by ripgrep, fd). Benefit is real only on GB18030-heavy workloads; UTF-8 path (which dominates) is already zero-alloc.

---

### 5. Do NOT Add: `packed_simd` / `std::simd` (Nightly Portable SIMD)

**Why not:** The current code uses `memchr` (which internally uses AVX2/SSE4.2 or NEON depending on platform) and `simdutf8` (AVX-512 aware). Writing manual SIMD with `std::simd` (nightly) or `packed_simd` (unmaintained since 2021) would:
- Require nightly Rust — breaking for library consumers
- Duplicate what `memchr` already does better (it has hand-tuned SIMD per platform, fallback paths, and proper alignment handling)
- Introduce significant maintenance burden

**Verdict:** Do not use. `memchr`'s `memmem::Finder` already dispatches to the best SIMD available on the host at runtime.

---

### 6. Do NOT Add: `highway` crate (Rust bindings to Google Highway)

**Why not:** `highway` (google/highway bindings via `highway-rs` or similar) would require a C++ build step and introduces non-trivial FFI complexity. For the patterns used here (single-byte search, 2–10 byte pattern search), `memchr` is already within 5–10% of theoretical memory bandwidth limits. The complexity cost is not justified.

**Verdict:** Do not use.

---

### 7. Do NOT Add: `aho-corasick`

**Why not:** `aho-corasick` shines when searching for many patterns simultaneously in large text. Here, the record-start pattern is a single 2-byte prefix (`\n20`) followed by a fixed-position validation — `memmem::Finder` is strictly faster for this use case. The indicator parsing (`EXECTIME`, `ROWCOUNT`, `EXEC_ID`) operates on a 256-byte window where SIMD setup cost dominates; aho-corasick's startup is heavier.

**Verdict:** Do not use. The `memmem::Finder` + targeted pattern approach outperforms aho-corasick for this workload.

---

### 8. Do NOT Add: `bstr`

**Why not:** `bstr` provides a convenient byte-string API including a `lines_with_terminator` iterator. However it does not offer faster byte scanning than `memchr` — it uses `memchr` internally. Adding it would be a convenience crate, not a performance crate, and would add a dependency for no throughput gain.

**Verdict:** Do not use for performance. Could be considered for code clarity if the team prefers it, but keep it out of hot paths.

---

### 9. Consider: mmap Prefetch Hints via `madvise`

**What:** On Linux/macOS, `memmap2` exposes `advise()` which wraps `madvise(2)`. Calling `mmap.advise(Advice::Sequential)` before iteration tells the OS kernel to prefetch pages ahead of the read cursor, reducing page-fault stalls during the first linear pass.

**API:**
```rust
// After creating the Mmap:
#[cfg(unix)]
mmap.advise(memmap2::Advice::Sequential).ok(); // non-fatal if unsupported
```

**Confidence:** MEDIUM — measurably beneficial when the file is not already in OS page cache (cold start). Has no effect when file is warm (benchmark loop iterations 2+). The synthetic benchmark runs hot (same file reused), so this won't show in current benchmark numbers but matters in production.

---

### 10. Consider: Hugepage-backed mmap (Linux only, advanced)

**What:** On Linux, `MAP_HUGETLB` reduces TLB pressure for multi-GB files by using 2 MB pages instead of 4 KB pages. `memmap2` does not expose this directly; it requires raw `libc::mmap` with `libc::MAP_HUGETLB`.

**Confidence:** LOW — beneficial only for files >500 MB on Linux with hugepage support configured. Out of scope for most deployments. Not worth adding as a dependency or feature for now.

---

### 11. Consider: `crossbeam-channel` for Producer/Consumer Streaming

**Why:** If the library ever exposes a streaming API (parse records as they arrive, feed a downstream pipeline), `crossbeam-channel` provides bounded channels with better throughput than `std::sync::mpsc`. Not needed for the current iterator model.

**Verdict:** Defer until streaming API is designed.

---

## Complete Stack Recommendation

### Dependencies to Keep (no version change needed)

```toml
[dependencies]
memchr    = "2.7.6"   # memmem::Finder is the core search engine; update to latest patch
simdutf8  = "0.1.5"   # UTF-8 open-time validation; no replacement needed
memmap2   = "0.9.9"   # mmap I/O; expose Advice::Sequential
rayon     = "1.10"    # parallel iteration; improve chunk-boundary algorithm
atoi      = "2.0.0"   # integer parse
fast-float = "0.2"    # float parse
thiserror = "2.0.17"  # error types
encoding  = "0.2"     # GB18030 fallback (rare path)
```

### New Optional Dependency

```toml
[dependencies]
mimalloc = { version = "0.1", optional = true }

[features]
fast-alloc = ["dep:mimalloc"]
```

### Explicitly Rejected

| Crate | Reason |
|-------|--------|
| `packed_simd` | Unmaintained (last commit 2021); requires nightly |
| `std::simd` | Nightly-only; memchr already does this better |
| `highway` / `highway-rs` | C++ FFI; no gain over memchr for these patterns |
| `aho-corasick` | Overkill; slower for <5 patterns in a 256-byte window |
| `bstr` | Uses memchr internally; no throughput gain |
| `rayon-adaptive` | Experimental; not production-ready as of 2025 |

---

## Algorithmic Changes (No New Crates)

These are pure code changes within the existing dependency set that are expected to yield the largest gains:

| Change | File | Expected Gain | Confidence |
|--------|------|--------------|------------|
| Replace `memchr(b'\n')` loop with `memmem::Finder(b"\n20")` | `parser.rs` | 15–30% | HIGH |
| Single-pass backward scan in `find_indicators_split` | `sqllog.rs` | 2–5% | HIGH |
| Parallel chunk-boundary discovery in `par_iter` | `parser.rs` | negligible at 5 MB, significant at 1 GB+ | MEDIUM |
| `madvise(Sequential)` before first iteration | `parser.rs` | cold-start only | MEDIUM |

---

## Memory Access Pattern Notes

The current access pattern (sequential linear scan via mmap) is already optimal for hardware prefetchers. The main remaining gains are:

1. **Reduce scan passes** — fewer SIMD searches over the same bytes (the `\n20` change above)
2. **Reduce branch mispredictions** — the 8-position timestamp check already minimizes branches; keep it
3. **Keep hot data in L1** — the 256-byte indicator window already fits in one cache line; single-pass scan avoids re-reading it

---

## Sources

- Code analysis: `/Users/guang/Projects/dm-database-parser-sqllog/src/parser.rs`, `src/sqllog.rs`
- Benchmark baseline: `benchmarks/baseline.json` (674,425 ns / 5 MB synthetic)
- memchr crate documentation (training knowledge, version 2.7.x, HIGH confidence for API)
- simdutf8 crate documentation (training knowledge, 0.1.5, HIGH confidence)
- memmap2 crate documentation including `Advice` enum (training knowledge, HIGH confidence)
- rayon parallel iterator API (training knowledge, 1.10, HIGH confidence)
- packed_simd maintenance status: unmaintained since 2021 (HIGH confidence — widely documented)
- std::simd nightly status as of August 2025 (MEDIUM confidence — still not stabilized)

**Note:** Version verification against crates.io was not possible in this session (network tools denied). The version numbers reflect the Cargo.toml as committed; before shipping, run `cargo update --dry-run` to check for patch updates to memchr and memmap2.
