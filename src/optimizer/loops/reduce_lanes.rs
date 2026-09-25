//! FP-reduction lane splitting for countable float loops.
//!
//! Companion pass of `engine_v2::unroll_peel_loops`: after a 2x chain
//! unroll of a float reduction the whole reduction still lives on ONE
//! serial fadd chain, which runs at fadd latency and gains no
//! instruction-level parallelism. This pass splits the two unrolled
//! copies onto two independent accumulator lanes (exactly what a 2-lane
//! vectorized reduction does); the loop exit combines them once.
//!
//! Soundness: both lanes sum the same pairs in the same per-lane order;
//! combining once at the end is exactly what a 2-lane vectorized
//! reduction does (PLAN P3 acceptance allows the combined summation
//! tree). Every step is guarded; any unproven precondition makes the
//! pass a no-op.

#![allow(clippy::all)]

use crate::dmir::{BasicBlockId, BlockParam, Function, Inst, Terminator, ValueId};
use crate::optimizer::cost_model::OptimizationDecisionTrace;
use std::collections::{HashMap, HashSet};

pub(crate) fn split_float_reduction_lanes(
    f: &mut Function,
    loop_blocks: &HashSet<BasicBlockId>,
    header_id: BasicBlockId,
    body_id: BasicBlockId,
    fresh: &mut usize,
    trace: &mut OptimizationDecisionTrace,
) {
    // --- snapshot header & body before any mutation ---
    let Some(header) = f.get_block(header_id) else {
        return;
    };
    let header_params: Vec<ValueId> = header.params.iter().map(|p| p.val).collect();
    let exit_id = match &header.terminator {
        Terminator::CondBranch { else_block, .. } => *else_block,
        _ => return,
    };
    let body_snap = match f.get_block(body_id) {
        Some(b) => b.clone(),
        None => return,
    };

    // Back-edge args of the (single) latch.
    let back_args: Vec<ValueId> = match &body_snap.terminator {
        Terminator::Branch { args, .. } => args.clone(),
        Terminator::CondBranch {
            then_args,
            else_args,
            ..
        } => then_args.iter().chain(else_args.iter()).copied().collect(),
        _ => return,
    };

    // --- locate the reduction chain left by the chained 2x unroll.
    // After the unroll the back edge carries ONLY the SECOND add's
    // result, so walk the chain backwards:
    //   final = float add/fma whose dest the back edge carries;
    //   prev  = final's accumulator operand (BinOp left / fma args[2]);
    //   first = float add/fma whose dest == prev and whose own
    //           accumulator operand is a header param.
    // Two shapes are recognized: plain BinOp("+"|"wrapping_+") and
    // Call fma(a, b, acc) (the lowering emits fma for `acc = acc + a*b`).
    let mut final_link: Option<(ValueId, ValueId, bool)> = None; // (dest, acc_operand, is_fma)
    for inst in &body_snap.instructions {
        match inst {
            Inst::BinOp {
                dest,
                op,
                left,
                ty,
                ..
            } => {
                if (op == "+" || op == "wrapping_+")
                    && (ty == "Float" || ty == "f64")
                    && back_args.contains(dest)
                {
                    final_link = Some((*dest, *left, false));
                }
            }
            Inst::Call {
                dest,
                func,
                args,
                ty,
            } => {
                if (func == "fma" || func == "datara_rt_fma" || func == "datara_rt_fmaf")
                    && args.len() == 3
                    && (ty == "Float" || ty == "f64")
                    && back_args.contains(dest)
                {
                    final_link = Some((*dest, args[2], true));
                }
            }
            _ => {}
        }
    }
    let Some((s2_vid, prev_acc, is_fma)) = final_link else {
        return;
    };

    // The preceding link must be the same shape, and its accumulator
    // operand must be a header param (the real reduction seed).
    let mut first_link: Option<(ValueId, ValueId)> = None; // (dest, acc_param)
    for inst in &body_snap.instructions {
        match inst {
            Inst::BinOp {
                dest,
                op,
                left,
                ty,
                ..
            } => {
                if !is_fma
                    && *dest == prev_acc
                    && (op == "+" || op == "wrapping_+")
                    && (ty == "Float" || ty == "f64")
                    && header_params.contains(left)
                {
                    first_link = Some((*dest, *left));
                }
            }
            Inst::Call {
                dest,
                func,
                args,
                ty,
            } => {
                if is_fma
                    && *dest == prev_acc
                    && (func == "fma" || func == "datara_rt_fma" || func == "datara_rt_fmaf")
                    && args.len() == 3
                    && (ty == "Float" || ty == "f64")
                    && header_params.contains(&args[2])
                {
                    first_link = Some((*dest, args[2]));
                }
            }
            _ => {}
        }
    }
    if std::env::var("DATARA_NO_LANE_SPLIT").is_ok() { return; }
    let Some((s1_vid, acc_vid)) = first_link else {
        // Not a 2-link chain: either an un-unrolled single add (its acc
        // operand is the param itself, no defining inst) or an
        // unrecognized shape — leave the loop alone.
        return;
    };
    let lane1_is_fma = is_fma;

    // Guard: the exit block must not forward acc through a branch
    // terminator to a block this pass does not rewrite. Returning the
    // accumulated value from the function is fine (handled below).
    if let Some(eb) = f.get_block(exit_id) {
        let forwards_acc = match &eb.terminator {
            Terminator::Branch { args, .. } => args.contains(&acc_vid),
            Terminator::CondBranch {
                then_args,
                else_args,
                ..
            } => then_args.contains(&acc_vid) || else_args.contains(&acc_vid),
            _ => false,
        };
        if forwards_acc {
            return;
        }
    }

    // --- fresh ids ---
    let acc2 = ValueId(*fresh);
    *fresh += 1;
    let s1_new = ValueId(*fresh);
    *fresh += 1;
    let s2_new = ValueId(*fresh);
    *fresh += 1;
    let sum_final = ValueId(*fresh);
    *fresh += 1;
    let acc2_exit = ValueId(*fresh);
    *fresh += 1;
    let acc2_init = ValueId(*fresh);
    *fresh += 1;

    // --- preheader edges: append the initial 0.0 for acc2 ---
    let mut preheader_ids: Vec<BasicBlockId> = Vec::new();
    for b in &f.blocks {
        if loop_blocks.contains(&b.id) {
            continue;
        }
        let feeds_header = match &b.terminator {
            Terminator::Branch { target, .. } => *target == header_id,
            Terminator::CondBranch {
                then_block,
                else_block,
                ..
            } => *then_block == header_id || *else_block == header_id,
            _ => false,
        };
        if feeds_header {
            preheader_ids.push(b.id);
        }
    }
    if preheader_ids.is_empty() {
        return; // unreachable loop; leave untouched
    }
    {
        let zero = Inst::ConstFloat {
            dest: acc2_init,
            value: 0.0,
        };
        for pid in &preheader_ids {
            let Some(pb) = f.get_block_mut(*pid) else {
                continue;
            };
            if std::env::var("DATARA_DEBUG_VERIF").is_ok() {
                eprintln!("DBG preheader {} existing defs of acc2={}: {:?}", pid.0, acc2.0,
                    pb.instructions.iter().filter_map(|i| visit_dest_of(i)).collect::<Vec<_>>());
            }
            pb.instructions.push(zero.clone());
            match &mut pb.terminator {
                Terminator::Branch { args, .. } => args.push(acc2_init),
                Terminator::CondBranch {
                    then_block,
                    then_args,
                    else_block,
                    else_args,
                    ..
                } => {
                    if *then_block == header_id {
                        then_args.push(acc2_init);
                    }
                    if *else_block == header_id {
                        else_args.push(acc2_init);
                    }
                }
                _ => {}
            }
        }
    }

    // --- rewrite the body: lane 1 keeps its add, lane 2 re-binds to
    //     acc2, so lane 1 no longer feeds lane 2 ---
    {
        let body = match f.get_block_mut(body_id) {
            Some(b) => b,
            None => return,
        };
        let mut new_insts: Vec<Inst> = Vec::with_capacity(body.instructions.len());
        for inst in body.instructions.drain(..) {
            match inst {
                Inst::BinOp {
                    dest,
                    op,
                    left,
                    right,
                    ty,
                } if !lane1_is_fma && dest == s1_vid => {
                    // lane 1 (BinOp shape): s1_new = acc + c1 (operands
                    // unchanged — acc keeps its original ValueId)
                    new_insts.push(Inst::BinOp {
                        dest: s1_new,
                        op,
                        left,
                        right,
                        ty,
                    });
                }
                Inst::BinOp {
                    dest,
                    op,
                    left: _,
                    right,
                    ty,
                } if !lane1_is_fma && dest == s2_vid => {
                    // lane 2 (BinOp shape): s2_new = acc2 + c2
                    new_insts.push(Inst::BinOp {
                        dest: s2_new,
                        op,
                        left: acc2,
                        right,
                        ty,
                    });
                }
                Inst::Call {
                    dest,
                    func,
                    args,
                    ty,
                } if lane1_is_fma && dest == s1_vid => {
                    // lane 1 (fma shape): keep s1 = fma(a, b, acc) — the
                    // 3rd arg is the lane-1 accumulator, unchanged.
                    new_insts.push(Inst::Call {
                        dest: s1_new,
                        func,
                        args,
                        ty,
                    });
                }
                Inst::Call {
                    dest,
                    func,
                    args,
                    ty,
                } if lane1_is_fma && dest == s2_vid && args.len() == 3 => {
                    // lane 2 (fma shape): s2_new = fma(a, b, acc2) — the
                    // 3rd arg re-binds to the second accumulator lane.
                    let mut new_args = args.clone();
                    new_args[2] = acc2;
                    new_insts.push(Inst::Call {
                        dest: s2_new,
                        func,
                        args: new_args,
                        ty,
                    });
                }
                other => new_insts.push(other),
            }
        }
        body.instructions = new_insts;

        // Back edge: acc's slot gets lane 1's final (s1_new); lane 2's
        // final (s2_new) is appended as the new acc2 slot argument.
        // Every other back-edge arg (induction etc.) is untouched.
        match &mut body.terminator {
            Terminator::Branch { args, .. } => {
                for a in args.iter_mut() {
                    if *a == s2_vid {
                        *a = s1_new;
                    }
                }
                args.push(s2_new);
            }
            Terminator::CondBranch {
                then_block,
                then_args,
                else_block,
                else_args,
                ..
            } => {
                let patch_args = |args: &mut Vec<ValueId>| {
                    for a in args.iter_mut() {
                        if *a == s2_vid {
                            *a = s1_new;
                        }
                    }
                };
                if *then_block == header_id {
                    patch_args(then_args);
                    then_args.push(s2_new);
                }
                if *else_block == header_id {
                    patch_args(else_args);
                    else_args.push(s2_new);
                }
            }
            _ => return,
        }
    }

    // --- header: append the acc2 block param and feed it to the exit ---
    {
        let header = match f.get_block_mut(header_id) {
            Some(b) => b,
            None => return,
        };
        header.params.push(BlockParam {
            val: acc2,
            ty: "Float".to_string(),
            name: None,
        });
        match &mut header.terminator {
            Terminator::CondBranch { else_args, .. } => else_args.push(acc2),
            _ => return,
        }
    }

    // --- exit block: receive acc2, combine the lanes once, and rewrite
    //     every use of acc (incl. `return acc`) to the combined value ---
    {
        let exit = match f.get_block_mut(exit_id) {
            Some(b) => b,
            None => return,
        };
        if exit.params.iter().any(|p| p.val == acc2_exit) {
            return; // already split by a previous invocation; do not corrupt
        }
        exit.params.push(BlockParam {
            val: acc2_exit,
            ty: "Float".to_string(),
            name: None,
        });
        let mut new_insts: Vec<Inst> = Vec::with_capacity(exit.instructions.len() + 1);
        new_insts.push(Inst::BinOp {
            dest: sum_final,
            op: "+".to_string(),
            left: acc_vid,
            right: acc2_exit,
            ty: "Float".to_string(),
        });
        let mut v_map_rw: HashMap<ValueId, ValueId> = HashMap::new();
        v_map_rw.insert(acc_vid, sum_final);
        for mut inst in exit.instructions.drain(..) {
            // Rename only USES of acc (→ sum_final). Exit-block defs keep
            // their ValueIds: other blocks reference them via block
            // params/branch args, and renaming would orphan those uses.
            match &mut inst {
                Inst::BinOp { left, right, .. } => {
                    if *left == acc_vid { *left = sum_final; }
                    if *right == acc_vid { *right = sum_final; }
                }
                Inst::Out { value, .. } | Inst::Err { value } => {
                    if *value == acc_vid { *value = sum_final; }
                }
                Inst::AssignVar { value, .. } => {
                    if *value == acc_vid { *value = sum_final; }
                }
                Inst::Call { args, .. } => {
                    for a in args.iter_mut() {
                        if *a == acc_vid { *a = sum_final; }
                    }
                }
                Inst::MethodCall { object, args, .. } => {
                    if *object == acc_vid { *object = sum_final; }
                    for a in args.iter_mut() {
                        if *a == acc_vid { *a = sum_final; }
                    }
                }
                Inst::UnOp { operand, .. } => {
                    if *operand == acc_vid { *operand = sum_final; }
                }
                Inst::Cast { value, .. } => {
                    if *value == acc_vid { *value = sum_final; }
                }
                Inst::Select { cond, then_val, else_val, .. } => {
                    if *cond == acc_vid { *cond = sum_final; }
                    if *then_val == acc_vid { *then_val = sum_final; }
                    if *else_val == acc_vid { *else_val = sum_final; }
                }
                Inst::SetField { value, .. } => {
                    if *value == acc_vid { *value = sum_final; }
                }
                Inst::GetField { object, .. } => {
                    if *object == acc_vid { *object = sum_final; }
                }
                Inst::Return { value: Some(v) } => {
                    if *v == acc_vid { *v = sum_final; }
                }
                _ => {}
            }
            new_insts.push(inst);
        }
        exit.instructions = new_insts;
        if let Terminator::Return { value: Some(v) } = &mut exit.terminator {
            if *v == acc_vid {
                *v = sum_final;
            }
        }
    }

    trace.record(
        "LoopUnroll",
        &format!("{}:bb{}_fp_lane_split", f.name, header_id.0),
        "Applied",
        "Restores instruction-level parallelism killed by the serial fadd chain (matches a 2-lane vectorized reduction)",
        "one extra F64 param + one final fadd",
        "FP reduction split onto two independent accumulator lanes; per-lane order preserved",
    );
}

/// Returns the destination ValueId of constant-defining instructions.
fn visit_dest_of(inst: &Inst) -> Option<ValueId> {
    match inst {
        Inst::ConstInt { dest, .. } | Inst::ConstFloat { dest, .. } => Some(*dest),
        _ => None,
    }
}
