use forgen::driver::ForgenCompiler;

#[test]
fn test_v143_stack_alloc_execution() {
    let compiler = ForgenCompiler::new("release");
    let code = r#"
fn main() {
    unsafe(justification: "stack buffer allocation") {
        let p = stack_alloc(32)
        ptr_write_i64(p, 0, 12345)
        let v = ptr_read_i64(p, 0)
        out v
    }
}
"#;
    let res = compiler.compile_source(code, "v143_stack_alloc.dtr", None);
    assert!(res.success, "Compilation failed: {:?}\n{}", res.error, res.diagnostics);
    let (stdout, _, code_res, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code_res, 0);
    assert_eq!(stdout.trim(), "12345");
}
