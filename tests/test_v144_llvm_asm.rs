//! v1.4.4: 4th Level Structured Inline Assembly under LLVM AOT tests.

use forgen::driver::ForgenCompiler;

#[test]
fn test_v144_structured_asm_supported_on_llvm_backend() {
    let source = r#"
fn compute_hardware_fast(x: Int) -> Int {
    mut counter = x
    unsafe(justification: "level 4 hardware inline asm") {
        asm {
            mov eax, counter
            add eax, 10
            mov counter, eax
        }
    }
    return counter
}

fn main() {
    let res = compute_hardware_fast(32)
    out res
}
"#;

    let mut compiler = ForgenCompiler::new("release");
    compiler.use_llvm = true;
    let res = compiler.check_source(source, "v144_asm_llvm.dtr");
    assert!(
        res.success,
        "Structured asm blocks must be supported on LLVM backend without E1405 error: {:?}",
        res.diagnostics
    );
    assert!(
        !res.diagnostics.contains("E1405"),
        "Diagnostics must not contain E1405 under LLVM: {}",
        res.diagnostics
    );
}

#[test]
fn test_v144_structured_asm_still_rejects_wasm_with_e1405() {
    let source = r#"
fn main() {
    mut x = 1
    unsafe(justification: "wasm probe") {
        asm {
            mov eax, x
            add eax, 1
            mov x, eax
        }
    }
    out x
}
"#;

    let compiler =
        ForgenCompiler::new("release").with_target(Some("wasm32-unknown-unknown".into()));
    let res = compiler.compile_source(source, "v144_asm_wasm.dtr", None);
    assert!(
        !res.success,
        "Structured asm on wasm must still be rejected"
    );
    assert!(
        res.diagnostics.contains("E1405"),
        "Diagnostics on wasm must contain E1405: {}",
        res.diagnostics
    );
}
