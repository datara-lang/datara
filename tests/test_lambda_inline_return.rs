//! Regression tests: `return e` inside a block-bodied lambda that is lowered
//! IN PLACE (named lambda, lambda-argument static dispatch, builtin
//! higher-order iteration) must exit the lambda with the value, not return
//! from the enclosing function. The old lowering turned the inline `return`
//! into a function-level `Return` terminator, which silently swallowed the
//! whole rest of the caller's output (PLAN_V150 §5, found 2026-09-30,
//! fixed 2026-10-18).

use forgen::driver::ForgenCompiler;

fn compile_and_run(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(res.success, "compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    (stdout, code)
}

#[test]
fn block_lambda_if_return_named() {
    let src = r#"
fn main() {
    val mx = (a, b) => {
        if b > a {
            return b
        }
        return a
    }
    out mx(3, 5)
    out mx(5, 3)
    out "end"
}
"#;
    let (stdout, code) = compile_and_run(src, "block_lambda_if_return_named");
    assert_eq!(code, 0);
    assert_eq!(
        stdout.trim().lines().collect::<Vec<_>>(),
        vec!["5", "5", "end"],
        "block-bodied lambda with if-return must return through the lambda"
    );
}

#[test]
fn block_lambda_if_return_binds_args() {
    let src = r#"
fn main() {
    val dec = x => {
        if x > 0 {
            return x - 1
        }
        return 0 - x
    }
    out dec(4)
    out dec(0 - 2)
    out "tail"
}
"#;
    let (stdout, code) = compile_and_run(src, "block_lambda_if_return_binds_args");
    assert_eq!(code, 0);
    assert_eq!(
        stdout.trim().lines().collect::<Vec<_>>(),
        vec!["3", "2", "tail"],
        "the not-taken branch must not overwrite the taken value"
    );
}

#[test]
fn block_lambda_unit_return_does_not_kill_caller() {
    // A lambda whose if-branch returns an Int while the tail returns the
    // second arm's value: both paths land in the join block; the caller
    // keeps running afterwards.
    let src = r#"
fn main() {
    val pick = (a, b) => {
        if a > b {
            return a
        }
        return b
    }
    out pick(9) == 9
}
"#;
    // Bool comparison on a lambda result; just assert it compiles and runs.
    let (stdout, code) = compile_and_run(src, "block_lambda_unit_return_does_not_kill_caller");
    assert_eq!(code, 0);
    assert!(
        stdout.contains("true"),
        "caller survived the lambda's inline return: {stdout:?}"
    );
}

#[test]
fn higher_order_builtin_lambda_if_return() {
    // Higher-order builtin iteration (lambda passed to map-like builtin):
    // the body is inlined per element; an `if { return x }` inside must
    // deliver per-item results without returning from main.
    let src = r#"
fn main() {
    let sq = x => {
        if x > 2 {
            return x
        }
        return x * x
    }
    val nums: List<Int> = [1, 2, 3, 4]
    let mapped = nums.map(sq)
    mut i = 0
    while i < mapped.len() {
        out mapped[i]
        i = i + 1
    }
    out "done"
}
"#;
    let (stdout, code) = compile_and_run(src, "higher_order_builtin_lambda_if_return");
    assert_eq!(code, 0);
    assert_eq!(
        stdout.trim().lines().collect::<Vec<_>>(),
        vec!["1", "4", "3", "4", "done"],
        "per-element inline return semantics broken: {stdout:?}"
    );
}
