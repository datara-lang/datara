//! Inline-body lowering helpers (lambda bodies lowered IN PLACE under an
//! inline-return frame). Split out of lowering/mod.rs for the 60 KB
//! release size gate.

use super::Lowering;
use crate::ast::Expr;
use super::InlineRetFrame;
use crate::dmir::ir::{BasicBlockId, Inst, Terminator, ValueId};

impl<'a> Lowering<'a> {
    pub fn block_falls_through(&self, id: BasicBlockId) -> bool {
        if id.0 < self.current_blocks.len() && self.current_blocks[id.0].id == id {
            return matches!(
                self.current_blocks[id.0].terminator,
                Terminator::Unreachable
            );
        }
        self.current_blocks
            .iter()
            .find(|b| b.id == id)
            .map(|b| matches!(b.terminator, Terminator::Unreachable))
            .unwrap_or(false)
    }

    /// Lower a lambda body IN PLACE under an inline-return frame.
    ///
    /// A `return e` inside the body becomes "store e into a synthetic slot and
    /// branch to the join block" instead of returning from the enclosing
    /// function (the unsound old behavior silently swallowed the rest of the
    /// caller when a block-bodied lambda used `if { return x }`). This
    /// function opens the frame, lowers the body, then — if any inline return
    /// fired — commits the fall-through edge to the join block and loads the
    /// slot there as the lambda-call result. Without an inline return the
    /// fall-through value is returned directly (unchanged fast path).
    ///
    /// `restore` is called with the result being installed (and, when used,
    /// running inside the join block) so shadow-restore side effects land in
    /// the right block; its closure receives the innermost active block.
    pub(crate) fn lower_inline_body_with_returns(
        &mut self,
        body: &Expr,
        cur_block: &mut BasicBlockId,
        mut restore: impl FnMut(&mut Self, BasicBlockId),
    ) -> Option<ValueId> {
        self.inline_ret_counter += 1;
        let slot = format!("@lam_ret_{}", self.inline_ret_counter);
        self.inline_ret_frames.push(InlineRetFrame {
            ret_slot: slot.clone(),
            join: None,
            used: false,
        });
        let res = self.lower_expr(body, cur_block);
        let frame = self
            .inline_ret_frames
            .pop()
            .expect("inline-return frame stack unbalanced");
        if !frame.used {
            // No inline return: the plain previous semantics.
            restore(self, *cur_block);
            return res;
        }
        let join = frame
            .join
            .expect("frame.used with no join block — Stmt::Return always creates it");
        // Fall-through exits: bind the tail value to the slot and branch to
        // the join block. If the fall-through block is already terminated
        // (e.g. the last body statement itself returned through the frame,
        // or control passed through an inline return), it already owns its
        // own edge to the join block — leave it alone.
        if self.block_falls_through(*cur_block) {
            let b = self.get_block_mut(*cur_block);
            if let Some(v) = res {
                b.instructions.push(Inst::AssignVar {
                    name: slot.clone(),
                    value: v,
                });
            }
            b.terminator = Terminator::Branch {
                target: join,
                args: Vec::new(),
            };
        }
        // Continue in the join block: load the slot as the call result, then
        // restore the caller's bindings there.
        let out = self.next_val();
        {
            let b = self.get_block_mut(join);
            b.instructions.push(Inst::LoadVar { dest: out, name: slot });
        }
        *cur_block = join;
        restore(self, join);
        Some(out)
    }

    /// Wires a loop back-edge from `from` to `target`.
    ///
    /// The edge is only installed when the block still falls through. A body
    /// that ends in `return` already has `Terminator::Return`; overwriting it
    /// would silently discard the early return and produce an infinite loop.
    pub fn set_back_edge(&mut self, from: BasicBlockId, target: BasicBlockId) {
        if self.block_falls_through(from) {
            self.get_block_mut(from).terminator = Terminator::Branch {
                target,
                args: Vec::new(),
            };
        }
    }
}
