//! v1.4.4: Compiler Diagnostics as Language Design Tests.

use forgen::driver::ForgenCompiler;
use forgen::diagnostics::{DiagnosticEngine, ErrorCode};

#[test]
fn test_use_after_move_diagnostic_clarity() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
class Resource {
    id: Int
}

fn consume(r: Resource) -> Int {
    return r.id
}

fn main() -> Int {
    let res = Resource { id: 42 }
    let a = consume(res)
    let b = consume(res)
    return a + b
}
"#;
    let res = compiler.check_source(src, "test_move.dtr");
    assert!(!res.success, "must fail typecheck on use-after-move");
    let diag = res.diagnostics;
    assert!(diag.contains("E-BORROW-001"), "must contain E-BORROW-001 code: {}", diag);
    assert!(diag.contains("Use of moved value") || diag.contains("previously moved") || diag.contains("Cannot move"), "message must explain move: {}", diag);
    assert!(diag.contains("help:"), "must provide actionable help suggestion: {}", diag);
}

#[test]
fn test_divide_by_zero_diagnostic_proven_contract() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
fn bad_div(a: Int, b: Int) -> Int {
    return a / b
}
"#;
    let res = compiler.check_source(src, "test_div.dtr");
    assert!(!res.success, "must fail on unproven divisor");
    let diag = res.diagnostics;
    assert!(diag.contains("E0941"), "must contain E0941 error code: {}", diag);
    assert!(diag.contains("divisor"), "must explain unproven non-zero divisor: {}", diag);
}

#[test]
fn test_json_machine_readable_diagnostic_export() {
    let mut engine = DiagnosticEngine::new("en");
    engine.error_with_help(
        ErrorCode::BorrowUseAfterMove,
        "Cannot use value 'packet' because it was moved into network send queue".to_string(),
        None,
        Some("pass a borrowed view '&packet' or call .clone()".to_string()),
    );

    let json_str = engine.format_json();
    assert!(json_str.starts_with('['), "JSON diagnostic must be a JSON array");
    assert!(json_str.contains("\"code\": \"E-BORROW-001\""));
    assert!(json_str.contains("\"severity\": \"ERROR\""));
    assert!(json_str.contains("pass a borrowed view"));

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).expect("must parse JSON");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["code"], "E-BORROW-001");
}
