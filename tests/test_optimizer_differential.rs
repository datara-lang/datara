use forgen::driver::ForgenCompiler;

#[test]
fn test_optimizer_differential_semantic_equivalence() {
    let test_cases = [
        (
            "Constant Folding & CSE",
            r#"
fn compute() -> Int {
    mut a = 0
    a = 100 * 2 + 50
    mut b = 0
    b = 100 * 2 + 50
    mut c = 0
    c = a + b
    return c
}
fn main() {
    out compute()
}
"#,
            "500",
        ),
        (
            "LICM Invariant Loop",
            r#"
fn loop_calc(n: Int) -> Int {
    mut sum = 0
    mut i = 0
    while i < n {
        mut c = 0
        c = 10 * 5
        sum = sum + c
        i = i + 1
    }
    return sum
}
fn main() {
    out loop_calc(20)
}
"#,
            "1000",
        ),
        (
            "SROA Struct Stack Scalarization",
            r#"
class Point {
    x: Int
    y: Int
}
fn process_points() -> Int {
    mut p1 = Point { x: 10, y: 20 }
    mut p2 = Point { x: 30, y: 40 }
    return p1.x + p1.y + p2.x + p2.y
}
fn main() {
    out process_points()
}
"#,
            "100",
        ),
        (
            "Inlining Pure Leaf Functions",
            r#"
fn multiply(a: Int, b: Int) -> Int => a * b
fn add(a: Int, b: Int) -> Int => a + b

fn main() {
    mut x = 0
    x = multiply(5, 6)
    mut y = 0
    y = add(x, 10)
    out y
}
"#,
            "40",
        ),
        (
            "Generic Box Monomorphization",
            r#"
class Box<T> {
    item: T
}
fn unwrap_box(b: Box<Int>) -> Int => b.item

fn main() {
    mut b = Box<Int> { item: 777 }
    out unwrap_box(b)
}
"#,
            "777",
        ),
        (
            "Polyhedral Loop Interchange",
            r#"
fn triangle_sum(n: Int) -> Int {
    mut total = 0
    mut i = 0
    while i < n {
        mut j = 0
        while j < n {
            total = total + i + j
            j = j + 1
        }
        i = i + 1
    }
    return total
}
fn main() {
    out triangle_sum(50)
}
"#,
            // sum over i,j in [0,50) of (i+j) = 50 * 2 * (49*50/2) = 122500
            "122500",
        ),
        (
            "Invariant Branch Unswitch",
            r#"
fn unswitch(n: Int) -> Int {
    mut sum = 0
    mut i = 0
    while i < n {
        if 5 > 1 {
            sum = sum + i
        }
        i = i + 1
    }
    return sum
}
fn main() {
    out unswitch(10)
}
"#,
            // sum of 0..10 guarded by a loop-invariant true branch
            "45",
        ),
        (
            "Proven Non-Zero Division",
            r#"
fn safe_div(a: Int, b: Int) -> Int {
    require b != 0
    return a / b
}
fn main() {
    out safe_div(1000, 4)
}
"#,
            // divisor statically proven non-zero via `require`
            "250",
        ),
    ];

    let start_compiler = ForgenCompiler::new("start"); // unoptimized
    let release_compiler = ForgenCompiler::new("release"); // optimized release
    let domain_compiler = ForgenCompiler::new("domain"); // domain specialized

    for (name, source, expected_stdout) in test_cases {
        // 1. Unoptimized build & run
        let res_unopt = start_compiler.compile_source(source, &format!("{}_unopt.dtr", name), None);
        assert!(
            res_unopt.success,
            "[{}] Unoptimized compilation failed: {:?}",
            name, res_unopt.error
        );
        let exe_unopt = res_unopt.exe_path.unwrap();
        let (out_unopt, err_unopt, code_unopt, _) = start_compiler
            .codegen
            .run_executable(&exe_unopt, &[])
            .unwrap();

        // 2. Optimized Release build & run
        let res_rel = release_compiler.compile_source(source, &format!("{}_rel.dtr", name), None);
        assert!(
            res_rel.success,
            "[{}] Release compilation failed: {:?}",
            name, res_rel.error
        );
        let exe_rel = res_rel.exe_path.unwrap();
        let (out_rel, err_rel, code_rel, _) = release_compiler
            .codegen
            .run_executable(&exe_rel, &[])
            .unwrap();

        // 3. Domain specialized build & run
        let res_dom = domain_compiler.compile_source(source, &format!("{}_dom.dtr", name), None);
        assert!(
            res_dom.success,
            "[{}] Domain compilation failed: {:?}",
            name, res_dom.error
        );
        let exe_dom = res_dom.exe_path.unwrap();
        let (out_dom, err_dom, code_dom, _) = domain_compiler
            .codegen
            .run_executable(&exe_dom, &[])
            .unwrap();

        // 4. Assert Exact Semantic Equivalence Across All Modes
        assert_eq!(code_unopt, 0, "[{}] Unoptimized non-zero exit code", name);
        assert_eq!(code_rel, 0, "[{}] Release non-zero exit code", name);
        assert_eq!(code_dom, 0, "[{}] Domain non-zero exit code", name);

        assert_eq!(
            out_unopt.trim(),
            expected_stdout.trim(),
            "[{}] Unoptimized output mismatch",
            name
        );
        assert_eq!(
            out_rel.trim(),
            expected_stdout.trim(),
            "[{}] Release output mismatch",
            name
        );
        assert_eq!(
            out_dom.trim(),
            expected_stdout.trim(),
            "[{}] Domain output mismatch",
            name
        );

        assert_eq!(
            out_unopt, out_rel,
            "[{}] Semantic difference between Unoptimized and Release!",
            name
        );
        assert_eq!(
            out_rel, out_dom,
            "[{}] Semantic difference between Release and Domain!",
            name
        );

        assert!(
            err_unopt.is_empty(),
            "[{}] Unoptimized stderr not empty",
            name
        );
        assert!(err_rel.is_empty(), "[{}] Release stderr not empty", name);
        assert!(err_dom.is_empty(), "[{}] Domain stderr not empty", name);
    }
}
