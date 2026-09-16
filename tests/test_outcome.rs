//! Outcome<T> typed errors: stdlib class, ok/err constructors, trapping
//! unwrap, err() accessor, and the checked-I/O builtins
//! (file_read_checked / env_get_checked) that return Outcome<Str>.
//!
//! Representation note: Outcome<T> uses the existing generic-class ABI of
//! `stdlib/result/result.dtr` (fields is_success: Bool, value: T,
//! error_msg: Str; three 8-byte slots) -- the same shape `?` propagation and
//! `Outcome<T> { ... }` literals already rely on. No new ABI was invented.

use forgen::driver::ForgenCompiler;

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

const PRELUDE: &str = "use stdlib.result.result.Outcome\n";

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

fn run_stdout(path: &std::path::Path) -> (String, String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(path, None);
    assert!(
        res.success,
        "Compilation should succeed:\n{}",
        res.diagnostics
    );
    let (stdout, stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    (stdout, stderr, code)
}

fn src(body: &str) -> String {
    format!("{}\n{}", PRELUDE, body)
}

#[test]
fn test_checked_read_missing_file_is_err_with_message() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let src = src(r#"fn main() {
    unsafe(justification: "test") {
        let r = file_read_checked("D:/definitely/not/a/real/file_xyz.dat")
        if r.is_err() {
            if r.err().len() > 0 {
                println("ERR_WITH_MSG")
                return
            }
        }
        println("FAIL")
    }
}
"#);
    let path = write_source(&dir, "outcome_missing.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("ERR_WITH_MSG"), "stdout: {}", stdout);
    assert!(!stdout.contains("FAIL"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_checked_read_existing_file_is_ok() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let content_file = dir.join("outcome_content.txt");
    std::fs::write(&content_file, "payload-123").unwrap();
    let src = src(&format!(
        r#"fn main() {{
    unsafe(justification: "test") {{
        let r = file_read_checked("{}")
        if r.is_ok() {{
            println("IS_OK")
            return
        }}
        println("FAIL")
    }}
}}
"#,
        content_file.to_str().unwrap().replace('\\', "/")
    ));
    let path = write_source(&dir, "outcome_ok.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("IS_OK"), "stdout: {}", stdout);
    assert!(!stdout.contains("FAIL"), "stdout: {}", stdout);
    cleanup(&path);
    let _ = std::fs::remove_file(&content_file);
}

#[test]
fn test_unwrap_ok_returns_content() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let content_file = dir.join("outcome_unwrap_ok.txt");
    std::fs::write(&content_file, "the-content").unwrap();
    let src = src(&format!(
        r#"fn main() {{
    unsafe(justification: "test") {{
        let r = file_read_checked("{}")
        println(r.unwrap())
    }}
}}
"#,
        content_file.to_str().unwrap().replace('\\', "/")
    ));
    let path = write_source(&dir, "outcome_unwrap_ok.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("the-content"), "stdout: {}", stdout);
    cleanup(&path);
    let _ = std::fs::remove_file(&content_file);
}

#[test]
fn test_unwrap_err_traps_nonzero_exit() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let src = src(r#"fn main() {
    unsafe(justification: "test") {
        let r = file_read_checked("D:/definitely/not/a/real/file_xyz.dat")
        println(r.unwrap())
    }
}
"#);
    let path = write_source(&dir, "outcome_unwrap_err.dtr", &src);
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(
        res.success,
        "Compilation should succeed:\n{}",
        res.diagnostics
    );
    let (_stdout, stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_ne!(code, 0, "unwrap on err must trap with a non-zero exit");
    assert!(
        stderr.contains("file read failed"),
        "trap message must reach stderr, got: {}",
        stderr
    );
    cleanup(&path);
}

#[test]
fn test_err_returns_message_and_empty_when_ok() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let src = src(r#"fn main() {
    let e = Outcome.err("custom failure")
    println(e.err())
    let ok = Outcome.ok(42)
    if ok.err().len() == 0 {
        println("EMPTY_ON_OK")
    }
}
"#);
    let path = write_source(&dir, "outcome_err_accessor.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("custom failure"), "stdout: {}", stdout);
    assert!(stdout.contains("EMPTY_ON_OK"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_err_constructor_is_ok_false() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let src = src(r#"fn main() {
    let e = Outcome.err("nope")
    if e.is_ok() {
        println("FAIL")
        return
    }
    if e.is_err() {
        println("CONSTRUCTOR_ERR")
    }
}
"#);
    let path = write_source(&dir, "outcome_err_ctor.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("CONSTRUCTOR_ERR"), "stdout: {}", stdout);
    assert!(!stdout.contains("FAIL"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_ok_constructor_payload_round_trip() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let src = src(r#"fn main() {
    let r = Outcome.ok(7)
    println(int_to_str(r.unwrap()))
    let s = Outcome.ok("str-payload")
    println(s.unwrap())
}
"#);
    let path = write_source(&dir, "outcome_ok_ctor.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains('7'), "stdout: {}", stdout);
    assert!(stdout.contains("str-payload"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_checked_read_empty_file_is_ok_with_empty_string() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let content_file = dir.join("outcome_empty.txt");
    std::fs::write(&content_file, "").unwrap();
    let src = src(&format!(
        r#"fn main() {{
    unsafe(justification: "test") {{
        let r = file_read_checked("{}")
        if r.is_ok() {{
            if r.unwrap().len() == 0 {{
                println("EMPTY_OK")
                return
            }}
        }}
        println("FAIL")
    }}
}}
"#,
        content_file.to_str().unwrap().replace('\\', "/")
    ));
    let path = write_source(&dir, "outcome_empty.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("EMPTY_OK"), "stdout: {}", stdout);
    assert!(!stdout.contains("FAIL"), "stdout: {}", stdout);
    cleanup(&path);
    let _ = std::fs::remove_file(&content_file);
}

#[test]
fn test_env_get_checked_missing_var_is_err() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let src = src(r#"fn main() {
    unsafe(justification: "test") {
        let r = env_get_checked("DATARA_TEST_VAR_DEFINITELY_UNSET_9x7")
        if r.is_err() {
            println("ENV_ERR")
            return
        }
        println("FAIL")
    }
}
"#);
    let path = write_source(&dir, "outcome_env_err.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("ENV_ERR"), "stdout: {}", stdout);
    assert!(!stdout.contains("FAIL"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_env_get_checked_set_var_is_ok() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let src = src(r#"fn main() {
    unsafe(justification: "test") {
        let r = env_get_checked("PATH")
        if r.is_ok() {
            println("ENV_OK")
            return
        }
        println("FAIL")
    }
}
"#);
    let path = write_source(&dir, "outcome_env_ok.dtr", &src);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("ENV_OK"), "stdout: {}", stdout);
    cleanup(&path);
}
