//! PLAN_V153 E2 / repro #10b SOLVED: mut module globals work across
//! functions. The inliner used to rename a global into a per-call-site
//! __il local, severing cross-function mutability (counter bumped by an
//! inlined helper stayed 0 in the caller). Fixed: module globals keep
//! their names through inlining (splice_callee_inst checks the globals
//! set); the codegen's global_data_map path then handles them.

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
fn mut_global_bump_via_fn() {
    let src = r#"
mut counter = 0

fn bump() {
    counter = counter + 1
}

fn get() -> Int {
    return counter
}

fn main() {
    counter = 5
    bump()
    bump()
    out int_to_str(counter)
    out int_to_str(get())
}
"#;
    let (out, code) = run_native(src, "mut_global_crossfn.dtr");
    assert_eq!(code, 0);
    // Before the fix: "5\n0" (each function saw its own zeroed slot).
    assert_eq!(out, "7\n7");
}
