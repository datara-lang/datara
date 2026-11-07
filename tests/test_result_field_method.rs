//! PLAN_V153 N2 repro #12: a method call on `parsed.value` (a Result
//! field access whose payload is a STRUCT) crashed the Cranelift ABI
//! (arity assert) because:
//!   1. GetField did not propagate the payload's class into
//!      val_to_class — the method call dispatched through the name-
//!      only fallback to an unrelated 1-arg builtin (`args_get`);
//!   2. DCE resolved method calls named like collection intrinsics
//!      ("get") as intrinsics WITHOUT keeping the same-named user
//!      methods reachable — the callee was removed from the module
//!      before declaration.
//! Both fixed: GetField payload-class propagation + DCE keeps
//! same-named candidates reachable when an intrinsic resolves.

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
fn result_field_method_call_works() {
    let src = r#"
use stdlib.devbox.view

fn main() {
    val items = ["--name", "world", "--verbose", "pos1"]
    val parsed = make_args_result(items)
    if parsed.is_success {
        val a = parsed.value
        out a.get("--name")
        if a.has("verbose") {
            out "VERBOSE_ON"
        }
        out int_to_str(a.rest_len())
    }
}
"#;
    let (out, code) = run_native(src, "result_field_method.dtr");
    assert_eq!(code, 0, "no ABI assert, no dispatch to unrelated builtins");
    // --verbose consumes pos1 as its value (the documented grammar).
    assert_eq!(out, "world\n0");
}

/// The DCE fix: a user method named like an intrinsic ("get") on a
/// NON-Result receiver (plain struct var) survives optimization.
#[test]
fn user_get_method_survives_dce() {
    let src = r#"
use stdlib.devbox.view

fn main() {
    val a = make_args(["--k", "v"])
    out a.get("--k")
}
"#;
    let (out, code) = run_native(src, "user_get_dce.dtr");
    assert_eq!(code, 0);
    assert_eq!(out, "v");
}
