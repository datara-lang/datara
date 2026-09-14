//! Milestone 3 - SoA ABI C-consumer round trip (v1.3.1 stability plan).
//!
//! A real C consumer library (`tests/fixtures/soa_consumer.{h,c}`) is
//! compiled to a static library and imported by Datara. The round trip
//! validates:
//! 1. Structure-of-Arrays descriptor layout agreement (sizeof == 24 on 64-bit).
//! 2. Struct-by-value ABI in both directions (Datara -> C fields intact).
//! 3. Cross-language callbacks: C folds elements produced by a Datara fn.
//! 4. Stride arithmetic: last-element offset `base + (len - 1) * stride`.
//! 5. Negative i64 values through struct fields.
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

#[test]
fn test_soa_c_consumer_round_trip() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_h = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("soa_consumer.h");
    let fixture_c = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("soa_consumer.c");
    let out_lib = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("soa_consumer.lib");

    assert!(fixture_h.exists(), "soa_consumer.h must exist");
    assert!(fixture_c.exists(), "soa_consumer.c must exist");
    compile_c_fixture_to_lib(&fixture_c, &out_lib)
        .expect("Must compile SoA C consumer into static library");

    let h_path_str = fixture_h.to_string_lossy().replace('\\', "/");
    let lib_path_str = out_lib.to_string_lossy().replace('\\', "/");

    let source = format!(
        r#"
import c "{h}" with link("{lib}");

fn elem_provider(i: Int) -> Int => i * 3

fn main() {{
    mut abi_version = 0
    mut comp_size = 0
    mut last_off = 0
    mut folded = 0
    mut scaled = 0
    mut neg_len = 0
    unsafe(justification: "SoA C-consumer ABI round trip: struct by value and cross-language callback") {{
        abi_version = soa_abi_version()
        comp_size = soa_component_sizeof()
        last_off = soa_last_offset(SoaComponent {{ base: 1000, len: 8, stride: 8 }})
        folded = soa_fold_indexed(SoaComponent {{ base: 0, len: 8, stride: 8 }}, elem_provider)
        scaled = soa_fold_scaled(SoaComponent {{ base: 0, len: 8, stride: 8 }}, 2, elem_provider)
        neg_len = soa_negated_len(SoaComponent {{ base: -4096, len: 64, stride: 8 }})
    }}

    out "SOA_ABI: " + abi_version
    out "SOA_SIZEOF: " + comp_size
    out "SOA_LAST_OFFSET: " + last_off
    out "SOA_FOLDED: " + folded
    out "SOA_SCALED: " + scaled
    out "SOA_NEG_LEN: " + neg_len
}}
"#,
        h = h_path_str,
        lib = lib_path_str
    );

    let res = compile_in_big_stack(source, "test_soa_abi_c_consumer.dtr");
    assert!(
        res.success,
        "SoA C-consumer compilation failed: {:?}\n{}",
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
        stdout.contains("SOA_ABI: 1"),
        "ABI version probe failed:\n{}",
        stdout
    );
    assert!(
        stdout.contains("SOA_SIZEOF: 24"),
        "SoA descriptor layout mismatch (expected 24 bytes on 64-bit):\n{}",
        stdout
    );
    assert!(
        stdout.contains("SOA_LAST_OFFSET: 1056"),
        "Stride arithmetic failed (1000 + 7*8 = 1056):\n{}",
        stdout
    );
    assert!(
        stdout.contains("SOA_FOLDED: 84"),
        "C fold over Datara callback failed (sum i*3 for i in 0..8 = 84):\n{}",
        stdout
    );
    assert!(
        stdout.contains("SOA_SCALED: 168"),
        "C scaled fold failed (84 * 2 = 168):\n{}",
        stdout
    );
    assert!(
        stdout.contains("SOA_NEG_LEN: -64"),
        "Negative i64 struct-field round trip failed:\n{}",
        stdout
    );

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    let _ = std::fs::remove_file(&out_lib);
}
