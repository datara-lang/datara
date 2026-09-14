//! Strict typechecker for ordering comparisons (v1.3.2 M1).
//!
//! SPEC_V1 Gate 7: "No implicit widening between `Int` and `Float`. Mixing
//! `Int` and `Float` in binary operations without an explicit cast is a
//! compile-time type mismatch error." and SPEC_V1 principle 2 (fail-closed):
//! type mismatches fail compilation instead of silent fallback.
//!
//! Before this suite, `i < 3.5` (Int vs Float) compiled because lowering
//! widened silently, and `while i < "three"` (Int vs Str) compiled because
//! the ordering check only required each operand to be orderable on its own.
//! Both are rejected now with `E-TYPE-008`.

use forgen::diagnostics::ErrorCode;
use forgen::driver::ForgenCompiler;

/// Compile a .dtr snippet through the full check pipeline and return
/// (success, formatted diagnostics). Error-path results carry the formatted
/// diagnostics string only (`diagnostic_records` stays empty on failure), so
/// code assertions match on the formatted text, like the rest of this suite.
fn check(source: &str) -> (bool, String) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.check_source(source, "test_typechecker_strictness.dtr");
    (res.success, res.diagnostics)
}

fn assert_has_code(diag: &str, code: &str, context: &str) {
    assert!(
        diag.contains(code),
        "expected {} in diagnostics for {}, got:\n{}",
        code,
        context,
        diag
    );
}

#[test]
fn test_int_lt_float_literal_rejected() {
    // The v1.3.1 known bug: Int loop counter compared against a Float
    // literal widened silently and compiled.
    let src = r#"
fn main() {
    mut i: Int = 0
    while i < 3.5 {
        i = i + 1
    }
}
"#;
    let (success, diag) = check(src);
    assert!(
        !success,
        "Int < Float comparison must be rejected, got success"
    );
    assert_has_code(
        &diag,
        ErrorCode::TypeIncomparableOperands.as_str(),
        "Int < Float",
    );
}

#[test]
fn test_float_lt_int_literal_rejected() {
    // Same rule in the other direction: Float variable against an Int literal.
    let src = r#"
fn main() {
    mut f: Float = 0.0
    while f < 10 {
        f = f + 0.5
    }
}
"#;
    let (success, diag) = check(src);
    assert!(
        !success,
        "Float < Int comparison must be rejected, got success"
    );
    assert_has_code(
        &diag,
        ErrorCode::TypeIncomparableOperands.as_str(),
        "Float < Int",
    );
}

#[test]
fn test_int_lt_string_rejected() {
    // The v1.3.1 known bug: number-vs-string ordering compiled because both
    // operands were individually orderable.
    let src = r#"
fn main() {
    mut i: Int = 0
    while i < "three" {
        i = i + 1
    }
}
"#;
    let (success, diag) = check(src);
    assert!(
        !success,
        "Int < Str comparison must be rejected, got success"
    );
    assert_has_code(
        &diag,
        ErrorCode::TypeIncomparableOperands.as_str(),
        "Int < Str",
    );
}

#[test]
fn test_string_lt_float_rejected() {
    // Cross-family ordering (Str vs Float) is rejected as well.
    let src = r#"
fn main() {
    let s: Str = "abc"
    let f: Float = 1.0
    if s > f {
        println("greater")
    }
}
"#;
    let (success, diag) = check(src);
    assert!(
        !success,
        "Str > Float comparison must be rejected, got success"
    );
    assert_has_code(
        &diag,
        ErrorCode::TypeIncomparableOperands.as_str(),
        "Str > Float",
    );
}

#[test]
fn test_mixed_ordering_operators_rejected() {
    // All four ordering operators enforce the same-type rule.
    let cases = [("<=", "i <= 2.5"), (">", "i > 2.5"), (">=", "i >= 2.5")];
    for (op_name, cond) in cases {
        let src = format!(
            r#"
fn main() {{
    let i: Int = 1
    if {cond} {{
        println("yes")
    }}
}}
"#
        );
        let (success, diag) = check(&src);
        assert!(
            !success,
            "Int {} Float must be rejected, got success",
            op_name
        );
        assert_has_code(
            &diag,
            ErrorCode::TypeIncomparableOperands.as_str(),
            &format!("Int {} Float", op_name),
        );
    }
}

#[test]
fn test_explicit_conversion_then_compare_accepted() {
    // The explicit conversion path that exists in the language today:
    // `str_to_int` / `str_to_float` produce a same-type operand, and the
    // comparison then passes. (There is no `as`/`.to_float()` cast
    // expression in the current grammar; see v1.3.2 plan notes.)
    let src = r#"
fn main() {
    let n = str_to_int("42")
    if n > 40 {
        println("big")
    }
    let f = str_to_float("2.5")
    if f < 3.0 {
        println("small")
    }
}
"#;
    let (success, diag) = check(src);
    assert!(
        success,
        "explicit str_to_* conversion then same-type compare must pass:\n{}",
        diag
    );
}

#[test]
fn test_same_type_comparisons_accepted() {
    // Spec-allowed literal cases: same-type comparisons keep working for
    // every ordering operator, in both literal positions.
    let src = r#"
fn main() {
    let i: Int = 5
    if i < 10 && i <= 5 && i > 1 && i >= 5 {
        println("int ok")
    }
    let f: Float = 1.5
    if f < 2.0 && f <= 1.5 && f > 0.5 && f >= 1.5 {
        println("float ok")
    }
    if 2 < 3 && 2.0 < 3.0 {
        println("literal ok")
    }
    let s: Str = "abc"
    if s < "abd" {
        println("str ok")
    }
    let c: Char = 'a'
    if c < 'b' {
        println("char ok")
    }
}
"#;
    let (success, diag) = check(src);
    assert!(
        success,
        "same-type ordering comparisons must pass:\n{}",
        diag
    );
}

#[test]
fn test_float_literal_declaration_still_strict() {
    // Declarations were already strict before M1 and stay strict: an Int
    // literal does not become a Float at the declaration site
    // (is_compatible has no numeric widening).
    let src = r#"
fn main() {
    let x: Float = 1
    println(x)
}
"#;
    let (success, diag) = check(src);
    assert!(!success, "let x: Float = 1 must stay rejected");
    assert_has_code(
        &diag,
        ErrorCode::TypeMismatch.as_str(),
        "let x: Float = 1 declaration",
    );
}

#[test]
fn test_number_vs_string_equality_still_rejected() {
    // Equality with an incompatible type was already rejected via the
    // compatibility path (E-TYPE-004) and keeps working after M1.
    let src = r#"
fn main() {
    let i: Int = 5
    if i == "three" {
        println("eq")
    }
}
"#;
    let (success, diag) = check(src);
    assert!(!success, "Int == Str must stay rejected");
    assert_has_code(
        &diag,
        ErrorCode::TypeInvalidBinaryOp.as_str(),
        "Int == Str equality",
    );
}

#[test]
fn test_dynamic_val_comparisons_still_permissive() {
    // An explicitly `Val`-annotated binding is the documented gradual
    // dynamic container: its operands are resolved dynamically and stay
    // outside the static ordering check.
    let src = r#"
fn main() {
    let v: Val = 1
    if v < 2.5 {
        println("dyn")
    }
}
"#;
    let (success, diag) = check(src);
    assert!(
        success,
        "Val dynamic operands remain permissive by design:\n{}",
        diag
    );
}
