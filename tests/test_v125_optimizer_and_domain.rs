use forgen::driver::ForgenCompiler;
use std::fs;

fn run_datara(code: &str, tag: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(code, tag, None);
    assert!(
        res.success,
        "Compilation failed for {}: error={:?} diag=\n{}",
        tag, res.error, res.diagnostics
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "{} exited with {}", tag, code);

    let _ = fs::remove_file(&exe);
    let _ = fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

#[test]
fn test_v125_stdlib_optimizer_recurrence() {
    let code = r#"
use optimizer.optimize

fn main() {
    let solver = RecurrenceSolver { a: 1, b: 1 }
    let f10 = solver.solve(10)
    println(int_to_str(f10))
}
"#;
    let out = run_datara(code, "test_v125_stdlib_optimizer_recurrence");
    // F(10) for Fibonacci sequence: 0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
    // with F(1)=1, F(2)=1, F(3)=2, F(4)=3, F(5)=5, F(6)=8, F(7)=13, F(8)=21, F(9)=34, F(10)=55
    assert_eq!(out, "55");
}

#[test]
fn test_v125_stdlib_optimizer_loop_reducer() {
    let code = r#"
use optimizer.optimize

fn main() {
    let term = LoopTerm {
        variable: "i",
        lower_bound: 0,
        upper_bound: 10,
        step: 1,
        degree: 1
    }
    let reducer = SymbolicLoopReducer { term: term }
    let res = reducer.solve_closed_form()
    if res.is_success {
        println(int_to_str(res.value))
    } else {
        println(res.error_msg)
    }
}
"#;
    let out = run_datara(code, "test_v125_stdlib_optimizer_loop_reducer");
    // sum(i, 0..10) = 10 * 9 / 2 = 45
    assert_eq!(out, "45");
}

#[test]
fn test_v125_domain_mode_compilation() {
    let compiler = ForgenCompiler::new("domain").with_native(true);
    let code = r#"
fn compute_sum(n: Int) -> Int {
    mut i = 0
    mut acc = 0
    while i < n {
        acc = acc + i * i * i
        i = i + 1
    }
    return acc
}

fn main() {
    let res = compute_sum(10)
    println(int_to_str(res))
}
"#;
    let res = compiler.compile_source_native(code, "test_v125_domain_mode", None);
    assert!(
        res.success,
        "Domain mode compilation failed: {:?}",
        res.error
    );
    let exe = res.exe_path.expect("must produce .exe");
    let (stdout, _, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must execute");
    assert_eq!(code, 0);
    // sum(i^3, 0..10) = (10 * 9 / 2)^2 = 45^2 = 2025
    assert_eq!(stdout.trim(), "2025");
    let _ = fs::remove_file(&exe);
    let _ = fs::remove_file(exe.with_extension("obj"));
}

#[test]
fn test_v125_error_propagation_cold_block() {
    let code = r#"
use stdlib.result.result.Outcome

fn try_divide(a: Int, b: Int) -> Outcome<Int> {
    if b == 0 {
        return Outcome<Int> { is_success: false, value: 0, error_msg: "division by zero" }
    }
    return Outcome<Int> { is_success: true, value: a / b, error_msg: "" }
}

fn safe_calc(a: Int, b: Int) -> Outcome<Int> {
    let q = try_divide(a, b)?
    return Outcome<Int> { is_success: true, value: q * 2, error_msg: "" }
}

fn main() {
    let ok_res = safe_calc(10, 2)
    if ok_res.is_success {
        println(fmt"OK:{ok_res.value}")
    } else {
        println(fmt"ERR:{ok_res.error_msg}")
    }

    let err_res = safe_calc(10, 0)
    if err_res.is_success {
        println(fmt"OK:{err_res.value}")
    } else {
        println(fmt"ERR:{err_res.error_msg}")
    }
}
"#;
    let out = run_datara(code, "test_v125_error_propagation_cold_block");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "OK:10");
    assert_eq!(lines[1], "ERR:division by zero");
}
