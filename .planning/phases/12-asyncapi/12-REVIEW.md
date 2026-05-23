---
phase: 12-asyncapi
reviewed: 2026-05-23T00:00:00Z
depth: standard
files_reviewed: 3
files_reviewed_list:
  - Cargo.toml
  - src/async_api/mod.rs
  - src/lib.rs
findings:
  critical: 0
  warning: 3
  info: 2
  total: 5
status: issues_found
---

# Phase 12: Code Review Report

**Reviewed:** 2026-05-23
**Depth:** standard
**Files Reviewed:** 3
**Status:** issues_found

## Summary

Phase 12 adds a tokio-based async wrapper (`AsyncLogParser`) around the existing synchronous
`LogParserBuilder` via `tokio::task::spawn_blocking`. The feature gate is correctly applied
(`#[cfg(feature = "async")]` in `lib.rs`), the library artifact compiles cleanly without the
feature, all existing tests pass, and total line coverage remains at 90.66% (requirement ≥90%
met). No security vulnerabilities or crashes were found.

Three warnings and two info items were identified. The most impactful is silent parse-error
dropping inside `parse()`, which can return an artificially short `Vec<Sqllog>` with no
diagnostic signal. The second is a mismatched tokio feature set between production and test
builds that could hide runtime configuration issues. The third is a missing doc note about the
caller-supplied runtime requirement.

---

## Warnings

### WR-01: Silent parse-error dropping makes data loss invisible to callers

**File:** `src/async_api/mod.rs:76-79`

**Issue:** Both code paths inside `parse()` use `filter_map(Result::ok)` to collect records,
silently discarding every `ParseError` that the iterator produces:

```rust
// with filter
iter.apply_filter(f).filter_map(Result::ok).collect()
// without filter
iter.filter_map(Result::ok).collect()
```

The function signature `-> Result<Vec<Sqllog>, AsyncError>` implies the `Err` arm signals a
failure to process the file. In practice `Ok(vec![])` is returned when the file exists but every
record is malformed, and `Ok(partial_vec)` is returned when only some records parse successfully.
There is no way for callers to distinguish "file had 1000 records, 950 parsed" from "file had 50
records". This is a silent data-loss risk in production pipelines that aggregate statistics across
many files.

The sync iterator already supports `apply_filter_keep_errors` / explicit `Err` propagation.
`parse()` should either document this behaviour explicitly and visibly in its doc-comment, or
expose a companion `parse_strict()` / return a tuple `(Vec<Sqllog>, Vec<ParseError>)`.

**Minimum fix (documentation):** add a clearly visible warning to the `parse()` doc:

```rust
/// # 注意
///
/// 单条记录的解析错误会被**静默丢弃**，不会影响返回值的 `Ok` 状态。
/// 若需获知被跳过的记录数，请使用 … （未来 API）。
pub async fn parse(self) -> Result<Vec<Sqllog>, AsyncError> {
```

**Preferred fix:** thread errors out so callers can decide:

```rust
pub async fn parse(self) -> Result<(Vec<Sqllog>, Vec<ParseError>), AsyncError> {
```

---

### WR-02: tokio `dev-dependency` always compiled in test builds regardless of feature flag

**File:** `Cargo.toml:49`

**Issue:** `tokio` appears in both `[dependencies]` (optional, feature-gated) and
`[dev-dependencies]` (unconditional, with `features = ["rt", "macros"]`):

```toml
[dependencies]
tokio = { version = "1", features = ["rt"], optional = true }

[dev-dependencies]
tokio = { version = "1", features = ["rt", "macros"] }
```

Because dev-dependencies are not feature-gated, `tokio` is compiled into every test binary even
when the `async` feature is disabled. This violates the stated constraint *"tokio must NOT appear
in non-async builds"* when interpreted to include test builds (`cargo test` without
`--features async`).

Although this does **not** affect the published library artifact (dev-deps never enter the
crate graph of downstream consumers), it means CI running `cargo test` (without
`--features async`) silently links tokio. If the intent is to enforce tokio-free non-async builds
end-to-end, the dev-dep must also be gated:

```toml
[dev-dependencies]
# only pull tokio when testing the async feature
tokio = { version = "1", features = ["rt", "macros"], optional = true }

[features]
async = ["dep:tokio"]
# or keep a separate test helper feature
```

Alternatively, gate the async tests with `#[cfg(all(test, feature = "async"))]` and keep the
current dev-dep structure but document that the constraint applies only to the library artifact.

---

### WR-03: `parse()` doc-comment omits the requirement for a caller-supplied tokio runtime

**File:** `src/async_api/mod.rs:60-65`

**Issue:** `AsyncLogParser::parse()` calls `tokio::task::spawn_blocking` internally. This
requires an active tokio runtime in the calling context at the time `.await` is evaluated. If a
user calls `parse().await` from outside a tokio runtime (e.g., inside a `std::thread::spawn`
without a runtime), it will panic with `"there is no reactor running"`. The public doc-comment
does not mention this requirement.

```rust
/// 在阻塞线程池中解析日志文件，返回所有匹配的记录。
///
/// # 错误
///
/// - [`AsyncError::Parse`]：文件不存在、格式错误等解析错误
/// - [`AsyncError::Panic`]：阻塞任务内部 panic
pub async fn parse(self) -> Result<Vec<Sqllog>, AsyncError> {
```

**Fix:** Add a `# 运行时要求` / `# Panics` section:

```rust
/// # Panics
///
/// 若调用方不在 tokio 运行时上下文中（如在裸 `std::thread` 中直接调用），
/// `spawn_blocking` 会 panic，此 panic 会被捕获并以 [`AsyncError::Panic`] 返回。
```

Note: because the panic is caught by the `map_err(|e| AsyncError::Panic(...))` wrapper on the
`JoinHandle`, this particular panic surfaces as `Err(AsyncError::Panic(_))` rather than
unwinding the caller. The doc-comment should clarify that runtime-absent panics are returned
(not propagated), unlike other Rust async contexts.

---

## Info

### IN-01: `async` feature re-specifying `tokio/rt` is redundant

**File:** `Cargo.toml:52`

**Issue:** The `async` feature is declared as:

```toml
[features]
async = ["tokio/rt"]
```

but the `tokio` optional dependency already declares `features = ["rt"]`:

```toml
tokio = { version = "1", features = ["rt"], optional = true }
```

When Cargo enables the optional dependency, it automatically applies the feature list from the
`[dependencies]` entry. The `"tokio/rt"` in the feature list adds nothing beyond enabling the
dependency itself. The idiomatic form is:

```toml
[features]
async = ["dep:tokio"]
```

This is not a bug — `"tokio/rt"` is a valid and accepted Cargo syntax — but the `dep:tokio` form
is more explicit and avoids confusion about whether the `rt` feature is being required for a
reason not apparent from the dependency declaration.

---

### IN-02: No test exercises the `AsyncError::Panic` return path via an actual panic

**File:** `src/async_api/mod.rs:196-213`

**Issue:** `test_async_error_is_error` (line 196) and `test_async_error_from_parse_error`
(line 204) test only the `Display` and `From` implementations of `AsyncError`. No test verifies
that a panic inside the `spawn_blocking` closure is correctly converted to
`Err(AsyncError::Panic(_))` rather than propagating or being lost.

The `map_err(|e| AsyncError::Panic(e.to_string()))` on the `JoinHandle` (line 83) is the
only code path for `AsyncError::Panic`, and it is unexercised. The `async_api/mod.rs` line
coverage report shows one missed line (99.09% / 110 lines, 1 missed) — this is the likely
candidate.

**Fix:** Add a test that deliberately panics inside spawn_blocking and asserts the variant:

```rust
#[cfg(not(miri))]
#[tokio::test]
async fn test_spawn_blocking_panic_returns_async_error_panic() {
    // Arrange: use a path that causes a panic by directly calling spawn_blocking
    // (AsyncLogParser has no inject point, so test via direct invocation of the pattern)
    let result: Result<(), _> = tokio::task::spawn_blocking(|| panic!("deliberate"))
        .await
        .map_err(|e| AsyncError::Panic(e.to_string()));
    assert!(matches!(result, Err(AsyncError::Panic(_))));
}
```

This is low-risk to add and plugs the only uncovered branch in `async_api/mod.rs`.

---

_Reviewed: 2026-05-23_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
