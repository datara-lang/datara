//! PLAN_V152 N2: flow-sensitive generic inference — after push_back(Int)
//! on a Deque<T>, pop_back() returns Int (not T). The checker binds the
//! TypeParam from the concrete argument type.

use forgen::driver::ForgenCompiler;

#[test]
fn test_generic_type_binding_push_pop() {
    let src = r#"
use stdlib.collections.deque.deque_new

fn main() {
    val dq = deque_new()
    dq.push_back(42)
    val v = dq.pop_back()
    out int_to_str(v)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "generic_inference.dtr", None);
    assert!(
        res.success,
        "generic type binding must work: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "42", "pop_back must return Int: {stdout}");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
}
