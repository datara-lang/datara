use forgen::driver::ForgenCompiler;

#[test]
fn test_generic_box_specialization() {
    let source = r#"
struct Box<T> {
    value: T
}

fn main() {
    mut a = Box<Int> { value: 42 }
    out a.value
}
"#;
    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "generic_box.dtr", None);
    assert!(
        res.success,
        "Generic box compilation failed: {:?}",
        res.error
    );

    let (stdout, _, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "42");

    let rep = res.optimization_report.unwrap();
    println!("Specializations: {:?}", rep.generic_specializations);
    assert!(
        rep.generic_specializations
            .iter()
            .any(|s| s.contains("Box<Int>"))
    );
}

/// v1.5.0: bare-generic callback binding. `fn iter_map<T, U>(items: List<T>,
/// f: U) -> List<U>` used to unify U with the lambda's own Function type
/// ("expected List, got List" at every call site). U now binds to the
/// lambda's RETURN type, and no monomorphized instance is recorded (the
/// call site inline-dispatches the lambda).
#[test]
fn test_bare_generic_callback_binds_lambda_result() {
    let source = r#"
fn iter_map<T, U>(items: List<T>, f: U) -> List<U> {
    mut res = []
    for x in items {
        res.push(f(x))
    }
    return res
}

fn main() {
    val doubled = iter_map([1, 2, 3], x => x * 2)
    out int_to_str(doubled[0])
    out int_to_str(doubled[2])
    val lened = iter_map(["aa", "bbbb"], w => w.len())
    out int_to_str(lened[0])
    out int_to_str(lened[1])
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, "generic_callback.dtr", None);
    assert!(
        res.success,
        "bare-generic callback compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let (stdout, _, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines, vec!["2", "6", "2", "4"]);
}

/// v1.5.0: cross-type generic folds in ONE caller. The backend kind sets
/// (string_vars/bool_vars/...) were monotone: a variable that ever held a
/// string stayed string-typed forever, so a subsequent Int fold reusing the
/// same inlined local names (`acc`) lowered `+` as string concatenation on
/// integers (silent pointer garbage). Re-assignment now retypes the
/// variable.
#[test]
fn test_generic_fold_order_independent() {
    let source = r#"
fn iter_fold<T, A, F>(items: List<T>, init: A, f: F) -> A {
    mut acc = init
    mut i = 0
    val n = items.len()
    while i < n {
        acc = f(acc, items.get(i))
        i = i + 1
    }
    return acc
}

fn main() {
    val joined = iter_fold(["a", "bb", "ccc"], "", (acc, w) => acc + w)
    out joined
    val total = iter_fold([1, 2, 3], 0, (acc, x) => acc + x)
    out int_to_str(total)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, "generic_fold_order.dtr", None);
    assert!(
        res.success,
        "generic fold compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let (stdout, _, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines, vec!["abbccc", "6"]);
}
