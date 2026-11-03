//! PLAN_V152 N4: watch SLA — incremental recheck after source modification
//! must complete in < 200 ms (the check cache hits at 15-31 ms per
//! PLAN_V150 M4). This test uses `check_source` (the incremental path)
//! to measure the recheck latency.

use forgen::driver::ForgenCompiler;
use std::time::Instant;

#[test]
fn test_watch_sla_incremental_check_latency() {
    let src = r#"
fn main() {
    val x = 42
    out int_to_str(x)
}
"#;
    let compiler = ForgenCompiler::new("release");

    // First check (cold cache) — not timed.
    let res1 = compiler.check_source(src, "watch_sla.dtr");
    assert!(res1.success, "initial check must succeed");

    // Modified source (same structure, different value) — incremental.
    let src2 = src.replace("42", "43");
    let start = Instant::now();
    let res2 = compiler.check_source(&src2, "watch_sla.dtr");
    let elapsed = start.elapsed();

    assert!(res2.success, "incremental check must succeed");
    let ms = elapsed.as_millis();
    assert!(
        ms < 200,
        "incremental check SLA violated: {} ms >= 200 ms",
        ms
    );
}
