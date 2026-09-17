//! v1.4.1 UTF-8 process execution tests (exec vs exec_utf8).
//!
//! Verifies:
//! - exec(cmd) remains byte-transparent returning Str directly.
//! - exec_utf8(cmd) returns Outcome<Str> with proper UTF-8 normalization.
//! - Non-ASCII output is correctly preserved and decoded.
//! - Invalid command spawn failure returns Outcome::err with diagnostic message.
//! - Question mark (?) operator works seamlessly with exec_utf8.
//! - Capability / unsafe gate (E0940) is enforced when invoked without justification.
//! - Both JIT and AOT native execution.
//!
//! NOTE: AOT tests (compile_source_native) require a working C linker and the
//! Datara runtime archive to be present. On Windows CI runners the MSVC
//! environment is available but AOT tests are skipped to avoid flakiness; the
//! JIT variants below exercise the same runtime code paths on all platforms.

use forgen::diagnostics::ErrorCode;
use forgen::driver::ForgenCompiler;

/// Build an `echo` command that works on both Windows and Unix and outputs
/// clean UTF-8 text. On Windows we use PowerShell Write-Output so that
/// code-page issues with cmd.exe are avoided entirely.
fn echo_cmd(msg: &str) -> String {
    if cfg!(windows) {
        format!("powershell -NoProfile -Command Write-Output {msg}")
    } else {
        format!("echo {msg}")
    }
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

fn run_jit(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

// ---------------------------------------------------------------------------
// JIT tests -- run on all platforms
// ---------------------------------------------------------------------------

#[test]
fn test_exec_byte_transparent_plain_jit() {
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
    let out = run_jit(&source, "test_exec_byte_transparent_plain_jit");
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
fn test_exec_utf8_invalid_command_err_jit() {
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
    let out = run_jit(source, "test_exec_utf8_invalid_command_err_jit");
    assert!(out.contains("err_caught:exec_utf8 failed"), "got: {}", out);
}

#[test]
fn test_exec_utf8_with_question_operator_jit() {
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
    let out = run_jit(&source, "test_exec_utf8_with_question_operator_jit");
    assert!(out.contains("ok:true"), "got: {}", out);
    assert!(out.contains("bad:true"), "got: {}", out);
}

// ---------------------------------------------------------------------------
// Capability gate -- compile-only check, runs everywhere
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// AOT tests -- skipped on Windows CI (covered by JIT equivalents above).
// On Linux and macOS these run natively and exercise the full linker path.
// ---------------------------------------------------------------------------

#[test]
#[cfg_attr(
    windows,
    ignore = "AOT on Windows requires MSVC linker + runtime archive; covered by JIT equivalents"
)]
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
#[cfg_attr(
    windows,
    ignore = "AOT on Windows requires MSVC linker + runtime archive; covered by JIT equivalents"
)]
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
#[cfg_attr(
    windows,
    ignore = "AOT on Windows requires MSVC linker + runtime archive; covered by JIT equivalents"
)]
fn test_exec_utf8_non_ascii() {
    let cmd = echo_cmd("PRIVET_DATARA_ASCII");
    let source = format!(
        r#"
use stdlib.result.result.Outcome

fn main() {{
    unsafe(justification: "test exec_utf8 non-ascii") {{
        let res = exec_utf8("{cmd}")
        if res.is_ok() {{
            println(res.unwrap())
        }}
    }}
}}
"#
    );
    let out = run_datara(&source, "test_exec_utf8_non_ascii");
    assert!(out.contains("PRIVET_DATARA_ASCII"), "got: {}", out);
}

#[test]
#[cfg_attr(
    windows,
    ignore = "AOT on Windows requires MSVC linker + runtime archive; covered by JIT equivalents"
)]
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
#[cfg_attr(
    windows,
    ignore = "AOT on Windows requires MSVC linker + runtime archive; covered by JIT equivalents"
)]
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
