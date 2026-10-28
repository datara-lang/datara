//! M7 TSU (Type-Set Unboxing) end-to-end tests.
//!
//! The dynamic value path: heterogeneous lists lower each element to a
//! NaN-boxed `Val` (one I64 word — zero heap boxes for primitives), `is`
//! checks the runtime tag, `.as_int()/.as_float()` convert checked
//! (Outcome), `.to_int()/.to_float()` are fail-loud, `.to_str()` renders
//! any tag, `if x is T` narrows (unboxes) x for the whole then-branch, and
//! a val value leaking into a static context is E-ANY-001.
//!
//! Every behavioral test runs on the native (Cranelift AOT) and JIT tiers;
//! the smoke block also covers the LLVM tier where available.

use forgen::driver::ForgenCompiler;

fn run_native(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation must succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "native run exited {}: {}", code, stdout);
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

fn run_jit(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

fn assert_both_tiers(source: &str, name: &str, expected: &str) {
    assert_eq!(run_native(source, name), expected, "native tier");
    assert_eq!(
        run_jit(source, &format!("{}_jit", name)),
        expected,
        "jit tier"
    );
}

/// 1. `is` tag checks on a heterogeneous list, all four primitive tags.
#[test]
fn tsu_is_checks_all_tags() {
    let src = r#"
fn main() {
    val xs = [7, 2.5, "hi", true]
    out xs[0] is Int
    out xs[1] is Float
    out xs[2] is Str
    out xs[3] is Bool
}
"#;
    assert_both_tiers(src, "tsu_is_tags", "true\ntrue\ntrue\ntrue");
}

/// 2. Negative `is` checks: each element rejects the other tags.
#[test]
fn tsu_is_negative_checks() {
    let src = r#"
fn main() {
    val xs = [7, 2.5]
    out xs[0] is Str
    out xs[0] is Float
    out xs[1] is Int
    out xs[1] is Bool
}
"#;
    assert_both_tiers(src, "tsu_is_negative", "false\nfalse\nfalse\nfalse");
}

/// 3. Dynamic printing: `out` on a val value dispatches on the tag.
#[test]
fn tsu_print_dynamic_values() {
    let src = r#"
fn main() {
    val xs = [42, 3.5, "text", false]
    out xs[0]
    out xs[1]
    out xs[2]
    out xs[3]
}
"#;
    assert_both_tiers(src, "tsu_print", "42\n3.5\ntext\nfalse");
}

/// 4. Checked conversions: `.as_int()`/`.as_float()` yield Outcome.
///    Ok on the right tag, err otherwise, composable with `or`.
#[test]
fn tsu_as_checked_conversions() {
    let src = r#"
fn main() {
    val xs = [7, 2.5]
    out xs[0].as_int() or -1
    out xs[1].as_int() or -1
    out xs[0].as_float() or -1.0
    out xs[1].as_float() or -1.0
}
"#;
    assert_both_tiers(src, "tsu_as_checked", "7\n-1\n-1\n2.5");
}

/// 5. Tagged string rendering: `.to_str()` on all four tags.
#[test]
fn tsu_to_str_renders_every_tag() {
    let src = r#"
fn main() {
    val xs = [9, 1.25, "raw", false]
    out xs[0].to_str()
    out xs[1].to_str()
    out xs[2].to_str()
    out xs[3].to_str()
}
"#;
    assert_both_tiers(src, "tsu_to_str", "9\n1.25\nraw\nfalse");
}

/// 6. Narrowing: `if x is Int` unboxes x for the whole then-branch.
///    The narrowed value is a REAL Int (passable to a static-typed parameter).
#[test]
fn tsu_narrowing_unboxes_in_branch() {
    let src = r#"
fn describe(v: Int) -> Str {
    return fmt"int {v}"
}
fn main() {
    val xs = [42, "hi"]
    val x = xs[0]
    if x is Int {
        out describe(x)
    }
    if xs[1] is Str {
        out xs[1].to_str()
    }
    out xs[0].as_int() or -1
}
"#;
    assert_both_tiers(src, "tsu_narrow", "int 42\nhi\n42");
}

/// 7. Degenerate set: `is` on a STATICALLY-typed value compiles to a
///    compile-time constant (true for the exact type, false otherwise),
///    zero runtime tag checks.
#[test]
fn tsu_degenerate_static_is() {
    let src = r#"
fn main() {
    val n = 5
    out n is Int
    out n is Str
    out 2.5 is Float
    out "s" is Int
}
"#;
    assert_both_tiers(src, "tsu_degenerate", "true\nfalse\ntrue\nfalse");
}

/// 8. E-ANY-001: a dynamic value passed to a statically-typed parameter is
///    a compile ERROR with the narrow/convert hint (no silent reinterpretation).
#[test]
fn tsu_e_any_leak_is_error() {
    let src = r#"
fn takes_int(v: Int) -> Int {
    return v
}
fn main() {
    val xs = [42, "hi"]
    out takes_int(xs[0])
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(src, "tsu_eany.dtr", None);
    assert!(
        !res.success,
        "val leak into a static parameter must be a compile error"
    );
    assert!(
        res.diagnostics.contains("E-ANY-001"),
        "must emit E-ANY-001, got:\n{}",
        res.diagnostics
    );
    assert!(
        res.diagnostics.contains("is Int") || res.diagnostics.contains("as_int"),
        "the hint must mention narrowing/conversion, got:\n{}",
        res.diagnostics
    );
}

/// 9. The dynamic list survives function boundaries: elements read through
///    a List<Val> parameter keep their tags (list params record the Val elem).
#[test]
fn tsu_dynamic_list_across_call() {
    let src = r#"
fn second_of(items: List<Val>) -> Str {
    return items[1].to_str()
}
fn main() {
    val xs = [1, "two"]
    out second_of(xs)
    out xs[0].as_int() or -1
}
"#;
    assert_both_tiers(src, "tsu_across_call", "two\n1");
}

/// 10. Mixed literal of 5+ elements (the >5 create+append path) keeps tags.
#[test]
fn tsu_large_mixed_literal() {
    let src = r#"
fn main() {
    val xs = [1, "a", 2.0, true, 3, "b"]
    mut ints = 0
    mut strs = 0
    mut i = 0
    while i < 6 {
        if xs[i] is Int {
            ints = ints + 1
        }
        if xs[i] is Str {
            strs = strs + 1
        }
        i = i + 1
    }
    out int_to_str(ints)
    out int_to_str(strs)
}
"#;
    assert_both_tiers(src, "tsu_large_mixed", "2\n2");
}

/// 11. `explain E-ANY-001` is wired (non-empty explanation with examples).
#[test]
fn tsu_explain_any_leak() {
    let text = forgen::diagnostics::explain_data::explain_for_code("E-ANY-001")
        .expect("E-ANY-001 must have an explain entry");
    assert!(text.contains("[INVALID]") && text.contains("[VALID]"));
}

/// 12. LLVM-tier smoke for the dynamic path (tag checks + printing).
#[cfg(target_os = "windows")]
#[test]
fn tsu_llvm_tier_smoke() {
    let src = r#"
fn main() {
    val xs = [5, 1.5, "s", true]
    out xs[0] is Int
    out xs[2] is Str
    out xs[0]
    out xs[1]
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "tsu_llvm.dtr", None);
    assert!(res.success, "llvm smoke compile failed: {:?}", res.error);
    if let Some(exe) = res.exe_path {
        let (stdout, _e, code, _) = compiler
            .cranelift
            .run_executable(&exe, &[])
            .expect("run exe");
        assert_eq!(code, 0);
        assert_eq!(stdout.trim().replace("\r\n", "\n"), "true\ntrue\n5\n1.5");
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(exe.with_extension("obj"));
    }
}
