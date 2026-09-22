//! Performance test: verify total examples/ compile time < 500ms (v1.4.5)

use forgen::driver::ForgenCompiler;
use std::fs;
use std::path::Path;
use std::time::Instant;

/// One full check pass over every example. Returns (file_count, total_ms).
fn measure_pass(compiler: &ForgenCompiler) -> (usize, u128) {
    let mut total_time_ms = 0u128;
    let mut checked_count = 0;

    let examples_dir = Path::new("examples");
    if let Ok(entries) = fs::read_dir(examples_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "dtr") {
                let start = Instant::now();
                let _ = compiler.check_file(&p);
                total_time_ms += start.elapsed().as_millis();
                checked_count += 1;
            }
        }
    }
    (checked_count, total_time_ms)
}

#[test]
fn test_examples_compilation_under_500ms() {
    let compiler = ForgenCompiler::new("check");

    // Warm-up pass: absorbs cold-start costs (disk cache, AV scanning on CI
    // runners) that are unrelated to compiler throughput. The budget applies
    // to the warm measurement, which is the honest steady-state number.
    let _ = measure_pass(&compiler);

    let (checked_count, total_time_ms) = measure_pass(&compiler);

    println!(
        "Total compile time for {} example files: {} ms",
        checked_count, total_time_ms
    );
    assert!(
        total_time_ms < 500,
        "Total examples compile time must be < 500ms, got {}ms",
        total_time_ms
    );
}
