Benchmark results (local run, Release build)

Environment:
- Windows, release build
- Rayon default thread pool

Files used:
- `sqllogs/dmsql_DSC0_20250812_092516.log` (56,528,299 bytes)
- `sqllogs/dmsql_OASIS_DB1_20251020_151030.log` (1,074,745,067 bytes)

Examples used:
- `examples/perf_record_only.rs` (bulk parse, parallel)
- `examples/streaming_parse.rs` (streaming parse)

Results:
- parse_records_from_file on `dmsql_DSC0_20250812_092516.log`:
  - parsed 199,971 records
  - time: 0.147 s
  - speed: ~1,361,045 records/s

- parse_records_from_file on `dmsql_OASIS_DB1_20251020_151030.log`:
  - parsed 3,018,125 records
  - time: 2.608 s
  - speed: ~1,157,463 records/s

- iter_records_from_file (streaming) on `dmsql_OASIS_DB1_20251020_151030.log`:
  - parsed 3,018,125 records
  - time: 2.751 s
  - speed: ~1,097,178 records/s

Notes:
- The parser is highly optimized; bulk parsing shows slightly higher throughput compared to streaming.
- For routine benchmarking (Criterion), use the small `dmsql_DSC0_20250812_092516.log` sample with reduced sample size and measurement time for fast results.

How to run these locally:

```pwsh
# release build
cargo build --release
# bulk parse benchmark (parallel)
cargo run --release --example perf_record_only -- sqllogs/dmsql_DSC0_20250812_092516.log
# streaming parse benchmark
cargo run --release --example streaming_parse -- sqllogs/dmsql_DSC0_20250812_092516.log
```

Optional: add a `benches` harness (Criterion) for file-based benchmarks and run `cargo bench` (be aware large files make Criterion runs slow).