//! v1.4.0 allocator tiers: `@arena` and `@pool(n)`.
//!
//! Covers the acceptance matrix:
//! * an `@arena` function allocating a million strings produces exactly the
//!   same result as its unannotated twin,
//! * returning a region-allocated heap value is rejected with E1401,
//! * `@pool(4)` with five provable (constant-count) allocations is rejected
//!   at compile time with E1406,
//! * `@pool` with a dynamic count compiles, works within capacity, and traps
//!   at runtime (non-zero exit) when the capacity is exceeded.

use forgen::driver::ForgenCompiler;

/// Windows linker discovery reads `ProgramFiles(x86)` to locate
/// vswhere/BuildTools. Some harnesses (sandboxed shells, stripped CI
/// containers) drop the variable entirely; restore the conventional
/// defaults so the machine paths resolve. Mirrors the hardcoded fallback
/// the other native tests use.
static MSVC_ENV: std::sync::Once = std::sync::Once::new();

fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| {
        // SAFETY: `set_var` is process-global; the call happens once,
        // only when the variable is absent, and before any linker
        // subprocess is spawned by this test process.
        unsafe {
            if std::env::var_os("ProgramFiles(x86)").is_none() {
                std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
            }
            if std::env::var_os("ProgramFiles").is_none() {
                std::env::set_var("ProgramFiles", r"C:\Program Files");
            }
        }
    });
}

fn write_source(name: &str, source: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(name);
    std::fs::write(&path, source).expect("must write test source");
    path
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("exe"));
    let _ = std::fs::remove_file(path.with_extension("pdb"));
    let _ = std::fs::remove_file(path.with_extension("obj"));
}

/// The arena and non-arena twins must agree; the arena version runs its
/// million string allocations through the runtime's bump region and resets
/// the whole region on return.
#[test]
fn arena_million_strings_matches_baseline() {
    let source = r#"
fn churn_baseline(n: Int) -> Int {
    mut acc = 0
    for i in 0..n {
        let s = "item_" + int_to_str(i)
        acc = acc + s.len()
    }
    return acc
}

@arena
fn churn_arena(n: Int) -> Int {
    mut acc = 0
    for i in 0..n {
        let s = "item_" + int_to_str(i)
        acc = acc + s.len()
    }
    return acc
}

fn main() {
    let n = 1000000
    let a = churn_baseline(n)
    let b = churn_arena(n)
    if a == b {
        out a
    } else {
        out 0
    }
}
"#;
    let path = write_source("v140_arena_million.dtr", source);
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(
        res.success,
        "arena compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    assert_eq!(code, 0, "execution failed: {}", stderr);
    let value: i64 = stdout.trim().parse().expect("stdout must be an integer");
    assert!(value > 0, "sum of lengths must be positive, got {}", value);
    // Independent check: sum(len("item_" + str(i))) for i in 0..1_000_000.
    let expected: i64 = (0..1_000_000i64)
        .map(|i| 5 + i.to_string().len() as i64)
        .sum();
    assert_eq!(value, expected, "arena result must equal the baseline sum");
    cleanup(&path);
}

/// Returning a list literal allocated in an `@arena` function is a region
/// escape: the arena is reclaimed on return, so the value would dangle.
#[test]
fn arena_escape_is_rejected() {
    let source = r#"
@arena
fn make_list() -> Int {
    let xs = [1, 2, 3]
    return xs
}

fn main() {
    out make_list()
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "v140_arena_escape.dtr", None);
    assert!(!res.success, "escape must be rejected");
    assert!(
        res.diagnostics.contains("E1401"),
        "must report E1401 (arena escape), got:\n{}",
        res.diagnostics
    );
}

/// A value bound to a local and returned from the region function is the
/// same escape, caught through the local-tracking rule.
#[test]
fn arena_escape_through_local_is_rejected() {
    let source = r#"
struct Box { payload: Int }

@arena
fn make_box() -> Int {
    mut b = Box { payload: 1 }
    b = Box { payload: 2 }
    return b
}

fn main() {
    out make_box()
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "v140_arena_escape_local.dtr", None);
    assert!(!res.success, "escaping local must be rejected");
    assert!(
        res.diagnostics.contains("E1401"),
        "must report E1401, got:\n{}",
        res.diagnostics
    );
}

/// Five allocations outside any loop cannot fit `@pool(4)`: provable at
/// compile time, so the compiler rejects it before codegen.
#[test]
fn pool_provable_overflow_is_rejected_at_compile_time() {
    let source = r#"
struct P { a: Int }

@pool(4)
fn five() -> Int {
    let a = P { a: 1 }
    let b = P { a: 2 }
    let c = P { a: 3 }
    let d = P { a: 4 }
    let e = P { a: 5 }
    return a.a + b.a + c.a + d.a + e.a
}

fn main() {
    out five()
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "v140_pool_provable.dtr", None);
    assert!(!res.success, "provable pool overflow must be rejected");
    assert!(
        res.diagnostics.contains("E1406"),
        "must report E1406 (pool capacity), got:\n{}",
        res.diagnostics
    );
}

/// The same function inside `@pool(8)` compiles and returns the right value.
#[test]
fn pool_within_capacity_runs() {
    let source = r#"
struct P { a: Int }

@pool(8)
fn five() -> Int {
    let a = P { a: 1 }
    let b = P { a: 2 }
    let c = P { a: 3 }
    let d = P { a: 4 }
    let e = P { a: 5 }
    return a.a + b.a + c.a + d.a + e.a
}

fn main() {
    out five()
}
"#;
    let path = write_source("v140_pool_within.dtr", source);
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(
        res.success,
        "pool compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    assert_eq!(code, 0, "execution failed: {}", stderr);
    assert_eq!(stdout.trim(), "15");
    cleanup(&path);
}

/// A dynamic count cannot be proven at compile time, so the program compiles
/// and the runtime slot counter traps when the pool is exhausted. The trap is
/// observed through the executable's exit code.
#[test]
fn pool_dynamic_overflow_traps_at_runtime() {
    let source = r#"
struct P { a: Int }

@pool(4)
fn fill(count: Int) -> Int {
    mut sum = 0
    for i in 0..count {
        let p = P { a: i }
        sum = sum + p.a
    }
    return sum
}

fn main() {
    out fill(3)
    out fill(10)
}
"#;
    let path = write_source("v140_pool_dynamic.dtr", source);
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(
        res.success,
        "dynamic pool program must compile: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce native executable");

    // One run covers both sides: the within-capacity call prints 3, then the
    // over-capacity call trips the slot counter, the process dies with the
    // dedicated trap message and a non-zero exit code.
    let (stdout, trap_stderr, trap_code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    assert_ne!(
        trap_code, 0,
        "pool overflow must trap with a non-zero exit code"
    );
    assert!(
        trap_stderr.contains("@pool capacity exceeded"),
        "trap must name the pool capacity, got:\n{}",
        trap_stderr
    );
    assert!(
        stdout.contains("3"),
        "the within-capacity call must have completed and printed 3 first, got:\n{}",
        stdout
    );
    cleanup(&path);
}
