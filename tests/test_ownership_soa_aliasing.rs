// Milestone 2 - Ownership Edge Cases: SoA field access and aliasing tests.
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
// Test 1: Two distinct Int fields read in sequence must not produce a panic.
// The compiler may accept or reject the program, but it must never crash.
// ---------------------------------------------------------------------------
#[test]
fn test_soa_two_fields_no_alias() {
    let src = r#"fn sum_fields(x: Int, y: Int) -> Int {
    let a = x
    let b = y
    return a + b
}
fn main() {
    out sum_fields(1, 2)
}
"#;

    let res = compile(src.to_string(), "test_soa_two_fields_no_alias.dtr");
    assert_graceful(&res, "soa_two_fields_no_alias");
}

// ---------------------------------------------------------------------------
// Test 2: Destroying a struct instance and then reading it again must be
// rejected. Scalar Int values are copyable, so a struct instance with an
// explicit `destroy` is the ownership-tracked subject here.
// ---------------------------------------------------------------------------
#[test]
fn test_soa_move_then_reuse_rejected() {
    let src = r#"struct Token {
    id: Int
}

fn drop_token(t: Token) {
    destroy(t)
}

fn main() {
    let val = Token { id: 42 }
    drop_token(val)
    out val.id
}
"#;

    let res = compile(src.to_string(), "test_soa_move_then_reuse_rejected.dtr");
    assert_graceful(&res, "soa_move_then_reuse_rejected");
    assert!(
        !res.success,
        "Expected compile failure for use-after-destroy, got success"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Attempting to reassign an immutable binding inside a while loop
// must be rejected gracefully.
// ---------------------------------------------------------------------------
#[test]
fn test_loop_move_conditional_rejected() {
    let src = r#"fn main() {
    mut i = 0
    let v = 10
    while i < 3 {
        i = i + 1
        v = v + 1
    }
    out v
}
"#;

    let res = compile(src.to_string(), "test_loop_move_conditional_rejected.dtr");
    assert_graceful(&res, "loop_move_conditional_rejected");
    assert!(
        !res.success,
        "Expected compile failure for immutable-reassign inside loop, got success"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Assigning to an immutable `let` binding must be rejected.
// ---------------------------------------------------------------------------
#[test]
fn test_immutable_binding_assignment_rejected() {
    let src = r#"fn main() {
    let x = 5
    x = 10
    out x
}
"#;

    let res = compile(
        src.to_string(),
        "test_immutable_binding_assignment_rejected.dtr",
    );
    assert_graceful(&res, "immutable_binding_assignment_rejected");
    assert!(
        !res.success,
        "Expected compile failure for immutable binding reassignment, got success"
    );
}
