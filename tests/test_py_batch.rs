use forgen::driver::ForgenCompiler;
use forgen::interop_audit::{render_table, run_bridge_audit};

static MSVC_ENV: std::sync::Once = std::sync::Once::new();
fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| unsafe {
        if std::env::var_os("ProgramFiles(x86)").is_none() {
            std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
        }
        if std::env::var_os("ProgramFiles").is_none() {
            std::env::set_var("ProgramFiles", r"C:\Program Files");
        }
    });
}

fn write_source(dir: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("failed to write test source");
    path
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("exe"));
    let _ = std::fs::remove_file(path.with_extension("pdb"));
    let _ = std::fs::remove_file(path.with_extension("obj"));
}

#[test]
fn test_py_eval_batch_successful_execution() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let res = py_eval_batch("[\"10 + 20\", \"2.5 * 4\", \"'data' + 'ra'\"]")
    println("BATCH_RES:" + res)
}
"#;
    let path = write_source(&dir, "py_batch_success.dtr", code);
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(res.success, "Compilation failed: {}", res.diagnostics);
    let (stdout, _stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code, 0);
    assert!(stdout.contains("BATCH_RES:"), "stdout: {}", stdout);
    assert!(stdout.contains("30"), "stdout: {}", stdout);
    assert!(stdout.contains("10.0"), "stdout: {}", stdout);
    assert!(stdout.contains("datara"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_py_eval_batch_isolated_error_in_batch() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let res = py_eval_batch("[\"1 + 1\", \"10 / 0\", \"5 * 5\"]")
    println("BATCH_ERR_RES:" + res)
}
"#;
    let path = write_source(&dir, "py_batch_error.dtr", code);
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(res.success, "Compilation failed: {}", res.diagnostics);
    let (stdout, _stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code, 0);
    assert!(stdout.contains("BATCH_ERR_RES:"), "stdout: {}", stdout);
    assert!(stdout.contains("2"), "stdout: {}", stdout);
    assert!(stdout.contains("error"), "stdout: {}", stdout);
    assert!(stdout.contains("25"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_py_eval_batch_empty_input() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let res = py_eval_batch("[]")
    println("BATCH_EMPTY:" + res)
}
"#;
    let path = write_source(&dir, "py_batch_empty.dtr", code);
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(res.success, "Compilation failed: {}", res.diagnostics);
    let (stdout, _stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code, 0);
    assert!(stdout.contains("BATCH_EMPTY:[]"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_doctor_bridges_reports_overhead_categories() {
    ensure_msvc_env();
    let scenarios = run_bridge_audit();
    let rendered = render_table(&scenarios);
    assert!(
        rendered.contains("[category: C-ABI (ns)]"),
        "Doctor bridges must report [category: C-ABI (ns)]. Output:\n{}",
        rendered
    );
    assert!(
        rendered.contains("[category: in-process (\u{00B5}s)]"),
        "Doctor bridges must report [category: in-process (\u{00B5}s)]. Output:\n{}",
        rendered
    );
}
