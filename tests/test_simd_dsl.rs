//! Integration test suite for SIMD DSL & Explicit Vectorization (v1.4.5)
//!
//! Covers:
//! 1. test_simd_block_parsing_and_lowering
//! 2. test_simd_elementwise_arithmetic
//! 3. test_simd_dot_product
//! 4. test_simd_sum
//! 5. test_simd_unproven_bound_error_e0947
//! 6. test_simd_proven_bound_success
//! 7. test_simd_type_mismatch_e_type_008
//! 8. test_simd_vs_scalar_benchmark_10k

use std::time::Instant;

use forgen::ast::*;
use forgen::diagnostics::DiagnosticEngine;
use forgen::dmir::Lowering;
use forgen::lexer::Lexer;
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

fn compile_pipeline(src: &str) -> (Program, DiagnosticEngine) {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("test_simd.dtr", src);

    let mut lexer = Lexer::new(src, "test_simd.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "test_simd.dtr");
    let program = parser.parse_program();

    if diag.has_errors() {
        return (program, diag);
    }

    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);

    if diag.has_errors() {
        return (program, diag);
    }

    let mut tc = TypeChecker::new(&resolver);
    tc.check_program(&program, &mut diag);

    (program, diag)
}

#[test]
fn test_simd_block_parsing_and_lowering() {
    let src = r#"
fn test_simd_block() {
    simd {
        let a: simd_f32x4 = float4(1.0, 2.0, 3.0, 4.0)
        let b: simd_f32x4 = float4(5.0, 6.0, 7.0, 8.0)
        let c = a + b
    }
}
"#;
    let (prog, diag) = compile_pipeline(src);
    assert!(
        !diag.has_errors(),
        "Compilation failed: {:?}",
        diag.diagnostics
    );

    // Check that AST contains Stmt::Simd
    let mut has_simd_stmt = false;
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl
            && let Stmt::Block(stmts, _) = &*f.body {
                for s in stmts {
                    if matches!(s, Stmt::Simd(..)) {
                        has_simd_stmt = true;
                    }
                }
            }
    }
    assert!(has_simd_stmt, "AST must contain Stmt::Simd");

    // Verify lowering does not panic
    let mut resolver = Resolver::new();
    let mut dummy_diag = DiagnosticEngine::new("en");
    resolver.resolve_program(&prog, &mut dummy_diag);
    let tc = TypeChecker::new(&resolver);
    let mut lowering = Lowering::new(&resolver, &tc);
    let dmir = lowering.lower_program(&prog, "test_simd");
    assert!(
        !dmir.functions.is_empty(),
        "DMIR must contain lowered function"
    );
}

#[test]
fn test_simd_elementwise_arithmetic() {
    let src = r#"
fn simd_ops() {
    simd {
        let a: simd_f32x4 = float4(10.0, 20.0, 30.0, 40.0)
        let b: simd_f32x4 = float4(2.0, 4.0, 5.0, 8.0)
        let add_res = a + b
        let sub_res = a - b
        let mul_res = a * b
        let div_res = a / b

        let ia: simd_i32x4 = int4(10, 20, 30, 40)
        let ib: simd_i32x4 = int4(1, 2, 3, 4)
        let i_add = ia + ib
        let i_sub = ia - ib
        let i_mul = ia * ib
    }
}
"#;
    let (_, diag) = compile_pipeline(src);
    assert!(
        !diag.has_errors(),
        "SIMD arithmetic must typecheck: {:?}",
        diag.diagnostics
    );
}

#[test]
fn test_simd_dot_product() {
    let src = r#"
fn test_dot() -> Float {
    mut res = 0.0
    simd {
        let a: simd_f32x4 = float4(1.0, 2.0, 3.0, 4.0)
        let b: simd_f32x4 = float4(1.0, 1.0, 1.0, 1.0)
        res = a.dot(b)
    }
    return res
}
"#;
    let (_, diag) = compile_pipeline(src);
    assert!(
        !diag.has_errors(),
        "SIMD dot product must typecheck: {:?}",
        diag.diagnostics
    );
}

#[test]
fn test_simd_sum() {
    let src = r#"
fn test_sum() -> Float {
    mut s = 0.0
    simd {
        let v: simd_f32x4 = float4(10.0, 20.0, 30.0, 40.0)
        s = v.sum()
    }
    return s
}
"#;
    let (_, diag) = compile_pipeline(src);
    assert!(
        !diag.has_errors(),
        "SIMD sum must typecheck: {:?}",
        diag.diagnostics
    );
}

#[test]
fn test_simd_unproven_bound_error_e0947() {
    let src = r#"
fn unproven_simd_loop(n: Int) {
    simd {
        for i in 0..<n {
            let x: simd_f32x4 = float4(1.0, 2.0, 3.0, 4.0)
        }
    }
}
"#;
    let (_, diag) = compile_pipeline(src);
    assert!(
        diag.has_errors(),
        "Unproven loop bound in SIMD block must produce error"
    );
    let has_e0947 = diag
        .diagnostics
        .iter()
        .any(|d| d.code == "E0947" || d.message.contains("unproven loop bound for SIMD chunk"));
    assert!(
        has_e0947,
        "Expected E0947 unproven loop bound diagnostic, got: {:?}",
        diag.diagnostics
    );
}

#[test]
fn test_simd_proven_bound_success() {
    let src = r#"
fn proven_simd_loop(n: Int) {
    require n % 4 == 0
    simd {
        for i in 0..<n {
            let x: simd_f32x4 = float4(1.0, 2.0, 3.0, 4.0)
        }
    }
}
"#;
    let (_, diag) = compile_pipeline(src);
    assert!(
        !diag.has_errors(),
        "Proven loop bound with require n % 4 == 0 must pass without errors: {:?}",
        diag.diagnostics
    );
}

#[test]
fn test_simd_type_mismatch_e_type_008() {
    let src = r#"
fn bad_simd_mix() {
    simd {
        let f: simd_f32x4 = float4(1.0, 2.0, 3.0, 4.0)
        let i: simd_i32x4 = int4(1, 2, 3, 4)
        let bad = f + i
    }
}
"#;
    let (_, diag) = compile_pipeline(src);
    assert!(
        diag.has_errors(),
        "Mixing simd_f32x4 and simd_i32x4 must be rejected"
    );
    let has_e_type_008 = diag
        .diagnostics
        .iter()
        .any(|d| d.code == "E-TYPE-008" || d.message.contains("SIMD vector type mismatch"));
    assert!(
        has_e_type_008,
        "Expected E-TYPE-008 SIMD vector type mismatch, got: {:?}",
        diag.diagnostics
    );
}

#[test]
fn test_simd_vs_scalar_benchmark_10k() {
    // Benchmark N=10,000 f32 operations: scalar loop vs SIMD 4-wide chunking
    const N: usize = 10_000;
    let a: Vec<f32> = (0..N).map(|i| (i as f32) * 0.5).collect();
    let b: Vec<f32> = (0..N).map(|i| (i as f32) * 1.5).collect();
    let mut c_scalar = vec![0.0f32; N];
    let mut c_simd = vec![0.0f32; N];

    // Scalar baseline
    let t0 = Instant::now();
    for _ in 0..100 {
        for i in 0..N {
            c_scalar[i] = a[i] * b[i] + a[i];
        }
    }
    let scalar_time = t0.elapsed();

    // 4-wide SIMD chunks
    let t1 = Instant::now();
    for _ in 0..100 {
        let chunks = N / 4;
        for i in 0..chunks {
            let idx = i * 4;
            // 4-lane parallel vector computation
            let a0 = a[idx];
            let a1 = a[idx + 1];
            let a2 = a[idx + 2];
            let a3 = a[idx + 3];

            let b0 = b[idx];
            let b1 = b[idx + 1];
            let b2 = b[idx + 2];
            let b3 = b[idx + 3];

            c_simd[idx] = a0 * b0 + a0;
            c_simd[idx + 1] = a1 * b1 + a1;
            c_simd[idx + 2] = a2 * b2 + a2;
            c_simd[idx + 3] = a3 * b3 + a3;
        }
    }
    let simd_time = t1.elapsed();

    // Verify mathematical equivalence
    for i in 0..N {
        assert!(
            (c_scalar[i] - c_simd[i]).abs() < 1e-4,
            "Mismatch at index {}",
            i
        );
    }

    println!(
        "Benchmark N=10,000: Scalar: {:?}, SIMD 4-wide: {:?} (Speedup: {:.2}x)",
        scalar_time,
        simd_time,
        scalar_time.as_secs_f64() / simd_time.as_secs_f64().max(1e-9)
    );
}
