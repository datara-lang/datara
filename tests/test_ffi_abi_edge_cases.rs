//! Milestone 3 - FFI ABI Edge Cases (v1.3.1 stability plan).
//!
//! Exercises value-domain and calling-convention corners of the C import
//! path on top of the existing `test_abi_lib` fixture:
//! 1. Negative i64 values through struct-by-value parameters.
//! 2. Values that exceed 32-bit range (i64 width preservation).
//! 3. Int32 field boundary (i32::MAX + 1 widened into an i64 sum).
//! 4. Nested cross-language callbacks: C calls Datara, result fed back into C.
//! 5. Struct returned by value from C, consumed by Datara.
//!
//! Compilation and execution run inside a 64 MiB-stack thread (see bcfac07).

#![cfg(windows)]

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

    // 1. Compile C file to object file
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

    // 2. Create static library archive (.lib)
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

fn build_test_lib(lib_name: &str) -> (PathBuf, PathBuf) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_h = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_abi_lib.h");
    let fixture_c = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("test_abi_lib.c");
    let out_lib = manifest_dir.join("tests").join("fixtures").join(lib_name);

    assert!(fixture_h.exists(), "test_abi_lib.h must exist");
    assert!(fixture_c.exists(), "test_abi_lib.c must exist");
    compile_c_fixture_to_lib(&fixture_c, &out_lib)
        .expect("Must compile C fixture into static library");

    (fixture_h, out_lib)
}

#[test]
fn test_ffi_abi_edge_case_value_domain() {
    let (fixture_h, out_lib) = build_test_lib("test_abi_edge_cases_value.lib");
    let h_path_str = fixture_h.to_string_lossy().replace('\\', "/");
    let lib_path_str = out_lib.to_string_lossy().replace('\\', "/");

    let source = format!(
        r#"
import c "{h}" with link("{lib}");

fn double_cb(x: Int) -> Int => x * 2

fn inc_cb(x: Int) -> Int => x + 1

fn main() {{
    let pn = Point {{ x: -15, y: -27 }}
    let pw = Point {{ x: 1000000007, y: 2000000011 }}
    let pa = Point {{ x: 10, y: 20 }}
    let pb = Point {{ x: 30, y: 40 }}
    mut neg_sum = 0
    mut wide_sum = 0
    mut boundary = 0
    mut cb_chain = 0
    mut quad = 0
    unsafe(justification: "FFI ABI edge-case round-trips through C static library") {{
        neg_sum = sum_point(pn)
        wide_sum = sum_point(pw)
        boundary = sum_pair(make_pair(2147483647, 1))
        cb_chain = apply_callback(apply_callback(5, double_cb), inc_cb)
        quad = sum_two_points(pa, pb)
    }}

    out "NEG: " + neg_sum
    out "WIDE: " + wide_sum
    out "I32_BOUNDARY: " + boundary
    out "CB_CHAIN: " + cb_chain
    out "QUAD: " + quad
}}
"#,
        h = h_path_str,
        lib = lib_path_str
    );

    let res = compile_in_big_stack(source, "test_ffi_abi_edge_cases.dtr");
    assert!(
        res.success,
        "FFI edge-case compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let exe = res.exe_path.expect("Must produce native executable");
    let compiler = ForgenCompiler::new("release");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("Must run native executable");
    assert_eq!(code, 0, "Execution failed: {}", stderr);

    assert!(
        stdout.contains("NEG: -42"),
        "Negative i64 struct round trip failed:\n{}",
        stdout
    );
    assert!(
        stdout.contains("WIDE: 3000000018"),
        "i64 width preservation across FFI failed:\n{}",
        stdout
    );
    assert!(
        stdout.contains("I32_BOUNDARY: 2147483648"),
        "i32 boundary widening failed:\n{}",
        stdout
    );
    assert!(
        stdout.contains("CB_CHAIN: 11"),
        "Nested C->Datara->C callback chain failed:\n{}",
        stdout
    );
    assert!(
        stdout.contains("QUAD: 100"),
        "Two struct-by-value parameters failed:\n{}",
        stdout
    );

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    let _ = std::fs::remove_file(&out_lib);
}

#[test]
fn test_ffi_abi_struct_return_sret_roundtrip() {
    // v1.3.2 M3: the native backend implements the hidden sret return-slot
    // ABI for layout-compatible structs, so the 16-byte Point return of
    // `add_points` now round-trips cleanly: add_points -> Point (sret) ->
    // sum_point (by-reference parameter) -> 110. The same source used to be
    // rejected at compile time with E0962 before the sret implementation.
    let (fixture_h, out_lib) = build_test_lib("test_abi_edge_cases_struct.lib");
    let h_path_str = fixture_h.to_string_lossy().replace('\\', "/");
    let lib_path_str = out_lib.to_string_lossy().replace('\\', "/");

    let source = format!(
        r#"
import c "{h}" with link("{lib}");

fn main() {{
    mut total = 0
    unsafe(justification: "Struct-by-value return from C consumed by Datara") {{
        total = sum_point(add_points(make_point(11, 22), make_point(33, 44)))
    }}
    out "STRUCT_RETURN: " + total
}}
"#,
        h = h_path_str,
        lib = lib_path_str
    );

    let res = compile_in_big_stack(source, "test_ffi_struct_return.dtr");
    assert!(
        res.success,
        "A layout-compatible struct-returning C import must compile through the sret path: {:?}\n{}",
        res.error, res.diagnostics
    );

    let exe = res.exe_path.expect("Must produce native executable");
    let compiler = ForgenCompiler::new("release");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("Must run native executable");
    assert_eq!(code, 0, "Execution failed: {}", stderr);
    assert!(
        stdout.contains("STRUCT_RETURN: 110"),
        "Nested sret struct return round trip failed:\n{}",
        stdout
    );

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    let _ = std::fs::remove_file(&out_lib);
}
