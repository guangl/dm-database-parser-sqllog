# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Test all
cargo test

# Run a single test
cargo test <test_name> -- --nocapture

# Run a specific test file
cargo test --test <file_name>   # e.g. --test performance_metrics

# Lint
cargo clippy

# Benchmarks
cargo bench --bench parser_benchmark
```

Coverage must remain ≥90% (`cargo llvm-cov --workspace --all-features --fail-under-lines 90`).

## Architecture

This is a high-performance Rust library for parsing DM (达梦) database SQL log files. Key design goals: zero-copy, lazy evaluation, memory-mapped I/O.

### Data Flow

```
File → LogParser (memory-mapped) → LogIterator → Sqllog<'a> (lazy fields)
```

### Core Modules

**`src/parser.rs` — `LogParser` / `LogIterator`**
- Memory-maps the file via `memmap2`; detects encoding (UTF-8 vs GB18030) by sampling the first 64 KB
- `LogIterator` splits records by scanning for lines that start with a timestamp (`20XX-MM-DD HH:MM:SS.mmm`); handles CRLF/LF and multi-line records

**`src/sqllog.rs` — `Sqllog<'a>`**
- Holds `Cow<'a, str>` / `Cow<'a, [u8]>` slices into the memory-mapped buffer — no heap allocation on parse
- Fields are parsed on demand: `body()`, `parse_meta()`, `parse_indicators()`, `parse_performance_metrics()`
- `parse_performance_metrics()` is the hot-path method; returns indicators (EXECTIME, ROWCOUNT, EXEC_ID) plus the SQL body in one call

**`src/tools.rs`** — byte-level helpers for timestamp validation (`is_ts_millis_bytes`) and record-start detection (`is_record_start_line`)

**`src/error.rs`** — `ParseError` enum (`InvalidFormat`, `FileNotFound`, `InvalidRecordStartLine`, `IntParseError`, `IoError`)

### Performance Notes

- `simdutf8` for fast UTF-8 validation; `memchr`/`memrchr` for field splitting
- Throughput target: >4 million records/sec (>1 GB/s) single-threaded
- Benchmark baseline is tracked in `benchmarks/baseline.json`
