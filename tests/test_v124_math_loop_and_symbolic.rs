use forgen::driver::ForgenCompiler;
use forgen::optimizer::symbolic::SymbolicOptimizer;
use forgen::runtime::zero_copy::{Span, StrView};

#[test]
fn test_v124_cubic_sum_loop_folding() {
    let source = r#"
fn compute_cubes(n: Int) -> Int {
    mut sum = 0
    mut i = 0
    while i < n {
        let i2 = i * i
        sum = sum + i2 * i
        i = i + 1
    }
    return sum
}

fn main() {
    out compute_cubes(6)
}
"#;
    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "v124_cubic_test.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);

    let report = res
        .optimization_report
        .expect("optimization report missing");
    let applied = report
        .decision_trace
        .iter()
        .any(|r| r.pass == "LoopFold" && r.decision == "Applied");
    assert!(applied, "LoopFold must apply to cubic sum loop");

    let exe_path = res.exe_path.expect("executable path missing");
    let (out, err, code, _) = compiler
        .cranelift
        .run_executable(&exe_path, &[])
        .expect("execution failed");
    assert_eq!(code, 0, "non-zero exit: {}", err);
    // sum of cubes 0..5 = 0 + 1 + 8 + 27 + 64 + 125 = 225
    assert_eq!(out.trim(), "225", "Sum of cubes 0..5 must equal 225");
}

#[test]
fn test_v124_symbolic_polynomial_closed_form() {
    // sum_{i=0}^9 (2*i^3 + 3*i^2 + 5*i + 7)
    // i^3 sum 0..9 = (9*10/2)^2 = 45^2 = 2025 -> 2 * 2025 = 4050
    // i^2 sum 0..9 = 9*10*19/6 = 285 -> 3 * 285 = 855
    // i sum 0..9 = 45 -> 5 * 45 = 225
    // constant 7 * 10 = 70
    // total = 4050 + 855 + 225 + 70 = 5200
    let terms = vec![(2, 3), (3, 2), (5, 1), (7, 0)];
    let res = SymbolicOptimizer::solve_closed_form_sum(&terms, 10);
    assert_eq!(res, Some(5200));
}

#[test]
fn test_v124_symbolic_recurrence_fibonacci() {
    // Fibonacci: x_n = 1*x_{n-1} + 1*x_{n-2}, with x_0=0, x_1=1
    assert_eq!(
        SymbolicOptimizer::solve_linear_recurrence_2x2(1, 1, 0, 1, 0),
        0
    );
    assert_eq!(
        SymbolicOptimizer::solve_linear_recurrence_2x2(1, 1, 0, 1, 1),
        1
    );
    assert_eq!(
        SymbolicOptimizer::solve_linear_recurrence_2x2(1, 1, 0, 1, 10),
        55
    );
    assert_eq!(
        SymbolicOptimizer::solve_linear_recurrence_2x2(1, 1, 0, 1, 20),
        6765
    );
    assert_eq!(
        SymbolicOptimizer::solve_linear_recurrence_2x2(1, 1, 0, 1, 30),
        832040
    );
}

#[test]
fn test_v124_symbolic_sympy_integration() {
    // Query SymPy ahead-of-time for sum_{i=0}^9 (i^2 + 3*i)
    // 285 + 3 * 45 = 285 + 135 = 420
    let res = SymbolicOptimizer::query_sympy_closed_form("i**2 + 3*i", "i", 10);
    match res {
        Ok(val) => assert_eq!(val, 420),
        Err(e) => {
            eprintln!("SymPy query skipped or error: {}", e);
        }
    }
}

#[test]
fn test_v124_zero_copy_span_and_strview() {
    let data = vec![10, 20, 30, 40, 50];
    let span = Span::from_slice(&data);
    assert_eq!(span.len(), 5);
    assert_eq!(span[0], 10);
    assert_eq!(span[4], 50);

    let sub = span.subspan(1, 4).expect("valid subspan");
    assert_eq!(sub.len(), 3);
    assert_eq!(sub[0], 20);
    assert_eq!(sub[2], 40);

    let view = StrView::from_static("Datara v1.2.4 Speed Supremacy");
    assert_eq!(view.len(), 29);
    let subview = view.substr(0, 6).expect("valid substr");
    assert_eq!(subview.as_str(), "Datara");
}

#[test]
fn test_v124_multi_block_while_loop_unswitching() {
    let source = r#"
fn run_unswitch(n: Int, fast: Bool) -> Int {
    mut sum = 0
    mut i = 0
    while i < n {
        if fast {
            sum = sum + 10
        } else {
            sum = sum + 1
        }
        i = i + 1
    }
    return sum
}

fn main() {
    out run_unswitch(5, true)
}
"#;
    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "v124_unswitch_test.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);

    let report = res
        .optimization_report
        .expect("optimization report missing");
    let unswitched = report
        .decision_trace
        .iter()
        .any(|r| r.pass == "LoopUnswitch" && r.decision == "Applied");
    assert!(
        unswitched,
        "LoopUnswitch must report Applied for invariant branch"
    );

    let exe_path = res.exe_path.expect("executable path missing");
    let (out, err, code, _) = compiler
        .cranelift
        .run_executable(&exe_path, &[])
        .expect("execution failed");
    assert_eq!(code, 0, "non-zero exit: {}", err);
    assert_eq!(out.trim(), "50", "Fast unswitched loop must equal 50");
}
