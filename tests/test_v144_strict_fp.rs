//! v1.4.4: Bit-exact IEEE-754 floating-point parity (--strict-fp) tests.

use forgen::driver::ForgenCompiler;
use std::sync::Mutex;

static STRICT_FP_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_v144_strict_fp_parity_flag_sets_env_and_compiles() {
    let _guard = STRICT_FP_MUTEX.lock().unwrap();
    let source = r#"
fn compute_pi_approximation() -> Float {
    let a: Float = 3.141592653589793
    let b: Float = 2.718281828459045
    let c: Float = 1.414213562373095
    return (a * b) + c
}

fn main() {
    let r = compute_pi_approximation()
    println(float_to_str(r))
}
"#;

    let compiler = ForgenCompiler::new("release").with_strict_fp(true);
    assert!(compiler.strict_fp, "strict_fp field must be set to true");
    assert_eq!(std::env::var("DATARA_STRICT_FP").as_deref(), Ok("1"));

    let res = compiler.check_source(source, "strict_fp_test.dtr");
    assert!(
        res.success,
        "Source must compile cleanly under --strict-fp: {:?}",
        res.diagnostics
    );
}
