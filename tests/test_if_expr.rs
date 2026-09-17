use forgen::driver::ForgenCompiler;

#[test]
fn test_if_as_expression() {
    let source = r#"
fn check_label(count: Int) -> Str {
    let label = if count > 0 { "Positive" } else { "Non-positive" }
    label
}

fn compare_val(a: Int, b: Int) -> Int {
    let cmp = if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
    cmp
}

fn main() {
    let p = check_label(10)
    let n = check_label(-5)
    let c1 = compare_val(2, 5)
    let c2 = compare_val(5, 2)
    let c3 = compare_val(3, 3)
    out(p)
    out(n)
    out(c1)
    out(c2)
    out(c3)
}
"#;

    let compiler = ForgenCompiler::new("quick");
    let res = compiler.run_source(source, "if_expr.dtr", &[], true);
    assert!(res.is_ok(), "Run must succeed: {:?}", res.err());
    let (stdout, _, code, _) = res.unwrap();
    assert_eq!(code, 0);
    assert!(stdout.contains("Positive"));
    assert!(stdout.contains("Non-positive"));
    assert!(stdout.contains("-1"));
    assert!(stdout.contains("1"));
    assert!(stdout.contains("0"));
}
