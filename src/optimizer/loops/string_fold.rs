//! String-build loop fold (P1b): O(n) `s = s + piece` accumulation loops
//! are replaced by a single `str_repeat` + `concat` closed form. Extracted
//! from `fold.rs` to honor the repo's 60 KB per-file limit (v1.4.7).

use super::{LoopOptimizer, fold::StringFoldPlan};
use crate::dmir::cfg::ControlFlowGraph;
use crate::dmir::{BasicBlockId, Function, Inst, Terminator, ValueId};
use crate::optimizer::cost_model::OptimizationDecisionTrace;
use std::collections::HashSet;

impl LoopOptimizer {
    /// String-build loop fold (P1b): `mut s = seed; mut i = 0;
    /// while i < n { s = s + piece; i += 1 }` with `piece` a
    /// loop-invariant string becomes
    /// `s = concat(seed, str_repeat(piece, trips))` — one allocation, zero
    /// per-iteration appends. Analogous to the countable-sum fold, but the
    /// accumulator is a string and the closed form is a repeat.
    ///
    /// Soundness:
    /// 1. String `+` is associative here because the fold PRESERVES the
    ///    exact concatenation order (seed, piece x trips) — no reassociation,
    ///    byte-identical result.
    /// 2. `trips` uses the same parity-safe trip-count formula as the sum
    ///    fold (`n - start`, guarded `<= 0` by select), so an empty loop
    ///    yields `seed` unmodified.
    /// 3. `piece` must be defined outside the loop (loop-invariant) and the
    ///    loop shape must match the strict 2-block countable pattern; any
    ///    deviation bails.
    pub(crate) fn fold_string_build_loops(
        f: &mut Function,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let cfg = ControlFlowGraph::build(f);
        for lp in &cfg.loops {
            if let Some(plan) = Self::match_string_build_loop(f, &cfg, lp) {
                let var_form_name = plan.acc_var.clone();
                Self::apply_string_fold(f, &plan, var_form_name, trace);
                return 1;
            }
        }
        0
    }

    /// Test surface: the same pass, callable from an integration test.
    pub fn fold_string_build_loops_for_test(
        f: &mut Function,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        Self::fold_string_build_loops(f, trace)
    }

    /// Recognizes `s = s + piece` accumulation loops over strings.
    fn match_string_build_loop(
        f: &Function,
        cfg: &ControlFlowGraph,
        lp: &crate::dmir::cfg::NaturalLoop,
    ) -> Option<StringFoldPlan> {
        let dbg = std::env::var("DATARA_DEBUG_STRFOLD").is_ok();
        if dbg {
            eprintln!(
                "STRFOLD: matcher entered, header={:?} body={:?}",
                lp.header, lp.back_edges
            );
        }
        macro_rules! bail {
            ($why:expr) => {{
                if dbg {
                    eprintln!("STRFOLD bail: {}", $why);
                }
                return None;
            }};
        }
        // Same strict 2-block countable shape as the sum fold.
        if lp.blocks.len() != 2 || lp.back_edges.len() != 1 {
            bail!("shape: blocks/back-edges");
        }
        let header = lp.header;
        let body = lp.back_edges[0];
        if body == header || !lp.blocks.contains(&body) {
            bail!("reason#2");
        }
        if f.entry_block == header || f.entry_block == body {
            bail!("reason#3");
        }

        let header_blk = f.get_block(header)?;
        let body_blk = f.get_block(body)?;

        // Header params: the induction slot must be present; the string
        // accumulator may live as a block param OR in a named variable
        // (the default lowering keeps `mut s` in a variable slot and the
        // body reloads it with `LoadVar` each iteration).
        if header_blk.params.is_empty() || header_blk.params.len() > 2 {
            bail!("reason#4");
        }
        if header_blk.instructions.len() != 1 {
            bail!("reason#5");
        }
        let (cond_vid, p_i, n, is_le) = match &header_blk.instructions[0] {
            Inst::BinOp {
                dest,
                op,
                left,
                right,
                ..
            } if op == "<" => (*dest, *left, *right, false),
            Inst::BinOp {
                dest,
                op,
                left,
                right,
                ..
            } if op == "<=" => (*dest, *left, *right, true),
            _ => {
                bail!("reason#5b: header inst is not an Int comparison");
            }
        };
        // i64::MAX bound would wrap on the +1 adjustment of the trip count.
        if is_le && Self::const_expr_int_value(f, n) == Some(i64::MAX) {
            bail!("reason#6");
        }
        // Locate the accumulator: a String param (named-variable form,
        // params.len() == 1) or the String slot of the 2-param form.
        let var_form = header_blk.params.len() == 1;
        let p_s: ValueId = if var_form {
            if !header_blk.params[0].ty.contains("Int") {
                bail!("reason#7-var-slot-not-int");
            }
            // The var form carries the accumulator in a named variable, not
            // in a header parameter; the body scan below locates it. p_s
            // aliases the induction parameter so the concat-detection
            // guards against param slots simply never fire for it.
            p_i
        } else {
            let (p_a, p_b) = (header_blk.params[0].val, header_blk.params[1].val);
            let ty_a = header_blk.params[0].ty.as_str();
            let ty_b = header_blk.params[1].ty.as_str();
            let i_is_a = p_i == p_a;
            let (p_s_slot, s_ty) = if i_is_a {
                (p_b, ty_b)
            } else if p_i == p_b {
                (p_a, ty_a)
            } else {
                bail!("reason#7");
            };
            if !(s_ty == "String" || s_ty.contains("Str")) {
                bail!("reason#8");
            }
            p_s_slot
        };
        let exit = match &header_blk.terminator {
            Terminator::CondBranch {
                cond,
                then_block,
                then_args,
                else_block,
                else_args,
            } if *cond == cond_vid
                && *then_block == body
                && then_args.is_empty()
                && else_args.is_empty() =>
            {
                *else_block
            }
            _ => return None,
        };
        if exit == body || exit == header {
            bail!("reason#9");
        }

        // Back edge must forward the accumulator + induction in the same
        // slots the header expects: a 2-param header takes (i_next, s_next)
        // as branch args; a 1-param (var-form) header takes only i_next and
        // the accumulator flows through the named variable slot.
        let args = match &body_blk.terminator {
            Terminator::Branch { target, args } if *target == header => args,
            _ => return None,
        };
        if !var_form && args.len() != 2 {
            bail!("reason#10");
        }
        if var_form && args.len() != 1 {
            bail!("reason#10-var");
        }
        let idx_i = header_blk.params.iter().position(|p| p.val == p_i)?;
        let expected_i = args[idx_i];
        // Var-form: accumulator is whichever LoadVar'd variable the body's
        // concat feeds and AssignVar writes back. Param-form: expected_s is
        // the back-edge arg in the accumulator's slot.
        let expected_s = if var_form {
            ValueId(usize::MAX)
        } else {
            args[1 - idx_i]
        };

        // Body: s_next = concat(p_s, piece) | concat(piece, p_s), plus i+1.
        // Var-form additionally matches `s_next = concat(LoadVar "s", piece)`
        // where the load's variable is written back by AssignVar(s_next).
        let mut s_next: Option<ValueId> = None;
        let mut piece: Option<ValueId> = None;
        let mut i_next: Option<ValueId> = None;
        let mut acc_var: Option<String> = None;
        // Every concat in the body appends the SAME invariant piece to the
        // accumulator. The 2x unrolled latch carries two concat/assign
        // pairs per iteration, so count them: the closed form repeats the
        // piece `appends_per_iter * trips` times. All appends must share
        // one piece value for the fold to be a pure count.
        let mut concat_count = 0usize;
        for inst in &body_blk.instructions {
            match inst {
                Inst::LoadVar { dest, name, .. } if var_form => {
                    // Candidate accumulator load: remember the name; its
                    // range is verified once the concat is located.
                    acc_var.get_or_insert_with(|| name.clone());
                    let _ = dest;
                }
                Inst::Call {
                    dest,
                    func,
                    args: cargs,
                    ty,
                    ..
                } if (func == "datara_rt_str_concat" || func == "datara_rt_str_concat_mut")
                    && cargs.len() == 2
                    && (ty == "String" || ty.contains("Str")) =>
                {
                    // Accumulator operand: the param (param form) or the
                    // var-form load feeding this concat (matched by being
                    // the load dest we saw; approximated by taking the
                    // first operand when the other is the invariant piece).
                    let acc_operand = if var_form {
                        // Var form: find the operand that is a LoadVar dest
                        // (the accumulator read) — the other is the piece.
                        let load_dests: Vec<ValueId> = body_blk
                            .instructions
                            .iter()
                            .filter_map(|i| match i {
                                Inst::LoadVar { dest, .. } => Some(*dest),
                                _ => None,
                            })
                            .collect();
                        if load_dests.contains(&cargs[0]) {
                            Some(cargs[0])
                        } else if load_dests.contains(&cargs[1]) {
                            Some(cargs[1])
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let is_acc = |v: ValueId| {
                        if var_form {
                            acc_operand == Some(v)
                        } else {
                            v == p_s
                        }
                    };
                    if *dest == expected_s || var_form {
                        if is_acc(cargs[0]) {
                            match piece {
                                Some(p) if p != cargs[1] => {}
                                _ => {
                                    piece = Some(cargs[1]);
                                    concat_count += 1;
                                    if acc_operand == Some(cargs[0]) || cargs[0] == p_s || var_form
                                    {
                                        s_next = Some(*dest);
                                    }
                                }
                            }
                        } else if is_acc(cargs[1]) {
                            match piece {
                                Some(p) if p != cargs[0] => {}
                                _ => {
                                    piece = Some(cargs[0]);
                                    concat_count += 1;
                                    if acc_operand == Some(cargs[1]) || cargs[1] == p_s || var_form
                                    {
                                        s_next = Some(*dest);
                                    }
                                }
                            }
                        }
                    }
                }
                Inst::BinOp {
                    dest,
                    op,
                    left,
                    right,
                    ty,
                } if (op == "+" || op == "wrapping_+") => {
                    if *dest == expected_i {
                        if (*left == p_i && Self::const_int_value(f, *right) == Some(1))
                            || (*right == p_i && Self::const_int_value(f, *left) == Some(1))
                        {
                            i_next = Some(*dest);
                        } else {
                            // Unrolled latch: follow the chain of +1 links
                            // rooted at the induction parameter.
                            let chain_ok = (|| {
                                let mut cursor = *left;
                                for _ in 0..8 {
                                    if cursor == p_i {
                                        return Self::const_int_value(f, *right) == Some(1);
                                    }
                                    let mut prev = None;
                                    for other in &body_blk.instructions {
                                        if let Inst::BinOp {
                                            dest: d2,
                                            op: op2,
                                            left: l2,
                                            right: r2,
                                            ..
                                        } = other
                                            && d2 == &cursor
                                            && (*op2 == "+" || *op2 == "wrapping_+")
                                        {
                                            prev = Some((*l2, *r2));
                                            break;
                                        }
                                    }
                                    let Some((l2, r2)) = prev else {
                                        return false;
                                    };
                                    if Self::const_int_value(f, r2) != Some(1) {
                                        return false;
                                    }
                                    cursor = l2;
                                }
                                false
                            })();
                            if chain_ok {
                                i_next = Some(*dest);
                            }
                        }
                    } else if *dest == expected_s
                        && (ty == "String" || ty.contains("Str"))
                        && op == "+"
                    {
                        if *left == p_s {
                            piece = Some(*right);
                            s_next = Some(*dest);
                        } else if *right == p_s {
                            piece = Some(*left);
                            s_next = Some(*dest);
                        }
                    }
                }
                Inst::ConstStr { .. } => {}
                Inst::ConstInt { .. } => {}
                _ => {}
            }
        }
        let appends_per_iter = concat_count;
        let (s_next, piece, i_next) = match (s_next, piece, i_next) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => bail!("reason#11b: no concat/increment chain matched in body"),
        };
        if !var_form && (s_next != expected_s || i_next != expected_i) {
            bail!("reason#11");
        }
        if var_form {
            // The var form requires the write-back: AssignVar(acc_var, s_next)
            // in the body, and i_next must be the back-edge arg.
            let written_back = body_blk.instructions.iter().any(|i| match i {
                Inst::AssignVar { name, value } => {
                    acc_var.as_deref() == Some(name.as_str()) && *value == s_next
                }
                _ => false,
            });
            if !written_back || i_next != expected_i {
                bail!("reason#11-var");
            }
        }

        // Everything defined inside the loop.
        let mut loop_defs: HashSet<ValueId> = HashSet::new();
        loop_defs.insert(p_s);
        loop_defs.insert(p_i);
        for blk in [header_blk, body_blk] {
            for inst in &blk.instructions {
                if let Some(d) = Self::dest(inst) {
                    loop_defs.insert(d);
                }
            }
        }

        // Loop bound and appended piece must be loop-invariant.
        if loop_defs.contains(&n) || loop_defs.contains(&piece) {
            bail!("reason#12");
        }

        // Exactly one entering edge, no edge into the body from outside.
        let mut preheader: Option<BasicBlockId> = None;
        for b in &f.blocks {
            if lp.blocks.contains(&b.id) {
                continue;
            }
            let (to_header, to_body) = match &b.terminator {
                Terminator::Branch { target, .. } => (*target == header, *target == body),
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => (
                    *then_block == header || *else_block == header,
                    *then_block == body || *else_block == body,
                ),
                _ => (false, false),
            };
            if to_body {
                bail!("reason#13");
            }
            if to_header {
                if preheader.is_some() {
                    bail!("reason#14");
                }
                preheader = Some(b.id);
            }
        }
        let preheader = preheader?;
        // Var form: the preheader seeds the accumulator via AssignVar and
        // forwards only the induction; param form forwards both. The seed
        // for the var form is resolved from the preheader's AssignVar of
        // the accumulator variable.
        let (s0, i0) = if var_form {
            let ph = f.get_block(preheader)?;
            let i_arg = match &ph.terminator {
                Terminator::Branch { target, args } if *target == header && args.len() == 1 => {
                    args[0]
                }
                _ => return None,
            };
            // The seed is the ConstStr/Call producing the variable's initial
            // value in the preheader — the LAST AssignVar before the branch.
            let mut seed = None;
            for inst in &ph.instructions {
                if let Inst::AssignVar { name, value } = inst
                    && acc_var.as_deref() == Some(name.as_str())
                {
                    seed = Some(*value);
                }
            }
            let s_arg = match seed {
                Some(v) => v,
                None => bail!("reason#15-var-no-seed"),
            };
            (s_arg, i_arg)
        } else {
            match &f.get_block(preheader)?.terminator {
                Terminator::Branch { target, args } if *target == header && args.len() == 2 => {
                    let s_arg = args[1 - idx_i];
                    let i_arg = args[idx_i];
                    (s_arg, i_arg)
                }
                _ => return None,
            }
        };
        if loop_defs.contains(&s0) {
            bail!("reason#15");
        }
        let start = match Self::const_expr_int_value(f, i0) {
            Some(0) => 0,
            Some(1) => 1,
            _ => return None,
        };

        // The exit block must be reachable only through the loop.
        let exit_blk = f.get_block(exit)?;
        if !exit_blk.params.is_empty() {
            bail!("reason#16");
        }
        let mut exit_preds = 0;
        for b in &f.blocks {
            let targets_exit = match &b.terminator {
                Terminator::Branch { target, .. } => *target == exit,
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => *then_block == exit || *else_block == exit,
                _ => false,
            };
            if targets_exit {
                exit_preds += 1;
            }
        }
        if exit_preds != 1 {
            bail!("reason#17");
        }

        // Uses outside the loop: only the accumulator's may survive.
        if !Self::outside_uses_ok(f, cfg, lp, exit, &loop_defs, p_s) {
            bail!("reason#18");
        }

        Some(StringFoldPlan {
            header,
            body,
            exit,
            preheader,
            p_s,
            s0,
            n,
            piece,
            start,
            is_le,
            appends_per_iter,
            acc_var,
        })
    }

    /// Applies the string-build fold: exit block computes
    /// `s_final = concat(s0, repeat(piece, trips))` with trips guarded <= 0.
    fn apply_string_fold(
        f: &mut Function,
        plan: &StringFoldPlan,
        var_form_name: Option<String>,
        trace: &mut OptimizationDecisionTrace,
    ) {
        let mut next = Self::max_vid(f) + 1;
        let mut insts: Vec<Inst> = Vec::new();

        let c0 = {
            let d = ValueId(next);
            next += 1;
            insts.push(Inst::ConstInt { dest: d, value: 0 });
            d
        };
        let c1 = {
            let d = ValueId(next);
            next += 1;
            insts.push(Inst::ConstInt { dest: d, value: 1 });
            d
        };

        // trips = n - start (+1 when <=), guarded <= 0 by select.
        let raw_trips = {
            let start_v = {
                let z = ValueId(next);
                next += 1;
                insts.push(Inst::ConstInt {
                    dest: z,
                    value: plan.start,
                });
                z
            };
            let d = ValueId(next);
            next += 1;
            insts.push(Inst::BinOp {
                dest: d,
                op: "-".to_string(),
                left: plan.n,
                right: start_v,
                ty: "Int".to_string(),
            });
            if plan.is_le {
                let t = ValueId(next);
                next += 1;
                insts.push(Inst::BinOp {
                    dest: t,
                    op: "+".to_string(),
                    left: d,
                    right: c1,
                    ty: "Int".to_string(),
                });
                t
            } else {
                d
            }
        };
        // After a 2x unroll the latch advances the induction by
        // `appends_per_iter` per pass, so the pass count is the total
        // iteration count divided by the per-pass step. `appends_per_iter`
        // is a match-time constant >= 1, so the divisor is proven non-zero.
        let raw_trips = if plan.appends_per_iter > 1 {
            let mult = {
                let z = ValueId(next);
                next += 1;
                insts.push(Inst::ConstInt {
                    dest: z,
                    value: plan.appends_per_iter as i64,
                });
                z
            };
            let d = ValueId(next);
            next += 1;
            insts.push(Inst::BinOp {
                dest: d,
                op: "/".to_string(),
                left: raw_trips,
                right: mult,
                ty: "Int".to_string(),
            });
            d
        } else {
            raw_trips
        };
        let trips_pos = {
            let d = ValueId(next);
            next += 1;
            insts.push(Inst::BinOp {
                dest: d,
                op: ">".to_string(),
                left: raw_trips,
                right: c0,
                ty: "Bool".to_string(),
            });
            d
        };
        let trips = {
            let d = ValueId(next);
            next += 1;
            insts.push(Inst::Select {
                dest: d,
                cond: trips_pos,
                then_val: raw_trips,
                else_val: c0,
                ty: "Int".to_string(),
            });
            d
        };
        // repeated = str_repeat(piece, appends_per_iter * trips)
        let repeated = {
            let total = if plan.appends_per_iter <= 1 {
                trips
            } else {
                let mult = {
                    let d = ValueId(next);
                    next += 1;
                    insts.push(Inst::ConstInt {
                        dest: d,
                        value: plan.appends_per_iter as i64,
                    });
                    d
                };
                let d = ValueId(next);
                next += 1;
                insts.push(Inst::BinOp {
                    dest: d,
                    op: "*".to_string(),
                    left: trips,
                    right: mult,
                    ty: "Int".to_string(),
                });
                d
            };
            let d = ValueId(next);
            next += 1;
            insts.push(Inst::Call {
                dest: d,
                func: "datara_rt_str_repeat".to_string(),
                args: vec![plan.piece, total],
                ty: "String".to_string(),
            });
            d
        };
        // s_final = concat(s0, repeated) — byte order identical to the loop.
        let s_final = {
            let d = ValueId(next);
            insts.push(Inst::Call {
                dest: d,
                func: "datara_rt_str_concat".to_string(),
                args: vec![plan.s0, repeated],
                ty: "String".to_string(),
            });
            d
        };

        // Prepend the closed form to the exit block. The var form keeps the
        // accumulator in a named variable that the exit reloads with
        // `LoadVar`, so the write-back `AssignVar(acc_var, s_final)` is
        // prepended too (before the loads).
        {
            if let Some(var_name) = var_form_name {
                insts.push(Inst::AssignVar {
                    name: var_name,
                    value: s_final,
                });
            }
            let exit_blk = f.get_block_mut(plan.exit).expect("exit block must exist");
            let tail = std::mem::take(&mut exit_blk.instructions);
            insts.extend(tail);
            exit_blk.instructions = insts;
        }

        // Redirect the preheader straight to the exit.
        {
            let ph = f
                .get_block_mut(plan.preheader)
                .expect("preheader block must exist");
            ph.terminator = Terminator::Branch {
                target: plan.exit,
                args: Vec::new(),
            };
        }

        // Delete the loop blocks, rewrite surviving uses of the accumulator.
        f.blocks
            .retain(|b| b.id != plan.header && b.id != plan.body);
        for b in f.blocks.iter_mut() {
            for inst in b.instructions.iter_mut() {
                Self::rewrite_uses(inst, plan.p_s, s_final);
            }
            Self::rewrite_term_uses(&mut b.terminator, plan.p_s, s_final);
        }

        trace.record(
            "StringLoopFold",
            &format!("{}:bb{}_string_loop_folded", f.name, plan.header.0),
            "Applied",
            "O(n) string appends replaced by one str_repeat allocation",
            "two extra Int ops in the exit block",
            "countable string-build loop: final value proven == seed + piece repeated \
             (n - start) times in exact source order; zero trips guarded by select",
        );
    }
}
