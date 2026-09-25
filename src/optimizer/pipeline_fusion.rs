use crate::dmir::{Function, Inst, Terminator, ValueId};
use crate::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use std::collections::{HashMap, HashSet};

pub struct PipelineFusionOptimizer;

impl PipelineFusionOptimizer {
    /// Applies high-level pipeline fusion optimizations:
    /// 1. N-ary String Polyhedral Fusion: collapses chained binary string
    ///    concatenations into single atomic 3/4/5-ary runtime calls with a single allocation.
    /// 2. Arithmetic Pipeline Reassociation: collapses multi-stage associative
    ///    constant operations ((x + C1) + C2 => x + (C1 + C2)).
    pub fn fuse_pipelines(
        f: &mut Function,
        _cost_model: &CostModel,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        Self::fuse_pipelines_with_globals(f, _cost_model, trace, &HashSet::new())
    }

    /// Public surface for tests/verification: raw use-count of every ValueId.
    pub fn count_uses_public(f: &Function) -> HashMap<ValueId, usize> {
        Self::count_uses(f)
    }

    /// Public surface for tests/verification: run ONLY the append-promotion
    /// stage. Trace records go to a throwaway trace so callers can assert on
    /// the IR delta alone.
    pub fn promote_unique_appends_public(
        f: &mut Function,
        uses: &HashMap<ValueId, usize>,
        module_mutable_globals: &HashSet<String>,
    ) -> usize {
        let mut trace = OptimizationDecisionTrace::new();
        Self::promote_unique_appends(f, uses, &mut trace, module_mutable_globals)
    }

    pub fn fuse_pipelines_with_globals(
        f: &mut Function,
        _cost_model: &CostModel,
        trace: &mut OptimizationDecisionTrace,
        module_mutable_globals: &HashSet<String>,
    ) -> usize {
        let mut total_fused = 0;

        // 0. Unique-owner append promotion (P1): rewrite `s = s + x` into
        // the in-place datara_rt_str_concat_mut entry when the previous
        // value of `s` is provably dead. Runs before N-ary fusion so the
        // binary append pattern survives.
        let uses = Self::count_uses(f);
        total_fused += Self::promote_unique_appends(f, &uses, trace, module_mutable_globals);

        // 1. String Concatenation Fusion
        total_fused += Self::fuse_string_concatenations(f, &uses, trace);

        // Recompute use counts: concat fusion rewrote instructions, so the
        // stale map would misreport single-use intermediates below.
        let uses = Self::count_uses(f);

        // 2. Arithmetic Pipeline Reassociation
        total_fused += Self::fuse_arithmetic_chains(f, &uses, trace);

        total_fused
    }

    fn count_uses(f: &Function) -> HashMap<ValueId, usize> {
        let mut uses = HashMap::new();
        for b in &f.blocks {
            for inst in &b.instructions {
                Self::for_each_inst_use(inst, |vid| {
                    *uses.entry(vid).or_insert(0) += 1;
                });
            }
            match &b.terminator {
                Terminator::Branch { args, .. } => {
                    for a in args {
                        *uses.entry(*a).or_insert(0) += 1;
                    }
                }
                Terminator::CondBranch {
                    cond,
                    then_args,
                    else_args,
                    ..
                } => {
                    *uses.entry(*cond).or_insert(0) += 1;
                    for a in then_args {
                        *uses.entry(*a).or_insert(0) += 1;
                    }
                    for a in else_args {
                        *uses.entry(*a).or_insert(0) += 1;
                    }
                }
                Terminator::Return { value: Some(v) } => {
                    *uses.entry(*v).or_insert(0) += 1;
                }
                _ => {}
            }
        }
        uses
    }

    fn for_each_inst_use<F: FnMut(ValueId)>(inst: &Inst, mut f: F) {
        Self::for_each_inst_use_dyn(inst, &mut f);
    }

    fn for_each_inst_use_dyn(inst: &Inst, f: &mut dyn FnMut(ValueId)) {
        match inst {
            Inst::BinOp { left, right, .. } => {
                f(*left);
                f(*right);
            }
            Inst::UnOp { operand, .. } => f(*operand),
            Inst::Call { args, .. } => {
                for a in args {
                    f(*a);
                }
            }
            Inst::MethodCall { object, args, .. } => {
                f(*object);
                for a in args {
                    f(*a);
                }
            }
            Inst::AssignVar { value, .. } => f(*value),
            Inst::Out { value, .. } => f(*value),
            Inst::Err { value } => f(*value),
            Inst::StructInit { fields, .. } => {
                for (_, v) in fields {
                    f(*v);
                }
            }
            Inst::GetField { object, .. } => f(*object),
            Inst::SetField { object, value, .. } => {
                f(*object);
                f(*value);
            }
            Inst::FormatStr { values, .. } => {
                for v in values {
                    f(*v);
                }
            }
            Inst::Select {
                cond,
                then_val,
                else_val,
                ..
            } => {
                f(*cond);
                f(*then_val);
                f(*else_val);
            }
            Inst::Decide { arms, else_val, .. } => {
                for (cond, val) in arms {
                    f(*cond);
                    f(*val);
                }
                if let Some(ev) = else_val {
                    f(*ev);
                }
            }
            Inst::WhileLoop {
                condition_insts,
                cond_val,
                body_insts,
            } => {
                f(*cond_val);
                for ci in condition_insts {
                    Self::for_each_inst_use_dyn(ci, f);
                }
                for bi in body_insts {
                    Self::for_each_inst_use_dyn(bi, f);
                }
            }
            Inst::Return { value: Some(v) } => f(*v),
            _ => {}
        }
    }

    /// Fuses trees of binary string concatenations into N-ary calls
    fn fuse_string_concatenations(
        f: &mut Function,
        uses: &HashMap<ValueId, usize>,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let mut fused_count = 0;

        let mut val_types: HashMap<ValueId, String> = HashMap::new();
        for b in &f.blocks {
            for inst in &b.instructions {
                match inst {
                    Inst::ConstStr { dest, .. } => {
                        val_types.insert(*dest, "String".to_string());
                    }
                    Inst::ConstInt { dest, .. } => {
                        val_types.insert(*dest, "Int".to_string());
                    }
                    Inst::ConstFloat { dest, .. } => {
                        val_types.insert(*dest, "Float".to_string());
                    }
                    Inst::BinOp { dest, ty, .. } => {
                        val_types.insert(*dest, ty.clone());
                    }
                    Inst::Call { dest, ty, .. } => {
                        val_types.insert(*dest, ty.clone());
                    }
                    _ => {}
                }
            }
        }

        for block in &mut f.blocks {
            // Map each dest of a concat to its (left, right) or arg list
            let mut concat_map: HashMap<ValueId, Vec<ValueId>> = HashMap::new();

            for inst in &mut block.instructions {
                let (dest, args) = match inst {
                    Inst::BinOp {
                        dest,
                        op,
                        left,
                        right,
                        ty,
                    } if op == "+" && (ty == "String" || ty.contains("Str")) => {
                        (*dest, vec![*left, *right])
                    }
                    Inst::Call {
                        dest, func, args, ..
                    } if func == "datara_rt_str_concat" && args.len() == 2 => (*dest, args.clone()),
                    Inst::Call {
                        dest, func, args, ..
                    } if func == "datara_rt_str_concat_3" && args.len() == 3 => {
                        (*dest, args.clone())
                    }
                    Inst::Call {
                        dest, func, args, ..
                    } if func == "datara_rt_str_concat_4" && args.len() == 4 => {
                        (*dest, args.clone())
                    }
                    _ => continue,
                };

                // Check if the first argument is itself a single-use string concat
                let first_arg = args[0];
                if uses.get(&first_arg).copied().unwrap_or(0) <= 1
                    && let Some(prev_args) = concat_map.get(&first_arg)
                {
                    let mut combined = prev_args.clone();
                    combined.extend(&args[1..]);

                    if combined.len() <= 5 {
                        // Check for direct-slot string+int wire-blit pattern [str, int, str, int]
                        let is_sisi = combined.len() == 4
                            && (val_types
                                .get(&combined[0])
                                .map(|s| s == "String")
                                .unwrap_or(true))
                            && val_types
                                .get(&combined[1])
                                .map(|s| s == "Int")
                                .unwrap_or(false)
                            && (val_types
                                .get(&combined[2])
                                .map(|s| s == "String")
                                .unwrap_or(true))
                            && val_types
                                .get(&combined[3])
                                .map(|s| s == "Int")
                                .unwrap_or(false);

                        // Rewrite the current instruction into an N-ary concat or direct-slot format call
                        let new_func = if is_sisi {
                            "datara_rt_format_str_i64_str_i64"
                        } else {
                            match combined.len() {
                                3 => "datara_rt_str_concat_3",
                                4 => "datara_rt_str_concat_4",
                                5 => "datara_rt_str_concat_5",
                                _ => "datara_rt_str_concat",
                            }
                        };

                        *inst = Inst::Call {
                            dest,
                            func: new_func.to_string(),
                            args: combined.clone(),
                            ty: "String".to_string(),
                        };

                        concat_map.insert(dest, combined);
                        fused_count += 1;

                        trace.record(
                                "StringConcatPolyhedralFusion",
                                &format!("{}:bb{}", f.name, block.id.0),
                                "Applied",
                                &format!("Fused string tree into {}", new_func),
                                "Eliminated intermediate string buffer allocations",
                                "Transformed chained string concatenation into single-allocation N-ary runtime call",
                            );
                        continue;
                    }
                }

                concat_map.insert(dest, args);
            }
        }

        fused_count
    }

    /// Reassociates arithmetic pipelines:
    /// ((x + C1) + C2 => x + (C1 + C2))
    /// ((x * C1) * C2 => x * (C1 * C2))
    /// ((x & C1) & C2 => x & (C1 & C2))
    /// ((x | C1) | C2 => x | (C1 | C2))
    fn fuse_arithmetic_chains(
        f: &mut Function,
        uses: &HashMap<ValueId, usize>,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let mut fused_count = 0;

        // Collect all ConstInt values in the function
        let mut const_ints = HashMap::new();
        for b in &f.blocks {
            for inst in &b.instructions {
                if let Inst::ConstInt { dest, value } = inst {
                    const_ints.insert(*dest, *value);
                }
            }
        }

        let mut next_vid = Self::max_vid(f) + 1;

        for block in &mut f.blocks {
            // Map dest -> (op, base_var, constant_value)
            let mut affine_chains: HashMap<ValueId, (String, ValueId, i64)> = HashMap::new();
            let mut new_instructions = Vec::new();

            for inst in &block.instructions {
                if let Inst::BinOp {
                    dest,
                    op,
                    left,
                    right,
                    ty,
                } = inst
                    && (ty == "Int" || ty == "i64")
                {
                    let is_c_right = const_ints.get(right).copied();
                    let is_c_left = const_ints.get(left).copied();

                    let is_commutative = matches!(op.as_str(), "+" | "*" | "&" | "|" | "^");

                    let (var, c_val) = match (is_c_right, is_c_left) {
                        (Some(c), None) => (*left, c),
                        (None, Some(c)) if is_commutative => (*right, c),
                        _ => (ValueId(usize::MAX), 0),
                    };

                    if var.0 != usize::MAX {
                        if uses.get(&var).copied().unwrap_or(0) <= 1
                            && let Some((prev_op, base_var, prev_c)) = affine_chains.get(&var)
                            && prev_op == op
                        {
                            let combined = match op.as_str() {
                                "+" => Some(prev_c.wrapping_add(c_val)),
                                "*" => Some(prev_c.wrapping_mul(c_val)),
                                "&" => Some(*prev_c & c_val),
                                "|" => Some(*prev_c | c_val),
                                "^" => Some(*prev_c ^ c_val),
                                _ => None,
                            };

                            if let Some(new_k) = combined {
                                let new_c_vid = ValueId(next_vid);
                                next_vid += 1;
                                const_ints.insert(new_c_vid, new_k);

                                new_instructions.push(Inst::ConstInt {
                                    dest: new_c_vid,
                                    value: new_k,
                                });

                                new_instructions.push(Inst::BinOp {
                                    dest: *dest,
                                    op: op.clone(),
                                    left: *base_var,
                                    right: new_c_vid,
                                    ty: ty.clone(),
                                });

                                affine_chains.insert(*dest, (op.clone(), *base_var, new_k));
                                fused_count += 1;

                                trace.record(
                                                "ArithmeticPipelineReassociation",
                                                &format!("{}:bb{}", f.name, block.id.0),
                                                "Applied",
                                                &format!("Folded associative pipeline on {}", op),
                                                "Eliminated chained arithmetic instruction latency",
                                                "Reassociated multi-stage constant pipeline into single constant operation",
                                            );
                                continue;
                            }
                        }

                        affine_chains.insert(*dest, (op.clone(), var, c_val));
                    }
                }

                new_instructions.push(inst.clone());
            }

            block.instructions = new_instructions;
        }

        fused_count
    }

    fn max_vid(f: &Function) -> usize {
        let mut m = 0;
        for (_, _, p_val) in &f.params {
            m = m.max(p_val.0);
        }
        for b in &f.blocks {
            for p in &b.params {
                m = m.max(p.val.0);
            }
            for inst in &b.instructions {
                Self::for_each_inst_dest(inst, &mut |vid| {
                    m = m.max(vid.0);
                });
            }
        }
        m
    }

    /// Concat runtime symbols (copy semantics): promotable to in-place
    /// mutation when the first operand is a proven-unique append target.
    fn is_consumed_concat(fn_name: &str) -> bool {
        fn_name == "datara_rt_str_concat"
            || fn_name == "datara_rt_str_concat_3"
            || fn_name == "datara_rt_str_concat_4"
            || fn_name == "datara_rt_str_concat_5"
    }

    /// String methods that only OBSERVE the receiver: they return a scalar
    /// (Int/Bool) computed from the bytes and cannot retain the buffer
    /// pointer. `s.len()` after an append chain is the canonical case —
    /// without this whitelist the final read would be treated as an alias
    /// and every append promotion would be rejected. Methods returning Str
    /// (slice, char_at, ...) may hand out pointers into the buffer and stay
    /// conservatively escaping.
    fn is_string_observer_method(method: &str) -> bool {
        matches!(
            method,
            "len"
                | "count"
                | "byte_len"
                | "char_len"
                | "is_empty"
                | "byte_at"
                | "index_of"
                | "starts_with"
                | "ends_with"
                | "contains"
                | "validate_utf8"
        )
    }

    /// P1: rewrite `s = s + x` into `datara_rt_str_concat_mut(s, x)` when the
    /// previous value of `s` is provably dead after the assignment, i.e. the
    /// in-place buffer mutation is sound.
    ///
    /// The DMIR shape after lowering is a linear triple inside one block:
    ///     %t = LoadVar s            (uses(%t) == 1: only the append reads it)
    ///     ...rhs insts...
    ///     %r = BinOp{op:"+", ty:String, left:%t}  (or a 2-arg Call concat)
    ///          AssignVar { name: s, value: %r }
    ///
    /// Soundness contract (the runtime still double-checks a per-buffer
    /// ownership magic and falls back to the copy concat):
    ///   1. `s` is a function-local `mut`: not a parameter, not a block param,
    ///      and not a mutable module global (all of those can alias external
    ///      buffers that in-place growth would corrupt).
    ///   2. The loaded LHS operand is used exactly once — by this append.
    ///   3. Every OTHER load of `s` in the function is transient: its value is
    ///      only observed (Out, FormatStr, string BinOp, branch) and never
    ///      stored into another slot, passed to an opaque call, written to a
    ///      field, or returned — none of those can alias the buffer.
    ///   4. Only 2-operand appends are rewritten; N-ary fused concats keep
    ///      copy semantics (they also fail check 2 by construction).
    fn promote_unique_appends(
        f: &mut Function,
        uses: &HashMap<ValueId, usize>,
        trace: &mut OptimizationDecisionTrace,
        module_mutable_globals: &HashSet<String>,
    ) -> usize {
        // SSA defs of loads: vid -> loaded name (each vid is defined once).
        // Cloned strings end the borrow of `f.blocks` before the rewrite pass.
        let mut load_defs: HashMap<ValueId, String> = HashMap::new();
        for b in &f.blocks {
            for inst in &b.instructions {
                if let Inst::LoadVar { dest, name } = inst {
                    load_defs.insert(*dest, name.clone());
                }
            }
        }

        // Name -> every vid ever loaded from it.
        let mut loads_of: HashMap<String, Vec<ValueId>> = HashMap::new();
        for (vid, name) in &load_defs {
            loads_of.entry(name.clone()).or_default().push(*vid);
        }

        // Buffers we may NOT grow in place: params, named block params,
        // mutable module globals. All of them can be aliased elsewhere.
        let mut nonlocal_names: HashSet<String> = HashSet::new();
        for p in &f.params {
            nonlocal_names.insert(p.0.clone());
        }
        for b in &f.blocks {
            for p in &b.params {
                if let Some(name) = &p.name {
                    nonlocal_names.insert(name.clone());
                }
            }
        }

        // Pass 1: find candidate appends. A candidate is a 2-operand string
        // concat (Call or string BinOp) immediately feeding an AssignVar back
        // into the slot its first operand was loaded from.
        struct Candidate {
            block: usize,
            at: usize,
            load_vid: ValueId,
            slot: String,
        }
        let mut candidates: Vec<Candidate> = Vec::new();
        for (bi, b) in f.blocks.iter().enumerate() {
            if b.instructions.len() < 2 {
                continue;
            }
            for i in 0..b.instructions.len() - 1 {
                let (vid0, slot) = match (&b.instructions[i], &b.instructions[i + 1]) {
                    (
                        Inst::Call {
                            dest, func, args, ..
                        },
                        Inst::AssignVar { name, value },
                    ) if Self::is_consumed_concat(func) && args.len() == 2 && value == dest => {
                        (args[0], name)
                    }
                    (
                        Inst::BinOp {
                            dest, op, left, ty, ..
                        },
                        Inst::AssignVar { name, value },
                    ) if op == "+" && (ty == "String" || ty.contains("Str")) && value == dest => {
                        (*left, name)
                    }
                    _ => continue,
                };
                if let Some(def_name) = load_defs.get(&vid0) {
                    if *def_name == slot.as_str() {
                        candidates.push(Candidate {
                            block: bi,
                            at: i,
                            load_vid: vid0,
                            slot: slot.clone(),
                        });
                    }
                }
            }
        }
        if candidates.is_empty() {
            return 0;
        }

        // Pass 1b: which value uses ESCAPE (create aliases)? A use escapes when
        // the value is stored into a slot, fed to an opaque callee, written to
        // a field, embedded in a struct, or returned. Pure observers (Out,
        // Err, FormatStr, string BinOp inputs, Cast, Select, branches) only
        // read the value and die: they never retain the buffer pointer.
        // `skip` marks the one deliberately-consumed operand of a candidate
        // append (the LHS of the concat itself).
        type UseKey = (usize, usize); // (block_idx, inst_idx)
        let mut escaping: HashSet<ValueId> = HashSet::new();
        let mut consumed: HashSet<(ValueId, UseKey)> = HashSet::new();
        for c in &candidates {
            consumed.insert((c.load_vid, (c.block, c.at)));
        }

        for (bi, b) in f.blocks.iter().enumerate() {
            for (i, inst) in b.instructions.iter().enumerate() {
                let key = (bi, i);
                match inst {
                    Inst::AssignVar { value, .. } => {
                        escaping.insert(*value);
                    }
                    Inst::Call { args, .. } => {
                        for (ai, a) in args.iter().enumerate() {
                            if ai == 0 && consumed.contains(&(*a, key)) {
                                continue; // the append's own consumed LHS
                            }
                            escaping.insert(*a);
                        }
                    }
                    Inst::MethodCall {
                        object,
                        method,
                        args,
                        ..
                    } => {
                        if !Self::is_string_observer_method(method) {
                            escaping.insert(*object);
                        }
                        for a in args {
                            escaping.insert(*a);
                        }
                    }
                    Inst::SetField { value, .. } => {
                        escaping.insert(*value);
                    }
                    Inst::StructInit { fields, .. } => {
                        for (_, v) in fields {
                            escaping.insert(*v);
                        }
                    }
                    Inst::Return { value: Some(v) } => {
                        escaping.insert(*v);
                    }
                    // Transient observers: no alias is retained.
                    Inst::Out { .. }
                    | Inst::Err { .. }
                    | Inst::FormatStr { .. }
                    | Inst::BinOp { .. }
                    | Inst::UnOp { .. }
                    | Inst::Cast { .. }
                    | Inst::Select { .. }
                    | Inst::LoadVar { .. }
                    | Inst::WhileLoop { .. } => {}
                    // Conservative default for anything unmodeled: any use it
                    // reads escapes (e.g. InlineAsm, GetField reading object).
                    _ => {
                        Self::for_each_inst_use_dyn(inst, &mut |vid| {
                            escaping.insert(vid);
                        });
                    }
                }
            }
            // Branch arguments outlive the block (phi inputs elsewhere).
            match &b.terminator {
                Terminator::Branch { args, .. } => {
                    for a in args {
                        escaping.insert(*a);
                    }
                }
                Terminator::CondBranch {
                    cond,
                    then_args,
                    else_args,
                    ..
                } => {
                    escaping.insert(*cond);
                    for a in then_args {
                        escaping.insert(*a);
                    }
                    for a in else_args {
                        escaping.insert(*a);
                    }
                }
                _ => {}
            }
        }

        // Pass 2: verify and rewrite.
        let mut count = 0;
        for c in &candidates {
            // (1) Local mut only: no params, no block params, no mut globals.
            if nonlocal_names.contains(c.slot.as_str()) || module_mutable_globals.contains(&c.slot)
            {
                continue;
            }
            // (2) The LHS load is read by this append alone.
            if uses.get(&c.load_vid).copied().unwrap_or(0) != 1 {
                continue;
            }
            // (3) All other loads of the slot are transient observers.
            let others_safe = loads_of
                .get(c.slot.as_str())
                .map(|vids| {
                    vids.iter()
                        .all(|v| *v == c.load_vid || !escaping.contains(v))
                })
                .unwrap_or(true);
            if !others_safe {
                continue;
            }

            let b = &mut f.blocks[c.block];
            match &b.instructions[c.at] {
                Inst::Call { .. } => {
                    if let Inst::Call { func, .. } = &mut b.instructions[c.at] {
                        *func = "datara_rt_str_concat_mut".to_string();
                    }
                }
                Inst::BinOp { .. } => {
                    let new_inst = match &b.instructions[c.at] {
                        Inst::BinOp {
                            dest, left, right, ..
                        } => Inst::Call {
                            dest: *dest,
                            func: "datara_rt_str_concat_mut".to_string(),
                            args: vec![*left, *right],
                            ty: "String".to_string(),
                        },
                        _ => unreachable!(),
                    };
                    b.instructions[c.at] = new_inst;
                }
                _ => continue,
            }
            count += 1;
            trace.record(
                "UniqueOwnerAppend",
                &format!("{}:bb{}:{}", f.name, b.id.0, c.slot),
                "Applied",
                "promoted append to datara_rt_str_concat_mut",
                "Amortized O(1) in-place string append",
                "Previous LHS value is provably dead (single use, no alias)",
            );
        }

        count
    }

    fn for_each_inst_dest<F: FnMut(ValueId)>(inst: &Inst, f: &mut F) {
        match inst {
            Inst::ConstInt { dest, .. }
            | Inst::ConstFloat { dest, .. }
            | Inst::ConstStr { dest, .. }
            | Inst::ConstBool { dest, .. }
            | Inst::LoadVar { dest, .. }
            | Inst::BinOp { dest, .. }
            | Inst::UnOp { dest, .. }
            | Inst::Cast { dest, .. }
            | Inst::Call { dest, .. }
            | Inst::MethodCall { dest, .. }
            | Inst::StructInit { dest, .. }
            | Inst::GetField { dest, .. }
            | Inst::FormatStr { dest, .. }
            | Inst::Select { dest, .. }
            | Inst::Decide { dest, .. }
            | Inst::GetFuncAddr { dest, .. } => f(*dest),
            Inst::InlineAsm { outputs, .. } => {
                for (_, d) in outputs {
                    f(*d);
                }
            }
            Inst::WhileLoop {
                condition_insts,
                body_insts,
                ..
            } => {
                for ci in condition_insts {
                    Self::for_each_inst_dest(ci, f);
                }
                for bi in body_insts {
                    Self::for_each_inst_dest(bi, f);
                }
            }
            _ => {}
        }
    }
}
