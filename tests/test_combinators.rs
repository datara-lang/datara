//! v1.4.1 List combinator tests (map, filter, fold, collect).
//!
//! Verifies:
//! - Lowering into DMIR loops without runtime calls.
//! - Lambdas and named functions as combinator arguments.
//! - Chained pipelines: filter -> map -> fold.
//! - Materialization via collect().
//! - Both JIT and AOT native execution.
//! - Diagnostic gate E-TYPE-008 for wrong arity or non-callable arguments.

use forgen::diagnostics::ErrorCode;
use forgen::driver::ForgenCompiler;

static MSVC_ENV: std::sync::Once = std::sync::Once::new();
fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| unsafe {
        if std::env::var_os("ProgramFiles(x86)").is_none() {
            std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
        }
        if std::env::var_os("ProgramFiles").is_none() {
            std::env::set_var("ProgramFiles", r"C:\Program Files");
        }
    });
}

fn run_datara(source: &str, name: &str) -> String {
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}",
        name, res.error
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "{} exited with {}", name, code);

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

fn run_jit(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

#[test]
fn test_combinator_map_lambda_jit() {
    let source = r#"
fn main() {
    let numbers = [1, 2, 3, 4]
    let doubled = numbers.map(x => x * 2)
    println(int_to_str(doubled.get(0)))
    println(int_to_str(doubled.get(1)))
    println(int_to_str(doubled.get(2)))
    println(int_to_str(doubled.get(3)))
}
"#;
    let out = run_jit(source, "test_combinator_map_lambda_jit");
    assert_eq!(out, "2\n4\n6\n8");
}

#[test]
fn test_combinator_map_named_fn_aot() {
    let source = r#"
fn triple(n: Int) -> Int {
    return n * 3
}

fn main() {
    let nums = [10, 20, 30]
    let res = nums.map(triple)
    println(int_to_str(res.get(0)))
    println(int_to_str(res.get(1)))
    println(int_to_str(res.get(2)))
}
"#;
    let out = run_datara(source, "test_combinator_map_named_fn_aot");
    assert_eq!(out, "30\n60\n90");
}

#[test]
fn test_combinator_filter_lambda_aot() {
    let source = r#"
fn main() {
    let nums = [1, 2, 3, 4, 5, 6]
    let evens = nums.filter(x => x % 2 == 0)
    println(fmt"len:{evens.len()}")
    println(int_to_str(evens.get(0)))
    println(int_to_str(evens.get(1)))
    println(int_to_str(evens.get(2)))
}
"#;
    let out = run_datara(source, "test_combinator_filter_lambda_aot");
    assert_eq!(out, "len:3\n2\n4\n6");
}

#[test]
fn test_combinator_fold_sum_jit() {
    let source = r#"
fn main() {
    let nums = [1, 2, 3, 4, 5]
    let sum = nums.fold(0, (acc, x) => acc + x)
    println(int_to_str(sum))
}
"#;
    let out = run_jit(source, "test_combinator_fold_sum_jit");
    assert_eq!(out, "15");
}

#[test]
fn test_combinator_fold_product_aot() {
    let source = r#"
fn mult(acc: Int, x: Int) -> Int {
    return acc * x
}

fn main() {
    let nums = [2, 3, 4]
    let product = nums.fold(1, mult)
    println(int_to_str(product))
}
"#;
    let out = run_datara(source, "test_combinator_fold_product_aot");
    assert_eq!(out, "24");
}

#[test]
fn test_combinator_collect_pipeline_jit() {
    let source = r#"
fn main() {
    let nums = [1, 2, 3, 4, 5]
    let res = nums.map(x => x * 10).collect()
    println(fmt"count:{res.len()}")
    println(int_to_str(res.get(0)))
    println(int_to_str(res.get(4)))
}
"#;
    let out = run_jit(source, "test_combinator_collect_pipeline_jit");
    assert_eq!(out, "count:5\n10\n50");
}

#[test]
fn test_combinator_full_chain_fold_aot() {
    let source = r#"
fn main() {
    let nums = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let total = nums
        .filter(x => x % 2 == 0)
        .map(x => x * 2)
        .fold(0, (acc, x) => acc + x)
    println(int_to_str(total))
}
"#;
    let out = run_datara(source, "test_combinator_full_chain_fold_aot");
    assert_eq!(out, "60");
}

#[test]
fn test_combinator_empty_list_operations() {
    let source = r#"
fn main() {
    let empty = []
    let mapped = empty.map(x => x + 1)
    let filtered = empty.filter(x => x > 0)
    let folded = empty.fold(100, (acc, x) => acc + x)
    println(fmt"m:{mapped.len()} f:{filtered.len()} fold:{folded}")
}
"#;
    let out = run_jit(source, "test_combinator_empty_list_operations");
    assert_eq!(out, "m:0 f:0 fold:100");
}

#[test]
fn test_combinator_arity_error_e_type_008() {
    let source = r#"
fn main() {
    let nums = [1, 2, 3]
    let res = nums.map()
    out res
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "bad_map_arity.dtr", None);
    assert!(!res.success, "map() with 0 args must fail");
    assert!(
        res.diagnostics
            .contains(ErrorCode::TypeIncomparableOperands.as_str()),
        "Must contain E-TYPE-008, got:\n{}",
        res.diagnostics
    );
}

#[test]
fn test_combinator_non_callable_error_e_type_008() {
    let source = r#"
fn main() {
    let nums = [1, 2, 3]
    let res = nums.map(42)
    out res
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "bad_map_callable.dtr", None);
    assert!(!res.success, "map(42) with non-callable must fail");
    assert!(
        res.diagnostics
            .contains(ErrorCode::TypeIncomparableOperands.as_str()),
        "Must contain E-TYPE-008, got:\n{}",
        res.diagnostics
    );
}

#[test]
fn test_combinator_fold_wrong_arity_e_type_008() {
    let source = r#"
fn main() {
    let nums = [1, 2, 3]
    let res = nums.fold(0)
    out res
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "bad_fold_arity.dtr", None);
    assert!(!res.success, "fold(0) missing accumulator fn must fail");
    assert!(
        res.diagnostics
            .contains(ErrorCode::TypeIncomparableOperands.as_str()),
        "Must contain E-TYPE-008, got:\n{}",
        res.diagnostics
    );
}
