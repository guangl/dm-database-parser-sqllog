use dm_database_parser_sqllog::iter_records_from_file;
use mimalloc::MiMalloc;
use std::env;
use std::time::Instant;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <log_file_path>", args[0]);
        return;
    }

    let path = &args[1];
    println!("Parsing file: {}", path);

    let start = Instant::now();
    let mut count = 0;
    let mut errors = 0;
    let mut _bytes_processed = 0; // Approximate

    for res in iter_records_from_file(path) {
        match res {
            Ok(log) => {
                count += 1;
                _bytes_processed += log.body.len(); // Very rough approximation
            }
            Err(e) => {
                errors += 1;
                eprintln!("Error: {}", e);
            }
        }
    }

    let duration = start.elapsed();
    println!("Done in {:?}", duration);
    println!("Records: {}", count);
    println!("Errors: {}", errors);

    let secs = duration.as_secs_f64();
    if secs > 0.0 {
        println!("Speed: {:.2} records/s", count as f64 / secs);
    }
}
