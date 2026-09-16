//! Closures with block bodies (deferred since 1.3.3).
//!
//! Implemented form: (a) the arrow-lambda with a statement block,
//! `x => { stmt; stmt; return expr }` (also `(a, b) => { ... }` and
//! `() => { ... }`). This is the lowest-risk path: only the parser changes
//! (src/parser/expr.rs `parse_lambda_body`), and the existing inline-lowering
//! lambda machinery is reused untouched. The alternative
//! `let f = fn(x: Int) -> Int { ... }` closure-expression form was NOT
//! implemented -- it would need a new anonymous-`fn` expression path through
//! parser/resolver/checker without adding semantics form (a) cannot express.
//!
//! Semantics pinned here:
//!   * a trailing `return expr` is the closure's result;
//!   * capture is by value for Int/Float/Bool/Str (same as arrow lambdas):
//!     the free variables are snapshotted at `let f = <lambda>` and writes
//!     inside the body never leak back to the enclosing variable;
//!   * passing a lambda to a function that invokes it uses static dispatch --
//!     the call site inlines the (single-`return`) function body with the
//!     lambda bound to the parameter (src/dmir/lowering/expr_call.rs);
//!   * an early `return` inside a block-lambda body is not a closure-level
//!     return (block lambdas are inline-lowered) and is not supported; only
//!     the trailing return acts as the result.

use forgen::driver::ForgenCompiler;

static MSVC_ENV: std::sync::Once = std::sync::Once::new();
fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| unsafe {
        if std::env::var_os("ProgramFiles(x86)").is_none() {
            std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
        }
        if std::env::var_os("ProgramFiles").is_none() {
            std::env::set_var("ProgramFiles", r"C:\Program Files");
        }
    });
}

fn write_source(dir: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("failed to write test source");
    path
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("exe"));
    let _ = std::fs::remove_file(path.with_extension("pdb"));
    let _ = std::fs::remove_file(path.with_extension("obj"));
}

fn run_stdout(path: &std::path::Path) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(path, None);
    assert!(
        res.success,
        "Compilation should succeed:\n{}",
        res.diagnostics
    );
    let (stdout, stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    if code != 0 {
        eprintln!("stderr: {}", stderr);
    }
    (stdout, code)
}

#[test]
fn test_block_lambda_with_three_statements() {
    ensure_msvc_env();
    let src = r#"fn main() {
    let f = x => {
        mut t = x * 2
        t = t + 1
        return t
    }
    println(int_to_str(f(5)))
}
"#;
    let path = write_source(&std::env::temp_dir(), "closure_block3.dtr", src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "11", "(5*2)+1 = 11");
    cleanup(&path);
}

#[test]
fn test_block_lambda_stored_and_called_twice() {
    ensure_msvc_env();
    let src = r#"fn main() {
    let f = x => {
        mut t = x
        t = t * 3
        return t
    }
    println(int_to_str(f(2)))
    println(int_to_str(f(4)))
}
"#;
    let path = write_source(&std::env::temp_dir(), "closure_twice.dtr", src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines, vec!["6", "12"], "the same lambda must be reusable");
    cleanup(&path);
}

#[test]
fn test_block_lambda_passed_as_argument() {
    ensure_msvc_env();
    let src = r#"fn apply<T>(g: T, v: Int) -> Int {
    return g(v)
}

fn main() {
    let f = x => {
        mut t = x
        t = t + 100
        return t
    }
    println(int_to_str(apply(f, 5)))
}
"#;
    let path = write_source(&std::env::temp_dir(), "closure_arg.dtr", src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "105", "static dispatch: g(v) inlined");
    cleanup(&path);
}

#[test]
fn test_block_lambda_captures_outer_by_value() {
    ensure_msvc_env();
    let src = r#"fn main() {
    mut outer = 100
    let g = z => {
        outer = outer + z
        return outer
    }
    let inside = g(5)
    println(int_to_str(inside))
    println(int_to_str(outer))
}
"#;
    let path = write_source(&std::env::temp_dir(), "closure_capture.dtr", src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines,
        vec!["105", "100"],
        "the body sees the snapshot (105) but the outer variable is untouched (100)"
    );
    cleanup(&path);
}

#[test]
fn test_recursion_via_named_fn_wrapper_with_block_lambda() {
    ensure_msvc_env();
    let src = r#"fn fib(n: Int) -> Int {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn main() {
    let step = x => {
        mut t = x
        t = t + 1
        return t
    }
    // step(step(0)) == 2, fib(2) == 1
    println(int_to_str(fib(step(step(0)))))
    println(int_to_str(fib(step(step(step(step(step(0))))))))
}
"#;
    let path = write_source(&std::env::temp_dir(), "closure_recursion.dtr", src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines, vec!["1", "5"], "fib(2)=1, fib(5)=5");
    cleanup(&path);
}

/// Disambiguation pin: a lambda body that scans as a map literal
/// (`{ "key": expr }`) must keep parsing as a map literal, not as a
/// statement block (the `arm_body_is_block` lookahead decides).
#[test]
fn test_map_literal_lambda_body_still_parses_as_map() {
    ensure_msvc_env();
    let src = r#"fn main() {
    let m = x => { "a": x }
    println((m(5))["a"])
}
"#;
    let path = write_source(&std::env::temp_dir(), "closure_map_literal.dtr", src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "5");
    cleanup(&path);
}
