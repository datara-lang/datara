//! End-to-end tests for the `as?` (checked) and `as!` (strict) cast
//! variants (PLAN_V150 M0/W5, designed 2026-09-21).
//!
//!   * `x as? T` yields `Outcome<T>`: ok when the value fits `T`, an err
//!     object otherwise — unpacked with `or` / `?`, never a silent
//!     truncation.
//!   * `x as! T` yields the plain value when it fits and panics with
//!     "cast overflow: <v> does not fit <T>" otherwise (fail-loud).
//!   * the plain `as` cast keeps its documented truncating semantics
//!     (`300 as Int8` is 44) on BOTH the optimized and unoptimized paths.

use forgen::driver::ForgenCompiler;

fn compile_and_run(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(res.success, "compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    (stdout, code)
}

#[test]
fn test_checked_cast_ok_and_err() {
    let src = r#"
fn main() {
    out (100 as? Int8) or -1
    out (300 as? Int8) or -1
    out ((300 as? Int8) or 0) + 1
    out (255 as? UInt8) or -2
    out (256 as? UInt8) or -2
    // `as` binds tighter than unary minus, so a negative operand must be
    // carried in a variable (or parenthesized) to reach the fit check.
    val neg5 = -5
    out (neg5 as? Int8) or -3
}
"#;
    let (stdout, _) = compile_and_run(src, "test_as_checked.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines,
        vec!["100", "-1", "1", "255", "-2", "-5"],
        "checked cast fit/overflow split is wrong"
    );
}

#[test]
fn test_checked_cast_question_propagation() {
    let src = r#"
fn checked(n: Int) -> Outcome<Int8> {
    return n as? Int8
}

fn rethrow(n: Int) -> Outcome<Int8> {
    let ok = checked(n)?
    return Outcome.ok(ok)
}

fn main() {
    out rethrow(100) or -1
    out rethrow(300) or -1
}
"#;
    let (stdout, _) = compile_and_run(src, "test_as_question.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["100", "-1"]);
}

#[test]
fn test_strict_cast_ok_values() {
    let src = r#"
fn main() {
    out (127 as! Int8)
    val neg128 = -128
    out (neg128 as! Int8)
    out (255 as! UInt8)
    val neg5 = -5
    out (neg5 as! Int8)
}
"#;
    let (stdout, _) = compile_and_run(src, "test_as_strict_ok.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["127", "-128", "255", "-5"]);
}

#[test]
fn test_strict_cast_panics_on_overflow() {
    let src = r#"
fn main() {
    out (300 as! Int8)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "test_as_strict_ovf.dtr", None);
    assert!(res.success, "compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = stdout;
    assert_ne!(code, 0, "strict cast overflow must exit non-zero");
    assert!(
        stderr.contains("cast overflow: 300 does not fit Int8"),
        "panic must name the value and target type, stderr: {}",
        stderr
    );
}

#[test]
fn test_plain_cast_truncates_both_paths() {
    let src = r#"
fn main() {
    out (300 as Int8)
    out (300 as UInt8)
}
"#;
    let (stdout, _) = compile_and_run(src, "test_as_plain.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    // 300 = 0x12C -> Int8 truncates to 44, UInt8 to 44.
    assert_eq!(lines, vec!["44", "44"]);
}

#[test]
fn test_checked_cast_non_integer_rejected() {
    let src = r#"
fn main() {
    out ("42" as? Int) or -1
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "test_as_str_src.dtr", None);
    assert!(
        !res.success,
        "checked cast from Str must be a compile-time error"
    );
    assert!(
        res.error
            .as_deref()
            .map(|e| e.contains("integer source and target types"))
            .unwrap_or(false),
        "diagnostic must explain the integer-only scope: {:?}",
        res.error
    );
}
