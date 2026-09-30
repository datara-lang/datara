//! Abstract range domain and induction-range proofs for the
//! overflow-elision pass (`--opt speed`). Split from `ovf_elide.rs` to honor
//! the v1.5.0 release file-size gate.
//!
//! `Range` is the abstract signed interval of one SSA value, `RState` its
//! fixpoint state (Bot = not yet derived, Top = unmodelable def),
//! `ProvenInduction`/`induction_range` extract the constant-bound trip facts
//! of one analyzable natural loop.

use crate::dmir::{Function, Inst, Terminator, ValueId};
use crate::optimizer::loops::LoopOptimizer;


/// Abstract signed range of one SSA value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Range {
    pub(super) lo: i64,
    pub(super) hi: i64,
}

impl Range {
    pub(super) fn const_range(c: i64) -> Range {
        Range { lo: c, hi: c }
    }

    /// Range of `a + b`. Saturates at i64 bounds instead of rejecting: a
    /// hull that already includes an i64 endpoint can never prove safety,
    /// so the caller keeps the trap. The corner arithmetic itself cannot
    /// overflow because it saturates.
    pub(super) fn add(self, other: Range) -> Range {
        Range {
            lo: self.lo.saturating_add(other.lo),
            hi: self.hi.saturating_add(other.hi),
        }
    }

    pub(super) fn neg(self) -> Range {
        Range {
            lo: self.hi.checked_neg().unwrap_or(i64::MIN),
            hi: self.lo.checked_neg().unwrap_or(i64::MAX),
        }
    }

    /// Range of `a - b`.
    pub(super) fn sub(self, other: Range) -> Range {
        self.add(other.neg())
    }

    /// Range of `a & b`. A bitwise AND never sets a bit that either operand
    /// lacks, so the result is bounded by BOTH operands: in particular a
    /// non-negative operand upper-bounds the result by itself. Negative
    /// operands still contribute a sound bound through two's-complement
    /// monotonicity (the result cannot exceed the non-negative operand's
    /// upper bound, and cannot go below the minimum representable value of
    /// the AND over the two hulls — modeled conservatively as 0 when either
    /// side is fully non-negative, i64::MIN otherwise).
    pub(super) fn bitand(self, other: Range) -> Range {
        let hi = self.hi.min(other.hi);
        let lo = if self.lo >= 0 || other.lo >= 0 {
            0
        } else {
            i64::MIN
        };
        Range { lo, hi: hi.max(lo) }
    }

    /// Range of `a | b`: an OR sets every bit either operand has, so the
    /// result is at least the max of both non-negative lower bounds and can
    /// reach the all-ones pattern when either operand's hull allows it. The
    /// safe hull: lo = min(lo), hi = -1 if either operand may be negative,
    /// else the OR of both upper bounds is an upper bound for the result
    /// when both operands are non-negative... For soundness with mixed
    /// signs we keep the union hull widened to full negative range.
    pub(super) fn bitor(self, other: Range) -> Range {
        let lo = self.lo.min(other.lo);
        let hi = if self.lo < 0 || other.lo < 0 {
            -1i64.max(self.hi.max(other.hi))
        } else {
            self.hi.max(other.hi)
        };
        Range { lo, hi }
    }

    /// Range of `a * b`. Takes the hull of the four corner products with
    /// saturating arithmetic. Sound for the elision direction: the true
    /// product range can only be narrower than the corner hull.
    pub(super) fn mul(self, other: Range) -> Range {
        let corners = [
            self.lo.saturating_mul(other.lo),
            self.lo.saturating_mul(other.hi),
            self.hi.saturating_mul(other.lo),
            self.hi.saturating_mul(other.hi),
        ];
        let lo = corners.iter().copied().min().unwrap_or(i64::MIN);
        let hi = corners.iter().copied().max().unwrap_or(i64::MAX);
        Range { lo, hi }
    }

    /// Hull (union) of two ranges — the sound join for a block param fed
    /// from several incoming edges.
    pub(super) fn union(self, other: Range) -> Range {
        Range {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    /// Range of `a ^ b`. XOR keeps only bit positions present in exactly one
    /// operand, so every result bit lives at or below the highest bit either
    /// operand may own. A width-free sound hull: each side of zero is bounded
    /// by (2×+1) the corresponding extreme of the operands — `2^q - 1 ≤
    /// 2·hi + 1` and `|negative result| ≤ 2^p ≤ |lo|` for the bit widths
    /// involved. Loose (an `& 63` mask after the xor collapses it), but sound
    /// for every operand pair and enough to keep mask-chains analyzable.
    pub(super) fn xor(self, other: Range) -> Range {
        let lo_src = self.lo.min(other.lo);
        let hi_src = self.hi.max(other.hi);
        let lo = if lo_src < 0 {
            lo_src
                .checked_mul(2)
                .and_then(|v| v.checked_add(1))
                .unwrap_or(i64::MIN)
        } else {
            0
        };
        let hi = if hi_src > 0 {
            hi_src
                .checked_mul(2)
                .and_then(|v| v.checked_add(1))
                .unwrap_or(i64::MAX)
        } else {
            0
        };
        Range { lo, hi }
    }
}

/// Abstract state of one SSA value in the range fixpoint. `Bot` is
/// transient ("not derived yet — the fixpoint will fill it, or its operand
/// chain degrades to sticky `Top`"); `Top` is the over-approximation for
/// defs the analysis cannot model (call results, loads, casts, non-Int
/// ops). Distinguishing them is what keeps loop-carried params sound:
/// joining a not-yet-computed edge arg must never poison the param with a
/// full hull that only-widen can never retract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum RState {
    Bot,
    Known(Range),
    Top,
}

/// Join of two abstract states: bottom is the identity, top is absorbing,
/// known ranges hull together. Monotone in both arguments.
pub(super) fn join_state(count: &mut u32, old: RState, s: RState) -> Option<RState> {
    let merged = match (old, s) {
        // Bot is the join identity: a not-yet-derived edge arg must never
        // poison an already-known range (the arg refines in a later round).
        (RState::Bot, _) => s,
        (_, RState::Bot) => return None,
        (RState::Top, _) | (_, RState::Top) => RState::Top,
        (RState::Known(x), RState::Known(y)) => RState::Known(x.union(y)),
    };
    if merged == old {
        return None;
    }
    *count += 1;
    if *count > 3 {
        Some(RState::Top)
    } else {
        Some(merged)
    }
}

/// A loop induction variable whose trip range was proven: on every body
/// iteration, `lo <= v <= hi`.
pub(super) struct ProvenInduction {
    /// Header parameter ValueId of the induction variable.
    pub(super) vid: ValueId,
    /// Inclusive body-entry range of the induction variable.
    pub(super) lo: i64,
    /// Largest value the header parameter can hold on ANY entry edge,
    /// including the final failing iteration: `hi + accumulated chain
    /// steps`. The latch can hand the bound itself back to the header, so
    /// uses outside the body must not assume the tight `hi`.
    pub(super) hi_edge: i64,
    /// Blocks that constitute this loop.
    pub(super) blocks: std::collections::HashSet<crate::dmir::BasicBlockId>,
    /// Header block id (for tracing).
    pub(super) header_id: crate::dmir::BasicBlockId,
}

/// Extracts the proven induction facts of one analyzable loop: condition
/// `i < B` / `i <= B` against a constant bound with a positive constant
/// step and a single latch, all entry edges passing the same constant
/// start. Returns the body-entry range `lo..=hi` of `i`.
pub(super) fn induction_range(f: &Function, lp: &crate::dmir::cfg::NaturalLoop) -> Option<ProvenInduction> {
    let dbg = std::env::var("DATARA_OVF_DEBUG").is_ok();
    let header = f.get_block(lp.header)?;
    let cond_vid = match &header.terminator {
        Terminator::CondBranch { cond, .. } => *cond,
        _ => {
            if dbg {
                eprintln!("[ovf-ind] bb{}: header terminator is not CondBranch", lp.header.0);
            }
            return None;
        }
    };
    let param_vids: Vec<ValueId> = header.params.iter().map(|p| p.val).collect();

    let mut found: Option<(ValueId, &str, i64)> = None;
    for inst in &header.instructions {
        if let Inst::BinOp {
            dest,
            op,
            left,
            right,
            ty,
        } = inst
        {
            if *dest != cond_vid || (ty != "Bool" && ty != "Int") {
                continue;
            }
            let (i_vid, bound_vid, normalized) = if param_vids.contains(left) {
                (*left, *right, op.as_str())
            } else if param_vids.contains(right) {
                (
                    *right,
                    *left,
                    match op.as_str() {
                        "<" => ">",
                        "<=" => ">=",
                        ">" => "<",
                        _ => "<=",
                    },
                )
            } else {
                continue;
            };
            if !matches!(normalized, "<" | "<=") {
                continue;
            }
            if let Some(b) = LoopOptimizer::const_expr_int_value(f, bound_vid) {
                found = Some((i_vid, normalized, b));
            }
        }
    }
    let (i_vid, op, bound) = found?;
    if dbg {
        eprintln!("[ovf-ind] bb{}: induction v{} op '{}' bound {}", lp.header.0, i_vid.0, op, bound);
    }
    if lp.back_edges.len() != 1 {
        return None;
    }

    let param_index = header.params.iter().position(|p| p.val == i_vid)?;
    let latch_id = lp.back_edges[0];
    let latch = f.get_block(latch_id)?;
    let next_vid = match &latch.terminator {
        Terminator::Branch { target, args } if *target == lp.header => {
            args.get(param_index).copied()?
        }
        _ => return None,
    };

    // Step must be a constant-step chain rooted at the induction parameter.
    // Factor-2 unrolling forwards multi-link latches (`k2 = k1 + S`,
    // `k1 = k + S`) and the unroller rewrites links to `wrapping_+`, so
    // walk the chain like `prove_loop` does, accepting checked and wrapping
    // forms alike (a wrapping link already lowers to the same plain `iadd`
    // an unchecked checked link lowers to).
    let mut chain: Vec<(ValueId, i64)> = Vec::new();
    let mut cursor = next_vid;
    while cursor != i_vid {
        if chain.len() >= 8 {
            return None;
        }
        let mut def = None;
        for inst in &latch.instructions {
            if let Inst::BinOp {
                dest,
                op: iop,
                left,
                right,
                ..
            } = inst
                && *dest == cursor
                && matches!(iop.as_str(), "+" | "wrapping_+" | "-" | "wrapping_-")
            {
                def = Some((iop.as_str(), *left, *right));
                break;
            }
        }
        let (iop, left, right) = def?;
        let (prev, delta) = match iop {
            "+" | "wrapping_+" => {
                if let Some(s) = LoopOptimizer::const_expr_int_value(f, right) {
                    (left, s)
                } else {
                    let s = LoopOptimizer::const_expr_int_value(f, left)?;
                    (right, s)
                }
            }
            _ => {
                let step = LoopOptimizer::const_expr_int_value(f, right)?;
                let d = step.checked_neg()?;
                (left, d)
            }
        };
        chain.push((cursor, delta));
        cursor = prev;
    }
    if chain.is_empty() {
        return None;
    }
    // The condition bounds the parameter in the travelled direction; a
    // non-positive step on any link means no statically bounded trip.
    if chain.iter().any(|&(_, d)| d <= 0) {
        return None;
    }

    // Entry values: every non-loop edge into the header must pass the same
    // compile-time constant for the induction slot.
    let mut start: Option<i64> = None;
    for b in &f.blocks {
        if lp.blocks.contains(&b.id) {
            continue;
        }
        let edge_args: Option<&Vec<ValueId>> = match &b.terminator {
            Terminator::Branch { target, args } if *target == lp.header => Some(args),
            Terminator::CondBranch {
                then_block,
                else_block,
                then_args,
                else_args,
                ..
            } => {
                if *then_block == lp.header {
                    Some(then_args)
                } else if *else_block == lp.header {
                    Some(else_args)
                } else {
                    None
                }
            }
            _ => None,
        };
        let Some(args) = edge_args else { continue };
        let &arg = args.get(param_index)?;
        let s = LoopOptimizer::const_expr_int_value(f, arg)?;
        start = match start {
            None => Some(s),
            Some(prev) if prev == s => Some(prev),
            _ => return None,
        };
    }
    let start = start?;
    if dbg {
        eprintln!("[ovf-ind] bb{}: start={} chain_deltas={:?}", lp.header.0, start, chain.iter().map(|(_, d)| d).collect::<Vec<_>>());
    }

    let hi = match op {
        "<" => bound.checked_sub(1)?,
        _ => bound,
    };
    if start > hi {
        return None;
    }
    // Every link runs only when its predecessors ran, so the chain extreme
    // accumulates all steps onto the loop-edge bound.
    let mut running = hi;
    for &(_, delta) in chain.iter().rev() {
        running = running.checked_add(delta)?;
    }
    Some(ProvenInduction {
        vid: i_vid,
        lo: start,
        hi_edge: running,
        blocks: lp.blocks.iter().copied().collect(),
        header_id: lp.header,
    })
}
