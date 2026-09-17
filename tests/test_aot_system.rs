use forgen::driver::ForgenCompiler;
use std::fs;

fn run_datara(code: &str, tag: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(code, tag, None);
    assert!(
        res.success,
        "Compilation failed for {}: {:?}\n{}",
        tag, res.error, res.diagnostics
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");

    let _ = fs::remove_file(&exe);
    let _ = fs::remove_file(exe.with_extension("obj"));
    (stdout.trim().replace("\r\n", "\n"), code)
}

#[test]
fn test_system_and_exec_variations() {
    let code = r#"
fn test_system_concat() -> Int {
    let a = "echo "
    let b = "system_concat_ok"
    let cmd = a + b
    mut rc = 0
    unsafe(justification: "test") {
        rc = system(cmd)
    }
    return rc
}

fn test_exec_concat() -> Str {
    let a = "echo "
    let b = "exec_concat_ok"
    let cmd = a + b
    mut res_out = ""
    unsafe(justification: "test") {
        res_out = exec(cmd)
    }
    return res_out
}

fn test_process_output_concat() -> Str {
    let a = "echo "
    let b = "proc_out_concat_ok"
    let cmd = a + b
    mut res_out = ""
    unsafe(justification: "test") {
        res_out = process_output(cmd)
    }
    return res_out
}

fn main() {
    let sys_rc = test_system_concat()
    println(fmt"SYS_RC: {sys_rc}")

    let ex_out = test_exec_concat()
    println(fmt"EXEC_OUT: {ex_out}")

    let proc_out = test_process_output_concat()
    println(fmt"PROC_OUT: {proc_out}")
}
"#;
    let (out, code) = run_datara(code, "test_sys_exec_var");
    assert_eq!(code, 0);
    assert!(out.contains("SYS_RC: 0"));
    assert!(out.contains("EXEC_OUT: exec_concat_ok"));
    assert!(out.contains("PROC_OUT: proc_out_concat_ok"));
}
