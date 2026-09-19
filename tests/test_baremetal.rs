//! Integration test suite for Bare-Metal Preparation (v1.4.5)
//!
//! Covers:
//! 1. test_mmio_volatile_load_store
//! 2. test_mmio_provenance_gate
//! 3. test_mmio_manifest_allowed
//! 4. test_bare_skeleton_generation
//! 5. test_bare_llvm_compilation

use std::fs;

use forgen::ast::*;
use forgen::codegen::llvm::LlvmEmitter;
use forgen::codegen::target::TargetInfo;
use forgen::diagnostics::DiagnosticEngine;
use forgen::dmir::{Inst, Lowering};
use forgen::lexer::Lexer;
use forgen::parser::Parser;
use forgen::project::ProjectInitializer;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

fn compile_pipeline(src: &str) -> (Program, DiagnosticEngine) {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("test_baremetal.dtr", src);

    let mut lexer = Lexer::new(src, "test_baremetal.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "test_baremetal.dtr");
    let program = parser.parse_program();

    let mut resolver = Resolver::new();
    if !diag.has_errors() {
        resolver.resolve_program(&program, &mut diag);
    }

    let mut tc = TypeChecker::new(&resolver);
    if !diag.has_errors() {
        tc.check_program(&program, &mut diag);
    }

    (program, diag)
}

#[test]
fn test_mmio_volatile_load_store() {
    let src = r#"
@mmio(0x40021000)
struct RCC {
    cr: Int at 0x00,
    cfgr: Int at 0x08,
}

fn test_io() -> Int {
    unsafe(justification: "MMIO hardware test") {
        RCC.cr = 0x01
        let val = RCC.cr
        return val
    }
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("test_baremetal.dtr", src);

    let mut lexer = Lexer::new(src, "test_baremetal.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "test_baremetal.dtr");
    let program = parser.parse_program();

    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);

    let mut tc = TypeChecker::new(&resolver);
    tc.check_program(&program, &mut diag);
    assert!(!diag.has_errors(), "Unexpected errors: {}", diag.format_all());

    // 1. Lower to DMIR and check for VolatileLoad / VolatileStore
    let mut lowering = Lowering::new(&resolver, &tc);
    let dmir_module = lowering.lower_program(&program, "test_baremetal");

    let mut found_volatile_load = false;
    let mut found_volatile_store = false;

    for (_name, func) in &dmir_module.functions {
        for block in &func.blocks {
            for inst in &block.instructions {
                match inst {
                    Inst::VolatileLoad { .. } => found_volatile_load = true,
                    Inst::VolatileStore { .. } => found_volatile_store = true,
                    _ => {}
                }
            }
        }
    }

    assert!(found_volatile_store, "DMIR must contain Inst::VolatileStore");
    assert!(found_volatile_load, "DMIR must contain Inst::VolatileLoad");

    // 2. Emit LLVM IR and verify volatile instructions
    let target = TargetInfo::from_triple("thumbv7em-none-eabihf").expect("Valid Cortex-M target");
    let emitter = LlvmEmitter::new(&target);
    let llvm_ir = emitter
        .emit_module(&dmir_module, &program, &tc)
        .expect("LLVM emission must succeed");

    assert!(
        llvm_ir.contains("store volatile"),
        "LLVM IR must contain 'store volatile', got:\n{}",
        llvm_ir
    );
    assert!(
        llvm_ir.contains("load volatile"),
        "LLVM IR must contain 'load volatile', got:\n{}",
        llvm_ir
    );
    assert!(
        llvm_ir.contains("inttoptr"),
        "LLVM IR must contain 'inttoptr' for MMIO addresses, got:\n{}",
        llvm_ir
    );
}

#[test]
fn test_mmio_provenance_gate() {
    let src = r#"
@mmio(0x40021000)
struct RCC {
    cr: Int at 0x00,
}

fn test_unauthorized() {
    RCC.cr = 0x01
}
"#;
    let (_prog, diag) = compile_pipeline(src);
    assert!(diag.has_errors(), "Expected provenance gate error");
    let err_str = diag.format_all();
    assert!(
        err_str.contains("E-MMIO-001"),
        "Expected E-MMIO-001 in diagnostics, got:\n{}",
        err_str
    );
}

#[test]
fn test_mmio_manifest_allowed() {
    let src = r#"
@mmio(0x40021000)
struct RCC {
    cr: Int at 0x00,
}

fn test_allowed() -> Int {
    let val = RCC.cr
    return val
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("test_baremetal.dtr", src);

    let mut lexer = Lexer::new(src, "test_baremetal.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "test_baremetal.dtr");
    let program = parser.parse_program();

    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);

    let mut tc = TypeChecker::new(&resolver);
    // Simulate [devices] allowed = ["RCC"] from datara.toml
    tc.allowed_devices.insert("RCC".to_string());
    tc.check_program(&program, &mut diag);

    assert!(
        !diag.has_errors(),
        "Device in allowed_devices must pass provenance gate without error: {}",
        diag.format_all()
    );
}

#[test]
fn test_bare_skeleton_generation() {
    let temp_base = std::env::temp_dir().join(format!("forgen_bare_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_base);
    let proj_dir = temp_base.join("cortex_app");

    let res = ProjectInitializer::init_bare(Some("cortex_app"), &temp_base, Some("cortex-m4"));
    assert!(res.is_ok(), "init_bare failed: {:?}", res.err());

    // 1. Verify datara.toml
    let manifest_path = proj_dir.join("datara.toml");
    assert!(manifest_path.exists(), "datara.toml must exist");
    let manifest_content = fs::read_to_string(&manifest_path).unwrap();
    assert!(manifest_content.contains("profile = \"bare\""));
    assert!(manifest_content.contains("arch = \"thumbv7em-none-eabihf\""));
    assert!(manifest_content.contains("cpu = \"cortex-m4\""));
    assert!(manifest_content.contains("allowed = [\"GPIOC\", \"RCC\"]"));

    // 2. Verify src/main.dtr
    let main_path = proj_dir.join("src").join("main.dtr");
    assert!(main_path.exists(), "src/main.dtr must exist");
    let main_content = fs::read_to_string(&main_path).unwrap();
    assert!(main_content.contains("@mmio(0x40021000)"));
    assert!(main_content.contains("struct RCC"));
    assert!(main_content.contains("struct GPIOC"));
    assert!(main_content.contains("unsafe(justification:"));

    // 3. Verify src/vectors.dtr
    let vectors_path = proj_dir.join("src").join("vectors.dtr");
    assert!(vectors_path.exists(), "src/vectors.dtr must exist");
    let vectors_content = fs::read_to_string(&vectors_path).unwrap();
    assert!(vectors_content.contains("Reset_Handler"));
    assert!(vectors_content.contains("Default_Handler"));

    // 4. Verify memory.ld
    let ld_path = proj_dir.join("memory.ld");
    assert!(ld_path.exists(), "memory.ld must exist");
    let ld_content = fs::read_to_string(&ld_path).unwrap();
    assert!(ld_content.contains("FLASH (rx) : ORIGIN = 0x08000000"));
    assert!(ld_content.contains("RAM (rwx)  : ORIGIN = 0x20000000"));

    let _ = fs::remove_dir_all(&temp_base);
}

#[test]
fn test_bare_llvm_compilation() {
    // 1. Verify Cranelift rejection via check_target_backend
    let check_res = forgen::cli::check_target_backend(Some("thumbv7em-none-eabihf"), false);
    assert!(check_res.is_err());
    let err_msg = check_res.unwrap_err();
    assert!(
        err_msg.contains("E-TARGET-001"),
        "Expected E-TARGET-001, got: {}",
        err_msg
    );

    // 2. Verify LLVM IR emission for Cortex-M target
    let src = r#"
@mmio(0x40021000)
struct RCC {
    cr: Int at 0x00,
}

fn Reset_Handler() {
    unsafe(justification: "Startup clock enable") {
        RCC.cr = 1
    }
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("test_baremetal.dtr", src);

    let mut lexer = Lexer::new(src, "test_baremetal.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "test_baremetal.dtr");
    let program = parser.parse_program();

    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);

    let mut tc = TypeChecker::new(&resolver);
    tc.check_program(&program, &mut diag);
    assert!(!diag.has_errors(), "Errors: {}", diag.format_all());

    let mut lowering = Lowering::new(&resolver, &tc);
    let dmir_module = lowering.lower_program(&program, "test_baremetal");

    let target = TargetInfo::from_triple("thumbv7em-none-eabihf").expect("Valid Cortex-M target");
    let emitter = LlvmEmitter::new(&target);
    let llvm_ir = emitter
        .emit_module(&dmir_module, &program, &tc)
        .expect("LLVM emission must succeed");

    assert!(
        llvm_ir.contains("target datalayout = \"e-m:e-p:32:32-Fi8-i64:64-v128:64:128-a:0:32-n32-S64\""),
        "Missing Cortex-M datalayout in LLVM IR"
    );
    assert!(
        llvm_ir.contains("target triple = \"thumbv7em-none-eabihf\""),
        "Missing Cortex-M target triple in LLVM IR"
    );
    assert!(
        llvm_ir.contains("store volatile"),
        "Missing volatile store in LLVM IR"
    );
}
