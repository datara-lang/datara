//! PLAN_V152 N2 repro #10: a `const` declared in a use'd module read from
//! one of the module's FUNCTIONS returned 0.
//!
//! Root cause (found via `forgen inspect clif`): module globals lower to
//! PER-FUNCTION explicit stack slots on the Cranelift tier — there is no
//! shared storage. The main-prologue init wrote only main's slot; a
//! module function (`fn scaled(x) { x * SCALE }`) read its own zeroed
//! slot. A direct read in main worked only because the prologue's
//! AssignVar seeded the local symbol table.
//!
//! Fix: immutable Int consts are comptime-folded at every identifier
//! read (lowering: const_global_values). Mut globals keep slot semantics
//! — cross-function mut globals remain a documented limitation (repro
//! #10b: the shared data-slot path is future work).
//!
//! The fixture module stdlib/devbox/box.dtr is a repro fixture, not part
//! of the stdlib surface (guarded by the size-gate test's module scan? no
//! — it is excluded from docs coverage by design).

use forgen::driver::ForgenCompiler;

fn run_native(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation must succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    if code != 0 {
        eprintln!("stderr: {stderr}");
    }
    (stdout.trim().replace("\r\n", "\n"), code)
}

#[test]
fn module_const_read_from_module_function() {
    let src = r#"
use stdlib.devbox.box

fn main() {
    out int_to_str(scaled(5))
    out int_to_str(SCALE)
}
"#;
    let (out, code) = run_native(src, "module_const_crossfn.dtr");
    assert_eq!(code, 0);
    // Before the fix: "0\n1000" (the function read a zeroed slot while
    // main's direct read folded).
    assert_eq!(out, "5000\n1000");
}
