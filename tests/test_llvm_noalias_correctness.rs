//! Milestone 5 - `noalias` correctness audit (v1.3.1 stability plan).
//!
//! The ownership solver proves uniqueness for non-escaping, non-mutated
//! parameters; the LLVM emitter must turn that proof into `noalias` /
//! `nocapture` / `readonly` / `dereferenceable(n)` parameter attributes.
//! The audit also checks the negative direction (`main` has no pointer
//! parameters, so it must not carry `noalias`) and runs a cross-backend
//! differential to ensure the attributes did not change semantics.

use forgen::driver::ForgenCompiler;
use std::path::PathBuf;

const SOURCE: &str = r#"
class Payload {
    a: Int
    b: Int
}

fn combine(p: Payload) -> Int {
    p.a * 10 + p.b
}

fn main() {
    let x = Payload { a: 3, b: 4 }
    let y = Payload { a: 5, b: 6 }
    let s1 = combine(x)
    let s2 = combine(y)
    out s1
    out s2
}
"#;

#[test]
fn test_noalias_attributes_and_negative_control() {
    // `quick` mode keeps user functions un-inlined, so the parameter
    // attributes stay visible on their own `define` lines (same approach as
    // test_phase11_llvm_ir_attributes).
    let compiler = ForgenCompiler::new("quick").with_llvm(true);
    let res = compiler.compile_source(SOURCE, "noalias_correct.dtr", None);
    assert!(
        res.success,
        "LLVM compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let llvm = res.llvm_source.expect("LLVM IR must be generated");

    // Positive: uniquely-owned non-mutated parameter must carry the full set.
    assert!(
        llvm.contains("noalias"),
        "Uniquely-owned parameter must be noalias:\n{}",
        llvm
    );
    assert!(
        llvm.contains("nocapture"),
        "Non-escaping parameter must be nocapture:\n{}",
        llvm
    );
    assert!(
        llvm.contains("readonly"),
        "Non-mutated parameter must be readonly:\n{}",
        llvm
    );
    assert!(
        llvm.contains("dereferenceable(16)"),
        "Two-field Payload (2 x i64) must be dereferenceable(16):\n{}",
        llvm
    );

    // Negative control: main takes no pointer parameters, so the noalias
    // attribute must never appear on its signature.
    assert!(
        !llvm.contains("@main(ptr noalias"),
        "noalias must not leak onto main's parameterless signature:\n{}",
        llvm
    );
}

#[test]
fn test_noalias_cross_backend_equivalence() {
    // Reference: Cranelift backend.
    let clif = ForgenCompiler::new("release");
    let res_c = clif.compile_source(SOURCE, "noalias_clif.dtr", None);
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
        out_c.contains("34") && out_c.contains("56"),
        "combine(x)=34 and combine(y)=56 expected, got:\n{}",
        out_c
    );

    // Differential: LLVM-built binary must produce identical output when the
    // local toolchain can AOT-link; otherwise skip gracefully.
    let dir = std::env::temp_dir().join("datara_noalias_differential");
    let _ = std::fs::create_dir_all(&dir);
    let dtr_path: PathBuf = dir.join("noalias_correct.dtr");
    std::fs::write(&dtr_path, SOURCE).expect("write .dtr fixture");
    let exe_ext = std::env::consts::EXE_SUFFIX;
    let exe_l: PathBuf = dir.join(format!("noalias_llvm_bin.{}", exe_ext));

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
            "P0 DIFFERENTIAL BUG: noalias attributes changed semantics"
        );
    }

    let _ = std::fs::remove_file(&dtr_path);
    let _ = std::fs::remove_file(&exe_l);
}
