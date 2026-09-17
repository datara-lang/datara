use std::process::Command;

#[test]
fn test_v143_benchmarks_executable() {
    let benchmarks = ["matmul_96", "vec_add", "sum_reduce", "vec_axpy", "vec_mul"];
    for name in &benchmarks {
        let exe_path = format!("benchmarks/{}.exe", name);
        if std::path::Path::new(&exe_path).exists() {
            let output = Command::new(&exe_path)
                .output()
                .unwrap_or_else(|e| panic!("Failed to run {}: {}", exe_path, e));
            assert!(
                output.status.success(),
                "Benchmark {} returned non-zero exit code: {:?}",
                name,
                output.status
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("DATARA_IN_PROCESS_US:"),
                "Benchmark {} missing timing in output: {}",
                name,
                stdout
            );
        }
    }
}
