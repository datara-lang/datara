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
fn test_socket_timeout_invalid_handle() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = src(r#"fn main() {
    unsafe(justification: "test") {
        let r = socket_set_timeout(-1, 100)
        if r.is_err() {
            if r.err() == "invalid socket handle" {
                println("INVALID_HANDLE_ERR_OK")
                return
            }
        }
        println("FAIL")
    }
}
"#);
    let path = write_source(&dir, "sock_timeout_inv.dtr", &code);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("INVALID_HANDLE_ERR_OK"),
        "stdout: {}",
        stdout
    );
    cleanup(&path);
}

#[test]
fn test_socket_nonblocking_invalid_handle() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = src(r#"fn main() {
    unsafe(justification: "test") {
        let r = socket_nonblocking(-1, true)
        if r.is_err() {
            if r.err() == "invalid socket handle" {
                println("INVALID_NB_ERR_OK")
                return
            }
        }
        println("FAIL")
    }
}
"#);
    let path = write_source(&dir, "sock_nb_inv.dtr", &code);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("INVALID_NB_ERR_OK"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_socket_set_timeout_and_nonblocking_valid() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = src(r#"fn main() {
    unsafe(justification: "test") {
        let sock = socket_create(1)
        if sock < 0 {
            println("FAIL_CREATE")
            return
        }
        let t_res = socket_set_timeout(sock, 250)
        let nb_res = socket_nonblocking(sock, true)
        if t_res.is_ok() && nb_res.is_ok() {
            println("TIMEOUT_AND_NB_OK")
        } else {
            println("FAIL_OP")
        }
        socket_close(sock)
    }
}
"#);
    let path = write_source(&dir, "sock_valid_opts.dtr", &code);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("TIMEOUT_AND_NB_OK"), "stdout: {}", stdout);
    cleanup(&path);
}

#[test]
fn test_socket_recv_outcome_nonblocking_would_block() {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let code = src(r#"fn main() {
    unsafe(justification: "test") {
        let sock = socket_create(0)
        if sock < 0 {
            println("FAIL_CREATE")
            return
        }
        let bind_res = socket_bind(sock, "127.0.0.1", 0)
        if bind_res != 0 {
            println("FAIL_BIND")
            socket_close(sock)
            return
        }
        let nb_res = socket_nonblocking(sock, true)
        if !nb_res.is_ok() {
            println("FAIL_NB")
            socket_close(sock)
            return
        }
        let recv_res = socket_recv_outcome(sock, 1024)
        if recv_res.is_err() {
            if recv_res.err() == "would-block" {
                println("RECV_WOULD_BLOCK_OK")
            } else {
                println("FAIL_ERR:" + recv_res.err())
            }
        } else {
            println("FAIL_EXPECTED_ERR")
        }
        socket_close(sock)
    }
}
"#);
    let path = write_source(&dir, "sock_would_block.dtr", &code);
    let (stdout, _stderr, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert!(stdout.contains("RECV_WOULD_BLOCK_OK"), "stdout: {}", stdout);
    cleanup(&path);
}
