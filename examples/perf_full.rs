/// Benchmark comparing iter() vs par_iter() on a synthetic 50MB log file.
///
/// Usage: cargo run --example perf_full --release
use dm_database_parser_sqllog::LogParser;
use rayon::prelude::*;
use std::time::{Duration, Instant};

const TARGET_SIZE: usize = 50 * 1024 * 1024; // 50 MB
const WARMUP_ITERS: usize = 3;
const BENCH_ITERS: usize = 20;

static RECORD_TEMPLATE: &[u8] = b"2025-08-12 10:57:09.548 (EP[0] sess:0x178ebca0 thrd:757455 user:HBTCOMS_V3_PROD trxid:0 stmt:0x285eb060 appname:myapp ip:::ffff:10.3.100.68) [SEL] SELECT id, name, value FROM some_table WHERE condition = 1 EXECTIME: 0(ms) ROWCOUNT: 1(rows) EXEC_ID: 289655178.\n";

fn generate_log_data(target_bytes: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(target_bytes + RECORD_TEMPLATE.len());
    while data.len() < target_bytes {
        data.extend_from_slice(RECORD_TEMPLATE);
    }
    data
}

fn bench_iter(path: &str) -> Vec<Duration> {
    let mut durations = Vec::with_capacity(BENCH_ITERS);
    for _ in 0..BENCH_ITERS {
        let parser = LogParser::from_path(path).expect("open file");
        let start = Instant::now();
        let count: usize = parser.iter().filter(|r| r.is_ok()).count();
        let elapsed = start.elapsed();
        durations.push(elapsed);
        // prevent optimizer from eliminating the work
        std::hint::black_box(count);
    }
    durations
}

fn bench_par_iter(path: &str) -> Vec<Duration> {
    let mut durations = Vec::with_capacity(BENCH_ITERS);
    for _ in 0..BENCH_ITERS {
        let parser = LogParser::from_path(path).expect("open file");
        let start = Instant::now();
        let count: usize = parser.par_iter().filter(|r| r.is_ok()).count();
        let elapsed = start.elapsed();
        durations.push(elapsed);
        std::hint::black_box(count);
    }
    durations
}

fn throughput_mb_s(bytes: usize, duration: Duration) -> f64 {
    bytes as f64 / 1024.0 / 1024.0 / duration.as_secs_f64()
}

fn report(label: &str, durations: &[Duration], file_bytes: usize) {
    let mut throughputs: Vec<f64> = durations
        .iter()
        .map(|d| throughput_mb_s(file_bytes, *d))
        .collect();
    throughputs.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min = throughputs.first().copied().unwrap_or(0.0);
    let max = throughputs.last().copied().unwrap_or(0.0);
    let avg = throughputs.iter().sum::<f64>() / throughputs.len() as f64;
    let median = if throughputs.len().is_multiple_of(2) {
        (throughputs[throughputs.len() / 2 - 1] + throughputs[throughputs.len() / 2]) / 2.0
    } else {
        throughputs[throughputs.len() / 2]
    };

    println!(
        "{label:<12} min={min:>8.1} MB/s  median={median:>8.1} MB/s  avg={avg:>8.1} MB/s  max={max:>8.1} MB/s"
    );
}

fn main() {
    println!(
        "Generating ~{} MB synthetic log data…",
        TARGET_SIZE / 1024 / 1024
    );
    let data = generate_log_data(TARGET_SIZE);
    let actual_bytes = data.len();
    println!(
        "Generated {:.2} MB ({} records)",
        actual_bytes as f64 / 1024.0 / 1024.0,
        actual_bytes / RECORD_TEMPLATE.len()
    );

    // Write to a temp file
    let tmp = tempfile_path();
    std::fs::write(&tmp, &data).expect("write temp file");
    drop(data); // free memory before benchmarking

    println!("\nWarm-up ({WARMUP_ITERS} iterations each)…");
    for _ in 0..WARMUP_ITERS {
        let parser = LogParser::from_path(&tmp).expect("open");
        std::hint::black_box(parser.iter().count());
    }
    for _ in 0..WARMUP_ITERS {
        let parser = LogParser::from_path(&tmp).expect("open");
        std::hint::black_box(parser.par_iter().count());
    }

    println!("\nBenchmarking ({BENCH_ITERS} iterations each)…\n");

    let iter_durations = bench_iter(&tmp);
    let par_durations = bench_par_iter(&tmp);

    println!("Results ({actual_bytes} bytes):");
    report("iter()", &iter_durations, actual_bytes);
    report("par_iter()", &par_durations, actual_bytes);

    // Speedup
    let iter_median = median_throughput(&iter_durations, actual_bytes);
    let par_median = median_throughput(&par_durations, actual_bytes);
    if iter_median > 0.0 {
        println!(
            "\npar_iter() speedup over iter(): {:.2}x",
            par_median / iter_median
        );
    }

    // Clean up
    let _ = std::fs::remove_file(&tmp);
}

fn median_throughput(durations: &[Duration], bytes: usize) -> f64 {
    let mut throughputs: Vec<f64> = durations
        .iter()
        .map(|d| throughput_mb_s(bytes, *d))
        .collect();
    throughputs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if throughputs.len().is_multiple_of(2) {
        (throughputs[throughputs.len() / 2 - 1] + throughputs[throughputs.len() / 2]) / 2.0
    } else {
        throughputs[throughputs.len() / 2]
    }
}

fn tempfile_path() -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("perf_full_{}.log", std::process::id()));
    path.to_string_lossy().into_owned()
}
