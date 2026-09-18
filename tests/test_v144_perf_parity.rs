use std::process::Command;

#[test]
fn test_benchmarks_parity() {
    let benchmarks = ["vec_mul", "sum_reduce", "matmul_96", "vec_add", "vec_axpy"];
    for name in &benchmarks {
        let dtr_exe = format!("benchmarks/{}{}", name, std::env::consts::EXE_SUFFIX);
        let rust_exe = format!(
            "benchmarks/reference/{}_rust{}",
            name,
            std::env::consts::EXE_SUFFIX
        );

        if std::path::Path::new(&dtr_exe).exists() && std::path::Path::new(&rust_exe).exists() {
            let out_dtr = Command::new(&dtr_exe).output().expect("run dtr");
            let out_rust = Command::new(&rust_exe).output().expect("run rust");
            assert!(out_dtr.status.success());
            assert!(out_rust.status.success());

            let s_dtr = String::from_utf8_lossy(&out_dtr.stdout);
            let s_rust = String::from_utf8_lossy(&out_rust.stdout);
            println!(
                "BENCH {}: dtr=[{}] rust=[{}]",
                name,
                s_dtr.trim(),
                s_rust.trim()
            );
        }
    }
}

#[test]
fn test_bce_on_benchmarks() {
    use forgen::driver::ForgenCompiler;
    println!("FIND_CLANG: {:?}", forgen::codegen::linker::find_clang());
    println!("FIND_LLC: {:?}", forgen::codegen::linker::find_llc());
    let source = std::fs::read_to_string("benchmarks/matmul_96.dtr").expect("read matmul_96");
    let compiler = ForgenCompiler::new("domain").with_llvm(true);
    let res = compiler.compile_source(&source, "matmul_96.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);
    if let Some(llvm) = &res.llvm_source {
        std::fs::write("test_matmul_96.ll", llvm).unwrap();
    }

    let source_mul = std::fs::read_to_string("benchmarks/vec_mul.dtr").expect("read vec_mul");
    let res_mul = compiler.compile_source(&source_mul, "vec_mul.dtr", None);
    assert!(res_mul.success, "Compilation failed: {:?}", res_mul.error);
    if let Some(llvm) = &res_mul.llvm_source {
        std::fs::write("test_vec_mul.ll", llvm).unwrap();
    }

    let report = res.optimization_report.expect("report");
    println!("AXPY BCE PROVEN: {}", report.bce_proven);
    for tr in &report.decision_trace {
        if tr.pass.contains("BCE") {
            println!("  TRACE: {:?} -> {:?}", tr.candidate, tr.reason);
        }
    }
    if let Some(llvm) = &res.llvm_source {
        let mut in_fn = false;
        for line in llvm.lines() {
            if line.contains("@compute_axpy__spec_n_100000") {
                in_fn = true;
            }
            if in_fn {
                println!("{}", line);
                if line == "}" {
                    break;
                }
            }
        }
    }
}
