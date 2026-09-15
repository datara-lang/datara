//! v1.4.0 structured `asm { ... }` blocks (x86-64 safe subset).
//!
//! The subset is `mov|add|sub` over the four GP scratch registers, `Int`
//! variables and integer immediates. It lowers to plain DMIR ops through the
//! normal backend (registers are simulated as DMIR temporaries), so the same
//! program compiles and runs under both the JIT and AOT paths. Anything
//! outside the subset is a hard error:
//!
//! * E1402 — unsupported instruction / register read before write,
//! * E1403 — non-Int operand,
//! * E1404 — missing `unsafe(justification: "...")`,
//! * E1405 — LLVM/WASM backend (Cranelift-only feature).

use forgen::driver::ForgenCompiler;
use std::path::PathBuf;

/// Windows linker discovery reads `ProgramFiles(x86)` to locate
/// vswhere/BuildTools. Some harnesses (sandboxed shells, stripped CI
/// containers) drop the variable entirely; restore the conventional
/// defaults so the machine paths resolve. Mirrors the hardcoded fallback
/// the other native tests use.
static MSVC_ENV: std::sync::Once = std::sync::Once::new();

fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| {
        // SAFETY: `set_var` is process-global; the call happens once,
        // only when the variable is absent, and before any linker
        // subprocess is spawned by this test process.
        unsafe {
            if std::env::var_os("ProgramFiles(x86)").is_none() {
                std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
            }
            if std::env::var_os("ProgramFiles").is_none() {
                std::env::set_var("ProgramFiles", r"C:\Program Files");
            }
        }
    });
}

fn write_source(name: &str, source: &str) -> PathBuf {
    let path = PathBuf::from(name);
    std::fs::write(&path, source).expect("must write test source");
    path
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("exe"));
    let _ = std::fs::remove_file(path.with_extension("pdb"));
    let _ = std::fs::remove_file(path.with_extension("obj"));
}

/// The user-facing example: `x = 42; mov eax, x; add eax, 1; mov x, eax`
/// yields 43 under both the JIT and the AOT path.
#[test]
fn asm_increment_runs_under_jit_and_aot() {
    let source = r#"
fn main() {
    mut x = 42
    unsafe(justification: "structured asm safe subset over an Int local") {
        asm {
            mov eax, x
            add eax, 1
            mov x, eax
        }
    }
    out x
}
"#;
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");

    // AOT: compile to a native executable and run it.
    let path = write_source("v140_asm_increment.dtr", source);
    let res = compiler.compile_file(&path, None);
    assert!(
        res.success,
        "AOT compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    assert_eq!(code, 0, "AOT run failed: {}", stderr);
    assert_eq!(stdout.trim(), "43", "AOT asm result must be 43");
    cleanup(&path);

    // JIT: same source through the in-memory pipeline.
    let (jit_out, jit_err, jit_code, _) = compiler
        .run_source(source, "v140_asm_increment_jit.dtr", &[], true)
        .expect("JIT run must succeed");
    assert_eq!(jit_code, 0, "JIT run failed: {}", jit_err);
    assert_eq!(jit_out.trim(), "43", "JIT asm result must be 43");
}

/// Multi-variable dataflow across several instructions and registers:
/// (10 + 7 - 5) computed in registers, stored back into the second local.
#[test]
fn asm_multi_variable_dataflow() {
    let source = r#"
fn compute() -> Int {
    mut a = 10
    mut b = 7
    unsafe(justification: "structured asm dataflow across two Int locals") {
        asm {
            mov rax, a
            mov rbx, b
            add rax, rbx
            sub rax, 5
            mov b, rax
        }
    }
    return b
}

fn main() {
    out compute()
}
"#;
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let (stdout, stderr, code, _) = compiler
        .run_source(source, "v140_asm_dataflow.dtr", &[], true)
        .expect("JIT run must succeed");
    assert_eq!(code, 0, "run failed: {}", stderr);
    assert_eq!(stdout.trim(), "12", "10 + 7 - 5 must be 12");

    let path = write_source("v140_asm_dataflow_aot.dtr", source);
    let res = compiler.compile_file(&path, None);
    assert!(res.success, "AOT compile failed: {}", res.diagnostics);
    let exe = res.exe_path.expect("must produce native executable");
    let (stdout_aot, stderr_aot, code_aot, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    assert_eq!(code_aot, 0, "AOT run failed: {}", stderr_aot);
    assert_eq!(stdout_aot.trim(), "12");
    cleanup(&path);
}

/// Immediate-only arithmetic (no variables) still works and keeps the
/// block's registers private to the block.
#[test]
fn asm_immediates_and_register_reset_between_blocks() {
    let source = r#"
fn twice() -> Int {
    mut x = 1
    unsafe(justification: "immediate arithmetic in registers") {
        asm {
            mov eax, 100
            add eax, 20
            mov x, eax
        }
    }
    mut y = 2
    unsafe(justification: "second block starts with fresh registers") {
        asm {
            mov ecx, 5
            sub ecx, 3
            mov y, ecx
        }
    }
    return x + y
}

fn main() {
    out twice()
}
"#;
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let (stdout, stderr, code, _) = compiler
        .run_source(source, "v140_asm_immediates.dtr", &[], true)
        .expect("JIT run must succeed");
    assert_eq!(code, 0, "run failed: {}", stderr);
    assert_eq!(stdout.trim(), "122", "120 + 2 must be 122");
}

/// Float operands are rejected with E1403.
#[test]
fn asm_float_operand_is_rejected() {
    let source = r#"
fn main() {
    mut f = 2.5
    unsafe(justification: "type check probe") {
        asm {
            mov rax, f
        }
    }
    out f
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "v140_asm_float.dtr", None);
    assert!(!res.success, "Float operand must be rejected");
    assert!(
        res.diagnostics.contains("E1403"),
        "must report E1403, got:\n{}",
        res.diagnostics
    );
}

/// Without an enclosing `unsafe(justification: ...)` the block is rejected
/// with E1404.
#[test]
fn asm_without_unsafe_is_rejected() {
    let source = r#"
fn main() {
    mut x = 1
    asm {
        mov rax, x
    }
    out x
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "v140_asm_no_unsafe.dtr", None);
    assert!(!res.success, "asm outside unsafe must be rejected");
    assert!(
        res.diagnostics.contains("E1404"),
        "must report E1404, got:\n{}",
        res.diagnostics
    );
}

/// Anything outside the subset reports E1402 and lists the supported forms.
#[test]
fn asm_unsupported_instruction_is_rejected() {
    let source = r#"
fn main() {
    mut x = 1
    unsafe(justification: "unsupported form probe") {
        asm {
            mov eax, [rsp]
        }
    }
    out x
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "v140_asm_unsupported.dtr", None);
    assert!(!res.success, "memory operand must be rejected");
    assert!(
        res.diagnostics.contains("E1402"),
        "must report E1402, got:\n{}",
        res.diagnostics
    );
    assert!(
        res.diagnostics.contains("supported forms"),
        "E1402 must list the supported forms, got:\n{}",
        res.diagnostics
    );
}

/// Reading a register before it is written in the same block is E1402.
#[test]
fn asm_register_read_before_write_is_rejected() {
    let source = r#"
fn main() {
    mut x = 1
    unsafe(justification: "register liveness probe") {
        asm {
            add eax, 1
            mov x, eax
        }
    }
    out x
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "v140_asm_unset_reg.dtr", None);
    assert!(!res.success, "unset register read must be rejected");
    assert!(
        res.diagnostics.contains("E1402"),
        "must report E1402, got:\n{}",
        res.diagnostics
    );
}

/// The WASM target rejects asm-bearing functions with E1405.
#[test]
fn asm_on_wasm_target_is_rejected() {
    let source = r#"
fn main() {
    mut x = 1
    unsafe(justification: "backend gate probe") {
        asm {
            mov eax, x
            add eax, 1
            mov x, eax
        }
    }
    out x
}
"#;
    let compiler =
        ForgenCompiler::new("release").with_target(Some("wasm32-unknown-unknown".into()));
    let res = compiler.compile_source(source, "v140_asm_wasm.dtr", None);
    assert!(!res.success, "asm on wasm must be rejected");
    assert!(
        res.diagnostics.contains("E1405"),
        "must report E1405, got:\n{}",
        res.diagnostics
    );
}

/// Legacy `asm! { "..." }` templates keep their raw (LLVM-only) contract and
/// are not affected by the structured subset gates.
#[test]
fn legacy_asm_template_still_parses() {
    let source = r#"
fn main() {
    mut x = 1
    unsafe(justification: "legacy template path") {
        asm! {
            "nop"
        }
    }
    out x
}
"#;
    let compiler = ForgenCompiler::new("debug");
    // The LLVM-only rejection (E0902) or a successful check both prove the
    // legacy path is untouched by the structured-subset validation; what
    // must NOT happen is an E1402/E1404 diagnostic.
    let res = compiler.check_source(source, "v140_asm_legacy.dtr");
    assert!(
        !res.diagnostics.contains("E1402") && !res.diagnostics.contains("E1404"),
        "legacy asm! must not hit the structured-subset gates, got:\n{}",
        res.diagnostics
    );
}
