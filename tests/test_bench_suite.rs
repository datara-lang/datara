//! v1.3.4: benches/ suite must stay compilable and runnable. The full
//! suite runs via "forgen bench"; this test pins the fast subset so a
//! language change that breaks a benchmark shape fails the gate instead
//! of the next bench run.

use forgen::driver::ForgenCompiler;
use std::path::Path;

const FAST_BENCHES: &[&str] = &[
    "b01_arith_int",
    "b02_arith_f64",
    "b05_branch_mix",
    "b06_fn_calls",
    "b07_class_fields",
    "b10_loop_dep_chain",
    "m01_matmul_96",
    "m02_primes_100k",
    "m03_string_parse",
];

#[test]
fn bench_suite_fast_subset_compiles_and_runs() {
    let compiler = ForgenCompiler::new("release");
    for name in FAST_BENCHES {
        let path = Path::new("benches").join(format!("{name}.dtr"));
        let res = compiler.compile_file(&path, None);
        assert!(
            res.success,
            "bench {name} failed to compile: {:?}",
            res.diagnostics
        );
        let (_, _, code, _) = compiler
            .codegen
            .run_executable(&res.exe_path.unwrap(), &[])
            .unwrap_or_else(|e| panic!("bench {name} failed to launch: {e}"));
        assert_eq!(code, 0, "bench {name} exited with {code}");
    }
}
