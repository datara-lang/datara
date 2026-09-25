//! v1.3.4 tiered overflow-check elision for provably bounded induction
//! variables (`--opt speed` only).
//!
//! # What this pass proves
//!
//! Datara integer `+`, `-` and `*` trap on overflow (the Cranelift backend
//! emits `sadd_overflow` / `ssub_overflow` / `smul_overflow` followed by
//! `trapnz(INTEGER_OVERFLOW)`, see `codegen/cranelift/backend/inst_binop.rs`).
//! Those traps are the language's correctness gate and stay ON for every
//! user-visible computation.
//!
//! For a loop's *induction variable increment* the reachable value range is
//! not a matter of guessing: when the loop condition compares the induction
//! variable against a compile-time constant bound and the step is a
//! compile-time constant, the value the increment computes is bounded by
//! (bound +/- 1) + step. If that constant arithmetic itself does not
//! overflow `i64`, the increment cannot trap, and the backend may emit the
//! plain (`iadd`/`isub`/`imul`) form for exactly that one instruction.
//!
//! # Exact conditions required to mark a `BinOp` unchecked
//!
//! All of the following must hold for the increment `i_next = i OP step`
//! (where `i` is a header block parameter), otherwise the pass leaves the
//! instruction alone:
//!
//! 1. The loop is a natural loop detected by `ControlFlowGraph` whose
//!    header is not the function entry block.
//! 2. The header's `CondBranch` condition is a comparison
//!    (`<`, `<=`, `>`, `>=`) of the header parameter `i` against a value
//!    `bound` that resolves to a compile-time constant `B`
//!    (`LoopOptimizer::const_expr_int_value`, which uses checked arithmetic
//!    so a wrapped expression never counts as a constant). A value that
//!    resolves to a compile-time constant cannot vary per iteration, so no
//!    separate "defined outside the loop" check is needed — and the bound
//!    literal typically lives inside the header block, which a naive
//!    loop-defs scan would misread as loop-varying.
//! 3. The loop has exactly one back edge, and that latch block's terminator
//!    forwards `i_next` into the header parameter slot of `i`.
//! 4. Inside that latch, `i_next` is a chain of `+`/`-` `BinOp` links (at
//!    most 8) rooted at `i`: each link adds or subtracts a compile-time
//!    constant step onto `i` itself or onto the previous link. A single link
//!    is the classic `i_next = i OP S`; a factor-2 unrolled latch forwards
//!    `i_next2 = i_next1 + S` with `i_next1 = i + S`, and every link of that
//!    chain is proved and marked.
//! 5. Each link's step points in the direction the condition travels:
//!    `<` / `<=` need `delta > 0`, `>` / `>=` need `delta < 0`, and
//!    `delta == 0` is accepted (adding zero never overflows). A condition
//!    and a step that disagree (`while i < n { i -= 1 }`) describe a loop
//!    with no static bound in the travelled direction, so it is refused.
//! 6. The constant bound arithmetic proves the increment cannot overflow:
//!
//!    | condition | increment runs while | extreme result | proof |
//!    |---|---|---|---|
//!    | `i <  B`  | `i <= B - 1` | `(B - 1) + delta` | `checked_sub(1)` then `checked_add(delta)` |
//!    | `i <= B`  | `i <= B`     | `B + delta`       | `checked_add(delta)` |
//!    | `i >  B`  | `i >= B + 1` | `(B + 1) + delta` | `checked_add(1)` then `checked_add(delta)` |
//!    | `i >= B`  | `i >= B`     | `B + delta`       | `checked_add(delta)` |
//!
//!    Any `None` from the checked arithmetic (including the `B - 1` /
//!    `B + 1` edges at `i64::MIN` / `i64::MAX`) rejects the loop.
//!
//! # What is deliberately NOT elided
//!
//! * **User body arithmetic.** A `sum = sum + x` or `acc * 2` inside the
//!   loop keeps its overflow trap even under `--opt speed`: the pass only
//!   ever marks the single induction-increment instruction, because that is
//!   the only one whose operand domain (`i` inside the loop) is pinned by
//!   the loop bound.
//! * **Anything outside a loop.** No marking happens for straight-line code.
//! * **`unsafe(justification:)` blocks** already compile without checks and
//!   are untouched by this pass.
//! * **The default tier.** The pass only runs when the optimizer tier is
//!   `OptTier::Speed`.
//!
//! The pass is *marking-only*: it never rewrites, inserts or removes an
//! instruction. The proof is attached to the `dest` `ValueId` of the
//! increment in `Function::proven_no_overflow`, and the backend consults it
//! when compiling that exact instruction. Because the marking pass runs
//! last, after every IR-reshaping pass, the ids still denote the
//! instructions the proof covered.
//!
//! # Range-guided body arithmetic (v1.4.7)
//!
//! In addition to induction increments, the pass proves abstract value
//! ranges for Int `+`/`-`/`*` *inside* loops whose induction variable is
//! bounded: ranges are seeded from constants and every proven induction
//! range, propagated over ALL blocks of the function to one global
//! fixpoint (outer roots like `i * N` in preheaders feed inner hot bodies
//! through copies and header params), and a BinOp is marked unchecked when
//! its converged result range fits strictly inside i64.
//! This covers the address arithmetic of matmul-style loops (`k * N + j`
//! with `k, j < N`), whose overflow traps cost real branches in the hot
//! body. Soundness: marking only when the proven range fits is one-sided —
//! an operand with an unknown range degrades to the full i64 hull, which
//! can never fit, so the trap is kept.

use crate::dmir::cfg::ControlFlowGraph;
use crate::dmir::{Function, Inst, Module, Terminator, ValueId};
use crate::optimizer::cost_model::OptimizationDecisionTrace;
use crate::optimizer::loops::LoopOptimizer;
use std::collections::HashMap;

/// Marks every provably non-overflowing induction increment in `module`.
///
/// Returns the number of increment instructions marked as safe. A return
/// value of `0` means no loop met every condition in the module docs — the
/// behavior of the program is then bit-identical to the default tier.
pub fn elide_induction_overflow(
    module: &mut Module,
    trace: &mut OptimizationDecisionTrace,
) -> usize {
    let mut names: Vec<String> = module.functions.keys().cloned().collect();
    names.sort();

    let mut total = 0usize;
    for name in &names {
        let (marked, proofs, rejections) = match module.functions.get_mut(name) {
            Some(f) => prove_induction_increments(f),
            None => (0, Vec::new(), Vec::new()),
        };
        for proof in proofs {
            trace.record(
                "OverflowElision",
                name,
                "Applied",
                "induction increment compiled unchecked",
                "None (proof-only marking)",
                &proof,
            );
        }
        for reason in rejections {
            trace.record("OverflowElision", name, "Rejected", "None", "None", &reason);
        }
        total += marked;
    }
    total
}

/// Proves and marks the induction increments of one function, plus the
/// range-guided body-arithmetic elision (see module docs, v1.4.6).
///
/// Returns `(marked_count, proof_descriptions, rejection_reasons)`.
/// Rejection reasons are recorded in the decision trace so a missed
/// optimization is visible rather than silent.
fn prove_induction_increments(f: &mut Function) -> (usize, Vec<String>, Vec<String>) {
    let cfg = ControlFlowGraph::build(f);
    let mut marked = 0usize;
    let mut plans: Vec<(ValueId, String)> = Vec::new();
    let mut rejections: Vec<String> = Vec::new();

    for lp in &cfg.loops {
        if lp.header == f.entry_block {
            continue;
        }
        match prove_loop(f, lp) {
            Ok(None) => {}
            Ok(Some((marked_vids, proof))) => {
                marked += marked_vids.len();
                for vid in marked_vids {
                    plans.push((vid, proof.clone()));
                }
            }
            Err(reason) => rejections.push(reason),
        }
    }

    for (vid, _proof) in &plans {
        f.proven_no_overflow.insert(*vid);
    }
    let mut proofs = plans.into_iter().map(|(_, p)| p).collect::<Vec<_>>();

    // Range-guided body-arithmetic elision: inside a loop whose induction
    // variable is provably bounded, mark every Int `+`/`-`/`*` whose
    // abstract range provably stays inside i64. This is the address-
    // arithmetic case of matmul-style loops (`k * N + j` with `k, j < N`).
    let (body_marked, body_proofs) = prove_body_arithmetic(f, &cfg);
    if body_marked > 0 {
        marked += body_marked;
        proofs.extend(body_proofs);
    }

    (marked, proofs, rejections)
}

/// The comparison of a header parameter against a constant bound,
/// normalized so the induction variable is the left operand.
struct BoundCheck {
    /// `<`, `<=`, `>` or `>=`, normalized relative to `i`.
    op: String,
    /// Compile-time value of the bound.
    bound: i64,
    /// `true` when the comparison was written with the bound on the left
    /// (`while n > i`) and therefore mirrored.
    mirrored: bool,
}

/// Tries to prove that the single induction increment of `lp` cannot
/// overflow. `Ok(None)` means the loop simply does not have the analyzable
/// shape (silent skip); `Err` carries a human-readable rejection reason for
/// a loop that *looks* analyzable but fails a soundness condition.
fn prove_loop(
    f: &Function,
    lp: &crate::dmir::cfg::NaturalLoop,
) -> Result<Option<(Vec<ValueId>, String)>, String> {
    let header = match f.get_block(lp.header) {
        Some(b) => b,
        None => return Ok(None),
    };

    // (2) The header must end in a conditional branch whose condition is a
    // comparison involving one of the header's own parameters.
    let cond_vid = match &header.terminator {
        Terminator::CondBranch { cond, .. } => *cond,
        _ => return Ok(None),
    };

    let param_vids: Vec<ValueId> = header.params.iter().map(|p| p.val).collect();

    // Locate the comparison instruction in the header.
    let mut found: Option<(ValueId, BoundCheck)> = None;
    for inst in &header.instructions {
        let (dest, op, left, right, ty) = match inst {
            Inst::BinOp {
                dest,
                op,
                left,
                right,
                ty,
            } => (*dest, op, *left, *right, ty),
            _ => continue,
        };
        if dest != cond_vid || (ty != "Bool" && ty != "Int") {
            continue;
        }
        if !matches!(op.as_str(), "<" | "<=" | ">" | ">=") {
            continue;
        }
        // Either operand order: `i < n` / `n > i`. The operator is mirrored
        // so the direction is always expressed relative to `i`.
        let (i_vid, bound_vid, mut check) = if param_vids.contains(&left) {
            (
                left,
                right,
                BoundCheck {
                    op: op.clone(),
                    bound: 0,
                    mirrored: false,
                },
            )
        } else if param_vids.contains(&right) {
            let mirrored_op = match op.as_str() {
                "<" => ">",
                "<=" => ">=",
                ">" => "<",
                _ => "<=",
            };
            (
                right,
                left,
                BoundCheck {
                    op: mirrored_op.to_string(),
                    bound: 0,
                    mirrored: true,
                },
            )
        } else {
            continue;
        };
        // No "defined inside the loop" check for the bound: it is accepted
        // only when `const_expr_int_value` resolves it to a compile-time
        // constant, and a value with a static constant result cannot vary
        // per iteration. (The literal typically sits in the header block
        // itself, which a naive loop-defs scan would misread as
        // loop-varying.)
        check.bound = match LoopOptimizer::const_expr_int_value(f, bound_vid) {
            Some(b) => b,
            None => continue,
        };
        found = Some((i_vid, check));
    }

    let (i_vid, check) = match found {
        Some(v) => v,
        None => return Ok(None),
    };

    let param_index = match header.params.iter().position(|p| p.val == i_vid) {
        Some(idx) => idx,
        None => return Ok(None),
    };

    // (3) Exactly one back edge, branching to the header.
    if lp.back_edges.len() != 1 {
        return Err(format!(
            "loop bb{} has {} back edges; only a single-latch loop is analyzable",
            lp.header.0,
            lp.back_edges.len()
        ));
    }
    let latch_id = lp.back_edges[0];
    let latch = match f.get_block(latch_id) {
        Some(b) => b,
        None => return Ok(None),
    };
    let latch_args = match &latch.terminator {
        Terminator::Branch { target, args } if *target == lp.header => args.clone(),
        _ => {
            return Err(format!(
                "loop bb{} latch bb{} does not branch unconditionally to the header",
                lp.header.0, latch_id.0
            ));
        }
    };
    let next_vid = match latch_args.get(param_index) {
        Some(v) => *v,
        None => return Ok(None),
    };

    // (4) The forwarded value must be a constant-step increment chain rooted
    // at the induction parameter. A factor-2 unrolled latch forwards
    // `i_next2 = i_next1 + S` where `i_next1 = i + S`; every link is a
    // straight-line `+`/`-` in this latch block, so each link runs exactly
    // when its predecessors ran and its overflow extreme is provable from
    // the condition bound plus the steps accumulated before it.
    let mut links: Vec<(ValueId, i64)> = Vec::new(); // (dest, delta), latch order
    let mut cursor = next_vid;
    while cursor != i_vid {
        if links.len() >= 8 {
            return Err(format!(
                "loop bb{} induction increment chain exceeds 8 constant-step links",
                lp.header.0
            ));
        }
        let mut def = None;
        for inst in &latch.instructions {
            if let Inst::BinOp {
                dest,
                op,
                left,
                right,
                ..
            } = inst
                && *dest == cursor
                && (*op == "+" || *op == "-")
            {
                def = Some((op.as_str(), *left, *right));
                break;
            }
        }
        let Some((op, left, right)) = def else {
            return Err(format!(
                "loop bb{} forwarded induction value v{} is not a '+/-' constant-step chain rooted at the induction parameter",
                lp.header.0, cursor.0
            ));
        };
        // One operand is the compile-time constant step, the other is the
        // chain predecessor: the parameter itself or the previous link.
        let (prev, delta) = match op {
            "+" => {
                if let Some(s) = LoopOptimizer::const_expr_int_value(f, right) {
                    (left, s)
                } else if let Some(s) = LoopOptimizer::const_expr_int_value(f, left) {
                    (right, s)
                } else {
                    return Err(format!(
                        "loop bb{} induction chain link v{} is not an 'i + const' update",
                        lp.header.0, cursor.0
                    ));
                }
            }
            _ => {
                let step = match LoopOptimizer::const_expr_int_value(f, right) {
                    Some(s) => s,
                    None => {
                        return Err(format!(
                            "loop bb{} induction chain link v{} is not an 'i - const' update",
                            lp.header.0, cursor.0
                        ));
                    }
                };
                match step.checked_neg() {
                    Some(d) => (left, d),
                    None => return Ok(None),
                }
            }
        };
        links.push((cursor, delta));
        cursor = prev;
    }
    if links.is_empty() {
        // The parameter is forwarded unchanged: there is no increment to prove.
        return Ok(None);
    }
    links.reverse(); // walk order becomes parameter-outward

    // (5) Every link's step must travel in the condition's direction.
    let increasing = matches!(check.op.as_str(), "<" | "<=");
    for &(dest, delta) in &links {
        if delta > 0 && !increasing {
            return Err(format!(
                "loop bb{} condition '{}' disagrees with step {} on chain link v{}; trip range is not statically bounded",
                lp.header.0, check.op, delta, dest.0
            ));
        }
        if delta < 0 && increasing {
            return Err(format!(
                "loop bb{} condition '{}' disagrees with step {} on chain link v{}; trip range is not statically bounded",
                lp.header.0, check.op, delta, dest.0
            ));
        }
    }

    // (6) Constant bound arithmetic must prove every link safe: the k-th
    // increment computes at most (loop edge bound) + (its own and all
    // preceding steps), and a link only runs when its predecessors ran.
    let running = match check.op.as_str() {
        "<" => check.bound.checked_sub(1),
        "<=" => Some(check.bound),
        ">" => check.bound.checked_add(1),
        ">=" => Some(check.bound),
        _ => None,
    };
    let Some(mut running) = running else {
        return Err(format!(
            "loop bb{} bound {} has no in-range loop edge; overflow trap kept",
            lp.header.0, check.bound
        ));
    };
    let mut marked: Vec<ValueId> = Vec::new();
    for &(dest, delta) in &links {
        let Some(extreme) = running.checked_add(delta) else {
            return Err(format!(
                "loop bb{} bound {} with accumulated step {} overflows i64 at the loop edge; overflow trap kept",
                lp.header.0, check.bound, running
            ));
        };
        running = extreme;
        marked.push(dest);
    }

    let proof = if check.mirrored {
        format!(
            "bb{}: bound {} (written on the left) with a {} link constant-step increment chain; constant range check passed",
            lp.header.0,
            check.bound,
            links.len()
        )
    } else {
        format!(
            "bb{}: condition 'i {} {}' with a {} link constant-step increment chain; constant range check passed",
            lp.header.0,
            check.op,
            check.bound,
            links.len()
        )
    };
    Ok(Some((marked, proof)))
}

/// Abstract signed range of one SSA value.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Range {
    lo: i64,
    hi: i64,
}

impl Range {
    fn const_range(c: i64) -> Range {
        Range { lo: c, hi: c }
    }

    /// Range of `a + b`. Saturates at i64 bounds instead of rejecting: a
    /// hull that already includes an i64 endpoint can never prove safety,
    /// so the caller keeps the trap. The corner arithmetic itself cannot
    /// overflow because it saturates.
    fn add(self, other: Range) -> Range {
        Range {
            lo: self.lo.saturating_add(other.lo),
            hi: self.hi.saturating_add(other.hi),
        }
    }

    fn neg(self) -> Range {
        Range {
            lo: self.hi.checked_neg().unwrap_or(i64::MIN),
            hi: self.lo.checked_neg().unwrap_or(i64::MAX),
        }
    }

    /// Range of `a - b`.
    fn sub(self, other: Range) -> Range {
        self.add(other.neg())
    }

    /// Range of `a * b`. Takes the hull of the four corner products with
    /// saturating arithmetic. Sound for the elision direction: the true
    /// product range can only be narrower than the corner hull.
    fn mul(self, other: Range) -> Range {
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
    fn union(self, other: Range) -> Range {
        Range {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
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
enum RState {
    Bot,
    Known(Range),
    Top,
}

/// Join of two abstract states: bottom is the identity, top is absorbing,
/// known ranges hull together. Monotone in both arguments.
fn join_state(count: &mut u32, old: RState, s: RState) -> Option<RState> {
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
struct ProvenInduction {
    /// Header parameter ValueId of the induction variable.
    vid: ValueId,
    /// Inclusive body-entry range of the induction variable.
    lo: i64,
    /// Largest value the header parameter can hold on ANY entry edge,
    /// including the final failing iteration: `hi + accumulated chain
    /// steps`. The latch can hand the bound itself back to the header, so
    /// uses outside the body must not assume the tight `hi`.
    hi_edge: i64,
    /// Blocks that constitute this loop.
    blocks: std::collections::HashSet<crate::dmir::BasicBlockId>,
    /// Header block id (for tracing).
    header_id: crate::dmir::BasicBlockId,
}

/// Extracts the proven induction facts of one analyzable loop: condition
/// `i < B` / `i <= B` against a constant bound with a positive constant
/// step and a single latch, all entry edges passing the same constant
/// start. Returns the body-entry range `lo..=hi` of `i`.
fn induction_range(f: &Function, lp: &crate::dmir::cfg::NaturalLoop) -> Option<ProvenInduction> {
    let header = f.get_block(lp.header)?;
    let cond_vid = match &header.terminator {
        Terminator::CondBranch { cond, .. } => *cond,
        _ => return None,
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

/// Proves value ranges for Int `+`/`-`/`*` across the whole function with
/// every proven induction as a seed, converged to one global fixpoint, and
/// marks each BinOp INSIDE some proven loop whose result range fits
/// strictly inside i64.
///
/// v1.4.7: one function-wide pass instead of per-loop scans. Hot inner
/// bodies consume roots defined outside their loop (`v47 = copy v27` in a
/// preheader, `v27 = i * 192` in the outer body); a body-only fixpoint
/// sees those as `full` and stays unmarked. The global sweep lets ranges
/// flow: constants → outer induction → outer body arithmetic → inner
/// header params → inner body. Marking stays confined to the union of the
/// proven loops' blocks, so straight-line code keeps its traps.
fn prove_body_arithmetic(f: &mut Function, cfg: &ControlFlowGraph) -> (usize, Vec<String>) {
    let mut inductions: Vec<ProvenInduction> = Vec::new();
    for lp in &cfg.loops {
        if let Some(pi) = induction_range(f, lp) {
            inductions.push(pi);
        }
    }
    if inductions.is_empty() {
        return (0, Vec::new());
    }

    // SSA constants and instruction defs have globally unique ValueIds, so
    // their ranges live in flat maps keyed by ValueId alone; cross-block
    // dominance references (`v47 = copy v27` in a preheader, consumed two
    // loops down) resolve without block scoping. Block params are the only
    // block-sensitive values and keep (block, vid) keys.
    let const_ranges: HashMap<ValueId, Range> = {
        let mut m: HashMap<ValueId, Range> = HashMap::new();
        for b in &f.blocks {
            for inst in &b.instructions {
                if let Inst::ConstInt { dest, value, .. } = inst {
                    m.insert(*dest, Range::const_range(*value));
                }
            }
        }
        m
    };
    // Defs the fixpoint can model: Int `+`/`-`/`*`/wrapping and Int copy.
    // Everything else (call results, loads, casts, Bool ops) is a sticky
    // `Top` operand.
    let mut analyzable: std::collections::HashSet<ValueId> = std::collections::HashSet::new();
    for b in &f.blocks {
        for inst in &b.instructions {
            match inst {
                Inst::BinOp { dest, op, ty, .. }
                    if ty == "Int"
                        && matches!(op.as_str(), "+" | "-" | "*" | "wrapping_+" | "wrapping_-") =>
                {
                    analyzable.insert(*dest);
                }
                Inst::UnOp { dest, op, ty, .. } if ty == "Int" && op == "copy" => {
                    analyzable.insert(*dest);
                }
                _ => {}
            }
        }
    }
    let mut inst_states: HashMap<ValueId, RState> = HashMap::new();
    let mut param_states: HashMap<(crate::dmir::BasicBlockId, ValueId), RState> = HashMap::new();

    let mut param_owner: HashMap<ValueId, crate::dmir::BasicBlockId> = HashMap::new();
    for b in &f.blocks {
        for p in &b.params {
            param_owner.insert(p.val, b.id);
        }
    }

    // Proven induction facts as param seeds: on ANY entry into the header
    // the param holds [lo, hi_edge] — entry edges pass `start`, the latch
    // hands back at most the accumulated chain extreme. These are floor
    // facts proven outside the fixpoint and are never widened by it.
    let mut proven_params: HashMap<(crate::dmir::BasicBlockId, ValueId), Range> = HashMap::new();
    for pi in &inductions {
        proven_params.insert(
            (pi.header_id, pi.vid),
            Range {
                lo: pi.lo,
                hi: pi.hi_edge,
            },
        );
    }

    // Incoming edges of every block that has params.
    let mut incoming: HashMap<
        crate::dmir::BasicBlockId,
        Vec<(crate::dmir::BasicBlockId, Vec<ValueId>)>,
    > = HashMap::new();
    for b in &f.blocks {
        let mut collect = |target: crate::dmir::BasicBlockId, args: &[ValueId]| {
            if let Some(tb) = f.get_block(target)
                && !tb.params.is_empty()
            {
                incoming
                    .entry(target)
                    .or_default()
                    .push((b.id, args.to_vec()));
            }
        };
        match &b.terminator {
            Terminator::Branch { target, args } => collect(*target, args),
            Terminator::CondBranch {
                then_block,
                then_args,
                else_block,
                else_args,
                ..
            } => {
                collect(*then_block, then_args);
                collect(*else_block, else_args);
            }
            _ => {}
        }
    }

    let range_of =
        |v: ValueId,
         inst_states: &HashMap<ValueId, RState>,
         param_states: &HashMap<(crate::dmir::BasicBlockId, ValueId), RState>| {
            if let Some(&r) = const_ranges.get(&v) {
                return RState::Known(r);
            }
            if let Some(&owner) = param_owner.get(&v) {
                if let Some(&r) = proven_params.get(&(owner, v)) {
                    return RState::Known(r);
                }
                return param_states
                    .get(&(owner, v))
                    .copied()
                    .unwrap_or(RState::Bot);
            }
            if analyzable.contains(&v) {
                return inst_states.get(&v).copied().unwrap_or(RState::Bot);
            }
            // A def this analysis cannot model: sticky unknown. It can never
            // mark, and every consumer of it becomes sticky unknown too.
            RState::Top
        };

    // Fixpoint over RState: Bot → (Known | Top) monotonically. Because
    // Bot means "not yet derived" (not "unbounded"), a param whose edge
    // arg is a forward instruction is NOT poisoned in round 1 — the arg
    // refines in a later round and the param joins with a real range.
    // Divergence is handled exactly once per value: the FIRST join that
    // would re-widen an already-known range proves the value is not stable
    // under iteration, so it degrades to sticky Top (unbounded), which can
    // never mark. A value whose inputs all stabilize joins to the same
    // range every round and never triggers the divergence guard.
    const MAX_ROUNDS: usize = 256;
    let mut rounds = 0usize;
    let mut param_change_count: HashMap<(crate::dmir::BasicBlockId, ValueId), u32> = HashMap::new();
    let mut inst_change_count: HashMap<ValueId, u32> = HashMap::new();
    loop {
        rounds += 1;
        if rounds > MAX_ROUNDS {
            // Analysis failed to stabilize: refuse to mark anything.
            return (0, Vec::new());
        }
        let mut changed = false;

        // (a) Join block-param states from incoming edge args.
        for (&target, preds) in incoming.iter() {
            let param_vids: Vec<ValueId> = f
                .get_block(target)
                .map(|tb| tb.params.iter().map(|p| p.val).collect())
                .unwrap_or_default();
            for (_pred_id, args) in preds {
                for (&p, a) in param_vids.iter().zip(args.iter()) {
                    if proven_params.contains_key(&(target, p)) {
                        continue; // floor fact: never widened
                    }
                    let s = range_of(*a, &inst_states, &param_states);
                    let key = (target, p);
                    let merged = match param_states.get(&key) {
                        None => Some(s),
                        Some(&old) => {
                            join_state(param_change_count.entry(key).or_insert(0), old, s)
                        }
                    };
                    match merged {
                        None => {}
                        Some(m) => {
                            param_states.insert(key, m);
                            changed = true;
                        }
                    }
                }
            }
        }

        // (b) Derive instruction states across all blocks.
        for blk in &f.blocks {
            for inst in &blk.instructions {
                let (dest, new_state) = match inst {
                    Inst::BinOp {
                        dest,
                        op,
                        left,
                        right,
                        ty,
                    } if ty == "Int"
                        && matches!(op.as_str(), "+" | "-" | "*" | "wrapping_+" | "wrapping_-") =>
                    {
                        let sl = range_of(*left, &inst_states, &param_states);
                        let sr = range_of(*right, &inst_states, &param_states);
                        let s = match (sl, sr) {
                            (RState::Known(rl), RState::Known(rr)) => {
                                RState::Known(match op.as_str() {
                                    "+" | "wrapping_+" => rl.add(rr),
                                    "-" | "wrapping_-" => rl.sub(rr),
                                    _ => rl.mul(rr),
                                })
                            }
                            (RState::Top, _) | (_, RState::Top) => RState::Top,
                            _ => RState::Bot,
                        };
                        (*dest, s)
                    }
                    Inst::UnOp {
                        dest,
                        op,
                        operand,
                        ty,
                    } if ty == "Int" && op == "copy" => {
                        (*dest, range_of(*operand, &inst_states, &param_states))
                    }
                    _ => continue,
                };
                let merged = match inst_states.get(&dest) {
                    None => Some(new_state),
                    Some(&old) => {
                        join_state(inst_change_count.entry(dest).or_insert(0), old, new_state)
                    }
                };
                match merged {
                    None => {}
                    Some(m) => {
                        inst_states.insert(dest, m);
                        changed = true;
                    }
                }
            }
        }

        if !changed {
            break;
        }
    }
    // Extract known ranges (Bot = unreachable/underived: not marked, and
    // its consumers saw Bot too, so nothing downstream can be unsound).
    let inst_ranges: HashMap<ValueId, Range> = inst_states
        .iter()
        .filter_map(|(&v, &s)| match s {
            RState::Known(r) => Some((v, r)),
            _ => None,
        })
        .collect();

    // Mark every BinOp whose def lies INSIDE some proven loop's blocks and
    // whose converged result range fits strictly inside i64. Strictness
    // matters: i64::MIN has no additive inverse and the checked lowering
    // traps exactly on the wrapped result.
    let in_any_loop =
        |bid: crate::dmir::BasicBlockId| inductions.iter().any(|pi| pi.blocks.contains(&bid));
    let mut marked_total = 0usize;
    let mut marked_blocks: std::collections::HashSet<crate::dmir::BasicBlockId> =
        std::collections::HashSet::new();
    let mut to_mark: Vec<ValueId> = Vec::new();
    for blk in &f.blocks {
        if !in_any_loop(blk.id) {
            continue;
        }
        for inst in &blk.instructions {
            if let Inst::BinOp { dest, op, ty, .. } = inst {
                if ty != "Int"
                    || !matches!(op.as_str(), "+" | "-" | "*" | "wrapping_+" | "wrapping_-")
                {
                    continue;
                }
                if let Some(&r) = inst_ranges.get(dest) {
                    if r.lo > i64::MIN && r.hi < i64::MAX {
                        to_mark.push(*dest);
                        marked_blocks.insert(blk.id);
                    }
                }
            }
        }
    }
    for vid in to_mark {
        f.proven_no_overflow.insert(vid);
        marked_total += 1;
    }

    let mut proofs: Vec<String> = Vec::new();
    if marked_total > 0 {
        let mut block_list: Vec<_> = marked_blocks.into_iter().collect();
        block_list.sort_by_key(|b| b.0);
        let blocks_str = block_list
            .iter()
            .map(|b| format!("bb{}", b.0))
            .collect::<Vec<_>>()
            .join(", ");
        proofs.push(format!(
            "{} Int BinOp(s) proven in-range from constants + {} proven induction range(s); global fixpoint over the whole function; unchecked in: {}",
            marked_total,
            inductions.len(),
            blocks_str
        ));
    }

    (marked_total, proofs)
}
