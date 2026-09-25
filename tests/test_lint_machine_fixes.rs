//! M2 Diagnostics Gold: machine-applicable fixes (PLAN_V150 M2). The
//! unused-variable fix renames ONLY the declaration to `_name` - safe by
//! construction because the variable is never read - and the fixed source
//! must still compile and run.

use forgen::lint::{self, LintProfile};
use std::path::Path;

#[test]
fn unused_variable_fix_renames_declaration_and_still_compiles() {
    let source = "fn main() {\n    val unused_thing = 5\n    mut counter = 0\n    counter = counter + 1\n    out counter\n}\n";
    let diags = lint::lint_source_with_profile(source, "fix_probe.dtr", LintProfile::Standard)
        .expect("source must lint");
    let unused: Vec<_> = diags
        .iter()
        .filter(|d| d.code == "style::unused_variable")
        .collect();
    assert_eq!(unused.len(), 1, "exactly one unused-variable diagnostic");
    let fix = unused[0].fix.as_ref().expect("fix must be attached");
    assert_eq!(fix.replacement, "_unused_thing");

    let fixed = lint::apply_fixes(source, &diags);
    assert!(
        fixed.contains("val _unused_thing = 5"),
        "declaration must be renamed, got:\n{fixed}"
    );
    assert!(!fixed.contains("val unused_thing"), "old name must be gone");

    // The fixed source must compile cleanly.
    let compiler = forgen::driver::ForgenCompiler::new("release");
    let res = compiler.compile_source_native(&fixed, "fix_probe_compiled.dtr", None);
    assert!(res.success, "fixed source must compile: {:?}", res.error);
    let exe = res.exe_path.expect("must produce executable");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "1", "runtime behavior must be unchanged");
    let _ = Path::new("fix_probe.dtr");
}
