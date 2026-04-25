# Feature Landscape: Rust Log Parser Throughput Optimization

**Domain:** High-performance line-oriented log parser (Rust, memory-mapped I/O)
**Researched:** 2026-04-18
**Baseline:** ~7.6 GB/s single-threaded counting, 674 µs/5 MB (synthetic uniform records)

---

## Current Architecture Snapshot

Before categorizing features it is essential to record what already exists, because the
baseline is unusually high. Many "table stakes" items for most parsers are already done.

| Hot Path | Mechanism | Status |
|----------|-----------|--------|
| File I/O | `memmap2` — zero-copy OS page cache | Done |
| UTF-8 validation | `simdutf8` file-level | Done |
| Newline scanning | `memchr(b'\n')` per line | Done — iterative |
| Timestamp check | 8-branch byte compare (23 bytes) | Done |
| Meta close search | `LazyLock<Finder>` for `") "` | Done |
| Indicators split | 3× `FinderRev` on last 256 bytes | Done |
| Parallel dispatch | `rayon::par_iter`, CPU-count chunks | Done — serial boundary scan |
| Field decoding | `Cow<'a, str>` zero-alloc on UTF-8 | Done |
| Lazy field parse | `parse_performance_metrics` on demand | Done |

At 7.6 GB/s on a single thread the bottleneck is almost certainly **branch misprediction +
memory bandwidth**, not missing SIMD. The research below is calibrated to this reality.

---

## Table Stakes

Features without which further optimization cannot be meaningfully measured or trusted.

| Feature | Why Essential | Complexity | Current Gap |
|---------|---------------|------------|-------------|
| **Flamegraph / perf profiling** | Cannot optimize what you cannot observe. At 7.6 GB/s every % matters; guessing is wasteful | Low | Not wired into CI or dev workflow |
| **Realistic benchmark corpus** | Synthetic uniform records hide multiline record cost and real-world branch distributions | Low | Bench uses synthetic single-line only |
| **`criterion` wall-time + throughput reporting** | Throughput (GB/s) must be reported, not just latency. `criterion::Throughput` already exists | Low | Bench only records `.count()` latency |
| **Regression gate in CI** | A 5% throughput regression gate prevents accidental regressions from touching hot paths | Medium | Not present |

### Table Stakes Detail

**Flamegraph / perf profiling**

`cargo-flamegraph` (wraps `perf` on Linux, `dtrace` on macOS) generates SVG flamegraphs
from a single command. `cargo flamegraph --bench parser_benchmark --bench-args
parse_sqllog_file_5mb` is enough. Needed before touching any code — at 7.6 GB/s the hot
loop is likely only a few hundred instructions deep and flamegraph will show exactly which
of `memchr`, timestamp check, or Cow construction dominates.

Dependency: none beyond `cargo-flamegraph` (installed once, not a `Cargo.toml` dep).
Confidence: HIGH — universal standard for Rust performance work.

**Realistic benchmark corpus**

The synthetic record is 206 bytes and single-line. Real DM logs contain:
- Multiline SQL (the `is_multiline` branch in `LogIterator::next`)
- Varying lengths (32 B to several KB)
- Occasional GB18030 records

Multiline records force extra `memchr` iterations in the hot loop. The current benchmark
cannot measure whether any multiline optimization is working. A 10–50 MB sample of real
log data in `benches/fixtures/` is required.

Confidence: HIGH — synthetic benchmarks misrepresent branch frequencies.

**`criterion::Throughput` reporting**

Change `group.throughput(Throughput::Bytes(file_size as u64))` — one line. Gives GB/s
directly in `cargo bench` output and in the saved `benchmarks/baseline.json`.

Confidence: HIGH — trivial, high value.

**Regression gate**

`cargo-criterion` or a custom CI step comparing `baseline.json` to current run.
The project already tracks `benchmarks/baseline.json`. A 5% threshold gate closes the
feedback loop without over-constraining experiments.

---

## Differentiators

Techniques with meaningful upside that require non-trivial implementation. Ordered by
estimated ROI (highest first) based on code analysis.

### D1 — Parallel Chunk-Boundary Pre-scan (HIGH ROI)

**What:** Move `find_next_record_start` off the critical path by pre-computing all chunk
boundaries concurrently before dispatching work to Rayon.

**Current problem:** `par_iter()` computes `num_threads` boundaries sequentially before
spawning. For a 4 GB file with 8 threads each boundary scan reads ~512 MB linearly — all
on the calling thread. This serializes the first ~8 ms of a large file parse.

**Approach:**
1. Divide file into `num_threads * 4` candidate positions.
2. Scatter boundary refinements as small Rayon tasks (each scans ≤ 4 KB).
3. Collect sorted boundary list, deduplicate, dispatch full slices.

**Complexity:** Medium — requires careful boundary deduplication.
**Win size:** Meaningful only on files > 100 MB; negligible on 5 MB synthetic bench.
**Dependency:** Builds on existing Rayon setup — no new crates.
Confidence: MEDIUM (ROI is file-size dependent).

### D2 — Vectorized Timestamp Validation (MEDIUM ROI)

**What:** Replace the 8-branch byte comparison in `LogIterator::next` (and
`find_next_record_start`) with a SIMD-based 23-byte comparison.

**Current code:** 8 individual `if byte[N] == b'X'` checks. Each is a branch, and
mispredictions are expensive when a log contains multiline records (many newlines that
are not record boundaries).

**Approach:** Load 23 bytes into a `u64` + `u32` + byte triple and mask against the
known separators (`-`, ` `, `:`, `.`) using bitwise AND + XOR. Or use `packed_simd2` /
`std::simd` (portable SIMD, stabilized in Rust 1.89 on nightly) to compare 16 bytes at
once.

Simpler alternative: pack the 8 positions into a single `u64` mask using `u64::from_ne_bytes`
and compare with a precomputed constant — zero branches, one integer compare.

```
positions:  4  7  10  13  16  19
bytes:      -  -  ' '  :   :   .

packed = buf[4] | buf[7]<<8 | buf[10]<<16 | buf[13]<<24 | buf[16]<<32 | buf[19]<<40
mask   = b'-'  | b'-'<<8   | b' '<<16    | b':'<<24    | b':'<<32    | b'.'<<40
if packed == mask { ... }
```

No new dependency. Single integer comparison replaces 8 branches.

**Complexity:** Low-Medium — straightforward refactor, easy to test.
**Win size:** ~5–15% on multiline-heavy files; negligible if records are all single-line.
Confidence: HIGH for the approach; MEDIUM for the actual gain magnitude.

### D3 — `find_indicators_split` Early-Exit Heuristic (MEDIUM ROI)

**What:** Before running 3 reverse SIMD finders on the last 256 bytes, check a single
cheap heuristic: if the last byte of `content_raw` is `b'.'` (which every record with
performance indicators ends with), skip the split entirely for records that lack it.

**Current code:** `find_indicators_split` always runs 3 `FinderRev::rfind` calls
regardless of whether the record has any indicators.

**Approach:**
```rust
fn find_indicators_split(&self) -> usize {
    let data = &self.content_raw;
    // Fast path: records without indicators never end with '.'
    if data.last() != Some(&b'.') {
        return data.len();
    }
    // ... existing 3-finder logic
}
```

No new dependency. Zero cost for records without indicators.

**Complexity:** Very Low — 2 lines of code.
**Win size:** Depends on the fraction of records without indicators. If 30% lack them,
this skips 3 SIMD searches per such record.
Confidence: HIGH — provably correct optimization.

### D4 — Single-Pass Record Boundary + Timestamp Detection (MEDIUM ROI)

**What:** In `LogIterator::next`, instead of calling `memchr(b'\n')` and then separately
checking 8 bytes of the next line, use `memchr::memmem::Finder` for `b"\n20"` — finding
newlines followed immediately by `"20"` (the century prefix shared by all DM timestamps).

**Current code:** Every `\n` triggers an 8-branch timestamp check. On a dense log with
rare multiline records this is `O(records)` comparisons — already fast. But `memmem` can
use SIMD to skip non-`\n` bytes in bulk, finding `\n20` in fewer CPU cycles than
iterating character by character.

**Limitation:** This only works if `20` never appears at the start of SQL body lines —
which is generally true for DM logs but cannot be guaranteed. The existing 8-byte check
is the correctness guard. After a `\n20` hit, still apply the remaining 6 separator
checks (they become cheap since false positives from `\n20` are rare).

**Complexity:** Medium — must handle the edge case of SQL bodies starting with `20`.
The correctness contract is delicate.
**Win size:** 10–25% on dense single-line record files.
Dependency: `memchr::memmem` already in Cargo.toml — no new crates.
Confidence: MEDIUM — the approach is sound; gain depends on pattern density.

### D5 — `#[inline(always)]` + `#[cold]` on Error Paths (LOW-MEDIUM ROI)

**What:** Annotate `parse_record_with_hint` and `find_indicators_split` with
`#[inline(always)]`. Annotate error branches (missing `(`, missing `)`, too-short lines)
with `#[cold]` / `[[unlikely]]` hints.

**Current code:** No explicit inlining annotations. The compiler may or may not inline
`parse_record_with_hint` across the call in `LogIterator::next`. Error branches are not
marked cold.

**Approach:**
- `#[inline(always)]` on `find_indicators_split` — it is called on every record.
- `[[cold]]` / `#[cold]` on every `return Err(ParseError::InvalidFormat {...})` arm.
- `#[inline]` (not always) on `parse_record_with_hint` — let LLVM decide after inlining
  hint given.

**Complexity:** Very Low — annotation only, no logic change.
**Win size:** 2–8%. Codegen-dependent; must be verified with flamegraph.
Confidence: MEDIUM — correct direction, magnitude uncertain without profiling.

### D6 — `mimalloc` as Default Global Allocator (LOW ROI for count workload)

**What:** Replace the system allocator with `mimalloc` globally.

**Current code:** `mimalloc` is already a dev-dependency (used in tests). The global
allocator is the system default.

**Relevance:** The hot path (`iter().count()`) allocates almost nothing — `Cow::Borrowed`
is zero-alloc. Allocator switching only helps when callers use `parse_performance_metrics`
and collect results into `Vec`. For the pure-counting benchmark this is a no-op.

**Complexity:** Very Low — `#[global_allocator]` attribute.
**Win size:** 0% on counting benchmarks; 5–15% on collect-to-Vec workloads.
**Dependency:** `mimalloc` already in dev-dependencies; needs to move to `dependencies`
if exposed as a library default — **avoid this for a library crate** (forces allocator
on users).
Confidence: HIGH — correct analysis; recommended only as an opt-in feature flag.

### D7 — Profile-Guided Optimization (PGO) (HIGH ROI, High Complexity)

**What:** Compile the binary with instrumentation, run on representative input, then
recompile with the profile data to guide branch prediction, inlining, and code layout.

**Approach:**
```bash
# Step 1: instrument build
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release
# Step 2: run on representative input
./target/release/... < real_log_file
# Step 3: merge profiles
llvm-profdata merge -output=/tmp/pgo.profdata /tmp/pgo-data/*.profraw
# Step 4: recompile with profile
RUSTFLAGS="-Cprofile-use=/tmp/pgo.profdata" cargo build --release
```

**Relevance:** PGO is most effective when branch frequencies are heavily skewed — exactly
the case here (nearly all lines pass the timestamp check; multiline records are rare;
error paths are nearly never taken). PGO can convert those cold branches into layout-friendly
code and may improve branch predictor accuracy by 5–20%.

**Complexity:** Medium — requires a representative input corpus (links to D from table
stakes). Adds a multi-step build process; not automated yet.
**Win size:** 5–20% on realistic workloads. Gains are higher the more diverse the input.
Confidence: HIGH for the technique; MEDIUM for the gain magnitude without profiling.

### D8 — BOLT Post-Link Optimization (LOW ROI, High Complexity)

**What:** LLVM BOLT rewrites the binary after linking to optimize instruction cache layout
based on runtime profiles. It reorders basic blocks to minimize branch target distance.

**Relevance:** At 7.6 GB/s the parser is almost certainly i-cache bound or branch
predictor bound. BOLT can improve i-cache utilization by 5–15% on top of PGO.

**Complexity:** High — requires BOLT installation (`llvm-bolt`), separate profile
collection step, binary rewriting. No standard Cargo integration exists as of 2025.
Works best on binaries (not library benchmarks); applying to a library benchmark requires
a thin wrapper binary.

**Win size:** 5–15% on top of PGO. Combined PGO+BOLT can approach 30% over baseline.
**Dependency:** LLVM BOLT (external tool, not a Rust crate).
Confidence: MEDIUM — technique is proven (used by Meta on production binaries); Rust
support is less mature than C++.

### D9 — Chunk Prefetch via `madvise(MADV_SEQUENTIAL)` (LOW ROI)

**What:** Call `madvise` on the mmap'd region to advise the OS kernel of sequential
access patterns, triggering aggressive read-ahead.

**Current code:** `memmap2::Mmap` does not call `madvise` by default.

**Approach:** Use `memmap2::MmapOptions::populate()` or call `mmap.advise(Advice::Sequential)`
via the `memmap2` API.

**Complexity:** Very Low — one method call.
**Win size:** Near-zero for files that fit in OS page cache (the 5 MB synthetic bench);
meaningful (5–15%) for cold reads of multi-GB files on Linux.
Confidence: HIGH for the mechanism; win depends on whether the benchmark measures cold
or warm reads.

---

## Anti-Features

Features to explicitly NOT build. Building these would consume effort with negative
or near-zero return given the current baseline and architecture.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Custom SIMD via `packed_simd2` / raw intrinsics for newline scan** | `memchr` already uses architecture-optimal SIMD (AVX2, NEON). Writing custom intrinsics duplicates this work and adds maintenance burden | Trust `memchr`; tune the timestamp check that follows each hit (D2) |
| **Per-record heap allocation in hot path** | Any `Box`, `String::new`, or `Vec::new` in `LogIterator::next` would immediately cap throughput below 1 GB/s | The existing `Cow::Borrowed` zero-alloc path is correct; do not add owned allocations |
| **Async / tokio integration** | mmap I/O is synchronous by nature; wrapping in async adds scheduling overhead with zero I/O benefit | Use Rayon for CPU parallelism; keep I/O synchronous |
| **regex crate for timestamp detection** | `regex` initialization cost and general-purpose DFA overhead would regress the timestamp check from 8 branches to tens of branches | Keep the hand-written 8-branch check (or improve to D2) |
| **Serde derive on `Sqllog`** | Derives force eager string allocation on every field; incompatible with the `Cow<'a, str>` lifetime model | Callers who need serialization should extract fields explicitly |
| **GB18030 hot-path optimization** | GB18030 records are rare (1% of production files); deep optimization gives < 1% overall throughput gain | Keep current fallback; document that UTF-8 is the fast path |
| **Lock-free concurrent data structures for record output** | Rayon's work-stealing is already lock-efficient; adding a crossbeam channel or dashmap adds contention overhead | Collect into `Vec` per chunk; merge in the calling thread |
| **Streaming / chunk-by-chunk file reading** | mmap gives OS-managed page-in; streaming would require double-buffering and coordination — more complex, slower | Keep mmap; tune `madvise` (D9) instead |

---

## Feature Dependencies

```
Table Stakes (must complete first)
    flamegraph setup
    realistic corpus
    criterion::Throughput
        ↓
D3 (early-exit heuristic)       ← independent, implement first
D5 (#[inline] / #[cold])        ← independent, implement second
D2 (vectorized timestamp)       ← depends on corpus to measure
D9 (madvise)                    ← independent, one-liner
D4 (memmem \n20 scan)           ← depends on corpus + D2 (validate correctness)
D1 (parallel boundary pre-scan) ← depends on realistic large file corpus
D7 (PGO)                        ← depends on realistic corpus + stable hot path
D8 (BOLT)                       ← depends on D7 completed
D6 (mimalloc feature flag)      ← independent, low priority
```

---

## MVP Recommendation

**Phase 1 — Measurement Infrastructure (zero risk, required first)**
1. Wire `criterion::Throughput` — 30 minutes.
2. Add realistic multiline fixture to `benches/fixtures/` — 1 hour.
3. Set up `cargo-flamegraph` and profile against real file.
4. Add CI regression gate comparing to `baseline.json`.

**Phase 2 — Zero-Risk Hot-Path Wins (implement in order)**
1. D3: Early-exit in `find_indicators_split` — 2 lines, verifiable by inspection.
2. D5: `#[inline(always)]` + `#[cold]` annotations — audit and add.
3. D9: `mmap.advise(Advice::Sequential)` — 1 line.

**Phase 3 — Medium-Complexity Wins (validate with flamegraph first)**
4. D2: Packed integer timestamp comparison — replace 8 branches, benchmark.
5. D4: `memmem` for `\n20` boundary detection — benchmark on multiline corpus.

**Phase 4 — Large Investments (only if Phase 3 leaves room)**
6. D7: PGO build pipeline — needs representative corpus from Phase 1.
7. D1: Parallel boundary pre-scan — only if large-file (> 100 MB) use case is primary.
8. D8: BOLT — only after PGO; diminishing returns.

**Defer indefinitely:** D6 (mimalloc as default) — wrong for a library crate.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Table stakes list | HIGH | These are universally required for serious perf work |
| D3 early-exit | HIGH | Correctness provable by inspection; gain is data-dependent |
| D2 packed timestamp | HIGH | Standard technique; branch count reduction is measurable |
| D5 inline/cold hints | MEDIUM | Compiler may already do this; verify with codegen output |
| D4 memmem boundary | MEDIUM | Correctness edge case (SQL body starting with "20") needs care |
| D1 parallel boundary | MEDIUM | Win is file-size dependent; minimal on small synthetic bench |
| D7 PGO | HIGH for technique; MEDIUM for gain | Well-understood; gain varies by branch profile |
| D8 BOLT | MEDIUM | Rust BOLT support less mature than C++ as of 2025 |
| D9 madvise | HIGH for mechanism; LOW for win on warm cache | OS-dependent |
| Anti-features list | HIGH | Each avoidance is grounded in architectural constraints |

---

## Sources

- Code analysis: `/Users/guang/Projects/dm-database-parser-sqllog/src/parser.rs`,
  `src/sqllog.rs`, `benches/parser_benchmark.rs`, `Cargo.toml`
- Project context: `.planning/PROJECT.md` (baseline: 674 µs / 5 MB ≈ 7.6 GB/s)
- `memchr` crate: uses architecture-optimal SIMD (Teddy algorithm for multi-pattern,
  PCMPESTRI / NEON for single-byte); trusting its newline scan is correct
- PGO technique: LLVM documentation, rustc reference (`-Cprofile-generate`,
  `-Cprofile-use`); confidence HIGH
- BOLT: LLVM project documentation; Rust support via `cargo-pgo` ecosystem; confidence MEDIUM
- `memmap2::MmapOptions` API: `advise()` method supports `libc::MADV_SEQUENTIAL` on Linux
- Branch-elimination via packed integer comparison: established technique in
  high-performance C/C++ parsing (simdjson, RapidJSON); directly applicable in Rust
  with `u64::from_ne_bytes`
