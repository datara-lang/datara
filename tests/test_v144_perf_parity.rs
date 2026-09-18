use std::process::Command;

#[test]
fn test_benchmarks_parity() {
    let benchmarks = ["vec_mul", "sum_reduce", "matmul_96", "vec_add", "vec_axpy"];
    for name in &benchmarks {
        let dtr_exe = format!("benchmarks/{}{}", name, std::env::consts::EXE_SUFFIX);
        let rust_exe = format!("benchmarks/reference/{}_rust{}", name, std::env::consts::EXE_SUFFIX);

        if std::path::Path::new(&dtr_exe).exists() && std::path::Path::new(&rust_exe).exists() {
            let out_dtr = Command::new(&dtr_exe).output().expect("run dtr");
            let out_rust = Command::new(&rust_exe).output().expect("run rust");
            assert!(out_dtr.status.success());
            assert!(out_rust.status.success());

            let s_dtr = String::from_utf8_lossy(&out_dtr.stdout);
            let s_rust = String::from_utf8_lossy(&out_rust.stdout);
            println!("BENCH {}: dtr=[{}] rust=[{}]", name, s_dtr.trim(), s_rust.trim());
        }
    }
}
