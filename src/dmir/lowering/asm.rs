//! v1.4.0: lowering of the structured `asm { ... }` safe subset.
//!
//! Each parsed line expands to plain DMIR ops (`LoadVar` / `ConstInt` /
//! `BinOp` / `AssignVar`); the four x86-64 GP scratch registers are
//! simulated as DMIR temporaries and the real register assignment stays
//! with the backend. This is a structured subset that compiles to native
//! ops through the normal pipeline, NOT a raw machine-code encoder.
//!
//! Malformed lines never reach here (the `asm_safe` driver validation
//! rejects them with E1402, including register reads before writes), so
//! unsupported lines and unset-register reads are skipped defensively.

use crate::ast::{AsmLine, AsmOp, AsmOperand};
use crate::dmir::ir::{BasicBlockId, Inst, ValueId};

use super::Lowering;

/// Expands one structured asm block into the current block.
///
/// Returns the same `(block, value)` shape the statement lowering arms
/// produce: an asm block is a statement with no value.
pub(crate) fn lower_structured_asm(
    lowering: &mut Lowering,
    structured: &[AsmLine],
    cur_block: BasicBlockId,
) -> (BasicBlockId, Option<ValueId>) {
    let mut regs: [Option<ValueId>; 4] = [None; 4];
    for line in structured {
        let AsmLine::Inst { op, dst, src, .. } = line else {
            continue;
        };
        match op {
            AsmOp::Mov => match &dst {
                AsmOperand::Reg(r) => {
                    if let Some(v) = lower_asm_read(lowering, src, &mut regs, cur_block) {
                        regs[r.slot()] = Some(v);
                    }
                }
                AsmOperand::Var(name, _) => {
                    if let Some(v) = lower_asm_read(lowering, src, &mut regs, cur_block) {
                        lowering
                            .get_block_mut(cur_block)
                            .instructions
                            .push(Inst::AssignVar {
                                name: name.clone(),
                                value: v,
                            });
                    }
                }
                AsmOperand::Imm(..) => {}
            },
            AsmOp::Add | AsmOp::Sub => {
                let lhs = lower_asm_read(lowering, dst, &mut regs, cur_block);
                if let Some(l) = lhs
                    && let Some(rhs) = lower_asm_read(lowering, src, &mut regs, cur_block)
                {
                    let res = lowering.next_val();
                    lowering
                        .get_block_mut(cur_block)
                        .instructions
                        .push(Inst::BinOp {
                            dest: res,
                            op: if *op == AsmOp::Add { "+" } else { "-" }.into(),
                            left: l,
                            right: rhs,
                            ty: "Int".into(),
                        });
                    match &dst {
                        AsmOperand::Reg(r) => regs[r.slot()] = Some(res),
                        AsmOperand::Var(name, _) => {
                            lowering
                                .get_block_mut(cur_block)
                                .instructions
                                .push(Inst::AssignVar {
                                    name: name.clone(),
                                    value: res,
                                });
                        }
                        AsmOperand::Imm(..) => {}
                    }
                }
            }
        }
    }
    (cur_block, None)
}

/// Reads one structured asm operand into a DMIR value. Registers resolve to
/// the value they currently hold (`None` when unset — the validation pass
/// rejects those with E1402 before lowering runs); variables load through
/// `LoadVar`; immediates are materialized as `ConstInt`.
fn lower_asm_read(
    lowering: &mut Lowering,
    opnd: &AsmOperand,
    regs: &mut [Option<ValueId>; 4],
    cur_block: BasicBlockId,
) -> Option<ValueId> {
    match opnd {
        AsmOperand::Reg(r) => regs[r.slot()],
        AsmOperand::Var(name, _) => {
            let v = lowering.next_val();
            lowering
                .get_block_mut(cur_block)
                .instructions
                .push(Inst::LoadVar {
                    dest: v,
                    name: name.clone(),
                });
            Some(v)
        }
        AsmOperand::Imm(n) => {
            let v = lowering.next_val();
            lowering
                .get_block_mut(cur_block)
                .instructions
                .push(Inst::ConstInt { dest: v, value: *n });
            Some(v)
        }
    }
}
