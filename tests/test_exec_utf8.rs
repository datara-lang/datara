//! v1.4.1 UTF-8 process execution tests (exec vs exec_utf8).
//!
//! Verifies:
//! - exec(cmd) remains byte-transparent returning Str directly.
//! - exec_utf8(cmd) returns Outcome<Str> with proper UTF-8 normalization.
//! - Non-ASCII and Cyrillic output is correctly preserved and decoded.
//! - Invalid command spawn failure returns Outcome::err with diagnostic message.
//! - Question mark (?) operator works seamlessly with exec_utf8.
//! - Capability / unsafe gate (E0940) is enforced when invoked without justification.
//! - Both JIT and AOT native execution.

use forgen::diagnostics::ErrorCode;
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

fn echo_cmd(msg: &str) -> String {
    if cfg!(windows) {
        format!("cmd /c echo {msg}")
    } else {
        format!("echo {msg}")
    }
}

fn run_datara(source: &str, name: &str) -> String {
    ensure_msvc_env();
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

fn run_jit(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

#[test]
fn test_exec_byte_transparent_plain() {
    let cmd = echo_cmd("HELLO_PLAIN_EXEC");
    let source = format!(
        r#"
fn main() {{
    unsafe(justification: "test plain exec") {{
        let out = exec("{cmd}")
        println(out)
    }}
}}
"#
    );
    let out = run_datara(&source, "test_exec_byte_transparent_plain");
    assert!(out.contains("HELLO_PLAIN_EXEC"), "got: {}", out);
}

#[test]
fn test_exec_utf8_success_jit() {
    let cmd = echo_cmd("HELLO_UTF8_JIT");
    let source = format!(
        r#"
use stdlib.result.result.Outcome

fn main() {{
    unsafe(justification: "test exec_utf8 in jit") {{
        let res = exec_utf8("{cmd}")
        if res.is_ok() {{
            println(fmt"ok:{{res.unwrap()}}")
        }} else {{
            println(fmt"err:{{res.err()}}")
        }}
    }}
}}
"#
    );
    let out = run_jit(&source, "test_exec_utf8_success_jit");
    assert!(out.contains("ok:HELLO_UTF8_JIT"), "got: {}", out);
}

#[test]
fn test_exec_utf8_success_aot() {
    let cmd = echo_cmd("HELLO_UTF8_AOT");
    let source = format!(
        r#"
use stdlib.result.result.Outcome

fn main() {{
    unsafe(justification: "test exec_utf8 native aot") {{
        let res = exec_utf8("{cmd}")
        if res.is_ok() {{
            println(fmt"ok:{{res.unwrap()}}")
        }} else {{
            println(fmt"err:{{res.err()}}")
        }}
    }}
}}
"#
    );
    let out = run_datara(&source, "test_exec_utf8_success_aot");
    assert!(out.contains("ok:HELLO_UTF8_AOT"), "got: {}", out);
}

#[test]
fn test_exec_utf8_cyrillic_non_ascii() {
    let cmd = echo_cmd("ПРИВЕТ_ДАТАРА");
    let source = format!(
        r#"
use stdlib.result.result.Outcome

fn main() {{
    unsafe(justification: "test exec_utf8 cyrillic") {{
        let res = exec_utf8("{cmd}")
        if res.is_ok() {{
            println(res.unwrap())
        }}
    }}
}}
"#
    );
    let out = run_datara(&source, "test_exec_utf8_cyrillic_non_ascii");
    assert!(out.contains("ПРИВЕТ_ДАТАРА"), "got: {}", out);
}

#[test]
fn test_exec_utf8_invalid_command_err() {
    let source = r#"
use stdlib.result.result.Outcome

fn main() {
    unsafe(justification: "test invalid command") {
        let res = exec_utf8("this_command_should_never_exist_12345.xyz")
        if res.is_err() {
            println(fmt"err_caught:{res.err()}")
        } else {
            println("unexpected_ok")
        }
    }
}
"#;
    let out = run_datara(source, "test_exec_utf8_invalid_command_err");
    assert!(out.contains("err_caught:exec_utf8 failed"), "got: {}", out);
}

#[test]
fn test_exec_utf8_with_question_operator() {
    let ok_cmd = echo_cmd("PROPAGATION_OK");
    let source = format!(
        r#"
use stdlib.result.result.Outcome

fn run_command(cmd: Str) -> Outcome<Str> {{
    unsafe(justification: "run command") {{
        let text = exec_utf8(cmd)?
        return Outcome.ok(text)
    }}
}}

fn main() {{
    let ok = run_command("{ok_cmd}")
    println(fmt"ok:{{ok.is_ok()}}")
    let bad = run_command("this_command_should_never_exist_98765.xyz")
    println(fmt"bad:{{bad.is_err()}}")
}}
"#
    );
    let out = run_datara(&source, "test_exec_utf8_with_question_operator");
    assert!(out.contains("ok:true"), "got: {}", out);
    assert!(out.contains("bad:true"), "got: {}", out);
}

#[test]
fn test_exec_utf8_requires_unsafe_or_cap_gate() {
    let source = r#"
use stdlib.result.result.Outcome

fn main() {
    let res = exec_utf8("echo fail")
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "err_gate.dtr", None);
    assert!(
        !res.success,
        "exec_utf8 without unsafe/cap must fail compilation"
    );
    assert!(
        res.diagnostics
            .contains(ErrorCode::SecurityViolation.as_str()),
        "Must emit E0940, got:\n{}",
        res.diagnostics
    );
}
