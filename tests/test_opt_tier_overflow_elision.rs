//! v1.3.4 tiered overflow-check strategy tests (`--opt speed`).
//!
//! Verified properties:
//! 1. A loop with a compile-time-constant bound and a constant step gets its
//!    induction increment marked as proven-overflow-free under the speed
//!    tier, and is left untouched under the default tier. (Step 2 is used:
//!    the v1.3.3 BCE pass already rewrites the `i + 1` case to `wrapping_+`,
//!    so step 2 is where a checked trap actually survives to this pass.)
//! 2. The program produces identical results under both tiers.
//! 3. USER body arithmetic inside the very same loop keeps its overflow
//!    trap under the speed tier: a deliberately overflowing accumulator
//!    still traps (non-zero exit) even though the induction increment was
//!    elided.
//! 4. A loop whose bound is a runtime value is never marked: without a
//!    static bound there is no proof. (Direct pass-level unit test, because
//!    the full pipeline constant-folds or removes such loops before the
//!    pass runs.)
//!
//! The pipeline test loops carry three loop variables (`s`, `t`, `i`) on
//! purpose: LoopFold's closed-form rewriting only matches two-parameter
//! countable loops, so these loops survive to codegen and the backend
//! overflow gates stay observable.

use forgen::codegen::cranelift::CraneliftBackend;
use forgen::diagnostics::DiagnosticEngine;
use forgen::dmir::Lowering;
use forgen::lexer::Lexer;
use forgen::optimizer::loops::ovf_elide;
use forgen::optimizer::{OptTier, Optimizer};
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;
use std::path::PathBuf;

fn lower(src: &str) -> forgen::dmir::Module {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("ovf.dtr", src);

    let mut lexer = Lexer::new(src, "ovf.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "ovf.dtr");
    let program = parser.parse_program();
    assert!(!diag.has_errors(), "parse errors: {:?}", diag.diagnostics);

    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);
    assert!(!diag.has_errors(), "resolve errors: {:?}", diag.diagnostics);

    let mut tc = TypeChecker::new(&resolver);
    tc.check_program(&program, &mut diag);
    assert!(
        !diag.has_errors(),
        "typecheck errors: {:?}",
        diag.diagnostics
    );

    let mut lowering = Lowering::new(&resolver, &tc);
    lowering.lower_program(&program, "ovf")
}

fn optimize(src: &str, tier: OptTier) -> forgen::dmir::Module {
    let mut module = lower(src);
    let mut opt = Optimizer::new("release");
    opt.opt_tier = tier;
    opt.optimize_module(&mut module)
        .expect("optimizer must not fail");
    module
}

fn compile_and_run(module: &forgen::dmir::Module, tag: &str) -> (String, i32) {
    let be = CraneliftBackend::for_host();
    let exe = PathBuf::from(format!("target/ovf_elide_{}.exe", tag));
    let p = be
        .compile_native(module, &exe)
        .unwrap_or_else(|e| panic!("[{}] native compilation failed: {}", tag, e));
    let (out, _err, code, _) = be
        .run_executable(&p, &[])
        .unwrap_or_else(|e| panic!("[{}] execution failed: {}", tag, e));
    (out.trim().to_string(), code)
}

/// Sum of the even values below 1_000_000 plus a trailing copy of the last
/// induction value. The induction step is 2: still a compile-time constant,
/// but not the `+1` case the v1.3.3 BCE pass already rewrites to a wrapping
/// increment, so the overflow trap genuinely reaches this pass.
const SUM_SRC: &str = r#"
fn sum_evens() -> Int {
    mut i = 0
    mut s = 0
    mut t = 0
    while i < 1000000 {
        s = s + i
        t = i
        i = i + 2
    }
    return s + t
}
fn main() {
    out sum_evens()
}
"#;

#[test]
fn speed_tier_marks_constant_bound_induction_increment() {
    let module = optimize(SUM_SRC, OptTier::Speed);
    let f = module
        .functions
        .get("sum_evens")
        .expect("sum_evens must exist in the DMIR module");
    assert!(
        !f.proven_no_overflow.is_empty(),
        "speed tier must mark the provably bounded induction increment of sum_evens"
    );
}

#[test]
fn default_tier_never_marks_overflow_elision() {
    let module = optimize(SUM_SRC, OptTier::Default);
    let f = module
        .functions
        .get("sum_evens")
        .expect("sum_evens must exist in the DMIR module");
    assert!(
        f.proven_no_overflow.is_empty(),
        "default tier must keep every overflow check; found markings: {:?}",
        f.proven_no_overflow
    );
}

#[test]
fn runtime_bound_loop_is_never_marked_even_at_speed() {
    // Direct pass-level test with a hand-built module (the evidence tests
    // use the same style): a `while i < n` loop where `n` is a function
    // parameter. `elide_induction_overflow` must NOT mark the increment.
    use forgen::dmir::{BasicBlock, BasicBlockId, Function, Inst, Terminator, ValueId};

    let build = |bound_is_const: bool| -> forgen::dmir::Module {
        // ValueIds: n=0 (function param), i0=1, s0=2, one=3, bound=4,
        // header: i=5, s=6, cond=7; body: s_next=8, i_next=9.
        let entry = BasicBlock {
            id: BasicBlockId(0),
            label: "entry".into(),
            params: vec![],
            instructions: vec![
                Inst::ConstInt {
                    dest: ValueId(1),
                    value: 0,
                },
                Inst::ConstInt {
                    dest: ValueId(2),
                    value: 0,
                },
                Inst::ConstInt {
                    dest: ValueId(3),
                    value: 1,
                },
                Inst::ConstInt {
                    dest: ValueId(4),
                    value: 100,
                },
            ],
            terminator: Terminator::Branch {
                target: BasicBlockId(1),
                args: vec![ValueId(1), ValueId(2)],
            },
        };
        let bound_vid = if bound_is_const {
            ValueId(4)
        } else {
            ValueId(0) // the `n` function parameter: a runtime value
        };
        let header = BasicBlock {
            id: BasicBlockId(1),
            label: "header".into(),
            params: vec![
                forgen::dmir::BlockParam {
                    val: ValueId(5),
                    ty: "Int".into(),
                    name: Some("i".into()),
                },
                forgen::dmir::BlockParam {
                    val: ValueId(6),
                    ty: "Int".into(),
                    name: Some("s".into()),
                },
            ],
            instructions: vec![Inst::BinOp {
                dest: ValueId(7),
                op: "<".into(),
                left: ValueId(5),
                right: bound_vid,
                ty: "Int".into(),
            }],
            terminator: Terminator::CondBranch {
                cond: ValueId(7),
                then_block: BasicBlockId(2),
                then_args: vec![],
                else_block: BasicBlockId(3),
                else_args: vec![],
            },
        };
        let body = BasicBlock {
            id: BasicBlockId(2),
            label: "body".into(),
            params: vec![],
            instructions: vec![
                Inst::BinOp {
                    dest: ValueId(8),
                    op: "+".into(),
                    left: ValueId(6),
                    right: ValueId(5),
                    ty: "Int".into(),
                },
                Inst::BinOp {
                    dest: ValueId(9),
                    op: "+".into(),
                    left: ValueId(5),
                    right: ValueId(3),
                    ty: "Int".into(),
                },
            ],
            terminator: Terminator::Branch {
                target: BasicBlockId(1),
                args: vec![ValueId(9), ValueId(8)],
            },
        };
        let exit = BasicBlock {
            id: BasicBlockId(3),
            label: "exit".into(),
            params: vec![],
            instructions: vec![],
            terminator: Terminator::Return {
                value: Some(ValueId(6)),
            },
        };
        let f = Function {
            name: "loop_fn".into(),
            params: vec![("n".into(), "Int".into(), ValueId(0))],
            return_type: "Int".into(),
            entry_block: BasicBlockId(0),
            blocks: vec![entry, header, body, exit],
            ..Default::default()
        };
        let mut module = forgen::dmir::Module::new("unit");
        module.functions.insert("loop_fn".into(), f);
        module
    };

    // Runtime bound: no static proof, nothing may be marked.
    let mut runtime = build(false);
    let mut trace = forgen::optimizer::cost_model::OptimizationDecisionTrace::new();
    assert_eq!(
        ovf_elide::elide_induction_overflow(&mut runtime, &mut trace),
        0,
        "a runtime bound gives no static proof; the increment must stay checked"
    );

    // Constant bound: the same shape IS provable and gets marked.
    let mut constant = build(true);
    let mut trace2 = forgen::optimizer::cost_model::OptimizationDecisionTrace::new();
    assert_eq!(
        ovf_elide::elide_induction_overflow(&mut constant, &mut trace2),
        1,
        "constant bound 100 with step 1 is provable; the increment must be marked"
    );
    let f = constant.functions.get("loop_fn").unwrap();
    assert!(f.proven_no_overflow.contains(&ValueId(9)));
}

#[test]
fn bounded_loop_produces_identical_results_under_both_tiers() {
    let expected = (0..1_000_000i64).step_by(2).sum::<i64>() + 999_998;

    let (out_default, code_default) =
        compile_and_run(&optimize(SUM_SRC, OptTier::Default), "default");
    assert_eq!(code_default, 0, "default-tier run must exit cleanly");
    assert_eq!(
        out_default,
        expected.to_string(),
        "default tier changed the loop result"
    );

    let (out_speed, code_speed) = compile_and_run(&optimize(SUM_SRC, OptTier::Speed), "speed");
    assert_eq!(code_speed, 0, "speed-tier run must exit cleanly");
    assert_eq!(
        out_speed,
        expected.to_string(),
        "speed tier (elided induction checks) changed the loop result"
    );
}

/// User body arithmetic (`x = x + i`) overflows on the second iteration.
/// The induction increment `i = i + 2` IS eligible for elision (constant
/// bound 100), but the user arithmetic must keep its trap under the speed
/// tier: elision covers the induction variable only.
#[test]
fn overflowing_user_body_arithmetic_still_traps_at_speed() {
    let src = r#"
fn boom() -> Int {
    mut i = 0
    mut x = 9223372036854775807
    mut t = 0
    while i < 100 {
        x = x + i
        t = i
        i = i + 2
    }
    return x + t
}
fn main() {
    out boom()
}
"#;
    for tier in [OptTier::Default, OptTier::Speed] {
        let module = optimize(src, tier);
        let (_, code) = compile_and_run(&module, &format!("boom_{:?}", tier));
        assert_ne!(
            code, 0,
            "[{:?}] overflowing user arithmetic must trap (non-zero exit) even at the speed tier",
            tier
        );
    }
}
