use forgen::driver::ForgenCompiler;
use std::io::Write;
use std::process::{Command, Stdio};

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

fn compile_and_run_with_input(
    src_path: &std::path::Path,
    input_data: &str,
) -> (String, String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(src_path, None);
    assert!(
        res.success,
        "Compilation should succeed:\n{}",
        res.diagnostics
    );
    let exe_path = res.exe_path.unwrap();
    let mut child = Command::new(&exe_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn executable");

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_data.as_bytes())
            .expect("failed to write to stdin");
    }

    let output = child.wait_with_output().expect("failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn test_fast_read_int() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let a = read_int()
    let b = read_int()
    let c = read_int()
    println(int_to_str(a + b + c))
}
"#;
    let path = write_source(&dir, "fast_read_int.dtr", code);
    let (stdout, _stderr, code) = compile_and_run_with_input(&path, "10 20 -5\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("25"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_input_int_with_prompt() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let age = input_int("Enter age: ")
    println("Age: " + int_to_str(age))
}
"#;
    let path = write_source(&dir, "input_int_prompt.dtr", code);
    let (stdout, _stderr, code) = compile_and_run_with_input(&path, "42\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("Enter age: "), "stdout: {}", stdout);
    assert!(stdout.contains("Age: 42"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_fast_read_float() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let x = read_float()
    let y = read_float()
    if x > 3.14 && x < 3.15 && y < -1.9 && y > -2.1 {
        println("FLOAT_READ_OK")
    } else {
        println("FAIL_FLOAT")
    }
}
"#;
    let path = write_source(&dir, "fast_read_float.dtr", code);
    let (stdout, _stderr, code) = compile_and_run_with_input(&path, "3.14159 -2.0\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("FLOAT_READ_OK"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_input_float_with_prompt() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let price = input_float("Enter price: ")
    if price > 19.98 && price < 20.00 {
        println("PRICE_OK")
    } else {
        println("FAIL_PRICE")
    }
}
"#;
    let path = write_source(&dir, "input_float_prompt.dtr", code);
    let (stdout, _stderr, code) = compile_and_run_with_input(&path, "19.99\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("Enter price: "), "stdout: {}", stdout);
    assert!(stdout.contains("PRICE_OK"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_read_line_dynamic_buffer() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = r#"
fn main() {
    let line1 = read_line()
    let line2 = read_line()
    println("L1_LEN:" + int_to_str(line1.len()))
    println("L2:" + line2)
}
"#;
    let path = write_source(&dir, "read_line_dynamic.dtr", code);
    let long_prefix = "abcdefghij".repeat(50); // 500 chars > 256 initial cap
    let input = format!("{}\nsecond line\n", long_prefix);
    let (stdout, _stderr, code) = compile_and_run_with_input(&path, &input);
    assert_eq!(code, 0);
    assert!(stdout.contains("L1_LEN:500"), "stdout: {}", stdout);
    assert!(stdout.contains("L2:second line"), "stdout: {}", stdout);
    cleanup(&path);
}
