use forgen::dmir::{cfg::ControlFlowGraph, BasicBlock, BasicBlockId, BlockParam, Function, Inst, Terminator, ValueId};
use forgen::optimizer::loops::LoopOptimizer;
use forgen::optimizer::cost_model::OptimizationDecisionTrace;

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

/// fn main() { s = "abc"; i = 0; while i < 5 { s = s + "x"; i = i + 1 } out s.len() }
fn build_string_loop_fn() -> Function {
    let mut f = base_function();
    let b0 = BasicBlockId(0);
    let b1 = BasicBlockId(1);
    let b2 = BasicBlockId(2);
    let b3 = BasicBlockId(3);
    f.blocks.push(BasicBlock { id: b0, label: "entry".into(), params: vec![], instructions: vec![], terminator: Terminator::Unreachable });
    eprintln!("DBG entry_block={:?} b0={:?}", f.entry_block, b0);
    f.blocks.push(BasicBlock { id: b1, label: "hdr".into(), params: vec![], instructions: vec![], terminator: Terminator::Unreachable });
    f.blocks.push(BasicBlock { id: b2, label: "body".into(), params: vec![], instructions: vec![], terminator: Terminator::Unreachable });
    f.blocks.push(BasicBlock { id: b3, label: "exit".into(), params: vec![], instructions: vec![], terminator: Terminator::Unreachable });

    let mut next = 0usize;

    // block 0: s0 = "abc"; i0 = 0; n = 5; piece = "x"; branch -> b1 (i0)
    let v_s0 = ValueId(next); next += 1;
    f.get_block_mut(b0).unwrap().instructions.push(Inst::ConstStr { dest: v_s0, value: "abc".into() });
    let v_i0 = ValueId(next); next += 1;
    f.get_block_mut(b0).unwrap().instructions.push(Inst::ConstInt { dest: v_i0, value: 0 });
    let v_n = ValueId(next); next += 1;
    f.get_block_mut(b0).unwrap().instructions.push(Inst::ConstInt { dest: v_n, value: 5 });
    let v_piece = ValueId(next); next += 1;
    f.get_block_mut(b0).unwrap().instructions.push(Inst::ConstStr { dest: v_piece, value: "x".into() });
    f.get_block_mut(b0).unwrap().terminator = Terminator::Branch { target: b1, args: vec![v_i0, v_s0] };

    // block 1: params (i: Int, s: String); cond = i < n; br b2 | b3
    let p_i = ValueId(next); next += 1;
    let p_s = ValueId(next); next += 1;
    {
        let blk = f.get_block_mut(b1).unwrap();
        blk.params.push(BlockParam { val: p_i, ty: "Int".into(), name: Some("i".into()) });
        blk.params.push(BlockParam { val: p_s, ty: "String".into(), name: Some("s".into()) });
        let cond = ValueId(next); next += 1;
        blk.instructions.push(Inst::BinOp { dest: cond, op: "<".into(), left: p_i, right: v_n, ty: "Int".into() });
        blk.terminator = Terminator::CondBranch { cond, then_block: b2, then_args: vec![], else_block: b3, else_args: vec![] };
    }

    // block 2: s2 = concat(p_s, piece); i2 = p_i + 1; br b1 (i2, s2)
    let s2 = ValueId(next); next += 1;
    f.get_block_mut(b2).unwrap().instructions.push(Inst::Call { dest: s2, func: "datara_rt_str_concat".into(), args: vec![p_s, v_piece], ty: "String".into() });
    let one = ValueId(next); next += 1;
    f.get_block_mut(b2).unwrap().instructions.push(Inst::ConstInt { dest: one, value: 1 });
    let i2 = ValueId(next); next += 1;
    f.get_block_mut(b2).unwrap().instructions.push(Inst::BinOp { dest: i2, op: "wrapping_+".into(), left: p_i, right: one, ty: "Int".into() });
    f.get_block_mut(b2).unwrap().terminator = Terminator::Branch { target: b1, args: vec![i2, s2] };

    // block 3 (exit): out p_s.len() — the fold rewrites p_s to s_final
    let l = ValueId(next); next += 1;
    f.get_block_mut(b3).unwrap().instructions.push(Inst::MethodCall { dest: l, object: p_s, method: "len".into(), args: vec![], ty: "Int".into() });
    f.get_block_mut(b3).unwrap().instructions.push(Inst::Out { value: l, ty: "Int".into() });
    f.get_block_mut(b3).unwrap().terminator = Terminator::Return { value: None };
    f
}

#[test]
fn debug_string_fold_match() {
    let mut f = build_string_loop_fn();
    let mut trace = OptimizationDecisionTrace::new();
    let n = LoopOptimizer::fold_string_build_loops_for_test(&mut f, &mut trace);
    eprintln!("fold result: {}", n);
    // CFG sanity probe: does the natural-loop detector even see the loop?
    let cfg = ControlFlowGraph::build(&f);
    eprintln!("cfg loops: {}", cfg.loops.len());
    for lp in &cfg.loops {
        eprintln!("loop header={:?} back_edges={:?} blocks={:?}", lp.header, lp.back_edges, lp.blocks);
    }
    for r in &trace.records {
        eprintln!("trace: {:?}", r);
    }
    assert_eq!(n, 1, "string loop fold should apply");
}
