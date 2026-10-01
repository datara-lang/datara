//! v1.5.0 (M1) process spawn / pipes / wait tests (stdlib.sys remainder).
//!
//! Mirrors tests/test_exec_utf8.rs:
//! - proc_spawn(cmd) -> Outcome<Int> (PID handle), proc_wait(pid) reaps the
//!   child's exit code.
//! - proc_run_input(cmd, input) -> Outcome<Str> pipes `input` into the
//!   child's stdin and captures merged stdout+stderr.
//! - Capability / unsafe gate (E0940) fires without justification.
//! - JIT paths run everywhere; AOT variants run on non-Windows hosts.
//!
//! NOTE: AOT tests require a working C linker and the Datara runtime
//! archive; they are skipped on Windows CI (JIT covers the same runtime
//! code paths).

use forgen::diagnostics::ErrorCode;
use forgen::driver::ForgenCompiler;

fn run_jit(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

fn run_datara(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}",
        name, res.error
    );

    let exe = res.exe_path.clone().expect("must produce a native exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "{} exited with {}", name, code);

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("o"));
    stdout.trim().replace("\r\n", "\n")
}

fn proc_run_input_helper() -> &'static str {
    r#"
use stdlib.sys.sys

fn run_echo(cmd: Str, input: Str) -> Outcome<Str> {
    unsafe(justification: "piped process probe") {
        return proc_run_input(cmd, input)
    }
}

fn spawn_child(cmd: Str) -> Outcome<Int> {
    unsafe(justification: "spawn probe") {
        return proc_spawn(cmd)
    }
}

fn reap(pid: Int) -> Outcome<Int> {
    unsafe(justification: "wait probe") {
        return proc_wait(pid)
    }
}

fn main() {
    OUT
}
"#
}

#[test]
fn test_proc_run_input_echo_jit() {
    let (win_cmd, posix_cmd, expect) = if cfg!(windows) {
        ("cmd /c more", "cat", "stdin_line_42")
    } else {
        ("cat", "cat", "stdin_line_42")
    };
    let cmd = if cfg!(windows) { win_cmd } else { posix_cmd };
    let body = format!(
        r#"
    let r = run_echo("{cmd}", "stdin_line_42")
    if r.is_ok() {{
        println(fmt"echo:{{r.unwrap()}}")
    }} else {{
        println("echo:ERR")
    }}
"#
    );
    let source = proc_run_input_helper().replace("OUT", &body);
    let out = run_jit(&source, "test_proc_run_input_echo_jit");
    assert!(out.contains(&format!("echo:{expect}")), "got: {}", out);
}

#[test]
fn test_proc_run_input_merged_streams_jit() {
    let cmd = if cfg!(windows) {
        "cmd /c echo A1 & echo B2 1>&2"
    } else {
        "echo A1; echo B2 1>&2"
    };
    let body = format!(
        r#"
    let m = run_echo("{cmd}", "")
    if m.is_ok() {{
        let text = m.unwrap()
        println(fmt"merged:{{str_contains(text, \"A1\")}}{{str_contains(text, \"B2\")}}")
    }} else {{
        println("merged:ERR")
    }}
"#
    );
    let source = proc_run_input_helper().replace("OUT", &body);
    let out = run_jit(&source, "test_proc_run_input_merged_streams_jit");
    assert!(out.contains("merged:truetrue"), "got: {}", out);
}

#[test]
fn test_proc_spawn_wait_exit_code_jit() {
    let cmd = if cfg!(windows) {
        "cmd /c exit /b 7"
    } else {
        "exit 7"
    };
    let body = format!(
        r#"
    let p = spawn_child("{cmd}")
    if p.is_ok() {{
        let w = reap(p.unwrap())
        println(fmt"code:{{w or -1}}")
    }} else {{
        println("code:SPAWN_ERR")
    }}
"#
    );
    let source = proc_run_input_helper().replace("OUT", &body);
    let out = run_jit(&source, "test_proc_spawn_wait_exit_code_jit");
    assert!(out.contains("code:7"), "got: {}", out);
}

#[test]
fn test_proc_run_input_nonzero_exit_is_err_jit() {
    let cmd = if cfg!(windows) {
        "cmd /c exit /b 3"
    } else {
        "exit 3"
    };
    let body = format!(
        r#"
    let e = run_echo("{cmd}", "")
    println(fmt"ok:{{e.is_ok()}}")
"#
    );
    let source = proc_run_input_helper().replace("OUT", &body);
    let out = run_jit(&source, "test_proc_run_input_nonzero_exit_is_err_jit");
    assert!(out.contains("ok:false"), "got: {}", out);
}

#[test]
fn test_proc_wait_invalid_pid_err_jit() {
    let body = r#"
    let w = reap(0)
    println(fmt"ok:{w.is_ok()}")
"#;
    let source = proc_run_input_helper().replace("OUT", body);
    let out = run_jit(&source, "test_proc_wait_invalid_pid_err_jit");
    assert!(out.contains("ok:false"), "got: {}", out);
}

#[test]
fn test_proc_run_input_echo_aot() {
    if cfg!(windows) {
        // AOT on Windows requires MSVC linker + runtime archive; the JIT
        // variants above exercise the same runtime code paths.
        return;
    }
    let cmd = "cat";
    let body = format!(
        r#"
    let r = run_echo("{cmd}", "stdin_line_42")
    if r.is_ok() {{
        println(fmt"echo:{{r.unwrap()}}")
    }} else {{
        println("echo:ERR")
    }}
"#
    );
    let source = proc_run_input_helper().replace("OUT", &body);
    let out = run_datara(&source, "test_proc_run_input_echo_aot");
    assert!(out.contains("echo:stdin_line_42"), "got: {}", out);
}

// ---------------------------------------------------------------------------
// Capability gate -- compile-only check, runs everywhere
// ---------------------------------------------------------------------------

#[test]
fn test_proc_calls_require_unsafe_or_cap_gate() {
    let source = r#"
use stdlib.sys.sys

fn main() {
    let r = proc_run_input("cmd /c echo x", "")
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "proc_gate.dtr", None);
    assert!(
        !res.success,
        "proc_run_input without unsafe/cap must fail compilation"
    );
    assert!(
        res.diagnostics
            .contains(ErrorCode::SecurityViolation.as_str()),
        "Must emit E0940, got:\n{}",
        res.diagnostics
    );
}

#[test]
fn test_proc_spawn_requires_unsafe_or_cap_gate() {
    let source = r#"
use stdlib.sys.sys

fn main() {
    let p = proc_spawn("cmd /c echo x")
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "proc_spawn_gate.dtr", None);
    assert!(
        !res.success,
        "proc_spawn without unsafe/cap must fail compilation"
    );
    assert!(
        res.diagnostics
            .contains(ErrorCode::SecurityViolation.as_str()),
        "Must emit E0940, got:\n{}",
        res.diagnostics
    );
}

#[test]
fn test_proc_wait_requires_unsafe_or_cap_gate() {
    let source = r#"
use stdlib.sys.sys

fn main() {
    let w = proc_wait(123)
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "proc_wait_gate.dtr", None);
    assert!(
        !res.success,
        "proc_wait without unsafe/cap must fail compilation"
    );
    assert!(
        res.diagnostics
            .contains(ErrorCode::SecurityViolation.as_str()),
        "Must emit E0940, got:\n{}",
        res.diagnostics
    );
}
