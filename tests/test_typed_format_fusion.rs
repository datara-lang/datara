use forgen::codegen::cranelift::backend::opts::JitCompilationTier;
use forgen::codegen::cranelift::backend::RealCraneliftBackend;
use forgen::codegen::cranelift::jit::JitSession;
use forgen::codegen::TargetInfo;
use forgen::driver::ForgenCompiler;

#[test]
fn test_format_fusion_mixed_types() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
fn main() -> Int {
    let name = "Datara"
    let ver = 144
    let score = 99.5
    let active = true
    out fmt"Lang: {name}, Ver: {ver}, Score: {score}, Active: {active}!"
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_fusion_mixed.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    session.load_module(&dmir_mod).expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "Lang: Datara, Ver: 144, Score: 99.5, Active: true!");
}

#[test]
fn test_format_fusion_more_than_five_parts() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
fn main() -> Int {
    let a = 1
    let b = 2
    let c = 3
    let d = 4
    let e = 5
    let f = 6
    out fmt"p1:{a}|p2:{b}|p3:{c}|p4:{d}|p5:{e}|p6:{f}|end"
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_fusion_many.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    // Verify format fusion: no Inst::FormatStr was emitted in the DMIR function!
    for (_name, func) in &dmir_mod.functions {
        for block in &func.blocks {
            for inst in &block.instructions {
                assert!(
                    !matches!(inst, forgen::dmir::Inst::FormatStr { .. }),
                    "Inst::FormatStr must be fused in out statement"
                );
            }
        }
    }

    session.load_module(&dmir_mod).expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "p1:1|p2:2|p3:3|p4:4|p5:5|p6:6|end");
}

#[test]
fn test_format_fusion_with_display_struct() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
@derive(Display)
struct Color {
    r: Int
    g: Int
    b: Int
}

fn main() -> Int {
    let c = Color { r: 255, g: 128, b: 0 }
    out fmt"Selected color: {c}"
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_fusion_display.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    session.load_module(&dmir_mod).expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Selected color: Color(r=255, g=128, b=0)"),
        "stdout was: '{}'",
        stdout
    );
}

#[test]
fn test_err_format_fusion() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
fn main() -> Int {
    let err_code = 404
    let err_msg = "Not Found"
    err fmt"HTTP error {err_code}: {err_msg}!"
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_err_fusion.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    // Verify format fusion: no Inst::FormatStr was emitted in the DMIR function!
    for (_name, func) in &dmir_mod.functions {
        for block in &func.blocks {
            for inst in &block.instructions {
                assert!(
                    !matches!(inst, forgen::dmir::Inst::FormatStr { .. }),
                    "Inst::FormatStr must be fused in err statement"
                );
            }
        }
    }

    session.load_module(&dmir_mod).expect("load module into JIT");
    let (_, stderr, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert!(
        stderr.contains("HTTP error 404: Not Found!"),
        "stderr was: '{}'",
        stderr
    );
}

