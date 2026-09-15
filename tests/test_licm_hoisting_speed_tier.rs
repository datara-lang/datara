//! v1.3.4 loop-invariant hoisting tests across optimization tiers.
//!
//! Verified properties:
//! 1. Existing conservative LICM (invariant arithmetic, e.g. `k * 5`) keeps
//!    working under both tiers: the computation is physically outside the
//!    loop and the result is identical.
//! 2. Under `--opt speed`, calls to functions the effect lattice proved
//!    pure are ALSO hoisted; under `--opt default` the same call stays in
//!    the loop (name-prefix purity does not cover it).
//! 3. Results are identical under both tiers.

use forgen::codegen::cranelift::CraneliftBackend;
use forgen::diagnostics::DiagnosticEngine;
use forgen::dmir::cfg::ControlFlowGraph;
use forgen::dmir::{Inst, Lowering};
use forgen::effects::EffectAnalyzer;
use forgen::lexer::Lexer;
use forgen::optimizer::{OptTier, Optimizer};
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;
use std::path::PathBuf;

fn analyze(src: &str) -> forgen::ast::Program {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("licm_tier.dtr", src);

    let mut lexer = Lexer::new(src, "licm_tier.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "licm_tier.dtr");
    let program = parser.parse_program();
    assert!(!diag.has_errors(), "parse errors: {:?}", diag.diagnostics);
    program
}

fn resolve_and_lower(program: &forgen::ast::Program) -> forgen::dmir::Module {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("licm_tier.dtr", "");

    let mut resolver = Resolver::new();
    resolver.resolve_program(program, &mut diag);
    assert!(!diag.has_errors(), "resolve errors: {:?}", diag.diagnostics);

    let mut tc = TypeChecker::new(&resolver);
    tc.check_program(program, &mut diag);
    assert!(
        !diag.has_errors(),
        "typecheck errors: {:?}",
        diag.diagnostics
    );

    let mut lowering = Lowering::new(&resolver, &tc);
    lowering.lower_program(program, "licm_tier")
}

fn optimize(src: &str, tier: OptTier) -> (forgen::dmir::Module, Optimizer) {
    let program = analyze(src);
    let mut effects = EffectAnalyzer::new();
    effects.analyze_program(&program);
    let effect_map = effects.function_effects.clone();

    let mut module = resolve_and_lower(&program);
    let mut opt = Optimizer::new("release");
    opt.opt_tier = tier;
    opt.set_function_effects(effect_map);
    opt.optimize_module(&mut module)
        .expect("optimizer must not fail");
    (module, opt)
}

fn compile_and_run(module: &forgen::dmir::Module, tag: &str) -> String {
    let be = CraneliftBackend::for_host();
    let exe = PathBuf::from(format!("target/licm_tier_{}.exe", tag));
    let p = be
        .compile_native(module, &exe)
        .unwrap_or_else(|e| panic!("[{}] native compilation failed: {}", tag, e));
    let (out, err, code, _) = be
        .run_executable(&p, &[])
        .unwrap_or_else(|e| panic!("[{}] execution failed: {}", tag, e));
    assert_eq!(code, 0, "[{}] non-zero exit; stderr={}", tag, err);
    out.trim().to_string()
}

/// All instructions contained in the blocks forming the natural loops of
/// `fn_name`.
/// The pipeline mangles constant-arg specializations as
/// `<name>__spec_<param>_<value>`, so the lookup falls back to a prefix
/// match (the same convention `test_optimizer_licm_proof` uses).
fn loop_instructions(module: &forgen::dmir::Module, fn_name: &str) -> Vec<Inst> {
    let f = module
        .functions
        .get(fn_name)
        .or_else(|| {
            module
                .functions
                .iter()
                .find(|(name, _)| name.starts_with(fn_name))
                .map(|(_, f)| f)
        })
        .unwrap_or_else(|| panic!("function '{}' must exist", fn_name));
    let cfg = ControlFlowGraph::build(f);
    cfg.loops
        .iter()
        .flat_map(|lp| lp.blocks.iter())
        .filter_map(|b| f.get_block(*b))
        .flat_map(|b| b.instructions.iter())
        .cloned()
        .collect()
}

/// The `out c` inside the loop makes the function impure, so the pipeline's
/// whole-call constant evaluator refuses it and the loop survives to codegen
/// with real runtime semantics (the fold and DSE would otherwise delete it).
const INVARIANT_ARITH_SRC: &str = r#"
fn count_iters(n: Int, k: Int) -> Int {
    mut i = 0
    mut c = 0
    while i < n {
        let step = k * 5
        c = c + step
        out c
        i = i + 1
    }
    return c
}
fn main() {
    out count_iters(21, 3)
}
"#;

#[test]
fn invariant_arithmetic_hoists_and_runs_at_both_tiers() {
    for tier in [OptTier::Default, OptTier::Speed] {
        let (module, _opt) = optimize(INVARIANT_ARITH_SRC, tier);
        let after = loop_instructions(&module, "count_iters");
        assert!(
            !after
                .iter()
                .any(|i| matches!(i, Inst::BinOp { op, .. } if op == "*")),
            "[{:?}] the invariant multiply must be hoisted out of the loop",
            tier
        );
        // count_iters(21, 3): step = 15; each iteration prints the running
        // total 15, 30, ..., 315; the returned value prints 315 again.
        let mut lines: Vec<String> = Vec::new();
        for i in 1..=21 {
            lines.push((15 * i).to_string());
        }
        lines.push("315".to_string());
        let expected = lines.join(
            "
",
        );
        assert_eq!(
            compile_and_run(&module, &format!("arith_{:?}", tier)),
            expected,
            "[{:?}] hoisting changed the result",
            tier
        );
    }
}

/// `combine` is a plain pure-arithmetic function: its name matches none of
/// the `is_pure_call` prefixes, so under the default tier the call stays in
/// the loop. The effect lattice proves it pure, so under `--opt speed` the
/// call is hoisted into the preheader and executes once.
/// `combine` is effect-lattice-pure arithmetic, but deliberately oversized
/// for every inliner: 21 chained additions exceed the cost-model inliner's
/// 20-instruction budget (pre-mem2reg) and IPO's 15-instruction cap
/// (post-mem2reg). The `out c` inside the loop keeps loop_user impure so
/// the whole-call constant evaluator refuses it and the loop survives to
/// codegen.
fn pure_call_src() -> String {
    let mut chain = String::new();
    chain.push_str(
        "    let t1 = a + 1
",
    );
    for k in 2..=21 {
        chain.push_str(&format!(
            "    let t{} = t{} + 1
",
            k,
            k - 1
        ));
    }
    format!(
        r#"
fn combine(a: Int, b: Int) -> Int {{
{chain}    return t21 * b + 7
}}
fn loop_user(n: Int, a: Int, b: Int) -> Int {{
    mut i = 0
    mut c = 0
    while i < n {{
        c = c + combine(a, b)
        out c
        i = i + 1
    }}
    return c
}}
fn main() {{
    out loop_user(5, 3, 4)
}}
"#
    )
}

/// Direct LICM unit test (same style as the evidence tests): a hand-built
/// module whose loop calls a function named `combine`. The name matches none
/// of the `is_pure_call` prefixes, so:
/// * with an empty `extra_pure` set (the `--opt default` configuration) the
///   call is not hoistable and stays inside the loop;
/// * with `extra_pure = {"combine"}` (what `optimize_function` derives from
///   the effect lattice under `--opt speed`) the very same call is hoisted
///   into the preheader.
///
/// The unit level is the right granularity here: at the pipeline level the
/// constant-arg specializer already inlines small pure callees into
/// specialized copies, which masks the tier delta for any construct it can
/// see through.
#[test]
fn licm_honors_extra_pure_set_only_at_speed() {
    use forgen::dmir::{BasicBlock, BasicBlockId, Function, Terminator, ValueId};
    use forgen::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
    use forgen::optimizer::loops::LoopOptimizer;

    let build = || -> forgen::dmir::Module {
        // ValueIds: a=9 (const 3), b=10 (const 4), one=3, bound=4,
        // header: i=5, c=6, cond=7; body: call=8, c_next=11, i_next=12.
        let entry = BasicBlock {
            id: BasicBlockId(0),
            label: "entry".into(),
            params: vec![],
            instructions: vec![
                Inst::ConstInt {
                    dest: ValueId(9),
                    value: 3,
                },
                Inst::ConstInt {
                    dest: ValueId(10),
                    value: 4,
                },
                Inst::ConstInt {
                    dest: ValueId(3),
                    value: 1,
                },
                Inst::ConstInt {
                    dest: ValueId(4),
                    value: 10,
                },
            ],
            terminator: Terminator::Branch {
                target: BasicBlockId(1),
                args: vec![ValueId(9), ValueId(3)],
            },
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
                    name: Some("c".into()),
                },
            ],
            instructions: vec![Inst::BinOp {
                dest: ValueId(7),
                op: "<".into(),
                left: ValueId(5),
                right: ValueId(4),
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
                Inst::Call {
                    dest: ValueId(8),
                    func: "combine".into(),
                    args: vec![ValueId(9), ValueId(10)],
                    ty: "Int".into(),
                },
                Inst::BinOp {
                    dest: ValueId(11),
                    op: "+".into(),
                    left: ValueId(6),
                    right: ValueId(8),
                    ty: "Int".into(),
                },
                Inst::BinOp {
                    dest: ValueId(12),
                    op: "+".into(),
                    left: ValueId(5),
                    right: ValueId(3),
                    ty: "Int".into(),
                },
            ],
            terminator: Terminator::Branch {
                target: BasicBlockId(1),
                args: vec![ValueId(12), ValueId(11)],
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
            params: vec![],
            return_type: "Int".into(),
            entry_block: BasicBlockId(0),
            blocks: vec![entry, header, body, exit],
            ..Default::default()
        };
        let mut module = forgen::dmir::Module::new("unit");
        module.functions.insert("loop_fn".into(), f);
        module
    };

    fn call_in_loop(module: &forgen::dmir::Module) -> bool {
        let f = module.functions.get("loop_fn").unwrap();
        let cfg = ControlFlowGraph::build(f);
        cfg.loops.iter().any(|lp| {
            lp.blocks.iter().any(|bid| {
                f.get_block(*bid)
                    .map(|b| {
                        b.instructions
                            .iter()
                            .any(|i| matches!(i, Inst::Call { func, .. } if func == "combine"))
                    })
                    .unwrap_or(false)
            })
        })
    }

    // Default-tier configuration: empty extra_pure set. The call must stay.
    let mut default_module = build();
    let cost_model = CostModel::new("release");
    let mut trace = OptimizationDecisionTrace::new();
    let empty: std::collections::HashSet<String> = std::collections::HashSet::new();
    LoopOptimizer::licm_pass(
        default_module.functions.get_mut("loop_fn").unwrap(),
        &cost_model,
        &mut trace,
        &empty,
    );
    assert!(
        call_in_loop(&default_module),
        "default tier (empty extra_pure) must not hoist a call that only the effect lattice proves pure"
    );

    // Speed-tier configuration: the effect lattice proved `combine` pure.
    let mut speed_module = build();
    let mut trace2 = OptimizationDecisionTrace::new();
    let extra: std::collections::HashSet<String> = ["combine".to_string()].into_iter().collect();
    let hoisted = LoopOptimizer::licm_pass(
        speed_module.functions.get_mut("loop_fn").unwrap(),
        &cost_model,
        &mut trace2,
        &extra,
    );
    assert!(hoisted >= 1, "the lattice-pure call must be hoisted");
    assert!(
        !call_in_loop(&speed_module),
        "speed tier must hoist the effect-lattice-pure call out of the loop"
    );
}

/// End-to-end: the hoisted (default tier) pipeline still produces identical
/// results at both tiers.
#[test]
fn hoisted_call_produces_identical_results_at_both_tiers() {
    // loop_user(5, 3, 4): combine(3, 4) = 103; running totals 103, 206, 309, 412, 515.
    let expected = "103
206
309
412
515
515";
    let (default_module, _) = optimize(&pure_call_src(), OptTier::Default);
    assert_eq!(
        compile_and_run(&default_module, "pure_default"),
        expected,
        "default tier changed the result"
    );
    let (speed_module, _) = optimize(&pure_call_src(), OptTier::Speed);
    assert_eq!(
        compile_and_run(&speed_module, "pure_speed"),
        expected,
        "speed tier (hoisted pure call) changed the result"
    );
}
