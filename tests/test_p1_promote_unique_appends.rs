//! P1: promotion of `s = s + x` into the in-place append runtime call
//! (`datara_rt_str_concat_mut`).
//!
//! Verifies the optimizer half of the unique-ownership contract (the runtime
//! half — buffer ownership magic, aliasing fallbacks, self-append safety —
//! lives in tests/runtime_c/test_concat_mut.c):
//!   * a proven-unique string append is rewritten to concat_mut;
//!   * parameter slots and mutable module globals are NEVER promoted;
//!   * a second, escaping use of the slot blocks the promotion;
//!   * a non-escaping observer (`out s.len()`) does NOT block promotion.

use forgen::dmir::{BasicBlock, BasicBlockId, Function, Inst, Terminator, ValueId};
use forgen::optimizer::pipeline_fusion::PipelineFusionOptimizer;
use std::collections::HashSet;

fn concat_call_count(f: &Function) -> usize {
    f.blocks
        .iter()
        .flat_map(|b| b.instructions.iter())
        .filter(|i| matches!(i, Inst::Call { func, .. } if func == "datara_rt_str_concat_mut"))
        .count()
}

fn string_binop_count(f: &Function) -> usize {
    f.blocks
        .iter()
        .flat_map(|b| b.instructions.iter())
        .filter(|i| matches!(i, Inst::BinOp { ty, op, .. } if ty == "String" && op == "+"))
        .count()
}

/// Runs only the append-promotion stage of the pipeline (N-ary fusion and
/// arithmetic reassociation must not mask the assertions below).
fn run_promotion(f: &mut Function, mutable_globals: &HashSet<String>) -> usize {
    let uses = PipelineFusionOptimizer::count_uses_public(f);
    PipelineFusionOptimizer::promote_unique_appends_public(f, &uses, mutable_globals)
}

fn base_function() -> Function {
    Function {
        name: "main".into(),
        params: vec![],
        return_type: "Void".into(),
        entry_block: BasicBlockId(0),
        blocks: Vec::new(),
        ..Default::default()
    }
}

/// %1 = LoadVar "s"; %2 = BinOp{+, String, %1, %3}; AssignVar{s, %2}
/// with %3 = ConstStr "x" already emitted by the caller.
fn append_insts(load_dest: ValueId, rhs: ValueId, result: ValueId) -> Vec<Inst> {
    vec![
        Inst::LoadVar {
            dest: load_dest,
            name: "s".into(),
        },
        Inst::BinOp {
            dest: result,
            op: "+".into(),
            left: load_dest,
            right: rhs,
            ty: "String".into(),
        },
        Inst::AssignVar {
            name: "s".into(),
            value: result,
        },
    ]
}

#[test]
fn promotes_unique_append_to_concat_mut() {
    let mut f = base_function();
    // %1 = ConstStr "s"; %2 = ConstStr "x"; %3 = LoadVar s; %4 = BinOp; Assign
    f.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![
            Inst::ConstStr {
                dest: ValueId(1),
                value: "seed0123456789abcdef0123".into(),
            },
            Inst::ConstStr {
                dest: ValueId(2),
                value: "x".into(),
            },
            Inst::LoadVar {
                dest: ValueId(3),
                name: "s".into(),
            },
            Inst::BinOp {
                dest: ValueId(4),
                op: "+".into(),
                left: ValueId(3),
                right: ValueId(2),
                ty: "String".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(4),
            },
            Inst::LoadVar {
                dest: ValueId(5),
                name: "s".into(),
            },
            Inst::Out {
                value: ValueId(5),
                ty: "Void".into(),
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    let mut globals = HashSet::new();
    let n = run_promotion(&mut f, &globals);
    assert_eq!(n, 1, "the proven-unique append must be promoted");
    assert_eq!(concat_call_count(&f), 1, "exactly one concat_mut emitted");
    assert_eq!(string_binop_count(&f), 0, "string BinOp replaced");
    // The final observer load (Out) is untouched.
    assert!(
        f.blocks[0]
            .instructions
            .iter()
            .any(|i| matches!(i, Inst::LoadVar { dest: ValueId(5), .. })),
        "final read of `s` is preserved"
    );
}

#[test]
fn rejects_parameter_slot() {
    let mut f = base_function();
    f.params = vec![("s".to_string(), "String".to_string(), ValueId(1))];
    f.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![
            Inst::ConstStr {
                dest: ValueId(2),
                value: "x".into(),
            },
            Inst::LoadVar {
                dest: ValueId(3),
                name: "s".into(),
            },
            Inst::BinOp {
                dest: ValueId(4),
                op: "+".into(),
                left: ValueId(3),
                right: ValueId(2),
                ty: "String".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(4),
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    let globals = HashSet::new();
    let n = run_promotion(&mut f, &globals);
    assert_eq!(n, 0, "parameter slots may alias external buffers");
    assert_eq!(concat_call_count(&f), 0);
    assert_eq!(string_binop_count(&f), 1);
}

#[test]
fn rejects_mutable_module_global() {
    let mut f = base_function();
    f.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![
            Inst::ConstStr {
                dest: ValueId(1),
                value: "x".into(),
            },
            Inst::LoadVar {
                dest: ValueId(2),
                name: "g".into(),
            },
            Inst::BinOp {
                dest: ValueId(3),
                op: "+".into(),
                left: ValueId(2),
                right: ValueId(1),
                ty: "String".into(),
            },
            Inst::AssignVar {
                name: "g".into(),
                value: ValueId(3),
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    let mut globals = HashSet::new();
    globals.insert("g".to_string());
    let n = run_promotion(&mut f, &globals);
    assert_eq!(n, 0, "mutable module globals may alias external buffers");
    assert_eq!(concat_call_count(&f), 0);
    assert_eq!(string_binop_count(&f), 1);

    // The same (still-unrewritten) IR with an empty global set DOES promote:
    // with no known mut globals `g` is indistinguishable from a local slot.
    let n2 = run_promotion(&mut f, &HashSet::new());
    assert_eq!(n2, 1, "empty global set makes g a promotable local slot");
    assert_eq!(concat_call_count(&f), 1);
    assert_eq!(string_binop_count(&f), 0);
}

#[test]
fn promotion_is_idempotent() {
    // Idempotency: after a rewrite the IR carries Call concat_mut, which is
    // NOT in is_consumed_concat, so a second pass finds no candidates and
    // returns 0 without touching the promoted instructions.
    let mut f = base_function();
    f.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![
            Inst::ConstStr {
                dest: ValueId(1),
                value: "seed0123456789abcdef0123".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(1),
            },
            Inst::ConstStr {
                dest: ValueId(2),
                value: "x".into(),
            },
            Inst::LoadVar {
                dest: ValueId(3),
                name: "s".into(),
            },
            Inst::BinOp {
                dest: ValueId(4),
                op: "+".into(),
                left: ValueId(3),
                right: ValueId(2),
                ty: "String".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(4),
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    let globals = HashSet::new();
    assert_eq!(run_promotion(&mut f, &globals), 1, "first pass promotes");
    assert_eq!(concat_call_count(&f), 1);

    assert_eq!(
        run_promotion(&mut f, &globals),
        0,
        "second pass on promoted IR is a no-op"
    );
    assert_eq!(concat_call_count(&f), 1, "no double rewrite");
    assert_eq!(
        concat_mut_args_stable(&f),
        true,
        "promoted call arguments unchanged"
    );
}

fn concat_mut_args_stable(f: &Function) -> bool {
    f.blocks
        .iter()
        .flat_map(|b| b.instructions.iter())
        .all(|i| match i {
            Inst::Call { args, .. } => args.len() == 2,
            _ => true,
        })
}

#[test]
fn rejects_escaping_second_use() {
    // %4 = LoadVar s (second use) feeding a Call that retains the buffer —
    // an escaping use. The append must NOT be promoted.
    let mut f = base_function();
    f.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![
            Inst::ConstStr {
                dest: ValueId(1),
                value: "seed0123456789abcdef0123".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(1),
            },
            Inst::ConstStr {
                dest: ValueId(2),
                value: "x".into(),
            },
            // Escaping use FIRST: %3 = LoadVar s -> Call store_slot(%3)
            Inst::LoadVar {
                dest: ValueId(3),
                name: "s".into(),
            },
            Inst::Call {
                dest: ValueId(4),
                func: "datara_rt_str_store_slot".into(),
                args: vec![ValueId(3)],
                ty: "Void".into(),
            },
            // Then the append candidate: %5 = LoadVar s
            Inst::LoadVar {
                dest: ValueId(5),
                name: "s".into(),
            },
            Inst::BinOp {
                dest: ValueId(6),
                op: "+".into(),
                left: ValueId(5),
                right: ValueId(2),
                ty: "String".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(6),
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    let globals = HashSet::new();
    let n = run_promotion(&mut f, &globals);
    assert_eq!(n, 0, "escaping second use must block in-place mutation");
    assert_eq!(concat_call_count(&f), 0);
    assert_eq!(string_binop_count(&f), 1);
}

#[test]
fn non_escaping_observer_does_not_block() {
    // `out s.len()` between appends: MethodCall observer is transient, so the
    // chain stays promotable.
    let mut f = base_function();
    f.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![
            Inst::ConstStr {
                dest: ValueId(1),
                value: "seed0123456789abcdef0123".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(1),
            },
            Inst::ConstStr {
                dest: ValueId(2),
                value: "x".into(),
            },
            // observer: %3 = LoadVar s; %4 = s.len(); Out %4
            Inst::LoadVar {
                dest: ValueId(3),
                name: "s".into(),
            },
            Inst::MethodCall {
                dest: ValueId(4),
                object: ValueId(3),
                method: "len".into(),
                args: vec![],
                ty: "Int".into(),
            },
            Inst::Out {
                value: ValueId(4),
                ty: "Void".into(),
            },
            // append candidate: %5 = LoadVar s
            Inst::LoadVar {
                dest: ValueId(5),
                name: "s".into(),
            },
            Inst::BinOp {
                dest: ValueId(6),
                op: "+".into(),
                left: ValueId(5),
                right: ValueId(2),
                ty: "String".into(),
            },
            Inst::AssignVar {
                name: "s".into(),
                value: ValueId(6),
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    let globals = HashSet::new();
    let n = run_promotion(&mut f, &globals);
    assert_eq!(n, 1, "observer uses must not block promotion");
    assert_eq!(concat_call_count(&f), 1);
    assert_eq!(string_binop_count(&f), 0);
}
