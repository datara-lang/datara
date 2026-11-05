//! PLAN_V152 N2: static constructor in a MemberAccess chain —
//! `Outcome.ok(1).is_success` on a GenericInstance call result.
//! Verified working 2026-11-05 (probe); pinned as a regression test.

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
fn static_ctor_member_chain() {
    let src = r#"
fn main() {
    val ok = Outcome.ok(1).is_success
    out fmt"success={ok}"
    val err = Outcome.err("boom").is_success
    out fmt"err_success={err}"
    val msg = Outcome.err("boom").err()
    out fmt"msg={msg}"
}
"#;
    let (out, code) = run_native(src, "static_ctor_chain.dtr");
    assert_eq!(code, 0);
    assert_eq!(out, "success=true\nerr_success=false\nmsg=boom");
}
