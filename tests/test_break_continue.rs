//! End-to-end tests for `break` and `continue` (v1.3.2).
//!
//! README_RU promises loop early-exit control flow ("Бесконечный цикл loop
//! с break и continue"). `break` leaves the innermost loop; `continue` jumps
//! to the next iteration — for counted `for` loops it targets the increment
//! block, so the induction variable still advances. `break` / `continue`
//! outside any loop body are a compile-time error (E0312).

use forgen::driver::ForgenCompiler;

fn compile_and_run(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(res.success, "compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "program must exit cleanly, stderr: {}", stderr);
    stdout
}

fn expect_compile_error(source: &str, name: &str) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        !res.success,
        "compilation must fail, but succeeded; diagnostics: {}",
        res.diagnostics
    );
}

#[test]
fn test_while_break_by_condition() {
    // A `while true` loop that only leaves through `break`.
    let src = r#"
fn main() {
    mut i = 0
    while true {
        i = i + 1
        if i >= 5 {
            break
        }
    }
    out i
}
"#;
    let stdout = compile_and_run(src, "test_break_while.dtr");
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn test_continue_skips_iteration() {
    // Odd numbers only: 1 + 3 + 5 + 7 + 9 = 25. `continue` must re-test the
    // while condition, not skip the rest of the loop forever.
    let src = r#"
fn main() {
    mut total = 0
    mut k = 0
    while k < 10 {
        k = k + 1
        if k % 2 == 0 {
            continue
        }
        total = total + k
    }
    out total
}
"#;
    let stdout = compile_and_run(src, "test_continue_while.dtr");
    assert_eq!(stdout.trim(), "25");
}

#[test]
fn test_continue_advances_for_induction_variable() {
    // `continue` in a counted for loop must still run `i = i + 1`;
    // sum of 0..10 excluding 3 is 42. If the increment were skipped the
    // loop would spin forever on i == 3.
    let src = r#"
fn main() {
    mut c = 0
    for i in 0..10 {
        if i == 3 {
            continue
        }
        c = c + i
    }
    out c
}
"#;
    let stdout = compile_and_run(src, "test_continue_for.dtr");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn test_nested_break_only_inner_loop() {
    // The inner break leaves only the inner loop: for each a in 0..5 the
    // inner loop counts b = 0, 1, 2 (b == 3 breaks), so 5 * 3 = 15.
    let src = r#"
fn main() {
    mut sum = 0
    for a in 0..5 {
        for b in 0..5 {
            if b == 3 {
                break
            }
            sum = sum + 1
        }
    }
    out sum
}
"#;
    let stdout = compile_and_run(src, "test_break_nested.dtr");
    assert_eq!(stdout.trim(), "15");
}

#[test]
fn test_break_in_list_for() {
    let src = r#"
fn main() {
    let xs = [10, 20, 30, 40]
    mut total = 0
    for x in xs {
        if x == 30 {
            continue
        }
        if total > 50 {
            break
        }
        total = total + x
    }
    out total
}
"#;
    let stdout = compile_and_run(src, "test_break_list_for.dtr");
    assert_eq!(stdout.trim(), "70");
}

#[test]
fn test_loop_with_break_terminates() {
    let src = r#"
fn main() {
    mut n = 0
    loop {
        n = n + 1
        if n == 7 {
            break
        }
    }
    out n
}
"#;
    let stdout = compile_and_run(src, "test_break_loop.dtr");
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn test_return_still_works_inside_for() {
    // A `return` inside a loop body must not fall through to the increment
    // block (the body does not reach the back edge).
    let src = r#"
fn find_it() -> Int {
    let xs = [1, 2, 3]
    for x in xs {
        if x == 2 {
            return x
        }
    }
    return 0
}
fn main() {
    out find_it()
}
"#;
    let stdout = compile_and_run(src, "test_return_in_for.dtr");
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn test_break_outside_loop_rejected() {
    expect_compile_error("fn main() {\n    break\n}\n", "test_break_outside.dtr");
    expect_compile_error(
        "fn main() {\n    if true {\n        continue\n    }\n}\n",
        "test_continue_outside.dtr",
    );
    // Inside `parallel`, break does not have a loop to target either.
    expect_compile_error(
        "fn main() {\n    parallel {\n        break\n    }\n}\n",
        "test_break_parallel.dtr",
    );
}

#[test]
fn test_dead_code_after_break_is_not_executed() {
    // Statements after `break` in the same block must never run.
    // x == 1 increments; x == 2 breaks before `hits` reaches 100;
    // if the dead assignment ran, the result would jump to 101+.
    let src = r#"
fn main() {
    let xs = [1, 2, 3]
    mut hits = 0
    for x in xs {
        if x == 2 {
            break
            hits = hits + 100
        }
        hits = hits + 1
    }
    out hits
}
"#;
    let stdout = compile_and_run(src, "test_dead_after_break.dtr");
    assert_eq!(stdout.trim(), "1");
}
