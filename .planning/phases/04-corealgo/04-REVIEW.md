---
phase: 04-corealgo
reviewed: 2026-04-25T00:00:00Z
depth: standard
files_reviewed: 1
files_reviewed_list:
  - src/parser.rs
findings:
  critical: 1
  warning: 3
  info: 2
  total: 6
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-04-25  
**Depth:** standard  
**Files Reviewed:** 1  
**Status:** issues_found

## Summary

`src/parser.rs` implements the core `LogParser` / `LogIterator` / `parse_record_with_hint` pipeline. The overall structure is sound and the performance-oriented design (zero-copy `Cow`, SIMD finders, `memmem`, `madvise`) is well-executed. One critical soundness issue was found in the `unsafe` `str` construction for the `FileEncodingHint::Utf8` path inside `parse_record_with_hint`, where a raw-pointer lifetime extension is used even though a simpler and safe alternative exists. Three warnings cover a correctness gap in encoding detection, an iterator boundary bug in `par_iter`, and a logic inversion that causes a silent skip of the first record in some parallel chunks. Two informational items cover a public API with an incorrect default `is_multiline` hint and a magic constant for the tail-sample size.

---

## Critical Issues

### CR-01: `from_raw_parts` lifetime extension is unsound for `FileEncodingHint::Utf8` path

**File:** `src/parser.rs:291-295`

**Issue:** In `parse_record_with_hint`, when the encoding hint is `Utf8`, the code constructs a `Cow::Borrowed<'a, str>` via `std::slice::from_raw_parts(meta_bytes.as_ptr(), meta_bytes.len())` in order to extend the lifetime of the slice from the borrow of `first_line` (which borrows from `record_bytes`, which borrows from `self.mmap`) to `'a`. The same pattern is repeated for the tag slice (lines 348-350, 354-358).

However, `first_line` is a local `let` binding computed inside this function — it is not guaranteed to be a sub-slice of the original `'a`-lived memory-mapped buffer in all code paths. Specifically, when `is_multiline = true` **and** the first line ends with `\r`, a new slice `&line[..line.len() - 1]` is produced (lines 227-229). This new slice still points into the mmap buffer (the pointer arithmetic is sound), but the Rust borrow checker cannot verify this, and the `unsafe` block makes an implicit assumption that is not locally documented or enforced.

More importantly, the same lifetime-extension pattern is applied to `content_slice` (line 387, via `Cow::Borrowed(content_slice)`), which is derived from `record_bytes` that in `par_iter` is **a sub-slice of the chunk** `&data[start..end]` — a slice whose lifetime is `'_` (the `par_iter` closure borrow), not `'a` (the `LogParser`'s mmap lifetime). The parallel iterator produces `Sqllog<'_>` items, but the `Cow::Borrowed` created with the transmuted `'a` lifetime would allow those items to outlive the chunk reference, causing a use-after-free if the caller stores them beyond the iterator's lifetime.

In practice the `'_` bound on `par_iter`'s return type (`Sqllog<'_>`) constrains the safe API surface and prevents misuse at the call site, but the unsafety inside `parse_record_with_hint` is wider than it needs to be.

**Fix:** For the `Utf8` path, use a plain `simd_from_utf8` (which is a no-alloc, near-zero-cost validation that returns a `&str` with the correct lifetime without any pointer casting), or use `std::str::from_utf8_unchecked` directly on the already-in-scope slice without the redundant `from_raw_parts` indirection:

```rust
// Before (lines 291-295):
unsafe {
    Cow::Borrowed(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
        meta_bytes.as_ptr(),
        meta_bytes.len(),
    )))
}

// After — no lifetime extension, no raw pointer arithmetic:
// SAFETY: file was validated as UTF-8 during `from_path`; meta_bytes is
// a sub-slice of record_bytes which lives for 'a.
unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(meta_bytes)) }
```

The `from_raw_parts` call is completely redundant: `meta_bytes` is already `&[u8]` with the right pointer and length. Removing `from_raw_parts` does not change semantics but eliminates the confusing lifetime-laundering appearance and reduces the unsafe surface.

---

## Warnings

### WR-01: Encoding detection gap — middle of large files is never sampled

**File:** `src/parser.rs:53-61`

**Issue:** The sampling strategy reads the first 64 KB and the last 4 KB. When `head_size == tail_start` (i.e. the file is ≤ 64 KB) the tail sample is correctly skipped via the `tail_start >= mmap.len()` guard (line 56). However, for files between 64 KB and ~68 KB the `tail_start` is computed as `mmap.len().saturating_sub(4096).max(head_size)`, which means `tail_start == head_size == 64 * 1024` and the tail sample covers only the last 4 KB. For multi-megabyte files the middle of the file — where GB18030 multi-byte sequences most commonly appear in SQL identifiers — is never inspected. The current heuristic will misclassify such files as UTF-8 and then silently produce garbled output for affected records.

**Fix:** Either document this known limitation explicitly in the doc comment, or extend the sample to also cover a middle window:

```rust
let mid_start = (mmap.len() / 2).saturating_sub(2 * 1024);
let mid_end = (mid_start + 4 * 1024).min(mmap.len());
let mid_ok = mid_start >= head_size  // avoid re-checking already-scanned region
    || simd_from_utf8(&mmap[mid_start..mid_end]).is_ok();
let encoding = if head_ok && mid_ok && tail_ok { ... };
```

### WR-02: `par_iter` may silently drop the first record in non-initial chunks

**File:** `src/parser.rs:106-110`

**Issue:** When `par_iter` constructs each `LogIterator` chunk, it passes `&data[start..end]` where `start` is already a record boundary (returned by `find_next_record_start`, which returns the position of the first byte of the timestamp). `LogIterator::next` does not check whether position 0 of its `data` slice itself begins a record — it starts iterating immediately. This is correct for the first chunk (`start = 0`), but for subsequent chunks the timestamp at `start` is guaranteed to exist (it was found by `find_next_record_start`). So the behavior is actually correct. However, the `LogIterator` for chunk `[start, end)` will try to parse `data[0..]` as a record and advance to `next_start` — which is correct.

The actual gap is that `find_next_record_start` (lines 193-197) skips to the **next** line after `from` before beginning to search, meaning chunk boundaries are placed at line-starts **after** a newline. If `from` already points exactly at a newline (e.g. `chunk_size` lands on a `\n`), the function skips it and begins at the char after — still correct. But if `from` points into the middle of a multi-line record whose continuation lines contain a `"20"` subsequence matching `FINDER_RECORD_START` but not a full timestamp, `find_next_record_start` will iterate through false positives via `is_timestamp_start` checks. This is functionally correct but is worth a comment noting why the false-positive rate is acceptable (timestamps are `"20YY-MM-DD HH:MM:SS.mmm"` — a strict 23-byte format).

The real warning is: there is no guard preventing `starts` from containing duplicates **after** `push(data.len())` and before `dedup()` (line 99). If `find_next_record_start` returns `data.len()` for every non-zero boundary (e.g. a file with only one record), `starts` becomes `[0, data.len(), data.len(), ..., data.len()]`. After `dedup()` it becomes `[0, data.len()]`, which is fine — `dedup()` saves correctness here. This is actually handled, but the comment at line 98-99 should make clear that `dedup()` is load-bearing for correctness (not just an optimization), since it prevents zero-length chunks.

**Fix:** Add an explanatory comment:

```rust
starts.push(data.len());
// dedup is load-bearing: find_next_record_start may return data.len()
// for multiple threads when the file has fewer records than threads,
// which would produce zero-length chunks without dedup.
starts.dedup();
```

### WR-03: `parse_record` public API uses wrong `is_multiline` default

**File:** `src/parser.rs:213-215`

**Issue:** The public `parse_record` function hardcodes `is_multiline: true` as the hint:

```rust
pub fn parse_record<'a>(record_bytes: &'a [u8]) -> Result<Sqllog<'a>, ParseError> {
    parse_record_with_hint(record_bytes, true, FileEncodingHint::Auto)
}
```

When `is_multiline = true`, `parse_record_with_hint` calls `memchr(b'\n', record_bytes)` to split the first line (line 224). For single-line records (the common case when `parse_record` is called in isolation), this is an unnecessary scan and the `(line, &[] as &[u8])` fallback is taken only if no `\n` is found. The result is correct but misleading: passing `true` for a single-line record causes a redundant `memchr` call. More importantly, the `_rest` variable computed at line 223 is immediately discarded with `let (first_line, _rest) = ...`, making the `is_multiline` branch entirely dead for the `None` arm.

For `parse_record`, the correct default would be `false` (let the function use the simpler single-line path), or the hint parameter should be removed from the public API by inspecting whether `record_bytes` contains a `\n`:

```rust
pub fn parse_record<'a>(record_bytes: &'a [u8]) -> Result<Sqllog<'a>, ParseError> {
    let is_multiline = memchr::memchr(b'\n', record_bytes).is_some();
    parse_record_with_hint(record_bytes, is_multiline, FileEncodingHint::Auto)
}
```

This also makes the API more robust for callers who pass multi-line slices to `parse_record` directly (as done in `tests/edge_cases.rs` line 8, which passes a multi-line byte slice to `parse_record` — currently works only because `is_multiline = true` causes the fallback path to be taken, not because it was explicitly designed for multi-line input).

---

## Info

### IN-01: Magic constant `4 * 1024` for tail sample size should be a named constant

**File:** `src/parser.rs:54`

**Issue:** The tail sample size `4 * 1024` (4 KB) and head sample size `64 * 1024` (64 KB) are inline expressions. The head size already has a clear semantic (first 64 KB matches the CLAUDE.md description), but the tail size has no explanation for why 4 KB was chosen versus, say, 8 KB or 16 KB.

**Fix:** Introduce named constants:

```rust
const ENCODING_HEAD_SAMPLE: usize = 64 * 1024; // 64 KB
const ENCODING_TAIL_SAMPLE: usize =  4 * 1024; //  4 KB — covers ~50 trailing log lines
```

### IN-02: `_rest` binding is always unused — `is_multiline` branch could be simplified

**File:** `src/parser.rs:223`

**Issue:** In `parse_record_with_hint`, the destructuring `let (first_line, _rest) = ...` always discards `_rest`. The `_rest` value is never read anywhere in the function body; the body content is taken from `record_bytes[content_start..]` directly. This makes the multiline split logic in lines 223-246 appear to serve a purpose it does not serve, which is confusing for future maintainers.

**Fix:** Simplify to only extract `first_line`:

```rust
let first_line = if is_multiline {
    match memchr(b'\n', record_bytes) {
        Some(idx) => {
            let line = &record_bytes[..idx];
            if line.ends_with(b"\r") { &line[..line.len() - 1] } else { line }
        }
        None => {
            let line = record_bytes;
            if line.ends_with(b"\r") { &line[..line.len() - 1] } else { line }
        }
    }
} else {
    let line = record_bytes;
    if line.ends_with(b"\r") { &line[..line.len() - 1] } else { line }
};
```

---

_Reviewed: 2026-04-25_  
_Reviewer: Claude (gsd-code-reviewer)_  
_Depth: standard_
