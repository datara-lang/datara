use forgen::driver::ForgenCompiler;

#[test]
fn test_v143_unsafe_fn_positive() {
    let compiler = ForgenCompiler::new("release");
    let code = r#"
unsafe fn raw_math(x: Int) -> Int {
    let p = mem_alloc(8)
    ptr_write_i64(p, 0, x * 2)
    let res = ptr_read_i64(p, 0)
    mem_free(p)
    return res
}

fn main() {
    unsafe(justification: "calling unsafe fn") {
        let val = raw_math(21)
        out val
    }
}
"#;
    let res = compiler.compile_source(code, "v143_unsafe_fn_pos.dtr", None);
    assert!(
        res.success,
        "Compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let (stdout, _, code_res, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code_res, 0);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn test_v143_unsafe_fn_negative_requires_unsafe_block() {
    let compiler = ForgenCompiler::new("release");
    let code = r#"
unsafe fn dangerous_op() -> Int {
    return 100
}

fn main() {
    let v = dangerous_op()
    out v
}
"#;
    let res = compiler.compile_source(code, "v143_unsafe_fn_neg.dtr", None);
    assert!(
        !res.success,
        "Expected failure when calling unsafe fn without unsafe block"
    );
    assert!(
        res.diagnostics
            .contains("Call to unsafe function 'dangerous_op' requires 'unsafe(justification:"),
        "Expected unsafe violation diagnostic, got:\n{}",
        res.diagnostics
    );
}
