//! Milestone M2 - cimport `extern "C"` blocks & multi-line declarations
//! (v1.3.2 stability plan).
//!
//! Extends the v1.3.1 cimport work (commit 51a833e, E0962 struct-return
//! gate) with two new accepted declaration forms:
//! 1. Block form: several signatures listed inside `extern "C" { ... }`
//!    linkage blocks in the imported header (preprocessor-guarded or not)
//!    are all imported by `import c`.
//! 2. Multi-line declarations: a single C function declaration whose return
//!    type, parameter list and terminating semicolon span several source
//!    lines parses correctly.
//!
//! The struct-return ABI boundary is form-independent: since v1.3.2 M3 a
//! layout-compatible struct return larger than one 64-bit machine word
//! takes the hidden sret slot, while a layout-incompatible one (or any
//! struct return on a backend without sret support) is still rejected with
//! E0962 even when declared inside an extern "C" block.
//!
//! Structural and diagnostics assertions run through `check_source` (no
//! native linking required). The native call round-trip mirrors the v1.3.1
//! fixture mechanism (cl.exe + lib.exe) and skips cleanly when no C/C++
//! toolchain is configured in the environment.

use forgen::ast::Decl;
use forgen::codegen::linker::ensure_linker;
use forgen::driver::{CompilationResult, ForgenCompiler};
use std::path::PathBuf;
use std::process::Command;

fn compile_in_big_stack(source: String, file: &'static str) -> CompilationResult {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let compiler = ForgenCompiler::new("release");
            compiler.check_source(&source, file)
        })
        .expect("failed to spawn compiler thread")
        .join()
        .expect("compiler thread panicked")
}

/// Imported C-ABI functions (`Decl::ExternFn`) plus zero-arg constants the
/// expansion emits as plain functions (enum variants, `#define` integers).
fn imported_c_symbol_names(res: &CompilationResult) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(program) = &res.program {
        for decl in &program.declarations {
            match decl {
                Decl::ExternFn(ef) => names.push(ef.name.clone()),
                Decl::Function(f) => names.push(f.name.clone()),
                _ => {}
            }
        }
    }
    names
}

/// Imported C struct/typedef names (expansion emits structs as classes).
fn imported_c_type_names(res: &CompilationResult) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(program) = &res.program {
        for decl in &program.declarations {
            match decl {
                Decl::Class(c) => names.push(c.name.clone()),
                Decl::Type(t) => names.push(t.name.clone()),
                _ => {}
            }
        }
    }
    names
}

#[test]
fn test_extern_block_form_imports_all_signatures() {
    let source = r#"
import c "tests/fixtures/test_extern_block.h";

fn main() {
    let base = BLOCK_BASE()
    let idle = BLOCK_IDLE()
    mut a = 0
    mut s = 0
    mut n = 0
    mut p = 0
    unsafe(justification: "Calling block-form extern C functions") {
        a = block_add(20, 22)
        s = block_scale(6, 7)
        n = block_negate(5)
        p = block_plain(4)
    }
    if a == 42 && s == 42 && n == -5 && p == 40 && base == 1000 && idle == 7 {
        out 42
    } else {
        out 0
    }
}
"#;
    let res = compile_in_big_stack(source.to_string(), "test_extern_block_form.dtr");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        res.success,
        "Block-form extern C imports must typecheck cleanly:\n{}",
        report
    );

    // Every signature from the linkage blocks plus the legacy per-item
    // declaration must be present as a typed C-ABI extern.
    let names = imported_c_symbol_names(&res);
    for expected in [
        "block_add",
        "block_scale",
        "block_negate",
        "block_plain",
        "BLOCK_BASE",
        "BLOCK_IDLE",
        "BLOCK_BUSY",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "Expected imported symbol '{}' from extern block form; got:\n{:?}",
            expected,
            names
        );
    }
}

#[test]
fn test_multiline_declaration_parses() {
    let source = r#"
import c "tests/fixtures/test_multiline_decl.h";

fn double_cb(x: Int) -> Int => x * 2

fn main() {
    let s = MlSpan { base: 10, len: 3 }
    mut a = 0
    mut m = 0
    mut f = 0
    mut c = 0
    unsafe(justification: "Calling multi-line declared C functions") {
        a = ml_add(2, 3)
        m = ml_mul(4, 5)
        f = ml_fold(s, 2)
        c = ml_apply(21, double_cb)
    }
    if a == 5 && m == 20 && f == 26 && c == 42 {
        out 42
    } else {
        out 0
    }
}
"#;
    let res = compile_in_big_stack(source.to_string(), "test_multiline_decl.dtr");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        res.success,
        "Multi-line C declarations must parse and typecheck:\n{}",
        report
    );

    for expected in ["ml_add", "ml_mul", "ml_fold", "ml_apply"] {
        let names = imported_c_symbol_names(&res);
        assert!(
            names.iter().any(|n| n == expected),
            "Expected imported symbol '{}' from multi-line declarations; got:\n{:?}",
            expected,
            names
        );
    }
    // The multi-line struct typedef imports as a typed record.
    let type_names = imported_c_type_names(&res);
    assert!(
        type_names.iter().any(|n| n == "MlSpan"),
        "Expected struct 'MlSpan' from the multi-line header; got:\n{:?}",
        type_names
    );
}

#[test]
fn test_extern_block_struct_return_still_rejected_e0962() {
    // The struct-return ABI boundary is form-independent: since v1.3.2 M3
    // the layout-compatible `big_add` return takes the hidden sret slot and
    // survives expansion, while `big_mixed` (C layout Datara cannot read
    // back) is rejected with E0962 and `big_sum` (struct-by-value params,
    // scalar return) keeps importing cleanly.
    let source = r#"
import c "tests/fixtures/test_extern_block_bad_ret.h";

fn main() {
    let a = BigPoint { x: 1, y: 2 }
    let b = BigPoint { x: 3, y: 4 }
    mut s = 0
    mut r = BigPoint { x: 0, y: 0 }
    unsafe(justification: "Calling block-form extern C functions") {
        r = big_add(a, b)
        s = big_sum(a, b)
    }
    out s + r.x + r.y
}
"#;
    let res = compile_in_big_stack(source.to_string(), "test_extern_block_bad_ret.dtr");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    // `big_add` returns a layout-compatible 16-byte struct through the sret
    // slot, so the program must now typecheck cleanly.
    assert!(
        res.success,
        "An eligible >8-byte struct return inside an extern block must compile through the sret path:\n{}",
        report
    );
    let names = imported_c_symbol_names(&res);
    assert!(
        names.iter().any(|n| n == "big_add"),
        "Eligible struct-returning 'big_add' must survive expansion:\n{:?}",
        names
    );
    assert!(
        names.iter().any(|n| n == "big_sum"),
        "Sibling function 'big_sum' from the same block must still import:\n{:?}",
        names
    );
    assert!(
        !names.iter().any(|n| n == "big_mixed"),
        "Layout-incompatible struct-returning 'big_mixed' must not survive expansion"
    );
    assert!(
        report.contains("big_mixed"),
        "Diagnostic must name the rejected C function:\n{}",
        report
    );
    assert!(
        report.contains("E0962"),
        "Diagnostic must carry the E0962 unsupported-construct code:\n{}",
        report
    );
}

#[test]
fn test_single_decl_linkage_spec_and_plain_extern() {
    let source = r#"
import c "tests/fixtures/test_extern_single_decl.h";

fn main() {
    mut d = 0
    mut p = 0
    unsafe(justification: "Calling single-decl linkage extern C functions") {
        d = solo_double(21)
        p = solo_plain(4)
    }
    if d == 42 && p == 4 {
        out 42
    } else {
        out 0
    }
}
"#;
    let res = compile_in_big_stack(source.to_string(), "test_extern_single_decl.dtr");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        res.success,
        "extern \"C\" <decl> and plain extern <decl> must parse:\n{}",
        report
    );
    let names = imported_c_symbol_names(&res);
    assert!(
        names.iter().any(|n| n == "solo_double"),
        "Expected solo_double from the single-decl linkage spec:\n{:?}",
        names
    );
    assert!(
        names.iter().any(|n| n == "solo_plain"),
        "Plain extern qualifier path must keep working:\n{:?}",
        names
    );
}

#[test]
fn test_one_line_guarded_block_no_noise() {
    // `#if defined(__cplusplus) extern "C" {` hides the block braces from
    // the declaration parser; the stray closing brace must be tolerated
    // without spurious unsupported-construct warnings.
    let source = r#"
import c "tests/fixtures/test_extern_guarded_oneline.h";

fn main() {
    mut a = 0
    unsafe(justification: "Calling one-line-guarded extern C function") {
        a = guard_add(40, 2)
    }
    out a
}
"#;
    let res = compile_in_big_stack(source.to_string(), "test_extern_guarded_oneline.dtr");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        res.success,
        "One-line-guarded extern C block must parse cleanly:\n{}",
        report
    );
    assert!(
        !report.contains("E0962"),
        "Stray linkage-block braces must not produce diagnostics:\n{}",
        report
    );
    assert!(
        imported_c_symbol_names(&res)
            .iter()
            .any(|n| n == "guard_add"),
        "guard_add must be imported from the guarded header"
    );
}

#[test]
fn test_legacy_per_item_import_unchanged() {
    // The pre-v1.3.2 per-item syntax (plain declarations in the header,
    // no linkage block) must expand identically to before.
    let source = r#"
import c "tests/fixtures/test_cimport.h" with link("kernel32.lib");

fn main() {
    let ok = TEST_OK()
    let running = STATUS_RUNNING()
    mut pid = 0
    unsafe(justification: "Calling Win32 GetCurrentProcessId") {
        pid = GetCurrentProcessId()
    }
    if pid > 0 && ok == 0 && running == 200 {
        out 42
    } else {
        out 0
    }
}
"#;
    let res = compile_in_big_stack(source.to_string(), "test_legacy_per_item.dtr");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        res.success,
        "Legacy per-item cimport must keep typechecking:\n{}",
        report
    );
    for expected in ["GetCurrentProcessId", "STATUS_RUNNING", "TEST_OK"] {
        assert!(
            imported_c_symbol_names(&res).iter().any(|n| n == expected),
            "Legacy import of '{}' must be unchanged:\n{:?}",
            expected,
            imported_c_symbol_names(&res)
        );
    }
}

#[test]
fn test_extern_block_jit_run_prints_value() {
    // End-to-end call of a block-form import through the in-memory JIT,
    // which resolves datara_rt_* runtime C symbols without a native
    // toolchain. Proves the block form lowers to a callable C symbol.
    let source = r#"
import c "tests/fixtures/test_extern_block_runtime.h";

fn main() {
    mut t = 0.0
    unsafe(justification: "Calling runtime time function declared in extern C block") {
        t = datara_rt_time_precise_ms()
    }
    if t > 0.0 {
        out 42
    } else {
        out 0
    }
}
"#;
    let compiler = ForgenCompiler::new("release");
    let (stdout, stderr, code, _) = compiler
        .run_source(source, "test_extern_block_jit.dtr", &[], true)
        .expect("JIT run of block-form extern C program");
    assert_eq!(code, 0, "JIT execution failed: {}", stderr);
    assert_eq!(
        stdout.trim(),
        "42",
        "Block-form import must be callable under JIT, got:\n{}",
        stdout
    );
}

#[test]
#[cfg(windows)]
fn test_extern_block_native_call_roundtrip() {
    // Full native round-trip through the v1.3.1 fixture mechanism: the C
    // implementation of the block-form fixture is compiled into a static
    // library and linked into the executable. Skips cleanly when the host
    // has no C/C++ toolchain configured (environmental limitation).
    let spec = match ensure_linker() {
        Ok(spec) => spec,
        Err(_) => {
            eprintln!(
                "SKIP: test_extern_block_native_call_roundtrip requires a native C/C++ \
                 linker (MSVC link.exe), which is not configured in this environment."
            );
            return;
        }
    };
    let bin_dir = spec
        .program
        .parent()
        .ok_or_else(|| "Failed to get linker directory".to_string())
        .expect("linker directory");
    let cl_exe = bin_dir.join("cl.exe");
    let lib_exe = bin_dir.join("lib.exe");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_h = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_extern_block.h");
    let fixture_c = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_extern_block.c");
    let out_lib = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_extern_block.lib");
    let obj_path = out_lib.with_extension("obj");

    let mut cl_cmd = Command::new(&cl_exe);
    cl_cmd.args([
        "/c",
        "/O2",
        "/nologo",
        "/GS-",
        &format!("/Fo:{}", obj_path.display()),
        &format!("{}", fixture_c.display()),
    ]);
    let cl_res = cl_cmd.output().expect("cl.exe invocation");
    assert!(
        cl_res.status.success(),
        "cl.exe failed: {}",
        String::from_utf8_lossy(&cl_res.stderr)
    );

    let mut lib_cmd = Command::new(&lib_exe);
    lib_cmd.args([
        "/nologo",
        &format!("/OUT:{}", out_lib.display()),
        &format!("{}", obj_path.display()),
    ]);
    let lib_res = lib_cmd.output().expect("lib.exe invocation");
    assert!(
        lib_res.status.success(),
        "lib.exe failed: {}",
        String::from_utf8_lossy(&lib_res.stderr)
    );
    let _ = std::fs::remove_file(&obj_path);

    let h_path_str = fixture_h.to_string_lossy().replace('\\', "/");
    let lib_path_str = out_lib.to_string_lossy().replace('\\', "/");
    let source = format!(
        r#"
import c "{h}" with link("{lib}");

fn main() {{
    let base = BLOCK_BASE()
    let idle = BLOCK_IDLE()
    mut a = 0
    mut s = 0
    mut n = 0
    mut p = 0
    unsafe(justification: "Calling block-form extern C functions from static library") {{
        a = block_add(20, 22)
        s = block_scale(6, 7)
        n = block_negate(5)
        p = block_plain(4)
    }}
    if a == 42 && s == 42 && n == -5 && p == 40 && base == 1000 && idle == 7 {{
        out 42
    }} else {{
        out 0
    }}
}}
"#,
        h = h_path_str,
        lib = lib_path_str
    );

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(&source, "test_extern_block_run.dtr", None);
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        res.success,
        "Block-form extern C native compilation failed:\n{}",
        report
    );

    let exe = res.exe_path.expect("Must produce native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("Must run native executable");
    assert_eq!(code, 0, "Execution failed: {}", stderr);
    assert_eq!(
        stdout.trim(),
        "42",
        "Block-form imports must be callable natively, got:\n{}",
        stdout
    );

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    let _ = std::fs::remove_file(&out_lib);
}
