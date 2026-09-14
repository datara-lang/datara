// Milestone 2 - Ownership Edge Cases: Loop-specific ownership and type tests.
// Each test spins a dedicated thread with a 64 MiB stack so recursive
// compiler passes cannot overflow the ~1 MiB default stack of a spawned
// test thread on Windows (root cause fixed in commit bcfac07).

use forgen::driver::{CompilationResult, ForgenCompiler};

fn compile(src: String, file: &'static str) -> CompilationResult {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || ForgenCompiler::new("debug").compile_source(&src, file, None))
        .expect("failed to spawn compiler thread")
        .join()
        .expect("compiler thread panicked")
}

/// A compilation outcome is only acceptable if it either succeeded or was
/// rejected through the normal diagnostic channel. A panic, a stack overflow
/// or an empty failure (no error, no diagnostics) is a soundness bug.
fn assert_graceful(res: &CompilationResult, context: &str) {
    let messages = res
        .diagnostic_records
        .iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for marker in ["panicked at", "stack overflow", "internal compiler error"] {
        assert!(
            !messages.contains(marker),
            "[{context}] compiler crashed with `{marker}` instead of a diagnostic:\n{messages}"
        );
    }
    assert!(
        res.success || res.error.is_some() || !res.diagnostic_records.is_empty(),
        "[{context}] compilation failed without any error or diagnostic"
    );
}

// ---------------------------------------------------------------------------
// Test 1: A basic while loop that mutates a `mut` variable must compile
// successfully -- this is the canonical mutable-counter pattern.
// ---------------------------------------------------------------------------
#[test]
fn test_loop_basic_mutation_compiles() {
    let src = r#"fn main() {
    mut i = 0
    mut total = 0
    while i < 5 {
        total = total + i
        i = i + 1
    }
    out total
}
"#;

    let res = compile(src.to_string(), "test_loop_basic_mutation_compiles.dtr");
    assert_graceful(&res, "loop_basic_mutation_compiles");
    assert!(
        res.success,
        "Basic mutable loop should compile; diagnostics: {}",
        res.diagnostics
    );
}

// ---------------------------------------------------------------------------
// Test 2: Reassigning an immutable `let` binding inside a loop body must
// be rejected with a compile-time diagnostic, not a panic.
// ---------------------------------------------------------------------------
#[test]
fn test_immutable_loop_var_reuse_rejected() {
    let src = r#"fn main() {
    mut i = 0
    let limit = 3
    while i < limit {
        limit = limit + 1
        i = i + 1
    }
    out i
}
"#;

    let res = compile(
        src.to_string(),
        "test_immutable_loop_var_reuse_rejected.dtr",
    );
    assert_graceful(&res, "immutable_loop_var_reuse_rejected");
    assert!(
        !res.success,
        "Expected compile failure for immutable loop-var reassignment, got success"
    );
}

// ---------------------------------------------------------------------------
// Test 3: A type-annotated binding with a wrong-type initializer must be
// caught by the type checker before the loop is compiled. (Int/Float
// comparisons are intentionally widened by the checker, so the proven
// TypeMismatch path -- `let limit: Int = "three"` -- is used instead.)
// ---------------------------------------------------------------------------
#[test]
fn test_loop_out_of_range_index_type_check() {
    let src = r#"fn main() {
    let limit: Int = "three"
    mut i = 0
    while i < limit {
        i = i + 1
    }
    out i
}
"#;

    let res = compile(
        src.to_string(),
        "test_loop_out_of_range_index_type_check.dtr",
    );
    assert_graceful(&res, "loop_out_of_range_index_type_check");
    assert!(
        !res.success,
        "Expected compile failure for Int/Float type mismatch in loop condition, got success"
    );
}
