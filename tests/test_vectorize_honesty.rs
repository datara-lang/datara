//! PLAN_V152 N5 honesty gate: PolyhedralVectorize used to record
//! "Affine independence analysis proved zero loop-carried dependencies"
//! for the FIRST loop of ANY function that has one — with no analysis at
//! all (the gcd case: a modulo-dependence loop, annotated as
//! "verified affine-independent" — the trace lied).
//!
//! Fix: vectorize metadata is applied ONLY when the loop body provably
//! has no memory-effecting ops (pure scalar arithmetic — dependence-free
//! by construction); loops with loads/stores/calls are recorded as
//! Skipped with an honest reason.

use forgen::dmir::{BasicBlock, BasicBlockId, Function, Inst, Module, Terminator, ValueId};
use forgen::optimizer::cost_model::CostModel;
use forgen::optimizer::loops::polyhedral::PolyhedralEngine;
use forgen::optimizer::cost_model::OptimizationDecisionTrace;

fn loop_function(body_insts: Vec<Inst>) -> Module {
    // Real-block loop: entry -> cond(header) -> body -> latch -> cond,
    // exit -> ret. The CFG natural-loop detection works on branch
    // terminators, not WhileLoop pseudo-insts.
    let mut module = Module::new("vec_honesty");
    let mut function = Function {
        name: "looped".into(),
        params: vec![],
        return_type: "Int".into(),
        entry_block: BasicBlockId(0),
        blocks: Vec::new(),
        ..Default::default()
    };
    let mut cond_insts = vec![Inst::LoadVar {
        dest: ValueId(2),
        name: "i".into(),
    }];
    cond_insts.push(Inst::ConstInt {
        dest: ValueId(3),
        value: 100,
    });
    cond_insts.push(Inst::BinOp {
        dest: ValueId(4),
        op: "<".into(),
        left: ValueId(2),
        right: ValueId(3),
        ty: "Int".into(),
    });
    function.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![Inst::ConstInt {
            dest: ValueId(1),
            value: 0,
        }],
        terminator: Terminator::Branch {
            target: BasicBlockId(1),
            args: vec![],
        },
    });
    function.blocks.push(BasicBlock {
        id: BasicBlockId(1),
        label: "cond".into(),
        params: vec![],
        instructions: cond_insts,
        terminator: Terminator::CondBranch {
            cond: ValueId(4),
            then_block: BasicBlockId(2),
            then_args: vec![],
            else_block: BasicBlockId(3),
            else_args: vec![],
        },
    });
    function.blocks.push(BasicBlock {
        id: BasicBlockId(2),
        label: "body".into(),
        params: vec![],
        instructions: body_insts,
        terminator: Terminator::Branch {
            target: BasicBlockId(1),
            args: vec![],
        },
    });
    function.blocks.push(BasicBlock {
        id: BasicBlockId(3),
        label: "exit".into(),
        params: vec![],
        instructions: vec![Inst::LoadVar {
            dest: ValueId(9),
            name: "i".into(),
        }],
        terminator: Terminator::Return {
            value: Some(ValueId(9)),
        },
    });
    module.functions.insert("looped".to_string(), function);
    module
}

/// gcd-shape loop: a Call inside the body (modulo via call / any call) —
/// the old code annotated this "proved zero loop-carried dependencies".
#[test]
fn gcd_shaped_loop_is_skipped_honestly() {
    let mut module = loop_function(vec![
        Inst::LoadVar {
            dest: ValueId(5),
            name: "i".into(),
        },
        Inst::ConstInt {
            dest: ValueId(6),
            value: 3,
        },
        Inst::Call {
            dest: ValueId(7),
            func: "datara_rt_mod_i64".into(),
            args: vec![ValueId(5), ValueId(6)],
            ty: "Int".into(),
        },
        Inst::AssignVar {
            name: "i".into(),
            value: ValueId(7),
        },
    ]);
    let f = module.functions.get_mut("looped").unwrap();
    let mut trace = OptimizationDecisionTrace::new();
    let cost = CostModel::new("release");
    let _ = PolyhedralEngine::optimize(f, &cost, &mut trace);
    let recs: Vec<_> = trace.records.iter()
        .filter(|r| r.pass == "PolyhedralVectorize")
        .collect();
    assert!(
        !recs.is_empty(),
        "the pass must record its decision for the loop"
    );
    assert!(
        recs.iter().all(|r| r.decision == "Skipped"),
        "a call-carrying loop must NOT be annotated as vectorizable: {:?}",
        recs.iter().map(|r| r.decision.as_str()).collect::<Vec<_>>()
    );
    assert!(
        recs.iter().all(|r| {
            r.reason.contains("loop-carried dependence NOT proven")
                || r.reason.contains("dependence analysis is future work")
        }),
        "the skip reason must be honest: {:?}",
        recs.iter().map(|r| r.reason.as_str()).collect::<Vec<_>>()
    );
}

/// Pure scalar loop: no memory ops — dependence-free by construction.
#[test]
fn pure_scalar_loop_is_annotated() {
    let mut module = loop_function(vec![
        Inst::LoadVar {
            dest: ValueId(5),
            name: "i".into(),
        },
        Inst::ConstInt {
            dest: ValueId(6),
            value: 3,
        },
        Inst::BinOp {
            dest: ValueId(7),
            op: "*".into(),
            left: ValueId(5),
            right: ValueId(6),
            ty: "Int".into(),
        },
        Inst::AssignVar {
            name: "i".into(),
            value: ValueId(7),
        },
    ]);
    let f = module.functions.get_mut("looped").unwrap();
    let mut trace = OptimizationDecisionTrace::new();
    let cost = CostModel::new("release");
    let _ = PolyhedralEngine::optimize(f, &cost, &mut trace);
    let recs: Vec<_> = trace.records.iter()
        .filter(|r| r.pass == "PolyhedralVectorize")
        .collect();
    assert!(!recs.is_empty());
    assert!(
        recs.iter().all(|r| r.decision == "Applied"),
        "a pure scalar loop is dependence-free by construction: {:?}",
        recs.iter().map(|r| (r.decision.as_str(), r.reason.as_str())).collect::<Vec<_>>()
    );
}
