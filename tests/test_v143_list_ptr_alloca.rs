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

#[test]
fn test_simd_dot_llvm_ir() {
    let source = r#"
fn main() {
    let a = float4(1.0, 2.0, 3.0, 4.0)
    let b = float4(4.0, 3.0, 2.0, 1.0)
    out(dot(a, b))
}
"#;
    let compiler = ForgenCompiler::new("release").with_llvm(true);
    let res = compiler.compile_source(source, "simd_app.dtr", None);
    let llvm = res.llvm_source.expect("LLVM IR source must be generated");
    assert!(
        llvm.contains("%var_a = alloca <4 x float>, align 16"),
        "Expected %var_a to be alloca <4 x float>, align 16, got:\n{}",
        llvm
    );
    assert!(
        llvm.contains("%var_b = alloca <4 x float>, align 16"),
        "Expected %var_b to be alloca <4 x float>, align 16, got:\n{}",
        llvm
    );
}
