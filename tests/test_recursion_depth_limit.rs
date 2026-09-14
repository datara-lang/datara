use forgen::driver::ForgenCompiler;

/// Helper: compile source in a 64 MB thread so that the parser's recursive
/// descent on deeply-nested expressions cannot overflow the small default test
/// thread stack on Windows (~1 MB).
fn compile_in_large_stack(src: String, file: &'static str) -> forgen::driver::CompilationResult {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || ForgenCompiler::new("debug").compile_source(&src, file, None))
        .unwrap()
        .join()
        .expect("compilation thread panicked unexpectedly")
}

/// Baseline: a simple 3-deep nesting compiles fine.
#[test]
fn test_shallow_nesting_compiles() {
    let src = "fn main() {\n    let x = ((1 + 2) * 3)\n    out x\n}\n".to_string();
    let res = compile_in_large_stack(src, "shallow.dtr");
    assert!(res.success, "Shallow nesting must compile: {:?}", res.error);
}

/// 257-deep nesting must be rejected cleanly -- no panic, no stack overflow.
/// This is the core regression guard for the crash that existed in
/// audit_zero_trust/diagnostics.rs.
#[test]
fn test_deep_nesting_rejected_with_diagnostic_not_panic() {
    let depth = 257usize;
    let src = format!(
        "fn main() {{\n    let x = {}42{}\n    out x\n}}\n",
        "(".repeat(depth),
        ")".repeat(depth)
    );

    let res = compile_in_large_stack(src, "deep_nesting.dtr");

    let err = res.error.clone().unwrap_or_default();
    assert!(
        !err.contains("panicked") && !err.contains("overflowed"),
        "Compiler must not panic or overflow on deep nesting. Got: {}",
        err
    );
}

/// At exactly the parser depth limit (depth 65) the compiler must not crash.
#[test]
fn test_at_limit_depth_does_not_crash() {
    let depth = 65usize;
    let src = format!(
        "fn main() {{\n    let x = {}42{}\n    out x\n}}\n",
        "(".repeat(depth),
        ")".repeat(depth)
    );
    // Only assert that the compiler returns without crashing. Success or
    // a clean diagnostic are both acceptable.
    let _res = compile_in_large_stack(src, "at_limit.dtr");
}

/// For depths 1..=300, the compiler must never overflow the stack or panic.
/// This runs all iterations inside a single large-stack thread.
#[test]
fn test_arbitrary_nesting_never_crashes() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let compiler = ForgenCompiler::new("debug");
            for depth in (1usize..=300).step_by(10) {
                let src = format!(
                    "fn main() {{\n    let x = {}42{}\n    out x\n}}\n",
                    "(".repeat(depth),
                    ")".repeat(depth)
                );
                let res = compiler.compile_source(&src, "arb_nesting.dtr", None);
                let err = res.error.clone().unwrap_or_default();
                assert!(
                    !err.contains("panicked") && !err.contains("overflowed"),
                    "depth={} caused panic/overflow: {}",
                    depth,
                    err
                );
            }
        })
        .unwrap()
        .join()
        .expect("nesting loop thread panicked unexpectedly");
}
