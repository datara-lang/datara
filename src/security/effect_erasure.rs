//! Zero-Cost Capability & Effect Erasure for Datara & Forgen (v1.2.3)
//!
//! Statically verified security capabilities, effect tokens, and contract
//! assertions are erased during DMIR-to-backend lowering.
//! Guarantees strictly 0 bytes in emitted binaries and 0 CPU cycles at runtime.

use crate::dmir::{Function, Inst};
use crate::optimizer::cost_model::OptimizationDecisionTrace;

pub struct EffectErasure;

impl EffectErasure {
    /// Statically erases redundant runtime capability checks from a function
    /// that has already passed compile-time capability and effect verification.
    pub fn erase_verified_effects(
        f: &mut Function,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let mut erased_count = 0;

        for block in &mut f.blocks {
            let mut retained_instructions = Vec::with_capacity(block.instructions.len());

            for inst in block.instructions.drain(..) {
                let is_erasure_candidate = match &inst {
                    Inst::Call { func, .. } => {
                        func == "cap_require"
                            || func == "cap_grant"
                            || func == "cap_revoke"
                            || func == "datara_rt_cap_require"
                            || func == "datara_rt_cap_grant"
                            || func == "datara_rt_cap_revoke"
                    }
                    _ => false,
                };

                if is_erasure_candidate {
                    erased_count += 1;
                    trace.record(
                        "EffectErasure",
                        &format!("{}:cap_op", f.name),
                        "Applied",
                        "Erased static capability witness from machine code",
                        "0",
                        "Capability was statically verified at compile-time; zero runtime cost",
                    );
                } else {
                    retained_instructions.push(inst);
                }
            }

            block.instructions = retained_instructions;
        }

        erased_count
    }
}
