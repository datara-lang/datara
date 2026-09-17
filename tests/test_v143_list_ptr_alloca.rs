use forgen::driver::ForgenCompiler;

#[test]
fn test_v143_list_alloca_emits_ptr_type() {
    let source = r#"
fn compute_sum(n: Int) -> Float {
    mut a: List<Float> = []
    mut i = 0
    while i < n {
        a.push(1.5)
        i = i + 1
    }
    let idx = 0
    return a[idx]
}

fn main() {
    let res = compute_sum(10)
    out res
}
"#;
    let compiler = ForgenCompiler::new("release").with_llvm(true);
    let res = compiler.compile_source(source, "test_list_alloca.dtr", None);
    assert!(
        res.success,
        "Compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let llvm = res
        .llvm_source
        .expect("LLVM IR source must be generated when compiling with --llvm");

    let mut found_alloca_ptr = false;
    let mut found_invalid_alloca_i64 = false;
    for line in llvm.lines() {
        if line.contains("%var_a = alloca ptr") {
            found_alloca_ptr = true;
        }
        if line.contains("%var_a = alloca i64") {
            found_invalid_alloca_i64 = true;
        }
    }
    assert!(
        found_alloca_ptr,
        "Expected %var_a to be alloca ptr, align 8"
    );
    assert!(
        !found_invalid_alloca_i64,
        "Found invalid %var_a = alloca i64"
    );
}
