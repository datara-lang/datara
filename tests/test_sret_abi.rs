//! Milestone 3 - sret ABI (v1.3.2 plan).
//!
//! C functions returning by-value structs larger than one machine word take
//! the hidden sret return-slot path on Microsoft x64: the caller allocates
//! the return buffer and passes its pointer as the first argument. This
//! suite pins down the full boundary:
//! 1. Native round trip: 16/24/32-byte layout-compatible structs come back
//!    through the sret slot and Datara reads both/all fields correctly.
//! 2. The 8-byte struct path (raw bits in RAX) is unchanged.
//! 3. Structs whose C layout cannot be read back through the Datara object
//!    layout stay rejected with E0962.
//! 4. Backends without sret support (LLVM, WASM) keep the E0962 rejection.
//!
//! Compilation and execution run inside a 64 MiB-stack thread (see bcfac07).

#![cfg(windows)]

use forgen::ast::Decl;
use forgen::codegen::linker::ensure_linker;
use forgen::driver::{CompilationResult, ForgenCompiler};
use std::path::{Path, PathBuf};
use std::process::Command;

fn compile_c_fixture_to_lib(c_src: &Path, out_lib: &Path) -> Result<(), String> {
    let spec = ensure_linker().map_err(|e| format!("Linker not found: {}", e))?;
    let bin_dir = spec
        .program
        .parent()
        .ok_or_else(|| "Failed to get linker directory".to_string())?;
    let cl_exe = bin_dir.join("cl.exe");
    let lib_exe = bin_dir.join("lib.exe");

    let obj_path = out_lib.with_extension("obj");

    let mut cl_cmd = Command::new(&cl_exe);
    cl_cmd.args([
        "/c",
        "/O2",
        "/nologo",
        "/GS-",
        &format!("/Fo:{}", obj_path.display()),
        &format!("{}", c_src.display()),
    ]);
    let cl_res = cl_cmd
        .output()
        .map_err(|e| format!("Failed to invoke cl.exe at {}: {}", cl_exe.display(), e))?;
    if !cl_res.status.success() {
        return Err(format!(
            "cl.exe failed: {}\nstdout: {}\nstderr: {}",
            cl_res.status,
            String::from_utf8_lossy(&cl_res.stdout),
            String::from_utf8_lossy(&cl_res.stderr)
        ));
    }

    let mut lib_cmd = Command::new(&lib_exe);
    lib_cmd.args([
        "/nologo",
        &format!("/OUT:{}", out_lib.display()),
        &format!("{}", obj_path.display()),
    ]);
    let lib_res = lib_cmd
        .output()
        .map_err(|e| format!("Failed to invoke lib.exe at {}: {}", lib_exe.display(), e))?;
    if !lib_res.status.success() {
        return Err(format!(
            "lib.exe failed: {}\nstdout: {}\nstderr: {}",
            lib_res.status,
            String::from_utf8_lossy(&lib_res.stdout),
            String::from_utf8_lossy(&lib_res.stderr)
        ));
    }

    let _ = std::fs::remove_file(&obj_path);
    Ok(())
}

fn compile_in_big_stack(source: String, file: &'static str) -> CompilationResult {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || ForgenCompiler::new("release").compile_source_native(&source, file, None))
        .expect("failed to spawn compiler thread")
        .join()
        .expect("compiler thread panicked")
}

fn check_in_big_stack(source: String, file: &'static str) -> CompilationResult {
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

/// Imported C-ABI functions (Decl::ExternFn) that survived the expansion.
fn extern_names(res: &CompilationResult) -> Vec<(String, Option<usize>)> {
    let mut names = Vec::new();
    if let Some(program) = &res.program {
        for decl in &program.declarations {
            if let Decl::ExternFn(ef) = decl {
                names.push((ef.name.clone(), ef.sret_size));
            }
        }
    }
    names
}

#[test]
#[cfg(windows)]
fn test_sret_struct_return_native_roundtrip() {
    // Full native round trip: the C fixture is compiled into a static
    // library (cl.exe + lib.exe) and linked into the executable. Skips
    // cleanly when the host has no C/C++ toolchain configured.
    let spec = match ensure_linker() {
        Ok(spec) => spec,
        Err(_) => {
            eprintln!(
                "SKIP: test_sret_struct_return_native_roundtrip requires a native C/C++ \
                 toolchain (cl.exe/lib.exe/link.exe), which is not configured here."
            );
            return;
        }
    };
    let _ = spec;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_h = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_sret_lib.h");
    let fixture_c = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_sret_lib.c");
    let out_lib = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_sret_abi.lib");

    assert!(fixture_h.exists(), "test_sret_lib.h must exist");
    assert!(fixture_c.exists(), "test_sret_lib.c must exist");
    compile_c_fixture_to_lib(&fixture_c, &out_lib)
        .expect("Must compile sret C fixture into static library");

    let h_path_str = fixture_h.to_string_lossy().replace('\\', "/");
    let lib_path_str = out_lib.to_string_lossy().replace('\\', "/");

    let source = format!(
        r#"
import c "{h}" with link("{lib}");

fn main() {{
    mut v = Vec2d {{ x: 0.0, y: 0.0 }}
    mut s = Vec2d {{ x: 0.0, y: 0.0 }}
    mut n = Vec2d {{ x: 0.0, y: 0.0 }}
    mut p = Pair {{ a: 0, b: 0 }}
    mut w = Word {{ v: 0 }}
    mut dot = 0.0
    mut ndot = 0.0
    mut psum = 0
    mut wval = 0
    unsafe(justification: "Sret struct returns round trip through C static library") {{
        v = make_vec2d(1.5, 2.5)
        s = add_vec2d(v, make_vec2d(10.0, 20.0))
        dot = vec2d_dot(v, s)
        n = make_vec2d(-1.5, -2.5)
        // Both read paths of a reassigned sret result are pinned: direct
        // field reads (NX/NY) and reading through another extern call that
        // forces the struct to escape scalar replacement (NDOT).
        ndot = vec2d_dot(n, n)
        p = make_pair(-7, 1000000000000)
        psum = sum_pair(p)
        w = make_word(21)
        wval = word_value(w)
    }}

    out fmt"VX: {{v.x}}"
    out fmt"VY: {{v.y}}"
    out fmt"SX: {{s.x}}"
    out fmt"SY: {{s.y}}"
    out fmt"NX: {{n.x}}"
    out fmt"NY: {{n.y}}"
    out fmt"NDOT: {{ndot}}"
    out fmt"DOT: {{dot}}"
    out "PSUM: " + psum
    out "WVAL: " + wval
}}
"#,
        h = h_path_str,
        lib = lib_path_str
    );

    let res = compile_in_big_stack(source, "test_sret_abi.dtr");
    assert!(
        res.success,
        "Sret struct-return compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let exe = res.exe_path.expect("Must produce native executable");
    let compiler = ForgenCompiler::new("release");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("Must run native executable");
    assert_eq!(code, 0, "Execution failed: {}", stderr);

    for expected in [
        "VX: 1.5",
        "VY: 2.5",
        "SX: 11.5",
        "SY: 22.5",
        "NX: -1.5",
        "NY: -2.5",
        "NDOT: 8.5",
        "DOT: 73.5",
        // -7 + 1_000_000_000_000 = 999_999_999_993 (proves negative i64
        // arguments and a 16-byte integer sret struct survive the round trip).
        "PSUM: 999999999993",
        "WVAL: 21",
    ] {
        assert!(
            stdout.contains(expected),
            "Sret round trip output must contain '{expected}':\n{}",
            stdout
        );
    }

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    let _ = std::fs::remove_file(&out_lib);
}

#[test]
#[cfg(windows)]
fn test_sret_import_classification_structural() {
    // Structural boundary, no native linking required: eligible struct
    // returns survive as typed externs carrying their sret size, the
    // 8-byte struct keeps the register path (no sret marker), and the
    // >16-byte and layout-incompatible structs are rejected with E0962.
    let source = r#"
import c "tests/fixtures/test_sret_lib.h";

fn main() {
    out 0
}
"#;
    let res = check_in_big_stack(source.to_string(), "test_sret_classification.dtr");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        res.success,
        "The sret classification program must check cleanly:\n{}",
        report
    );

    let externs = extern_names(&res);
    let find = |name: &str| {
        externs
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("Expected imported extern '{name}' to survive expansion"))
            .1
    };

    assert_eq!(find("make_vec2d"), Some(16), "Vec2d is a 16-byte sret");
    assert_eq!(find("add_vec2d"), Some(16));
    assert_eq!(
        find("make_pair"),
        Some(16),
        "Pair is a 16-byte integer sret"
    );
    assert_eq!(
        find("make_word"),
        None,
        "8-byte Word must keep the raw-register path"
    );
    assert_eq!(find("vec2d_dot"), None, "scalar returns carry no sret size");
    assert_eq!(find("sum_pair"), None, "scalar returns carry no sret size");
    for rejected in [
        "make_triple",
        "make_quad",
        "make_float_triple",
        "make_mixed",
    ] {
        assert!(
            !externs.iter().any(|(n, _)| n == rejected),
            "'{rejected}' must not survive expansion (above the 16-byte sret window or layout-incompatible)"
        );
        assert!(
            report.contains(rejected),
            "Diagnostic must name the rejected C function '{rejected}':\n{}",
            report
        );
    }
    assert!(
        !report.contains("make_vec2d") || !report.contains("E0962"),
        "Eligible struct returns must not be rejected:\n{}",
        report
    );
    assert!(
        report.contains("E0962"),
        "Diagnostic must carry the E0962 unsupported-construct code:\n{}",
        report
    );
}

#[test]
#[cfg(windows)]
fn test_sret_rejected_for_backends_without_support() {
    // LLVM keeps its compile-time rejection: it has no sret lowering for
    // C imports, so the declaration must not survive expansion there.
    let source = r#"
import c "tests/fixtures/test_sret_lib.h";

fn main() {
    out 0
}
"#;
    let compiler = ForgenCompiler::new("release").with_llvm(true);
    let res = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || compiler.check_source(source, "test_sret_llvm_gate.dtr"))
        .expect("failed to spawn compiler thread")
        .join()
        .expect("compiler thread panicked");
    let report = format!(
        "{}\n{}",
        res.error.clone().unwrap_or_default(),
        res.diagnostics
    );
    assert!(
        report.contains("make_vec2d"),
        "Diagnostic must name the rejected C function:\n{}",
        report
    );
    assert!(
        report.contains("E0962"),
        "Diagnostic must carry the E0962 unsupported-construct code:\n{}",
        report
    );
    assert!(
        !extern_names(&res).iter().any(|(n, _)| n == "make_vec2d"),
        "Vec2d-returning import must not survive expansion for the LLVM backend"
    );
}
