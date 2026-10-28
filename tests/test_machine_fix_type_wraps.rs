//! M2 Diagnostics Gold, fixes 4/5 and 5/5 (PLAN_V150).
//!
//! Fix 4/5: a Str initializer/assignment where Int/Float is expected emits
//! E-TYPE-001 with a machine-applicable fix that source-slices the offending
//! expression and wraps it in `str_to_int(...)` / `str_to_float(...)`.
//! Fix 5/5: a `?` on a non-Result operand emits its diagnostic with a
//! machine fix that removes the trailing `?`.
//!
//! Every fix is validated two ways: the replacement text, and an
//! apply-the-edit-and-recompile round trip — the fixed source must compile.

use forgen::diagnostics::MachineFix;
use forgen::driver::ForgenCompiler;

/// Apply every machine fix of one record set to the source (char-based
/// line/col spans, matching the lexer) and return the edited text.
fn apply_fix(source: &str, fix: &MachineFix) -> String {
    let mut lines: Vec<String> = source.split('\n').map(|l| l.to_string()).collect();
    assert_eq!(
        fix.span.start_line, fix.span.end_line,
        "single-line fixes only in this harness"
    );
    let li = fix.span.start_line - 1;
    let chars: Vec<char> = lines[li].chars().collect();
    let s = fix.span.start_col - 1;
    let e = fix.span.end_col.min(chars.len());
    let mut edited: String = chars[..s].iter().collect();
    edited.push_str(&fix.replacement);
    edited.extend(chars[e..].iter());
    lines[li] = edited;
    lines.join("\n")
}

fn first_fix_with_prefix(compiler: &ForgenCompiler, source: &str, file: &str) -> MachineFix {
    let res = compiler.check_source(source, file);
    assert!(!res.success, "source must fail to compile: {}", source);
    res.diagnostic_records
        .iter()
        .find_map(|d| d.machine_fix.clone())
        .unwrap_or_else(|| {
            panic!(
                "expected a machine fix; records: {:?}",
                res.diagnostic_records
                    .iter()
                    .map(|d| (&d.code, &d.message))
                    .collect::<Vec<_>>()
            )
        })
}

#[test]
fn str_to_int_wrap_fix_on_declaration() {
    let source = "fn main() {\n    let n: Int = \"42\"\n    out n\n}\n";
    let compiler = ForgenCompiler::new("release");
    let fix = first_fix_with_prefix(&compiler, source, "fix_int_decl.dtr");
    assert_eq!(
        fix.replacement, "str_to_int(\"42\")",
        "fix must wrap the sliced expression"
    );

    // The applied fix must compile.
    let fixed = apply_fix(source, &fix);
    let res = compiler.check_source(&fixed, "fix_int_decl_fixed.dtr");
    assert!(
        res.success,
        "fixed source must compile: {}\n{}",
        fixed, res.diagnostics
    );
}

#[test]
fn str_to_float_wrap_fix_on_declaration() {
    let source = "fn main() {\n    let f: Float = \"3.5\"\n    out f\n}\n";
    let compiler = ForgenCompiler::new("release");
    let fix = first_fix_with_prefix(&compiler, source, "fix_float_decl.dtr");
    assert_eq!(fix.replacement, "str_to_float(\"3.5\")");

    let fixed = apply_fix(source, &fix);
    let res = compiler.check_source(&fixed, "fix_float_decl_fixed.dtr");
    assert!(res.success, "fixed source must compile: {}\n{}", fixed, res.diagnostics);
}

#[test]
fn str_to_int_wrap_fix_on_assignment() {
    let source = "fn main() {\n    mut n = 0\n    n = \"7\"\n    out n\n}\n";
    let compiler = ForgenCompiler::new("release");
    let fix = first_fix_with_prefix(&compiler, source, "fix_int_assign.dtr");
    assert_eq!(
        fix.replacement, "str_to_int(\"7\")",
        "assignment RHS must get the wrap fix too"
    );

    let fixed = apply_fix(source, &fix);
    let res = compiler.check_source(&fixed, "fix_int_assign_fixed.dtr");
    assert!(res.success, "fixed source must compile: {}\n{}", fixed, res.diagnostics);
}

#[test]
fn question_mark_removal_fix() {
    // `x?` on a plain Int operand: the mechanical fix removes the `?`.
    let source = "fn main() {\n    let x = 5\n    val y = x?\n    out y\n}\n";
    let compiler = ForgenCompiler::new("release");
    let res = compiler.check_source(source, "fix_qmark.dtr");
    assert!(!res.success, "`?` on a non-Result must be an error");

    let fix = res
        .diagnostic_records
        .iter()
        .find_map(|d| d.machine_fix.clone())
        .unwrap_or_else(|| {
            panic!(
                "'?' misuse must carry a machine fix; records: {:?}",
                res.diagnostic_records
                    .iter()
                    .map(|d| (&d.code, &d.message))
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        fix.replacement, "x",
        "fix must drop the trailing '?' from the sliced expression"
    );

    let fixed = apply_fix(source, &fix);
    let res = compiler.check_source(&fixed, "fix_qmark_fixed.dtr");
    assert!(res.success, "fixed source must compile: {}\n{}", fixed, res.diagnostics);
}
