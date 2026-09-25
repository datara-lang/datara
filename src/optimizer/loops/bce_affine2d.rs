//! 2D Row-Major Affine Index Bounds Check Elimination (P7 module split).
//!
//! Split out of `bce.rs` to honor the "0 files > 60 KB" repo invariant. The
//! proof: `0 <= (I * N + J) < len(arr)` when `I < N`, `J < N`, and
//! `len(arr) >= N * N`. Applicable to all matrix operations, 2D
//! convolutions, image processing, and grids.
//!
//! The phase only rewrites `datara_rt_list_{get,set}` into their
//! `_unchecked` variants; the shared analysis maps are therefore taken as
//! immutable references and the caller (`bce_pass`) receives the
//! elimination count back instead of a mutated lattice.

use crate::dmir::{Function, Inst, Terminator, ValueId};
use crate::optimizer::cost_model::OptimizationDecisionTrace;
use std::collections::HashSet;

use super::bce::BceCtxt;

/// Eliminates bounds checks for row-major affine accesses `arr[I * N + J]`
/// where both `I` and `J` are loop-bounded by the same constant `N` and the
/// array length is provably `N * N` (or a constant length covering it).
///
/// Returns the number of accesses rewritten to `_unchecked`.
pub(super) fn eliminate_affine_2d(
    f: &mut Function,
    ctx: &BceCtxt<'_>,
    trace: &mut OptimizationDecisionTrace,
) -> usize {
    let resolve_vid = ctx.resolve_vid_fn();
    let resolve_name = ctx.resolve_name_fn();
    let resolve_name_deep = ctx.resolve_name_deep_fn();
    let const_val = ctx.const_val_fn();
    let find_mul = ctx.find_mul_fn();

    // --- Loop bound collection -------------------------------------------------
    // loop_bounds: (induction, induction_name, bound, bound_name, loop blocks)
    let mut loop_bounds: Vec<(
        ValueId,
        Option<String>,
        ValueId,
        Option<String>,
        HashSet<crate::dmir::BasicBlockId>,
    )> = Vec::new();
    for lp in &ctx.cfg.loops {
        let header_block = match f.get_block(lp.header) {
            Some(b) => b,
            None => continue,
        };
        if let Terminator::CondBranch { cond, .. } = &header_block.terminator {
            for inst in &header_block.instructions {
                if let Inst::BinOp {
                    dest,
                    op,
                    left,
                    right,
                    ..
                } = inst
                {
                    if dest == cond && op == "<" {
                        let left_res = resolve_vid(*left);
                        let right_res = resolve_vid(*right);
                        let left_name = resolve_name_deep(*left).or_else(|| resolve_name(*left));
                        let right_name = resolve_name_deep(*right).or_else(|| resolve_name(*right));
                        let loop_blocks: HashSet<_> = lp.blocks.iter().copied().collect();
                        loop_bounds.push((left_res, left_name, right_res, right_name, loop_blocks));
                    }
                }
            }
        }
    }

    // --- Rewrite loop ----------------------------------------------------------
    let mut affine_2d_eliminated = 0;
    for blk in &mut f.blocks {
        let blk_id = blk.id;
        for inst in &mut blk.instructions {
            if let Inst::Call { func, args, .. } = inst {
                if (func == "datara_rt_list_get" || func == "datara_rt_list_set") && args.len() >= 2
                {
                    let arr_vid = resolve_vid(args[0]);
                    let arr_name = resolve_name_deep(args[0]);
                    let idx_vid = resolve_vid(args[1]);

                    if let Some(&(add_l, add_r)) = ctx.add_map.get(&idx_vid) {
                        let pairs = [(add_l, add_r), (add_r, add_l)];
                        for (mul_cand, off_cand) in pairs {
                            if let Some(resolved_mul) = find_mul(mul_cand) {
                                if let Some(&(m1, m2)) = ctx.mul_map.get(&resolved_mul) {
                                    let factor_choices = [(m1, m2), (m2, m1)];
                                    for (i_val, n_val) in factor_choices {
                                        let j_val = off_cand;
                                        let is_bounded = |val: ValueId, bound: ValueId| -> bool {
                                            let v_name = resolve_name_deep(val);
                                            let b_name = resolve_name_deep(bound);
                                            let v_res = resolve_vid(val);
                                            let b_res = resolve_vid(bound);
                                            loop_bounds.iter().any(|(lv, ln, rv, rn, blocks)| {
                                                if !blocks.contains(&blk_id) {
                                                    return false;
                                                }
                                                let var_matches = *lv == val
                                                    || *lv == v_res
                                                    || (ln.is_some() && ln == &v_name);
                                                let bound_matches = *rv == bound
                                                    || *rv == b_res
                                                    || (rn.is_some() && rn == &b_name);
                                                var_matches && bound_matches
                                            })
                                        };

                                        if is_bounded(i_val, n_val) && is_bounded(j_val, n_val) {
                                            let arr_len_match = {
                                                let mut ok = false;
                                                let check_len_vid = |len_v: ValueId| -> bool {
                                                    if let Some(resolved_len) = find_mul(len_v) {
                                                        if let Some(&(ml, mr)) =
                                                            ctx.mul_map.get(&resolved_len)
                                                        {
                                                            let ml_res = resolve_vid(ml);
                                                            let mr_res = resolve_vid(mr);
                                                            let n_res = resolve_vid(n_val);
                                                            let ml_name = resolve_name_deep(ml);
                                                            let mr_name = resolve_name_deep(mr);
                                                            let n_name = resolve_name_deep(n_val);
                                                            let m_ok = |r, n: &Option<String>| {
                                                                r == n_res
                                                                    || (n.is_some() && n == &n_name)
                                                            };
                                                            if m_ok(ml_res, &ml_name)
                                                                && m_ok(mr_res, &mr_name)
                                                            {
                                                                return true;
                                                            }
                                                        }
                                                    }
                                                    if let (Some(nc), Some(lc)) =
                                                        (const_val(n_val), const_val(len_v))
                                                    {
                                                        if nc * nc <= lc {
                                                            return true;
                                                        }
                                                    }
                                                    false
                                                };
                                                let len_cand =
                                                    ctx.list_len.get(&arr_vid).or_else(|| {
                                                        arr_name
                                                            .as_ref()
                                                            .and_then(|an| ctx.var_len.get(an))
                                                    });
                                                match len_cand {
                                                    Some(&super::bce::LenVal::Vid(v)) => {
                                                        ok = check_len_vid(v);
                                                    }
                                                    Some(&super::bce::LenVal::Const(lc)) => {
                                                        let n_const =
                                                            const_val(n_val).or_else(|| {
                                                                resolve_name_deep(n_val).and_then(
                                                                    |name| {
                                                                        ctx.var_const
                                                                            .get(&name)
                                                                            .copied()
                                                                    },
                                                                )
                                                            });
                                                        if let Some(nc) = n_const {
                                                            if nc * nc <= lc {
                                                                ok = true;
                                                            }
                                                        }
                                                    }
                                                    None => {}
                                                }
                                                ok
                                            };

                                            if arr_len_match {
                                                *func = format!("{}_unchecked", func);
                                                affine_2d_eliminated += 1;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if func.ends_with("_unchecked") {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if affine_2d_eliminated > 0 {
        trace.record(
            "Affine2D:BCE",
            &format!("{}:matrix_indexing", f.name),
            "Applied",
            &format!(
                "+{} BCE proven 2D row-major unchecked access",
                affine_2d_eliminated
            ),
            "0",
            &format!(
                "BCE proven: 0 <= i * N + j < N * N <= arr.len() for {} accesses",
                affine_2d_eliminated
            ),
        );
    }

    affine_2d_eliminated
}
