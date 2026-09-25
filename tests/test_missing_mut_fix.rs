//! M2 Diagnostics Gold, fix 2/5 (PLAN_V150): assigning to an immutable
//! `val` emits E-BORROW-002 with a machine-applicable fix that rewrites
//! the DECLARATION keyword `val` -> `mut` at the declaration line - the
//! assignment site itself needs no edit, so the fix span differs from the
//! diagnostic span.

use forgen::driver::ForgenCompiler;

#[test]
fn immutable_assign_carries_declaration_fix() {
    let source = "fn main() {\n    val x = 1\n    x = 2\n    out x\n}\n";
    let compiler = ForgenCompiler::new("release");
    let res = compiler.check_source(source, "mut_fix_probe.dtr");
    assert!(!res.success, "immutable assignment must be an error");

    let with_fix: Vec<_> = res
        .diagnostic_records
        .iter()
        .filter(|d| d.machine_fix.is_some())
        .collect();
    assert!(
        !with_fix.is_empty(),
        "E-BORROW-002 must carry a machine fix; records: {:?}",
        res.diagnostic_records
            .iter()
            .map(|d| (&d.code, &d.message))
            .collect::<Vec<_>>()
    );
    let d = with_fix[0];
    assert_eq!(d.code, "E-BORROW-002");
    let fix = d.machine_fix.as_ref().unwrap();
    assert_eq!(
        fix.replacement, "mut",
        "fix must rewrite the declaration keyword"
    );
    // The fix span points at the declaration (line 2), not the assignment (line 3).
    assert_eq!(
        fix.span.start_line, 2,
        "fix must target the declaration line"
    );
}
