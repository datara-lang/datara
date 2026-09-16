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

use crate::dmir::cfg::ControlFlowGraph;
use crate::dmir::{Function, Inst, Module, Terminator, ValueId};
use crate::optimizer::cost_model::OptimizationDecisionTrace;
use crate::optimizer::loops::LoopOptimizer;

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

/// Proves and marks the induction increments of one function.
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
    let proofs = plans.into_iter().map(|(_, p)| p).collect();
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
