//! Performance test: verify total examples/ compile time < 500ms (v1.4.5)

use forgen::driver::ForgenCompiler;
use std::fs;
use std::path::Path;
use std::time::Instant;

#[test]
fn test_examples_compilation_under_500ms() {
    let compiler = ForgenCompiler::new("check");
    let mut total_time_ms = 0u128;
    let mut checked_count = 0;

    let examples_dir = Path::new("examples");
    if let Ok(entries) = fs::read_dir(examples_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "dtr") {
                let start = Instant::now();
                let _ = compiler.check_file(&p);
                total_time_ms += start.elapsed().as_millis();
                checked_count += 1;
            }
        }
    }

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
