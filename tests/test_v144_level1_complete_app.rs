use forgen::driver::ForgenCompiler;

#[test]
fn test_level1_complete_application_pipeline() {
    let source = r#"
struct Transaction {
    id: Int
    category: Str
    amount: Float
}


behavior Transaction {
    fn is_high_value(threshold: Float) -> Bool {
        return this.amount > threshold
    }

    fn to_display() -> Str {
        return fmt"Tx #{this.id}: {this.category} (${this.amount})"
    }
}

fn compute_total(items: List<Float>) -> Float {
    mut total: Float = 0.0
    for amt in items {
        total = total + amt
    }
    return total
}

fn range_counter(limit: Int) -> Int {
    mut sum: Int = 0
    for i in 0..limit {
        if i == 5 {
            continue
        }
        if i >= 8 {
            break
        }
        sum = sum + i
    }
    return sum
}

fn main() {
    // 1. Level 1 ergonomic list and loop
    mut amounts = [10.5, 20.0, 99.9, 150.0]
    let total = compute_total(amounts)
    println(fmt"Total: {total}")

    // 2. Class and method dispatch
    let tx = Transaction {
        id: 101,
        category: "Hardware",
        amount: 250.0
    }
    let is_big = tx.is_high_value(100.0)
    let summary = tx.to_display()
    println(summary)

    // 3. For range with continue and break
    let loop_res = range_counter(10)
    println(fmt"Loop sum: {loop_res}")

    // 4. String interpolation and conditions
    let status = if is_big { "Approved" } else { "Review" }
    println(fmt"Status: {status}")
}
"#;

    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_source(source, "test_level1_app.dtr");
    assert!(
        res.success,
        "Level 1 complete application must typecheck cleanly: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_level1_loop_ergonomics_and_flow() {
    let source = r#"
fn test_while_and_for_parity() -> Int {
    mut while_sum = 0
    mut i = 0
    while i < 10 {
        while_sum = while_sum + i
        i = i + 1
    }

    mut for_sum = 0
    for j in 0..10 {
        for_sum = for_sum + j
    }

    require while_sum == for_sum
    return for_sum
}

fn main() {
    let res = test_while_and_for_parity()
    println(int_to_str(res))
}
"#;

    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_source(source, "test_loops.dtr");
    assert!(
        res.success,
        "Loops and conditions must compile without diagnostic errors: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_level1_execution_produces_correct_output() {
    let source = r#"
fn calculate_pipeline(n: Int) -> Int {
    mut acc: Int = 0
    mut i: Int = 0
    while i < n {
        if i % 2 == 0 {
            acc = acc + (i * 3)
        } else {
            acc = acc + i
        }
        i = i + 1
    }
    return acc
}

fn main() {
    let res = calculate_pipeline(10)
    println(int_to_str(res))
}
"#;

    let compiler = ForgenCompiler::new("run");
    let res = compiler.compile_source(source, "test_exec_pipeline.dtr", None);
    assert!(res.success, "Compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.unwrap();
    let (stdout, stderr, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(code, 0, "Execution failed: {}", stderr);
    assert!(
        stdout.trim().ends_with("85"),
        "Expected 85, got stdout: {}",
        stdout
    );
}
