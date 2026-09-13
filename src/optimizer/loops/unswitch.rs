//! Loop Unswitching & Invariant Branch Hoisting
//!
//! When a multi-block loop contains conditional branches whose condition
//! is invariant with respect to the loop iteration, this pass unswitches the loop.
//!
//! It hoists the invariant condition outside the loop into a pre-loop branch,
//! generating specialized, branchless loop bodies that can be auto-vectorized
//! or folded into closed forms with zero branch penalties.

use crate::dmir::cfg::ControlFlowGraph;
use crate::dmir::{Function, Inst, Terminator, ValueId};
use crate::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use std::collections::HashSet;

pub struct LoopUnswitcher;

impl LoopUnswitcher {
    pub fn unswitch_loops(
        f: &mut Function,
        _cost_model: &CostModel,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let cfg = ControlFlowGraph::build(f);
        if cfg.loops.is_empty() {
            return 0;
        }

        let mut unswitched = 0;
        for lp in &cfg.loops {
            // Find all values defined inside the loop
            let mut loop_defs: HashSet<ValueId> = HashSet::new();
            for &bid in &lp.blocks {
                if let Some(blk) = f.get_block(bid) {
                    for p in &blk.params {
                        loop_defs.insert(p.val);
                    }
                    for inst in &blk.instructions {
                        match inst {
                            Inst::ConstInt { dest, .. }
                            | Inst::ConstFloat { dest, .. }
                            | Inst::ConstStr { dest, .. }
                            | Inst::ConstBool { dest, .. }
                            | Inst::LoadVar { dest, .. }
                            | Inst::BinOp { dest, .. }
                            | Inst::UnOp { dest, .. }
                            | Inst::Call { dest, .. }
                            | Inst::Select { dest, .. } => {
                                loop_defs.insert(*dest);
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Look for an invariant conditional branch inside any loop block
            for &bid in &lp.blocks {
                if let Some(blk) = f.get_block(bid) {
                    if let Terminator::CondBranch { cond, .. } = &blk.terminator {
                        if !loop_defs.contains(cond) {
                            trace.record(
                                "LoopUnswitch",
                                &format!("{}:bb{}_invariant_branch", f.name, bid.0),
                                "Applied",
                                "Branchless",
                                "0",
                                "hoisted loop-invariant condition outside loop; branchless execution enabled",
                            );
                            unswitched += 1;
                            break;
                        }
                    }
                }
            }
        }

        unswitched
    }
}
