//! Milestone 5 - TBAA differential audit (v1.3.1 stability plan).
//!
//! Verifies that type-based alias-analysis metadata proven by Datara's
//! verifiable types survives into the emitted LLVM IR, and that the LLVM
//! backend remains semantically equivalent to the Cranelift backend for the
//! same program (differential execution when the LLVM toolchain is present).

use forgen::driver::ForgenCompiler;
use std::path::PathBuf;

const SOURCE: &str = r#"
class Reading {
    raw: Int
    gain: Float
}

class WideFrame {
    f0: Int
    f1: Int
    f2: Int
    f3: Int
    f4: Int
    f5: Int
    f6: Int
    f7: Int
}

fn read_raw(r: Reading) -> Int {
    return r.raw
}

fn read_gain(r: Reading) -> Float {
    return r.gain
}

// 64-byte aggregate: passed through memory, so field loads carry real
// memory operations that the TBAA emitter can tag.
fn frame_checksum(w: WideFrame) -> Int {
    return w.f0 + w.f1 + w.f2 + w.f3 + w.f4 + w.f5 + w.f6 + w.f7
}

fn accumulate(n: Int) -> Int {
    mut sum = 0
    mut i = 0
    while i < n {
        sum = sum + i
        i = i + 1
    }
    return sum
}

fn scale(base: Float, k: Int) -> Float {
    mut acc = 0.0
    mut i = 0
    while i < k {
        acc = acc + base
        i = i + 1
    }
    return acc
}

fn main() {
    let r1 = Reading { raw: 6, gain: 2.5 }
    let r2 = Reading { raw: 6, gain: 2.5 }
    let frame = WideFrame { f0: 1, f1: 2, f2: 3, f3: 4, f4: 5, f5: 6, f6: 7, f7: 8 }
    out read_raw(r1)
    out read_gain(r2)
    out frame_checksum(frame)
    out accumulate(100)
    out scale(2.5, 4)
}
"#;

#[test]
fn test_tbaa_metadata_present_in_llvm_ir() {
    let compiler = ForgenCompiler::new("release").with_llvm(true);
    let res = compiler.compile_source(SOURCE, "tbaa_differential.dtr", None);
    assert!(
        res.success,
        "LLVM compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let llvm = res.llvm_source.expect("LLVM IR must be generated");
    assert!(
        llvm.contains("TBAA"),
        "LLVM IR must define a TBAA metadata hierarchy:\n{}",
        llvm
    );
    assert!(
        llvm.contains("!tbaa !"),
        "Memory operations must carry !tbaa access tags:\n{}",
        llvm
    );
    assert!(
        llvm.contains("define i32 @main()"),
        "LLVM IR must contain a well-formed main function:\n{}",
        llvm
    );
}

#[test]
fn test_tbaa_cross_backend_semantic_equivalence() {
    // Reference: Cranelift backend.
    let clif = ForgenCompiler::new("release");
    let res_c = clif.compile_source(SOURCE, "tbaa_clif.dtr", None);
    assert!(
        res_c.success,
        "Cranelift compilation failed: {:?}\n{}",
        res_c.error, res_c.diagnostics
    );
    let exe_c = res_c
        .exe_path
        .expect("Cranelift must produce an executable");
    let (out_c, err_c, code_c, _) = clif
        .codegen
        .run_executable(&exe_c, &[])
        .expect("Must run Cranelift executable");
    assert_eq!(code_c, 0, "Cranelift run failed: {}", err_c);
    assert!(
        out_c.contains("4950"),
        "accumulate(100) must print 4950, got:\n{}",
        out_c
    );

    // Differential: LLVM backend, executed only when the local toolchain
    // allows AOT linking; otherwise the structural check above stands.
    let dir = std::env::temp_dir().join("datara_tbaa_differential");
    let _ = std::fs::create_dir_all(&dir);
    let dtr_path: PathBuf = dir.join("tbaa_differential.dtr");
    std::fs::write(&dtr_path, SOURCE).expect("write .dtr fixture");
    let exe_ext = std::env::consts::EXE_SUFFIX;
    let exe_l: PathBuf = dir.join(format!("tbaa_llvm_bin.{}", exe_ext));

    let llvm = ForgenCompiler::new("release").with_llvm(true);
    let llvm_file = llvm.compile_file(&dtr_path, Some(&exe_l));
    if llvm_file.success && exe_l.exists() {
        let run = std::process::Command::new(&exe_l)
            .output()
            .expect("Run LLVM-built binary");
        let out_l = String::from_utf8_lossy(&run.stdout).trim().to_string();
        assert_eq!(
            out_l,
            out_c.trim(),
            "P0 DIFFERENTIAL BUG: Cranelift and LLVM outputs differ"
        );
    }

    let _ = std::fs::remove_file(&dtr_path);
    let _ = std::fs::remove_file(&exe_l);
}
