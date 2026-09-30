//! Loop-carried accumulator deltas and the range-guided body-arithmetic
//! proof for the overflow-elision pass (`--opt speed`). Split from
//! `ovf_elide.rs` to honor the v1.5.0 release file-size gate.
//!
//! `AccDelta` registers the full-run delta of a proven accumulator param
//! (innermost-first build order), `resolve_arm_delta` resolves diamond-arm
//! deltas join-soundly (never summing alternative edges), and
//! `prove_body_arithmetic` runs the function-wide range fixpoint that marks
//! bounded Int arithmetic inside proven loops.

use super::ovf_range::{induction_range, join_state, ProvenInduction, Range, RState};
use crate::dmir::cfg::ControlFlowGraph;
use crate::dmir::{Function, Inst, Terminator, ValueId};
use std::collections::HashMap;

/// Full-run delta facts for a proven accumulator param (v1.5.0). `dlo`/`dhi`
/// bound `param_exit - param_entry` over ONE complete run of the owning
/// loop; `links` are the `param ± term` BinOps whose results are partial
/// sums of that run (markable once an enclosing hull exists); `blocks` is
/// the owning loop's block set, needed to locate the entry edges when an
/// outer proof consumes the delta.
pub(super) struct AccDelta {
    pub(super) dlo: i64,
    pub(super) dhi: i64,
    pub(super) links: Vec<ValueId>,
    blocks: std::collections::HashSet<crate::dmir::BasicBlockId>,
}

/// All `(source block, edge args)` pairs whose terminator targets `target`.
///
/// Candidate pre-pass (v1.5.0): structural purity check for the inter-
/// procedural delta table. A function qualifies when every instruction is
/// one of the effect-free forms below — no loads/stores, no calls, no
/// allocations — and its effects, if known, are Pure. This is deliberately
/// stricter than the inliner's gate: the ovf proof also relies on the
/// callee having exactly one loop-driven return, so `heavy(acc, i)` shaped
/// functions are in, everything with a second exit is out.
///
/// Currently unused: the inter-procedural delta-table route was evaluated
/// and rejected (see PLAN_V150.md M8 — cross-function marking cannot reach
/// call sites). Kept as a documented building block for the self-hosting
/// inter-procedural passes planned after 1.5.0.
#[allow(dead_code)]
pub(super) fn structural_pure_int_fn(f: &Function) -> bool {
    if f.return_type != "Int" {
        return false;
    }
    if f.blocks.len() > 64 {
        return false; // cost guard: table building walks blocks, not user code
    }
    for b in &f.blocks {
        for inst in &b.instructions {
            if !matches!(
                inst,
                Inst::ConstInt { .. }
                    | Inst::ConstFloat { .. }
                    | Inst::ConstBool { .. }
                    | Inst::ConstStr { .. }
                    | Inst::BinOp { .. }
                    | Inst::UnOp { .. }
                    | Inst::Select { .. }
                    | Inst::Decide { .. }
            ) {
                return false;
            }
        }
        match &b.terminator {
            Terminator::Branch { .. } | Terminator::CondBranch { .. } => {}
            Terminator::Return { value: Some(_) } => {}
            Terminator::Return { value: None } | Terminator::Unreachable => {}
        }
    }
    true
}

pub(super) fn incoming_edges(
    f: &Function,
    target: crate::dmir::BasicBlockId,
) -> Vec<(crate::dmir::BasicBlockId, &Vec<ValueId>)> {
    let mut out = Vec::new();
    for b in &f.blocks {
        match &b.terminator {
            Terminator::Branch { target: t, args } if *t == target => {
                out.push((b.id, args));
            }
            Terminator::CondBranch {
                then_block,
                then_args,
                else_block,
                else_args,
                ..
            } => {
                if *then_block == target {
                    out.push((b.id, then_args));
                }
                if *else_block == target && else_block != then_block {
                    out.push((b.id, else_args));
                }
            }
            _ => {}
        }
    }
    out
}

/// Resolves the delta of `val` relative to the accumulator `param`: the
/// range of `value(val) - value(param)` at the point `val` is handed to the
/// header on a back edge. Returns the joined `[lo, hi]` plus the BinOps
/// (links) whose results are partial sums covered by the eventual hull.
///
/// Soundness rules:
/// - an invariant arm contributes the empty delta;
/// - a `param ± term` link contributes the term's VALUE range (a proven
///   induction param is monotone-bounded at every program point);
/// - a join param contributes the MIN/MAX join of its edge deltas (exactly
///   one edge defines the value — never sum them);
/// - a param of ANOTHER loop contributes only via a registered full-run
///   delta (joining its latch edge as a one-step delta under-approximates
///   the run and is unsound); loops without a registered delta fail closed;
/// - a param of the SAME loop mid-run is not a delta at all: fail closed.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_arm_delta(
    f: &Function,
    val: ValueId,
    param: ValueId,
    cur_header: crate::dmir::BasicBlockId,
    pi_blocks: &std::collections::HashSet<crate::dmir::BasicBlockId>,
    param_owner: &HashMap<ValueId, crate::dmir::BasicBlockId>,
    all_loop_headers: &std::collections::HashSet<crate::dmir::BasicBlockId>,
    registry: &HashMap<(crate::dmir::BasicBlockId, ValueId), AccDelta>,
    proven_params: &HashMap<(crate::dmir::BasicBlockId, ValueId), Range>,
    const_ranges: &HashMap<ValueId, Range>,
    inst_ranges: &HashMap<ValueId, Range>,
    depth: usize,
) -> Option<(i64, i64, Vec<ValueId>)> {
    const MAX_DEPTH: usize = 6;
    if depth > MAX_DEPTH {
        return None;
    }
    // 1. Invariant arm.
    if val == param {
        return Some((0, 0, Vec::new()));
    }
    if let Some(owner) = param_owner.get(&val).copied() {
        if !pi_blocks.contains(&owner) {
            return None;
        }
        let idx = f
            .get_block(owner)?
            .params
            .iter()
            .position(|p| p.val == val)?;
        if all_loop_headers.contains(&owner) {
            if owner == cur_header {
                return None;
            }
            // 2. Another loop's header param with a registered full-run
            // delta: the exit value is `entry + D`; resolve the entry-edge
            // args (from inside the analyzed loop, outside the owning loop)
            // and add D. Fail closed on any other entry path.
            let d = registry.get(&(owner, val))?;
            let mut jlo = i64::MAX;
            let mut jhi = i64::MIN;
            let mut links = d.links.clone();
            let mut any = false;
            for (src, args) in incoming_edges(f, owner) {
                if d.blocks.contains(&src) {
                    continue; // the latch: already accounted for in D
                }
                if !pi_blocks.contains(&src) {
                    return None;
                }
                let Some(&a) = args.get(idx) else {
                    continue;
                };
                let (elo, ehi, elinks) = resolve_arm_delta(
                    f,
                    a,
                    param,
                    cur_header,
                    pi_blocks,
                    param_owner,
                    all_loop_headers,
                    registry,
                    proven_params,
                    const_ranges,
                    inst_ranges,
                    depth + 1,
                )?;
                jlo = jlo.min(elo);
                jhi = jhi.max(ehi);
                links.extend(elinks);
                any = true;
            }
            if !any {
                return None;
            }
            let lo = (jlo as i128).saturating_add(d.dlo as i128);
            let hi = (jhi as i128).saturating_add(d.dhi as i128);
            return Some((
                lo.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                hi.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                links,
            ));
        }
        // 3. Plain join param (diamond phi): exactly one incoming edge
        // defines the value per iteration, so the delta is the MIN/MAX join
        // of the per-edge deltas — never their sum. Fail closed on edges
        // from outside the analyzed loop (multi-entry path: that value's
        // delta is not resolvable here).
        let mut jlo = i64::MAX;
        let mut jhi = i64::MIN;
        let mut links: Vec<ValueId> = Vec::new();
        let mut any = false;
        for (src, args) in incoming_edges(f, owner) {
            if !pi_blocks.contains(&src) {
                return None;
            }
            let Some(&a) = args.get(idx) else {
                continue;
            };
            let (elo, ehi, elinks) = resolve_arm_delta(
                f,
                a,
                param,
                cur_header,
                pi_blocks,
                param_owner,
                all_loop_headers,
                registry,
                proven_params,
                const_ranges,
                inst_ranges,
                depth + 1,
            )?;
            jlo = jlo.min(elo);
            jhi = jhi.max(ehi);
            links.extend(elinks);
            any = true;
        }
        if !any {
            return None;
        }
        return Some((jlo, jhi, links));
    }
    // 4. Single `param ± term` link: the arm value is the link result, so
    // the delta is the term's value range and the link is markable.
    for b in &f.blocks {
        if !pi_blocks.contains(&b.id) {
            continue;
        }
        for inst in &b.instructions {
            if let Inst::BinOp {
                dest,
                op,
                left,
                right,
                ty,
            } = inst
                && *ty == "Int"
                && *dest == val
                && matches!(op.as_str(), "+" | "-")
            {
                let neg = op == "-";
                let (term, sign) = if *left == param {
                    (*right, if neg { -1i64 } else { 1i64 })
                } else if *right == param && !neg {
                    (*left, 1i64)
                } else {
                    continue;
                };
                let tr = inst_ranges
                    .get(&term)
                    .or_else(|| const_ranges.get(&term))
                    .copied()
                    .or_else(|| {
                        param_owner
                            .get(&term)
                            .and_then(|own| proven_params.get(&(*own, term)))
                            .copied()
                    })?;
                let r = tr;
                let (lo, hi) = if sign > 0 {
                    (r.lo, r.hi)
                } else {
                    (-r.hi, -r.lo)
                };
                return Some((lo, hi, vec![*dest]));
            }
        }
    }
    // 5. Any other def (including `Call(pure_fn, args)`) fails closed: an
    // arm value produced by a call has no intra-procedural delta proof (the
    // inter-procedural delta-table idea was evaluated and rejected — see
    // PLAN_V150.md M8), so the caller keeps its overflow checks.
    None
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
pub(super) fn prove_body_arithmetic(
    f: &mut Function,
    cfg: &ControlFlowGraph,
) -> (usize, Vec<String>) {
    let mut inductions: Vec<ProvenInduction> = Vec::new();
    for lp in &cfg.loops {
        if let Some(pi) = induction_range(f, lp) {
            inductions.push(pi);
        }
    }
    if inductions.is_empty() {
        return (0, Vec::new());
    }
    if std::env::var("DATARA_OVF_DEBUG").is_ok() {
        eprintln!(
            "[ovf-delta] proving fn '{}': {} block(s), {} loop(s)",
            f.name,
            f.blocks.len(),
            inductions.len()
        );
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
                        && matches!(
                            op.as_str(),
                            "+" | "-" | "*" | "wrapping_+" | "wrapping_-
                            " | "&" | "|" | "^"
                        ) =>
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
    // Function parameters mirror as entry-block params (single definition,
    // see the verifier): seed them with the FULL i64 hull so the fixpoint
    // treats any call-site argument as possible. A masked term like
    // `((i ^ k) & 63)` still refines through the `&` hull, so the delta
    // extracted from this function is sound for ANY caller argument.
    for (_, _, pv) in &f.params {
        if let Some(entry) = f.blocks.iter().find(|b| b.id == f.entry_block)
            && entry.params.iter().any(|bp| bp.val == *pv)
        {
            proven_params
                .entry((f.entry_block, *pv))
                .or_insert(Range { lo: i64::MIN, hi: i64::MAX });
        }
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
                        && matches!(
                            op.as_str(),
                            "+" | "-" | "*" | "wrapping_+" | "wrapping_-" | "&" | "|" | "^"
                        ) =>
                    {
                        let sl = range_of(*left, &inst_states, &param_states);
                        let sr = range_of(*right, &inst_states, &param_states);
                        let s = match (sl, sr) {
                            (RState::Known(rl), RState::Known(rr)) => {
                                RState::Known(match op.as_str() {
                                    "+" | "wrapping_+" => rl.add(rr),
                                    "-" | "wrapping_-" => rl.sub(rr),
                                    "&" => rl.bitand(rr),
                                    "|" => rl.bitor(rr),
                                    "^" => rl.xor(rr),
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
    if std::env::var("DATARA_OVF_DEBUG").is_ok() {
        for (v, r) in &inst_ranges {
            eprintln!("[ovf-delta] range v{} = [{}, {}]", v.0, r.lo, r.hi);
        }
    }
    for blk in &f.blocks {
        if !in_any_loop(blk.id) {
            continue;
        }
        for inst in &blk.instructions {
            if let Inst::BinOp { dest, op, ty, .. } = inst {
                // Checked lowering only exists for `+`/`-`/`*` — the only
                // Int ops that can overflow. `&`/`|`/`^` results never trap,
                // but their converged ranges still matter here: they feed
                // accumulator-chain terms, so include them when bounded
                // (e.g. `i ^ k & 63` in a hot inner loop).
                if ty != "Int"
                    || !matches!(
                        op.as_str(),
                        "+" | "-" | "*" | "wrapping_+" | "wrapping_-" | "&" | "|" | "^"
                    )
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

    // Registry of full-run accumulator deltas (v1.5.0). Built innermost-
    // first: a nested loop's per-outer-run delta is only sound once that
    // inner loop's own chain has been walked, so enclosed loops consume
    // `registry` entries and the release into `enclosed` happens after all
    // loops have been processed.
    let mut registry: HashMap<(crate::dmir::BasicBlockId, ValueId), AccDelta> = HashMap::new();
    let mut enclosed: std::collections::HashSet<ValueId> = std::collections::HashSet::new();
    let all_loop_headers: std::collections::HashSet<crate::dmir::BasicBlockId> =
        inductions.iter().map(|p| p.header_id).collect();
    let mut innermost_order: Vec<(crate::dmir::BasicBlockId, ValueId)> = Vec::new();
    for (i, a) in inductions.iter().enumerate() {
        let innermost = !inductions.iter().enumerate().any(|(j, b)| {
            i != j && b.blocks.len() < a.blocks.len() && b.blocks.iter().all(|x| a.blocks.contains(x))
        });
        if innermost {
            innermost_order.push((a.header_id, a.vid));
        }
    }
    innermost_order.sort_by_key(|(h, v)| {
        inductions
            .iter()
            .find(|p| p.header_id == *h && p.vid == *v)
            .map(|p| p.blocks.len())
            .unwrap_or(usize::MAX)
    });

    // Loop-carried accumulator proof (v1.5.0): `acc = phi(start, acc ± x)`
    // whose feed expression `x` has a converged range. The fixpoint cannot
    // model this phi (it would diverge to Top), but the TRAP the backend
    // keeps on `acc + x` is provably unreachable under a simple hull:
    // every intermediate value of a monotone accumulation stays inside
    // [start_lo + trip*lo, start_hi + trip*hi] — the partial sums of a
    // sequence whose every term is in [lo, hi] never leave that hull. When
    // the hull fits strictly inside i64 (checked with checked_mul so huge
    // trip counts reject instead of wrapping), the checked add can never
    // trap and may compile unchecked. This is what unblocks `acc += i & 255`
    // style loops whose per-element add is the entire hot work.
    for pi in &inductions {
        let trip = match pi.hi_edge.checked_sub(pi.lo).and_then(|d| d.checked_add(1)) {
            Some(t) if t > 0 => t,
            _ => continue,
        };
        let Some(header) = f.get_block(pi.header_id) else {
            continue;
        };
        let param_idx_by_val: std::collections::HashMap<ValueId, usize> = header
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.val, i))
            .collect();
        // The `header` borrow ends at the map construction above; the rest
        // of this loop body only touches `f` through the collected ids.
        for (param, param_idx) in &param_idx_by_val {
            if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                eprintln!("[ovf-acc] examining param v{} (idx {}) of loop bb{}", param.0, param_idx, pi.header_id.0);
            }
            // Structural latch feed of this param: the arg the unconditional
            // latch hands back. Mid-run back edges (continue-style) carry a
            // PARTIAL state, not a run delta, and would break the hull
            // argument when terms can be negative — fail closed on them.
            let mut latch: Option<ValueId> = None;
            let mut bypass: Vec<ValueId> = Vec::new();
            for b in &f.blocks {
                if !pi.blocks.contains(&b.id) {
                    continue;
                }
                match &b.terminator {
                    Terminator::Branch { target, args } if *target == pi.header_id => {
                        if let Some(&a) = args.get(*param_idx) {
                            latch = Some(a);
                        }
                    }
                    Terminator::CondBranch {
                        then_block,
                        then_args,
                        else_block,
                        else_args,
                        ..
                    } => {
                        if *then_block == pi.header_id {
                            if let Some(&a) = then_args.get(*param_idx) {
                                bypass.push(a);
                            }
                        } else if *else_block == pi.header_id {
                            if let Some(&a) = else_args.get(*param_idx) {
                                bypass.push(a);
                            }
                        }
                    }
                    _ => {}
                }
            }
            let Some(latch) = latch else {
                continue;
            };
            if !bypass.is_empty() {
                if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                    eprintln!(
                        "[ovf-acc] param v{}: {} mid-run back edge(s) bypass the latch; keeping their checks",
                        param.0,
                        bypass.len()
                    );
                }
                continue;
            }
            // Per-feed chain hull: walk the `param ± term` chain of the
            // latch feed and sum the signed term ranges into this feed's
            // FULL-RUN delta. Every term must have a converged range (the
            // walk is bounded, so it cannot hang); an unresolvable term
            // fails closed. `param` on the right of a `-` is rejected: the
            // chain would need `value - 2·param`, not `-term`.
            let delta_of = |feed: ValueId| -> Option<Range> {
                let mut cursor = feed;
                let mut x_lo: i128 = 0;
                let mut x_hi: i128 = 0;
                let mut ok = false;
                for _ in 0..9 {
                    if cursor == *param {
                        ok = true;
                        break;
                    }
                    let mut link: Option<(ValueId, bool, ValueId)> = None; // (prev, neg, term)
                    for b in &f.blocks {
                        if !pi.blocks.contains(&b.id) {
                            continue;
                        }
                        for inst in &b.instructions {
                            if let Inst::BinOp {
                                dest,
                                op,
                                left,
                                right,
                                ty,
                            } = inst
                                && *ty == "Int"
                                && *dest == cursor
                                && matches!(op.as_str(), "+" | "-")
                            {
                                // Follow the operand that heads back toward
                                // the param; the other operand is this link's
                                // term. The term must have a provable range.
                                let has_range = |v: ValueId| {
                                    v == *param
                                        || inst_ranges.contains_key(&v)
                                        || const_ranges.contains_key(&v)
                                };
                                let (prev, neg, term) = if *left == *param {
                                    (*left, op == "-", *right)
                                } else if *right == *param && op == "+" {
                                    (*right, false, *left)
                                } else if has_range(*left) && !has_range(*right) {
                                    // right has no standalone range -> it is
                                    // the chain predecessor; left is the term.
                                    (*right, op == "-", *left)
                                } else {
                                    (*left, op == "-", *right)
                                };
                                link = Some((prev, neg, term));
                                break;
                            }
                        }
                        if link.is_some() {
                            break;
                        }
                    }
                    let (prev, neg, term) = link?;
                    let r = inst_ranges
                        .get(&term)
                        .or_else(|| const_ranges.get(&term))
                        .copied()
                        .or_else(|| {
                            // A term that is itself a proven induction param
                            // (e.g. `s + i`) has a body-entry range from the
                            // induction proof even though it never had a
                            // computed instruction range.
                            param_owner
                                .get(&term)
                                .and_then(|own| proven_params.get(&(*own, term)))
                                .copied()
                        });
                    let r = r?;
                    if neg {
                        x_lo = x_lo.saturating_sub(r.hi as i128);
                        x_hi = x_hi.saturating_sub(r.lo as i128);
                    } else {
                        x_lo = x_lo.saturating_add(r.lo as i128);
                        x_hi = x_hi.saturating_add(r.hi as i128);
                    }
                    cursor = prev;
                }
                if !ok {
                    return None;
                }
                Some(Range {
                    lo: x_lo.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                    hi: x_hi.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                })
            };
            let Some(latch_delta) = delta_of(latch) else {
                if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                    eprintln!("[ovf-acc] param v{}: latch chain unresolvable", param.0);
                }
                continue;
            };
            // Register the FULL-RUN delta BEFORE the hull test: an enclosed
            // loop consumes it via the registry regardless of whether THIS
            // loop's own hull fits (an enclosing hull can still prove both).
            // The consumer is an enclosing loop whose ONE iteration runs this
            // loop to completion, so the delta it sees is the whole inner
            // run (trip × per-iteration latch delta), not one inner step.
            // checked_mul rejects a run delta that would wrap (fail closed:
            // the enclosing proof then stays unresolvable and keeps its
            // checks).
            let full_lo = trip.checked_mul(latch_delta.lo);
            let full_hi = trip.checked_mul(latch_delta.hi);
            let (Some(full_lo), Some(full_hi)) = (full_lo, full_hi) else {
                if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                    eprintln!(
                        "[ovf-acc] param v{}: full-run delta {}x[{}..{}] wraps; no registry entry",
                        param.0, trip, latch_delta.lo, latch_delta.hi
                    );
                }
                continue;
            };
            registry.insert(
                (pi.header_id, *param),
                AccDelta {
                    dlo: full_lo,
                    dhi: full_hi,
                    links: vec![latch],
                    blocks: pi.blocks.clone(),
                },
            );
            // Entry value range of the param: constants on entry edges.
            let mut entry_lo: i64 = i64::MAX;
            let mut entry_hi: i64 = i64::MIN;
            let mut has_entry = false;
            for b in &f.blocks {
                if pi.blocks.contains(&b.id) {
                    continue;
                }
                let args: Option<&Vec<ValueId>> = match &b.terminator {
                    Terminator::Branch { target, args } if *target == pi.header_id => Some(args),
                    Terminator::CondBranch {
                        then_block,
                        then_args,
                        else_block,
                        else_args,
                        ..
                    } => {
                        if *then_block == pi.header_id {
                            Some(then_args)
                        } else if *else_block == pi.header_id {
                            Some(else_args)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(args) = args
                    && let Some(&a) = args.get(*param_idx)
                    && let Some(Range { lo, hi }) = const_ranges.get(&a)
                {
                    entry_lo = entry_lo.min(*lo);
                    entry_hi = entry_hi.max(*hi);
                    has_entry = true;
                }
            }
            if !has_entry {
                if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                    eprintln!(
                        "[ovf-acc] param v{} feed v{}: no constant entry arg (entry_lo={}, entry_hi={})",
                        param.0, latch.0, entry_lo, entry_hi
                    );
                }
                continue;
            }
            // Full-run hull: `trip` iterations, each adding a term inside
            // [latch_delta.lo, latch_delta.hi] to an accumulator starting in
            // [entry_lo, entry_hi]. Every intermediate partial sum lies in
            // [entry_lo + trip·dlo, entry_hi + trip·dhi]; strictly inside
            // i64, the chain's checked adds cannot trap.
            let lo_term = trip.checked_mul(latch_delta.lo);
            let hi_term = trip.checked_mul(latch_delta.hi);
            let (Some(lo_term), Some(hi_term)) = (lo_term, hi_term) else {
                continue;
            };
            let hull_lo = entry_lo.checked_add(lo_term);
            let hull_hi = entry_hi.checked_add(hi_term);
            let (Some(hull_lo), Some(hull_hi)) = (hull_lo, hull_hi) else {
                continue;
            };
            if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                eprintln!(
                    "[ovf-acc] param v{} feed v{} x [{}..{}] entry [{}..{}] trip {} hull [{}..{}]",
                    param.0,
                    latch.0,
                    latch_delta.lo,
                    latch_delta.hi,
                    entry_lo,
                    entry_hi,
                    trip,
                    hull_lo,
                    hull_hi
                );
            }
            // The hull must be strictly inside i64 (see the full-run check
            // below) — same one-sided soundness argument as the fixpoint.
            if !(hull_lo > i64::MIN && hull_hi < i64::MAX) {
                continue;
            }
            // Mark the partial sums this proof covers: the latch link plus
            // every intermediate chain link of the same bounded run. The
            // block param itself is NOT marked — it is not a BinOp, so a
            // marking there is unobservable. Bypassing (mid-run) edges were
            // rejected above, so the chain is the whole story.
            let mut marked_any = false;
            {
                let mut cursor = latch;
                for _ in 0..9 {
                    if cursor != *param && f.proven_no_overflow.insert(cursor) {
                        marked_total += 1;
                        marked_any = true;
                    }
                    if cursor == *param {
                        break;
                    }
                    // Find the defining BinOp of `cursor` inside the loop
                    // and step to its chain-predecessor operand.
                    let mut next: Option<ValueId> = None;
                    'blk: for b in &f.blocks {
                        if !pi.blocks.contains(&b.id) {
                            continue;
                        }
                        for inst in &b.instructions {
                            if let Inst::BinOp {
                                dest,
                                op,
                                left,
                                right,
                                ty,
                            } = inst
                                && *ty == "Int"
                                && *dest == cursor
                                && matches!(op.as_str(), "+" | "-")
                            {
                                next = if *left == *param {
                                    Some(*left)
                                } else if *right == *param {
                                    Some(*right)
                                } else {
                                    // The operand WITHOUT a proven range is
                                    // the chain predecessor; the one with a
                                    // range is this link's term.
                                    let left_ranged = inst_ranges.contains_key(left)
                                        || const_ranges.contains_key(left);
                                    let right_ranged = inst_ranges.contains_key(right)
                                        || const_ranges.contains_key(right);
                                    if left_ranged && !right_ranged {
                                        Some(*right)
                                    } else {
                                        Some(*left)
                                    }
                                };
                                break 'blk;
                            }
                        }
                    }
                    match next {
                        Some(prev) => cursor = prev,
                        None => break,
                    }
                }
            }
            if marked_any {
                for b in &f.blocks {
                    if pi.blocks.contains(&b.id)
                        && b.instructions
                            .iter()
                            .any(|i| {
                                matches!(i, Inst::BinOp { dest, .. } if f.proven_no_overflow.contains(dest))
                            })
                    {
                        marked_blocks.insert(b.id);
                    }
                }
            }
        }
    }

    // Registry release: every innermost accumulator's partial sums become
    // provable inside any enclosing loop's hull. Marking happens only when
    // an enclosing proof actually covers them (branch pass below), so this
    // set just gates which links are eligible.
    for (header_id, vid) in &innermost_order {
        if let Some(d) = registry.get(&(*header_id, *vid)) {
            for b in &f.blocks {
                if !d.blocks.contains(&b.id) {
                    continue;
                }
                for inst in &b.instructions {
                    if let Inst::BinOp { dest, .. } = inst
                        && d.links.contains(dest)
                    {
                        enclosed.insert(*dest);
                    }
                }
            }
        }
    }
    for vid in to_mark {
        if f.proven_no_overflow.insert(vid) {
            marked_total += 1;
        }
    }

    // Branch-aware accumulator proof (v1.5.0): the latch feed is a phi/join
    // of per-branch arms (diamond: `if c { acc = acc + t1 } else { acc = acc
    // + t2 }`), each a bounded `param ± term` sum or itself a nested
    // accumulator's exit value. `resolve_arm_delta` computes each arm's
    // delta relative to `param`; the arms JOIN (min/max — exactly one arm
    // defines the value per iteration), giving the per-iteration delta
    // range [hull_lo, hull_hi]. The full-run hull is
    // [entry_lo + trip·min, entry_hi + trip·max]; strictly inside i64 ⇒
    // every arm's checked add cannot trap and lowers unchecked. Nested
    // accumulator arms contribute through the registry of full-run deltas
    // built by the single-feed pass, so an inner loop's whole run is
    // accounted soundly. Only the structural latch participates: a mid-run
    // back edge carries a partial state, not a per-iteration delta.
    for pi in &inductions {
        let trip = match pi.hi_edge.checked_sub(pi.lo).and_then(|d| d.checked_add(1)) {
            Some(t) if t > 0 => t,
            _ => continue,
        };
        let header = match f.get_block(pi.header_id) {
            Some(h) => h,
            None => continue,
        };
        let param_idx_by_val: std::collections::HashMap<ValueId, usize> = header
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.val, i))
            .collect();
        for (param, param_idx) in &param_idx_by_val {
            let mut latch_arm: Option<ValueId> = None;
            let mut mid_run = false;
            for b in &f.blocks {
                if !pi.blocks.contains(&b.id) {
                    continue;
                }
                match &b.terminator {
                    Terminator::Branch { target, args } if *target == pi.header_id => {
                        if let Some(&a) = args.get(*param_idx) {
                            latch_arm = Some(a);
                        }
                    }
                    Terminator::CondBranch {
                        then_block,
                        else_block,
                        ..
                    } => {
                        if *then_block == pi.header_id || *else_block == pi.header_id {
                            mid_run = true;
                        }
                    }
                    _ => {}
                }
            }
            let Some(latch_arm) = latch_arm else {
                continue;
            };
            if mid_run {
                if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                    eprintln!(
                        "[ovf-branch] param v{}: mid-run back edge; keeping checks",
                        param.0
                    );
                }
                continue;
            }
            let arm_vals: Vec<ValueId> = vec![latch_arm];
            let mut links_to_mark: Vec<ValueId> = Vec::new();
            let mut hull_lo: i128 = i64::MAX as i128;
            let mut hull_hi: i128 = i64::MIN as i128;
            let mut all_resolved = true;
            for arm in &arm_vals {
                match resolve_arm_delta(
                    f,
                    *arm,
                    *param,
                    pi.header_id,
                    &pi.blocks,
                    &param_owner,
                    &all_loop_headers,
                    &registry,
                    &proven_params,
                    &const_ranges,
                    &inst_ranges,
                    0,
                ) {
                    Some((dlo, dhi, mut links)) => {
                        hull_lo = hull_lo.min(dlo as i128);
                        hull_hi = hull_hi.max(dhi as i128);
                        links_to_mark.append(&mut links);
                    }
                    None => {
                        all_resolved = false;
                        break;
                    }
                }
            }
            if !all_resolved || links_to_mark.is_empty() {
                if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                    eprintln!(
                        "[ovf-branch] param v{}: arms unresolved (all_resolved={}), arms={:?}",
                        param.0,
                        all_resolved,
                        arm_vals.iter().map(|v| v.0).collect::<Vec<_>>(),
                    );
                }
                continue;
            }
            // Registry links are only covered by THIS hull when they belong
            // to an innermost accumulator nested in this loop; mid-layer
            // links would need their own enclosing proof first. Fail closed.
            links_to_mark.retain(|l| enclosed.contains(l));
            // Entry hull of the accumulator from constants on entry edges.
            let mut entry_lo: i64 = i64::MAX;
            let mut entry_hi: i64 = i64::MIN;
            let mut has_entry = false;
            for b in &f.blocks {
                if pi.blocks.contains(&b.id) {
                    continue;
                }
                let args: Option<&Vec<ValueId>> = match &b.terminator {
                    Terminator::Branch { target, args } if *target == pi.header_id => Some(args),
                    Terminator::CondBranch {
                        then_block,
                        then_args,
                        else_block,
                        else_args,
                        ..
                    } => {
                        if *then_block == pi.header_id {
                            Some(then_args)
                        } else if *else_block == pi.header_id {
                            Some(else_args)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(args) = args
                    && let Some(&a) = args.get(*param_idx)
                    && let Some(Range { lo, hi }) = const_ranges.get(&a)
                {
                    entry_lo = entry_lo.min(*lo);
                    entry_hi = entry_hi.max(*hi);
                    has_entry = true;
                }
            }
            if !has_entry {
                if std::env::var("DATARA_OVF_DEBUG").is_ok() {
                    eprintln!("[ovf-branch] param v{}: no const entry", param.0);
                }
                continue;
            }
            // Full-run hull: trip outer iterations, each contributing a delta
            // inside [hull_lo, hull_hi] (the joined per-arm deltas).
            let (Some(lo_term), Some(hi_term)) = (
                trip.checked_mul(hull_lo.clamp(i64::MIN as i128, i64::MAX as i128) as i64),
                trip.checked_mul(hull_hi.clamp(i64::MIN as i128, i64::MAX as i128) as i64),
            ) else {
                continue;
            };
            let (Some(hull_full_lo), Some(hull_full_hi)) = (
                entry_lo.checked_add(lo_term),
                entry_hi.checked_add(hi_term),
            ) else {
                continue;
            };
            // `>=`/`<=` against the type extremes: the hull must be strictly
            // inside i64 for the checked-add elision to be sound.
            if !(hull_full_lo > i64::MIN && hull_full_hi < i64::MAX) {
                continue;
            }
            // Every link in the diamond is a partial sum bounded by the same
            // monotone hull: mark them all unchecked.
            let mut marked_any = false;
            for link in &links_to_mark {
                if f.proven_no_overflow.insert(*link) {
                    marked_total += 1;
                    marked_any = true;
                }
            }
            if marked_any {
                for b in &f.blocks {
                    if pi.blocks.contains(&b.id)
                        && b.instructions.iter().any(|i| {
                            matches!(
                                i,
                                Inst::BinOp { dest, .. }
                                    if links_to_mark.contains(dest)
                            )
                        })
                    {
                        marked_blocks.insert(b.id);
                    }
                }
            }
            let _ = param_idx; // silence unused in non-debug builds
        }
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
