use forgen::codegen::TargetInfo;
use forgen::codegen::cranelift::backend::RealCraneliftBackend;
use forgen::codegen::cranelift::backend::opts::JitCompilationTier;
use forgen::codegen::cranelift::jit::JitSession;
use forgen::diagnostics::{DiagnosticEngine, ErrorCode};
use forgen::driver::ForgenCompiler;

#[test]
fn test_use_after_move_diagnostic_clarity() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
struct Resource {
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
    assert!(
        diag.contains("E-BORROW-001"),
        "must contain E-BORROW-001 code: {}",
        diag
    );
    assert!(
        diag.contains("Use of moved value")
            || diag.contains("previously moved")
            || diag.contains("Cannot move"),
        "message must explain move: {}",
        diag
    );
    assert!(
        diag.contains("help:"),
        "must provide actionable help suggestion: {}",
        diag
    );
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
    assert!(
        diag.contains("E0941"),
        "must contain E0941 error code: {}",
        diag
    );
    assert!(
        diag.contains("divisor"),
        "must explain unproven non-zero divisor: {}",
        diag
    );
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
    assert!(
        json_str.starts_with('['),
        "JSON diagnostic must be a JSON array"
    );
    assert!(json_str.contains("\"code\": \"E-BORROW-001\""));
    assert!(json_str.contains("\"severity\": \"ERROR\""));
    assert!(json_str.contains("pass a borrowed view"));

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).expect("must parse JSON");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["code"], "E-BORROW-001");
}

#[test]
fn test_out_unprintable_composite_types_rejected_e_out_001() {
    let compiler = ForgenCompiler::new("check");

    // 1. Map cannot be printed directly
    let src_map = r#"
fn main() -> Int {
    let m = { "key": 42 }
    out m
    return 0
}
"#;
    let res_map = compiler.check_source(src_map, "test_out_map.dtr");
    assert!(!res_map.success, "out Map must be rejected");
    assert!(
        res_map.diagnostics.contains("E-OUT-001"),
        "expected E-OUT-001 for Map, got: {}",
        res_map.diagnostics
    );

    // 2. List cannot be printed directly
    let src_list = r#"
fn main() -> Int {
    let l = [1, 2, 3]
    out l
    return 0
}
"#;
    let res_list = compiler.check_source(src_list, "test_out_list.dtr");
    assert!(!res_list.success, "out List must be rejected");
    assert!(
        res_list.diagnostics.contains("E-OUT-001"),
        "expected E-OUT-001 for List, got: {}",
        res_list.diagnostics
    );

    // 3. Struct without Display cannot be printed
    let src_struct = r#"
struct Point {
    x: Int
    y: Int
}

fn main() -> Int {
    let p = Point { x: 10, y: 20 }
    out p
    return 0
}
"#;
    let res_struct = compiler.check_source(src_struct, "test_out_struct_no_display.dtr");
    assert!(
        !res_struct.success,
        "out struct without Display must be rejected"
    );
    assert!(
        res_struct.diagnostics.contains("E-OUT-001"),
        "expected E-OUT-001 for struct, got: {}",
        res_struct.diagnostics
    );
}

#[test]
fn test_out_struct_with_derive_display_permitted() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
@derive(Display)
struct Point {
    x: Int
    y: Int
}

fn main() -> Int {
    let p = Point { x: 10, y: 20 }
    out p
    return 0
}
"#;
    let res = compiler.check_source(src, "test_out_display.dtr");
    assert!(
        res.success,
        "out struct with @derive(Display) must pass check, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_out_struct_with_derive_display_jit() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
@derive(Display)
struct Point {
    x: Int
    y: Int
}

fn main() -> Int {
    let p = Point { x: 10, y: 20 }
    out p
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_out_display_jit.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    session
        .load_module(&dmir_mod)
        .expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Point") && stdout.contains("10") && stdout.contains("20"),
        "stdout was: '{}'",
        stdout
    );
}

#[test]
fn test_out_format_str_fusion_jit() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
fn main() -> Int {
    let name = "Datara"
    let ver = 144
    out fmt"Language: {name} v{ver} OK"
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_out_fusion.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    // Verify format fusion: no Inst::FormatStr in DMIR instructions for out fmt"..."
    let mut has_format_str = false;
    for (_name, func) in &dmir_mod.functions {
        for block in &func.blocks {
            for inst in &block.instructions {
                if matches!(inst, forgen::dmir::Inst::FormatStr { .. }) {
                    has_format_str = true;
                }
            }
        }
    }
    assert!(
        !has_format_str,
        "out fmt'...' must be fused; found Inst::FormatStr"
    );

    session
        .load_module(&dmir_mod)
        .expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "Language: Datara v144 OK");
}

#[test]
fn test_out_format_str_unprintable_rejected_e_out_001() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
fn main() -> Int {
    let m = { "a": 1 }
    out fmt"Map is: {m}"
    return 0
}
"#;
    let res = compiler.check_source(src, "test_out_fusion_err.dtr");
    assert!(
        !res.success,
        "unprintable type in format str must be rejected"
    );
    assert!(
        res.diagnostics.contains("E-OUT-001"),
        "expected E-OUT-001, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_deprecated_print_function_warning_w0102() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
fn main() -> Int {
    println("legacy call")
    return 0
}
"#;
    let res = compiler.check_source(src, "test_legacy_print.dtr");
    assert!(res.success, "deprecated warning must not fail check");
    assert!(
        res.diagnostics.contains("W0102"),
        "expected W0102 warning, got: {}",
        res.diagnostics
    );
    assert!(
        res.diagnostics.contains("deprecated") && res.diagnostics.contains("out"),
        "diagnostic message must advise using out: {}",
        res.diagnostics
    );
}

#[test]
fn test_print_function_canonical_no_warning() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
fn main() -> Int {
    print("canonical print without newline")
    return 0
}
"#;
    let res = compiler.check_source(src, "test_canonical_print.dtr");
    assert!(res.success, "print must succeed check");
    assert!(
        !res.diagnostics.contains("W0102"),
        "canonical print must NOT produce W0102 warning, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_eprintln_function_warning_w0102() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
fn main() -> Int {
    eprintln("stderr message")
    return 0
}
"#;
    let res = compiler.check_source(src, "test_legacy_eprintln.dtr");
    assert!(res.success, "eprintln warning must not fail check");
    assert!(
        res.diagnostics.contains("W0102"),
        "expected W0102 warning, got: {}",
        res.diagnostics
    );
    assert!(
        res.diagnostics.contains("err"),
        "eprintln warning must advise using err: {}",
        res.diagnostics
    );
}

#[test]
fn test_print_and_out_jit_execution() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
fn main() -> Int {
    print("Loading... ")
    out "Done!"
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_print_out_jit.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    session
        .load_module(&dmir_mod)
        .expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert_eq!(stdout, "Loading... Done!\n");
}

#[test]
fn test_print_unprintable_rejected_e_out_001() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
fn main() -> Int {
    let l = [1, 2, 3]
    print(l)
    return 0
}
"#;
    let res = compiler.check_source(src, "test_print_list_err.dtr");
    assert!(!res.success, "print List must be rejected");
    assert!(
        res.diagnostics.contains("E-OUT-001"),
        "expected E-OUT-001 for print(List), got: {}",
        res.diagnostics
    );
}
