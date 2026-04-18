# Domain Pitfalls: Rust Parser Throughput Optimization

**Domain:** High-performance Rust byte-stream parser (mmap + SIMD + zero-copy)
**Project:** dm-database-parser-sqllog
**Researched:** 2026-04-18
**Confidence:** HIGH (based on direct code inspection + Rust SIMD/perf domain knowledge)

---

## Critical Pitfalls

Mistakes that invalidate benchmark results or require architecture rewrites.

---

### Pitfall 1: Synthetic Benchmark Masks Real-World Performance Collapse

**What goes wrong:**
The benchmark generates records of identical length (206 bytes, single-line, no multiline,
uniform `[SEL]` tag, always UTF-8). Real log files have:
- Variable SQL length: short DDL (`ALTER SESSION`) to multi-KB stored procedure bodies
- Multiline records: SQL with embedded newlines causes `is_multiline = true`, triggering an
  extra `memchr(b'\n')` pass inside `parse_record_with_hint`
- Mix of `is_multiline = true` (slow path) and `false` (fast path) — synthetic data always
  hits the fast path
- GB18030 content triggers heap allocation via `encoding::all::GB18030.decode()` — absent
  from synthetic data

The current baseline (7.6 GB/s on 5 MB synthetic) almost certainly overstates throughput on
production log files by 2–5x. Throughput improvements measured only against synthetic data
may not transfer.

**Why it happens:**
Synthetic data is a best-case input: uniform length enables perfect branch prediction, no GB18030
path is exercised, and `is_multiline` is always `false` (single-line records). The OS also
trivially prefetches 5 MB; TLB pressure doesn't appear at this scale.

**Consequences:**
- Optimization work targets the wrong hot path
- Regression introduced in the multiline path goes undetected
- Throughput claims in documentation/README are misleading

**Warning signs:**
- `parse_sqllog_file_5mb` benchmark time drops significantly, but profiling on a real log file
  shows no change or regression
- `perf stat` shows high branch-misprediction rates on real files but not on benchmarks
- `is_multiline` codepath has zero coverage in benchmark runs

**Prevention:**
1. Add a second synthetic generator that creates mixed-length records (10 B to 4 KB SQL bodies)
   with ~20% multiline rate and ~5% GB18030 records, mirroring real DM log distributions
2. Run both "uniform" and "realistic" benchmarks; treat the realistic one as the primary metric
3. If a real log file (`sqllogs/dmsql_DSC0_20250812_092516.log`) is available, gate CI on it

**Phase:** Address in the first optimization phase, before any hot-path changes are made.
Benchmark validity is a prerequisite for knowing whether any optimization actually helps.

---

### Pitfall 2: Unsafe `from_utf8_unchecked` Relies on a File-Level Invariant That Can Break

**What goes wrong:**
`parse_record_with_hint` and `decode_content_bytes` use `str::from_utf8_unchecked` on
sub-slices of the mmap when `encoding_hint == FileEncodingHint::Utf8`. The safety argument is:
"the file was validated as UTF-8 during `from_path` by sampling the first 64 KB."

This invariant has two silent holes:

1. **Partial sample:** Only the first 65,536 bytes are sampled. A file that starts with valid
   UTF-8 but contains GB18030 multi-byte sequences after byte 65536 will be misclassified as
   UTF-8. `from_utf8_unchecked` on those bytes produces undefined behavior (misaligned `char`
   boundaries cause memory corruption or panics in downstream string operations).

2. **Record boundary vs. file boundary:** `parse_record` (the public standalone function, not
   `parse_record_with_hint`) passes `FileEncodingHint::Auto` unconditionally. But internally,
   when the Auto path runs `simd_from_utf8` and succeeds, it also uses `from_utf8_unchecked`
   via `slice::from_raw_parts` to "re-borrow" the slice with lifetime `'a`. The SAFETY comment
   says the caller guarantees the lifetime, but `parse_record` accepts an arbitrary `&'a [u8]`
   from the caller — there is no mmap involved, so the lifetime `'a` may not actually outlive
   the returned `Sqllog<'a>`. If the caller drops the input buffer while holding the `Sqllog`,
   the borrowed `Cow` becomes a dangling reference.

**Why it happens:**
The 64 KB sampling is a pragmatic heuristic borrowed from encoding-detection libraries. The
unsafe re-borrow pattern (using `slice::from_raw_parts` to smuggle a sub-slice lifetime) is
subtle and not caught by the borrow checker because the `unsafe` block explicitly opts out.

**Consequences:**
- Silent memory corruption or panic on GB18030 files larger than ~64 KB (real production files
  are typically 50–500 MB)
- Undefined behavior when `parse_record` is called from a context where the buffer is
  stack-allocated or short-lived (violates the undocumented lifetime contract)
- Rust's Miri will flag both issues if run against the affected code paths

**Warning signs:**
- Panics or garbled output only on large GB18030 log files
- `parse_record` test with a stack-allocated buffer that is dropped before accessing the
  returned `Sqllog` fields — no test currently covers this

**Prevention:**
1. Extend encoding detection: after the 64 KB sample, scan a random 64 KB window near the
   middle and end of the file, or validate the full file asynchronously
2. For `FileEncodingHint::Utf8`, validate per-record (not per-file) — `simdutf8` is fast
   enough that the overhead is negligible compared to mmap page faults
3. For `parse_record` (public API), remove the `from_raw_parts` lifetime extension and return
   `Sqllog<'_>` with a safe `Cow::Borrowed` only after a confirmed `from_utf8` check, or
   document the lifetime contract with a `# Safety` section visible to callers
4. Add Miri CI job: `cargo +nightly miri test` catches the dangling-reference scenario

**Phase:** Address before any performance work that touches the unsafe decode paths.
Correctness trumps throughput.

---

## Moderate Pitfalls

---

### Pitfall 3: memchr on ARM Does Not Get AVX2 — Throughput Gap Is Real but Manageable

**What goes wrong:**
`memchr` 2.x uses runtime CPUID dispatch on x86/x86_64 to select SSE2, AVX2, or scalar paths.
On Apple Silicon (ARM64/AArch64) it uses NEON via compile-time feature detection
(`target_feature = "neon"`). The throughput difference matters:

- x86 AVX2: processes 32 bytes/cycle for `memchr`, ~64 bytes/cycle for `memmem` (two-way search)
- ARM NEON: processes 16 bytes/cycle for `memchr`; `memmem` uses a different algorithm

At 7.6 GB/s on Apple Silicon, the parser is likely memory-bandwidth-bound, not SIMD-compute-bound.
But on Linux/x86 CI servers (the deployment target for most users), AVX2 is available only if
`RUSTFLAGS="-C target-cpu=native"` or `target-feature=+avx2` is set. The default `cargo bench`
on CI uses `target-cpu=generic`, which disables AVX2 dispatch in `memchr` and halves throughput.

**Why it happens:**
`memchr`'s runtime dispatch requires the AVX2 path to be compiled in. Without
`target-feature=+avx2`, the AVX2 codegen is absent and the runtime check falls through to SSE2.

**Consequences:**
- CI benchmarks (generic target) run 1.5–2x slower than developer benchmarks (native target)
- Benchmark comparisons across machines are apples-to-oranges without explicit target-feature flags
- A "regression" in CI might just be a CI runner that switched from AVX2 to SSE2

**Warning signs:**
- Benchmark results on developer machine (Apple M-series) differ from CI benchmark by >50%
- `cargo bench` and `RUSTFLAGS="-C target-cpu=native" cargo bench` produce very different numbers
  on the same x86 machine

**Prevention:**
1. In `benchmarks/baseline.json` and CI, record the target CPU and SIMD features alongside
   the benchmark number
2. Set `RUSTFLAGS="-C target-cpu=native"` in the CI benchmark job explicitly and document this
3. Do not compare absolute throughput numbers across architectures; use ratio-to-baseline instead
4. Verify ARM NEON is active: `RUSTFLAGS="-C target-feature=+neon" cargo bench` on ARM

**Phase:** Document before publishing throughput claims; enforce in CI benchmark job.

---

### Pitfall 4: mmap TLB Pressure Appears Only on Large Files — 5 MB Benchmark Hides It

**What goes wrong:**
The synthetic benchmark uses a 5 MB tempfile. At 5 MB with 4 KB pages, the OS needs ~1,280
TLB entries. Most modern CPUs have 1,024–4,096 L2 TLB entries — the file fits. A real 200 MB
log file needs ~51,200 entries; the OS must refill the TLB continuously (TLB thrashing).

Additionally, `mmap` on macOS (where development happens) uses 16 KB pages by default on Apple
Silicon; Linux uses 4 KB. This changes the TLB entry count by 4x, meaning a benchmark on macOS
may not expose TLB pressure that exists on the Linux deployment target.

Hugepages (2 MB on x86, 2 MB on arm64) reduce TLB entries by 512x but require either
`madvise(MADV_HUGEPAGE)` on Linux or `MAP_HUGETLB` at mmap time. `memmap2` does not enable
hugepages by default.

**Why it happens:**
5 MB is below the TLB thrashing threshold on all common CPUs. Developers never see the problem
in benchmarks.

**Consequences:**
- Measured 7.6 GB/s drops to 2–4 GB/s on 200 MB+ real log files due to page fault and TLB
  miss overhead
- Hugepage optimization looks unnecessary in benchmarks but is significant in production

**Warning signs:**
- `perf stat` shows high `dTLB-load-misses` or `page-faults` when running on large files
- Throughput per unit time decreases as file size increases (should be flat if memory-bandwidth-bound)
- `vmstat` shows elevated `si`/`so` during parse runs on large files

**Prevention:**
1. Add a 256 MB synthetic benchmark in addition to the 5 MB one; observe if throughput drops
2. On Linux, add `madvise(MADV_SEQUENTIAL)` via `memmap2::MmapOptions::populate()` or
   `Mmap::advise(memmap2::Advice::Sequential)` to trigger readahead and reduce page fault stalls
3. Evaluate `madvise(MADV_HUGEPAGE)` via `Mmap::advise(memmap2::Advice::HugePage)` on Linux;
   measure TLB miss reduction with `perf stat`
4. Do not benchmark only on macOS ARM; always include a Linux x86 run on large files

**Phase:** Profile before optimizing. Add large-file benchmark first, then evaluate madvise.

---

### Pitfall 5: Rayon `par_iter` Thread Overhead Dominates for Small Files

**What goes wrong:**
`par_iter()` splits the file into `rayon::current_num_threads()` chunks unconditionally.
On a machine with 8–16 threads, a 5 MB file gets split into 8–16 chunks of ~312–625 KB each.
The Rayon work-stealing overhead (task queue push, thread wakeup, join synchronization) costs
roughly 1–10 µs per thread. For a 5 MB file parsed in ~674 µs single-threaded, adding 16
threads adds ~16–160 µs of overhead — that is 2–24% pure overhead before any parallel speedup.

The chunk-boundary scan (`find_next_record_start`) is serial and runs on the calling thread
before work is dispatched. This scan itself iterates `num_threads` times over the file to find
record boundaries, adding an O(n) serial prefix that limits Amdahl's law scaling.

**Why it happens:**
The current `par_iter` implementation does not check file size or record count before parallelizing.
Rayon's thread pool is always fully activated regardless of input size.

**Consequences:**
- `par_iter` on files <10 MB may be slower than `iter` due to thread overhead
- The boundary-scan serial prefix limits parallel speedup to approximately 2–3x even on 16 cores
  for typical log files (Amdahl's law with a serial prefix proportional to `num_threads * record_size`)

**Warning signs:**
- `par_iter().count()` is not faster than `iter().count()` on the 5 MB benchmark
- Parallel speedup plateaus at 2–3x regardless of thread count
- `perf record` shows significant time in Rayon thread-pool overhead functions

**Prevention:**
1. Add a size threshold: if `data.len() < threshold` (e.g., 32 MB), fall back to serial `iter()`
   internally inside `par_iter`, or document that `par_iter` is intended for large files only
2. Move the boundary scan into the parallel work: each thread can scan its own rough chunk
   boundary without a serial pre-scan
3. Benchmark `par_iter` against `iter` explicitly at 1 MB, 10 MB, 100 MB, and 500 MB file sizes
   to establish the crossover point

**Phase:** Implement after the single-threaded hot-path is optimized; parallel speedup multiplies
single-threaded gains.

---

### Pitfall 6: False Sharing Between Rayon Threads on Chunk Boundaries

**What goes wrong:**
When `par_iter` splits `data: &[u8]` into chunks, each thread operates on a sub-slice of the
mmap. The chunk boundaries (start and end of each sub-slice) are computed to align to record
boundaries (via `find_next_record_start`), but they are NOT guaranteed to be aligned to cache
line boundaries (64 bytes on x86/ARM). The last cache line of one thread's chunk and the first
cache line of the next thread's chunk overlap in the same 64-byte cache line.

If a thread writes any mutable state adjacent to these boundaries (e.g., a statistics counter,
an output buffer, or even a local `Vec` that references the slice metadata), the hardware
coherence protocol will serialize those writes across cores, stalling all threads.

In the current implementation, `LogIterator` itself is read-only (no writes to the mmap).
However, if a future optimization introduces per-chunk output buffers (`Vec<Sqllog>`) allocated
close together in memory, or per-thread counters, false sharing can appear.

**Why it happens:**
False sharing is invisible in single-threaded benchmarks and difficult to detect without
`perf c2c` or Intel VTune. Developers often add per-thread state without considering cache
line alignment.

**Consequences:**
- Parallel speedup degrades unpredictably as thread count increases (often looks like memory
  bandwidth saturation but is actually coherence traffic)
- Symptoms appear only on NUMA machines or high core-count CPUs

**Warning signs:**
- `perf c2c` (Linux) shows high "LLC load misses" on shared cache lines near chunk boundaries
- Parallel throughput drops when pinning threads to separate NUMA nodes
- Adding `#[repr(align(64))]` to per-thread state structures improves throughput

**Prevention:**
1. If adding per-thread state (counters, output buffers), pad with `#[repr(align(64))]` or
   use `crossbeam::CachePadded`
2. Ensure chunk boundaries are at least 64-byte aligned (round up `boundary` to the next
   multiple of 64, then re-align to the next record start)
3. Run `perf c2c` as part of the parallel benchmark validation step

**Phase:** Address during parallel optimization phase; not a current concern for the serial path.

---

### Pitfall 7: SIMD Intrinsics on Stable Rust — What Is and Is Not Available

**What goes wrong:**
Rust's portable SIMD API (`std::simd`) is nightly-only as of Rust 1.85 (stable). Platform
intrinsics (`std::arch::x86_64::_mm256_*`, `std::arch::aarch64::*`) are stable but require
`unsafe` and manual `#[cfg(target_feature)]` guards. There is no stable, cross-platform SIMD
abstraction that works on both x86 AVX2 and ARM NEON without conditional compilation.

The project currently avoids raw intrinsics by delegating to `memchr` and `simdutf8`, which
handle SIMD internally. If a future optimization attempts to write custom SIMD (e.g., a
vectorized timestamp scanner or a custom record-boundary finder), there are three failure modes:

1. **Missing feature gate:** Using `_mm256_cmpeq_epi8` without `#[target_feature(enable="avx2")]`
   compiles but silently falls back to scalar at runtime (LLVM auto-vectorizes poorly)
2. **Nightly-only `std::simd`:** Code that uses `std::simd::Simd<u8, 32>` will fail to compile
   on stable Rust, breaking library users who do not use nightly
3. **`is_x86_feature_detected!` at runtime:** Runtime feature detection is correct but adds a
   branch per call site; must be hoisted above hot loops

**Why it happens:**
The stable/nightly boundary for SIMD is non-obvious. `std::arch` intrinsics are stable but
require per-platform `cfg`; `std::simd` is ergonomic but nightly. Most blog posts from 2022–2023
describe `std::simd` as "experimental" — it is still experimental in 2025.

**Consequences:**
- MSRV policy broken if `std::simd` is added on stable
- Custom SIMD code compiles without AVX2 feature gate, produces scalar code, developer assumes
  SIMD is active and reports incorrect throughput gains
- CI on ARM runners silently skips AVX2 paths, masking divergence

**Prevention:**
1. Prefer `memchr` and `simdutf8` for all byte-search and UTF-8-validation tasks — both
   maintain their own SIMD backends with proper platform guards
2. If custom SIMD is needed, use the `wide` crate (stable, cross-platform SIMD wrapper) or
   the `pulp` crate (stable, AVX/NEON abstraction with runtime dispatch)
3. Never add `std::simd` unless a nightly feature gate (`#![feature(portable_simd)]`) is
   explicitly gated behind `#[cfg(feature = "nightly")]`
4. Add a CI check: `cargo build --target aarch64-unknown-linux-gnu` and
   `cargo build --target x86_64-unknown-linux-gnu` to catch platform-specific compilation failures

**Phase:** Enforce during any phase that introduces new low-level byte scanning optimizations.

---

### Pitfall 8: Micro-Benchmark vs. Real-World Gap from `iter().count()` vs. Actual Field Access

**What goes wrong:**
The benchmark measures `parser.iter().count()` — it calls `Iterator::next()` on every record
but never calls `parse_meta()`, `body()`, `parse_performance_metrics()`, or `indicators_raw()`.

Because `Sqllog` uses lazy field parsing, `iter().count()` exercises only:
- Record boundary detection (`memchr` + timestamp pattern match)
- `parse_record_with_hint`: timestamp slice, meta slice, content slice extraction
- `find_indicators_split` is NOT called (no caller requests it)

In real usage, callers will call at least `parse_performance_metrics()` on every record,
which additionally runs:
- `find_indicators_split()`: 3 reverse SIMD Finder calls on the last 256 bytes
- `decode_content_bytes`: UTF-8 validation or GB18030 decode of the body
- `parse_indicators_from_bytes`: 3 forward SIMD Finder calls + `atoi` + `fast_float::parse`

The additional per-record work for `parse_performance_metrics()` is probably 2–4x the work of
the record-splitting path alone. The benchmark does not measure this, so reported throughput is
for a workload that real users never run.

**Why it happens:**
`iter().count()` is the minimal correctness check and the easiest benchmark to write. It
maximizes throughput numbers but does not reflect realistic use.

**Warning signs:**
- No benchmark calls any parse method beyond `count()`
- Optimization of `parse_record_with_hint` shows diminishing returns because the expensive
  part (`find_indicators_split`) is not being measured

**Prevention:**
1. Add a benchmark variant: `parser.iter().filter_map(|r| r.ok()).map(|s| s.parse_performance_metrics()).count()`
2. Add a variant: `parser.iter().filter_map(|r| r.ok()).map(|s| s.parse_meta()).count()`
3. Use these as the primary metrics, not raw `count()`; they reflect the actual hot path
4. Profile with `cargo flamegraph` using the realistic benchmark to confirm where time is spent

**Phase:** Establish before hot-path optimization begins — you need to measure the right thing
before optimizing it.

---

## Minor Pitfalls

---

### Pitfall 9: `find_indicators_split` Scans Only the Last 256 Bytes — Fragile Assumption

**What goes wrong:**
`find_indicators_split` restricts the SIMD reverse search to `data[len-256..]`. This is a
deliberate optimization to avoid scanning the entire SQL body. However, if a record has a very
long SQL body followed by indicators, and the indicators happen to contain keywords like
`EXECTIME` embedded in the SQL itself before the actual indicators section, the reverse search
finds the SQL occurrence first and splits at the wrong boundary.

More subtly, if a SQL body is longer than 256 bytes and the indicators are at the very end,
`find_indicators_split` correctly finds them. But if a SQL body contains the string
`"EXECTIME: "` within the last 256 bytes (e.g., a SQL comment like `-- tuned for EXECTIME: low`),
the split position will be inside the SQL body, not at the indicators.

**Why it happens:**
The 256-byte window is an undocumented assumption that "indicators are always in the last 256 bytes."
This is likely true for DM logs but is not enforced or validated.

**Prevention:**
1. Add test cases with SQL bodies containing `EXECTIME:`, `ROWCOUNT:`, `EXEC_ID:` as substrings
   in the SQL text
2. Validate with real log files that contain long SQL bodies (>1 KB)
3. Consider using a more reliable delimiter between SQL body and indicators if the DM log
   format guarantees one (e.g., a specific whitespace pattern)

**Phase:** Address during correctness hardening; low priority if real logs never contain this pattern.

---

### Pitfall 10: `mimalloc` in Dev-Dependencies Does Not Apply to Benchmarks by Default

**What goes wrong:**
`mimalloc = "0.1.48"` is in `dev-dependencies`, which means it is available for tests and
benchmarks. However, to actually use `mimalloc` as the global allocator, the benchmark file
must explicitly declare `#[global_allocator] static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;`.
The current `benches/parser_benchmark.rs` does not do this.

Without `mimalloc`, benchmarks use the system allocator (jemalloc on macOS, ptmalloc2 on Linux).
If future optimizations reduce zero-copy guarantees and introduce more heap allocation (e.g., for
owned `String` fields in GB18030 paths), the allocator choice will materially affect benchmark
results.

**Prevention:**
1. Add `mimalloc` global allocator declaration to the benchmark if heap allocation is being
   optimized
2. Alternatively, add a benchmark variant with `mimalloc` and one without, to isolate
   allocator effects from parser effects

**Phase:** Minor; note it when introducing optimization that affects allocation patterns.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Benchmark setup | Pitfall 1 (synthetic data) | Add realistic benchmark before measuring anything |
| Hot-path unsafe decode | Pitfall 2 (unsafe lifetime) | Run Miri; extend encoding validation coverage |
| ARM vs x86 SIMD | Pitfall 3 (platform gap) | Record RUSTFLAGS in baseline; use ratio metrics |
| Large file optimization | Pitfall 4 (TLB pressure) | Add 256 MB benchmark; measure with `perf stat` |
| Parallel optimization | Pitfall 5 (Rayon overhead) | Benchmark par vs serial at multiple file sizes |
| Per-thread output buffers | Pitfall 6 (false sharing) | Pad with `align(64)` from the start |
| Custom SIMD | Pitfall 7 (stable/nightly) | Use `wide`/`pulp`; never use `std::simd` on stable |
| Measuring field-parse perf | Pitfall 8 (count() gap) | Add `parse_performance_metrics()` benchmark variant |
| Indicators split correctness | Pitfall 9 (256-byte window) | Add SQL-contains-indicator-keyword test |
| Allocation-heavy paths | Pitfall 10 (mimalloc unused) | Declare global allocator explicitly in bench |

---

## Sources

- Direct code inspection: `src/parser.rs`, `src/sqllog.rs`, `benches/parser_benchmark.rs`
- `memchr` 2.7.x internals: uses `packedpair` two-way algorithm on x86 AVX2/SSE2 and ARM NEON
  via compile-time `target_feature` detection; runtime dispatch on x86 via `is_x86_feature_detected!`
- `simdutf8` 0.1.5: `basic::from_utf8` uses AVX2 on x86 and NEON on ARM64, both stable Rust;
  `compat::from_utf8` provides error position at small overhead
- Rust `std::simd` (portable SIMD): nightly-only as of Rust 1.85; `std::arch` intrinsics are stable
- `memmap2` 0.9.x: does not call `madvise` by default; `Mmap::advise()` exposes `madvise` flags
  including `Sequential` and `HugePage` (Linux only)
- Rayon 1.10: work-stealing overhead per-task is in the 1–5 µs range; `flat_map_iter` is the
  correct pattern for parallel iterator-of-iterators (as used in current `par_iter` implementation)
- Confidence: HIGH for pitfalls 1, 2, 5, 7, 8 (directly observable in code); MEDIUM for pitfalls
  3, 4, 6 (platform-specific; require profiling to confirm magnitude)
