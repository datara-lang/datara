use super::*;
use crate::ast::Refinement;
use crate::dmir::cfg::ControlFlowGraph;
use crate::dmir::{BasicBlockId, Function, Inst, Terminator, ValueId};
use crate::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use std::collections::{HashMap, HashSet};

impl LoopOptimizer {
    pub fn bce_pass(
        f: &mut Function,
        _cost_model: &CostModel,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let mut eliminated = 0;
        let cfg = ControlFlowGraph::build(f);

        // Whole-function value lattice
        #[derive(Clone, Copy, PartialEq, Debug)]
        enum LenVal {
            Const(i64),
            Vid(ValueId),
        }

        let mut consts: HashMap<ValueId, i64> = HashMap::new();
        let mut list_len: HashMap<ValueId, LenVal> = HashMap::new();
        let mut len_to_arr: HashMap<ValueId, ValueId> = HashMap::new();
        let mut val_to_name: HashMap<ValueId, String> = HashMap::new();
        let mut var_of_val: HashMap<ValueId, String> = HashMap::new();
        let mut name_to_val: HashMap<String, ValueId> = HashMap::new();
        let mut copy_of: HashMap<ValueId, ValueId> = HashMap::new();
        let mut assigned: HashSet<String> = HashSet::new();
        let mut var_len_of_arr: HashMap<String, String> = HashMap::new();
        let mut var_len: HashMap<String, LenVal> = HashMap::new();

        for (p_name, _ty, p_val) in &f.params {
            name_to_val.insert(p_name.clone(), *p_val);
            val_to_name.insert(*p_val, p_name.clone());
            var_of_val.insert(*p_val, p_name.clone());
        }
        for block in &f.blocks {
            for param in &block.params {
                if let Some(n) = &param.name {
                    name_to_val.insert(n.clone(), param.val);
                    val_to_name.insert(param.val, n.clone());
                    var_of_val.insert(param.val, n.clone());
                }
            }
        }

        for block in &f.blocks {
            for inst in &block.instructions {
                match inst {
                    Inst::ConstInt { dest, value } => {
                        consts.insert(*dest, *value);
                    }
                    Inst::Call {
                        func, args, dest, ..
                    } if func == "datara_rt_list_create_repeat" && args.len() == 2 => {
                        if let Some(&c) = consts.get(&args[1]) {
                            list_len.insert(*dest, LenVal::Const(c));
                        } else if let Some(&src) = copy_of.get(&args[1]) {
                            if let Some(&c) = consts.get(&src) {
                                list_len.insert(*dest, LenVal::Const(c));
                            } else {
                                list_len.insert(*dest, LenVal::Vid(args[1]));
                            }
                        } else {
                            list_len.insert(*dest, LenVal::Vid(args[1]));
                        }
                    }
                    Inst::Call {
                        func, args, dest, ..
                    } if (func == "datara_rt_list_append"
                        || func == "datara_rt_list_set"
                        || func == "datara_rt_list_set_unchecked"
                        || func == "datara_rt_list_set_f64_unchecked")
                        && !args.is_empty() =>
                    {
                        copy_of.insert(*dest, args[0]);
                        if let Some(&l) = list_len.get(&args[0]) {
                            list_len.insert(*dest, l);
                        }
                    }
                    Inst::MethodCall {
                        method,
                        object,
                        dest,
                        ..
                    } if method == "push" || method == "append" => {
                        copy_of.insert(*dest, *object);
                        if let Some(&l) = list_len.get(object) {
                            list_len.insert(*dest, l);
                        }
                    }
                    Inst::Call { func, dest, .. } if func == "datara_rt_list_create_1" => {
                        list_len.insert(*dest, LenVal::Const(1));
                    }
                    Inst::Call { func, dest, .. } if func == "datara_rt_list_create_2" => {
                        list_len.insert(*dest, LenVal::Const(2));
                    }
                    Inst::Call { func, dest, .. } if func == "datara_rt_list_create_3" => {
                        list_len.insert(*dest, LenVal::Const(3));
                    }
                    Inst::Call { func, dest, .. } if func == "datara_rt_list_create_4" => {
                        list_len.insert(*dest, LenVal::Const(4));
                    }
                    Inst::Call { func, dest, .. } if func == "datara_rt_list_create_5" => {
                        list_len.insert(*dest, LenVal::Const(5));
                    }
                    Inst::Call {
                        func, args, dest, ..
                    } if (func == "datara_rt_list_len" || func == "len") && !args.is_empty() => {
                        list_len.insert(args[0], LenVal::Vid(*dest));
                        list_len.insert(*dest, LenVal::Vid(*dest));
                        len_to_arr.insert(*dest, args[0]);
                    }
                    Inst::MethodCall {
                        dest,
                        object,
                        method,
                        args,
                        ..
                    } if (method == "len" || method == "length") && args.is_empty() => {
                        list_len.insert(*object, LenVal::Vid(*dest));
                        list_len.insert(*dest, LenVal::Vid(*dest));
                        len_to_arr.insert(*dest, *object);
                    }
                    Inst::Call {
                        func, args, dest, ..
                    } if func == "datara_rt_slice" && args.len() == 3 => {
                        if consts.get(&args[1]) == Some(&0) {
                            if let Some(&c) = consts.get(&args[2]) {
                                list_len.insert(*dest, LenVal::Const(c));
                            } else {
                                list_len.insert(*dest, LenVal::Vid(args[2]));
                            }
                        }
                    }
                    Inst::AssignVar { name, value } => {
                        assigned.insert(name.clone());
                        var_of_val.insert(*value, name.clone());
                        if let Some(&l) = list_len.get(value) {
                            var_len.insert(name.clone(), l);
                        } else if let Some(&c) = consts.get(value) {
                            var_len.insert(name.clone(), LenVal::Const(c));
                        }
                        if let Some(prev) = name_to_val.get(name) {
                            if *prev != *value {
                                val_to_name.remove(prev);
                                name_to_val.remove(name);
                            }
                        } else {
                            name_to_val.insert(name.clone(), *value);
                            val_to_name.insert(*value, name.clone());
                        }
                    }
                    Inst::LoadVar { dest, name } => {
                        val_to_name.insert(*dest, name.clone());
                        var_of_val.insert(*dest, name.clone());
                        if let Some(&v) = name_to_val.get(name) {
                            copy_of.insert(*dest, v);
                        }
                        if let Some(&l) = var_len.get(name) {
                            list_len.insert(*dest, l);
                        }
                    }
                    Inst::UnOp {
                        dest, op, operand, ..
                    } if op == "copy" => {
                        copy_of.insert(*dest, *operand);
                        if let Some(&l) = list_len.get(operand) {
                            list_len.insert(*dest, l);
                        }
                    }
                    Inst::Select {
                        dest,
                        then_val,
                        else_val,
                        ..
                    } => {
                        if then_val == else_val {
                            copy_of.insert(*dest, *then_val);
                        }
                    }
                    Inst::BinOp {
                        dest,
                        op,
                        left,
                        right,
                        ..
                    } => {
                        if op == "|" || op == "or" || op == "wrapping_|" {
                            if consts.get(right) == Some(&0) {
                                copy_of.insert(*dest, *left);
                            } else if consts.get(left) == Some(&0) {
                                copy_of.insert(*dest, *right);
                            }
                        } else if op == "+" || op == "wrapping_+" {
                            if consts.get(right) == Some(&0) {
                                copy_of.insert(*dest, *left);
                            } else if consts.get(left) == Some(&0) {
                                copy_of.insert(*dest, *right);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Link variable names to array lengths where n = arr.len()
        for (len_vid, arr_vid) in &len_to_arr {
            let len_name = val_to_name.get(len_vid);
            let arr_name = val_to_name.get(arr_vid);
            if let (Some(ln), Some(an)) = (len_name, arr_name) {
                var_len_of_arr.insert(ln.clone(), an.clone());
            }
        }

        fn resolve_vid(mut v: ValueId, copy_of: &HashMap<ValueId, ValueId>) -> ValueId {
            for _ in 0..32 {
                match copy_of.get(&v) {
                    Some(&next) if next != v => v = next,
                    _ => return v,
                }
            }
            v
        }

        // Chase copy/name chains; a dead end is itself a valid identity key.
        fn resolve(
            mut v: ValueId,
            copy_of: &HashMap<ValueId, ValueId>,
            list_len: &HashMap<ValueId, LenVal>,
            consts: &HashMap<ValueId, i64>,
        ) -> LenVal {
            for _ in 0..32 {
                if let Some(l) = list_len.get(&v) {
                    match l {
                        LenVal::Const(c) => return LenVal::Const(*c),
                        LenVal::Vid(lv) => {
                            if let Some(c) = consts.get(lv) {
                                return LenVal::Const(*c);
                            }
                            if let Some(&next) = copy_of.get(lv) {
                                if next != *lv {
                                    v = next;
                                    continue;
                                }
                            }
                            return *l;
                        }
                    }
                }
                if let Some(c) = consts.get(&v) {
                    return LenVal::Const(*c);
                }
                match copy_of.get(&v) {
                    Some(&next) if next != v => v = next,
                    _ => return LenVal::Vid(v),
                }
            }
            LenVal::Vid(v)
        }

        // Resolve a value to the variable name it was loaded from.
        fn resolve_name(
            mut v: ValueId,
            copy_of: &HashMap<ValueId, ValueId>,
            val_to_name: &HashMap<ValueId, String>,
        ) -> Option<String> {
            for _ in 0..32 {
                if let Some(n) = val_to_name.get(&v) {
                    return Some(n.clone());
                }
                match copy_of.get(&v) {
                    Some(&next) if next != v => v = next,
                    _ => return None,
                }
            }
            None
        }

        let const_val = |vid: ValueId| -> Option<i64> { consts.get(&vid).copied() };

        let mut block_param_incoming: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
        for blk in &f.blocks {
            match &blk.terminator {
                Terminator::Branch { target, args } => {
                    if let Some(tb) = f.get_block(*target) {
                        for (p, a) in tb.params.iter().zip(args) {
                            block_param_incoming.entry(p.val).or_default().push(*a);
                        }
                    }
                }
                Terminator::CondBranch {
                    then_block,
                    then_args,
                    else_block,
                    else_args,
                    ..
                } => {
                    if let Some(tb) = f.get_block(*then_block) {
                        for (p, a) in tb.params.iter().zip(then_args) {
                            block_param_incoming.entry(p.val).or_default().push(*a);
                        }
                    }
                    if let Some(eb) = f.get_block(*else_block) {
                        for (p, a) in eb.params.iter().zip(else_args) {
                            block_param_incoming.entry(p.val).or_default().push(*a);
                        }
                    }
                }
                _ => {}
            }
        }

        let resolve_name_deep = |v: ValueId| -> Option<String> {
            let mut visited = HashSet::new();
            let mut queue = vec![v];
            while let Some(curr) = queue.pop() {
                if let Some(n) = val_to_name.get(&curr).or_else(|| var_of_val.get(&curr)) {
                    return Some(n.clone());
                }
                let resolved = resolve_vid(curr, &copy_of);
                if let Some(n) = val_to_name
                    .get(&resolved)
                    .or_else(|| var_of_val.get(&resolved))
                {
                    return Some(n.clone());
                }
                if !visited.insert(resolved) {
                    continue;
                }
                if let Some(incomings) = block_param_incoming.get(&resolved) {
                    for &inc in incomings {
                        queue.push(inc);
                    }
                }
            }
            None
        };

        let mut mul_map: HashMap<ValueId, (ValueId, ValueId)> = HashMap::new();
        let mut add_map: HashMap<ValueId, (ValueId, ValueId)> = HashMap::new();
        for blk in &f.blocks {
            for inst in &blk.instructions {
                match inst {
                    Inst::BinOp {
                        dest,
                        op,
                        left,
                        right,
                        ..
                    } => {
                        if op == "*" || op == "wrapping_*" {
                            mul_map.insert(
                                *dest,
                                (resolve_vid(*left, &copy_of), resolve_vid(*right, &copy_of)),
                            );
                        } else if op == "+" || op == "wrapping_+" {
                            add_map.insert(
                                *dest,
                                (resolve_vid(*left, &copy_of), resolve_vid(*right, &copy_of)),
                            );
                        }
                    }
                    Inst::Call {
                        func, args, dest, ..
                    } if args.len() == 2 => {
                        if func == "datara_rt_checked_mul" {
                            mul_map.insert(
                                *dest,
                                (
                                    resolve_vid(args[0], &copy_of),
                                    resolve_vid(args[1], &copy_of),
                                ),
                            );
                        } else if func == "datara_rt_checked_add" {
                            add_map.insert(
                                *dest,
                                (
                                    resolve_vid(args[0], &copy_of),
                                    resolve_vid(args[1], &copy_of),
                                ),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        let find_mul = |v: ValueId| -> Option<ValueId> {
            let mut visited = HashSet::new();
            let mut queue = vec![v];
            while let Some(curr) = queue.pop() {
                let resolved = resolve_vid(curr, &copy_of);
                if mul_map.contains_key(&resolved) {
                    return Some(resolved);
                }
                if !visited.insert(resolved) {
                    continue;
                }
                if let Some(incomings) = block_param_incoming.get(&resolved) {
                    for &inc in incomings {
                        queue.push(inc);
                    }
                }
            }
            None
        };

        // Track list lengths populated in countable loops: `while i < bound { list.push(...) }`
        for lp in &cfg.loops {
            let header_block = match f.get_block(lp.header) {
                Some(b) => b,
                None => continue,
            };
            let mut bound_opt = None;
            if let Terminator::CondBranch { cond, .. } = &header_block.terminator {
                for inst in &header_block.instructions {
                    if let Inst::BinOp {
                        dest, op, right, ..
                    } = inst
                    {
                        if dest == cond && op == "<" {
                            bound_opt = Some(*right);
                        }
                    }
                }
            }
            if let Some(bound_vid) = bound_opt {
                for &bid in &lp.blocks {
                    if let Some(blk) = f.get_block(bid) {
                        for inst in &blk.instructions {
                            match inst {
                                Inst::Call {
                                    func, args, dest, ..
                                } if (func == "datara_rt_list_append"
                                    || func == "push"
                                    || func == "append")
                                    && !args.is_empty() =>
                                {
                                    list_len.insert(args[0], LenVal::Vid(bound_vid));
                                    list_len.insert(*dest, LenVal::Vid(bound_vid));
                                    if let Some(an) = resolve_name_deep(args[0]) {
                                        var_len.insert(an, LenVal::Vid(bound_vid));
                                    }
                                }
                                Inst::MethodCall {
                                    method,
                                    object,
                                    dest,
                                    ..
                                } if method == "push" || method == "append" => {
                                    list_len.insert(*object, LenVal::Vid(bound_vid));
                                    list_len.insert(*dest, LenVal::Vid(bound_vid));
                                    if let Some(an) = resolve_name_deep(*object) {
                                        var_len.insert(an, LenVal::Vid(bound_vid));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // Automatic List Preallocation: rewrite `datara_rt_list_create(0)` to `bound`
        let mut preallocated_lists: HashSet<String> = HashSet::new();
        let mut defined_vids: HashSet<ValueId> = f.params.iter().map(|(_, _, pv)| *pv).collect();
        for blk in &mut f.blocks {
            for inst in &mut blk.instructions {
                if let Inst::Call {
                    func, args, dest, ..
                } = inst
                {
                    if func == "datara_rt_list_create"
                        && args.len() == 1
                        && consts.get(&args[0]) == Some(&0)
                    {
                        if let Some(an) = resolve_name_deep(*dest) {
                            if let Some(&vl) = var_len.get(&an) {
                                let find_c_vid = |c: i64| -> Option<ValueId> {
                                    consts
                                        .iter()
                                        .find(|(k, v)| {
                                            **v == c
                                                && (defined_vids.contains(k)
                                                    || f.params.iter().any(|(_, _, pv)| pv == *k))
                                        })
                                        .map(|(k, _)| *k)
                                };
                                match vl {
                                    LenVal::Const(c) if c > 0 => {
                                        if let Some(c_vid) = find_c_vid(c) {
                                            args[0] = c_vid;
                                            preallocated_lists.insert(an);
                                        }
                                    }
                                    LenVal::Vid(b_vid) => {
                                        let mut q = vec![b_vid];
                                        let mut vis = HashSet::new();
                                        let mut chosen = None;
                                        while let Some(cur) = q.pop() {
                                            let r = resolve_vid(cur, &copy_of);
                                            if defined_vids.contains(&r)
                                                || f.params.iter().any(|(_, _, pv)| *pv == r)
                                            {
                                                chosen = Some(r);
                                                break;
                                            }
                                            if let Some(&c) = consts.get(&r) {
                                                if c > 0 {
                                                    if let Some(c_vid) = find_c_vid(c) {
                                                        chosen = Some(c_vid);
                                                        break;
                                                    }
                                                }
                                            }
                                            if vis.insert(r) {
                                                if let Some(incs) = block_param_incoming.get(&r) {
                                                    q.extend(incs);
                                                }
                                            }
                                        }
                                        if let Some(ch) = chosen {
                                            args[0] = ch;
                                            preallocated_lists.insert(an);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                match inst {
                    Inst::ConstInt { dest, .. }
                    | Inst::ConstFloat { dest, .. }
                    | Inst::ConstStr { dest, .. }
                    | Inst::ConstBool { dest, .. }
                    | Inst::LoadVar { dest, .. }
                    | Inst::BinOp { dest, .. }
                    | Inst::UnOp { dest, .. }
                    | Inst::Call { dest, .. }
                    | Inst::MethodCall { dest, .. }
                    | Inst::StructInit { dest, .. } => {
                        defined_vids.insert(*dest);
                    }
                    _ => {}
                }
            }
        }

        // Rewrite append/push inside preallocated population loops to unchecked variants
        if !preallocated_lists.is_empty() {
            for lp in &cfg.loops {
                for &bid in &lp.blocks {
                    if let Some(blk) = f.get_block_mut(bid) {
                        for inst in &mut blk.instructions {
                            match inst {
                                Inst::Call { func, args, .. }
                                    if func == "datara_rt_list_append" && !args.is_empty() =>
                                {
                                    if let Some(an) = resolve_name_deep(args[0]) {
                                        if preallocated_lists.contains(&an) {
                                            *func = "datara_rt_list_append_unchecked".to_string();
                                        }
                                    }
                                }
                                Inst::MethodCall { method, object, .. }
                                    if method == "push" || method == "append" =>
                                {
                                    if let Some(an) = resolve_name_deep(*object) {
                                        if preallocated_lists.contains(&an) {
                                            *method = "push_unchecked".to_string();
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // --- Evidence Gate Bounds Check Elimination (BCE) ---
        // Proves 0 <= idx < len(arr) from parameter refinement types (e.g. idx: Int in 0..<arr.len())
        // or contract preconditions (require 0 <= idx && idx < arr.len()).
        let mut proven_bounds: HashMap<String, String> = HashMap::new();

        let is_param = |name: &str| f.params.iter().any(|(p, _, _)| p == name);

        for (p_name, refn_opt) in &f.param_refinements {
            if let Some(refn) = refn_opt {
                match refn {
                    Refinement::Range {
                        start,
                        end,
                        inclusive,
                    } => {
                        if !inclusive
                            && Self::is_zero_expr(start)
                            && let Some(arr_name) = Self::extract_len_target(end)
                        {
                            proven_bounds.insert(p_name.clone(), arr_name);
                        }
                    }
                    Refinement::Predicate {
                        var_name,
                        predicate,
                    } => {
                        if let Some(arr_name) =
                            Self::extract_predicate_len_target(var_name, predicate)
                        {
                            proven_bounds.insert(p_name.clone(), arr_name);
                        }
                    }
                }
            }
        }

        for req in &f.requires {
            if let Some((idx_name, arr_name)) =
                Self::extract_index_bound_from_contract(&req.condition)
            {
                if is_param(&idx_name) && is_param(&arr_name) {
                    proven_bounds.insert(idx_name, arr_name);
                }
            }
        }

        let mut evidence_gate_count = 0;
        if !proven_bounds.is_empty() {
            for block in &mut f.blocks {
                for inst in &mut block.instructions {
                    if let Inst::Call { func, args, .. } = inst
                        && (func == "datara_rt_list_get" || func == "datara_rt_list_set")
                        && args.len() >= 2
                    {
                        let arr_name = resolve_name(args[0], &copy_of, &val_to_name);
                        let idx_name = resolve_name(args[1], &copy_of, &val_to_name);
                        if let (Some(arr), Some(idx)) = (arr_name, idx_name)
                            && proven_bounds.get(&idx) == Some(&arr)
                            && !assigned.contains(&idx)
                            && !assigned.contains(&arr)
                        {
                            *func = format!("{}_unchecked", func);
                            eliminated += 1;
                            evidence_gate_count += 1;
                        }
                    }
                }
            }
        }

        if evidence_gate_count > 0 {
            trace.record(
                "EvidenceGate:BCE",
                &format!("{}:param_refinement", f.name),
                "Applied",
                &format!("+{} BCE proven unchecked access", evidence_gate_count),
                "0",
                &format!(
                    "BCE proven: 0 <= idx < len(arr) via refinement/contract for {} accesses: bypassed runtime bounds check",
                    evidence_gate_count
                ),
            );
        }

        // --- Condition Dominator Bounds Check Elimination ---
        // If block B is dominated by a conditional branch testing idx < len(arr) (and idx >= 0),
        // then bounds checks on arr[idx] inside B are guaranteed redundant.
        let mut dominator_eliminated = 0;
        for block_idx in 0..f.blocks.len() {
            let block_id = f.blocks[block_idx].id;
            let mut dom_checks: Vec<(ValueId, ValueId)> = Vec::new();
            for other_block in &f.blocks {
                if let Terminator::CondBranch {
                    cond, then_block, ..
                } = &other_block.terminator
                {
                    if cfg.dominates(*then_block, block_id) {
                        for inst in &other_block.instructions {
                            if let Inst::BinOp {
                                dest,
                                op,
                                left,
                                right,
                                ..
                            } = inst
                            {
                                if dest == cond && op == "<" {
                                    let resolved_right = resolve_vid(*right, &copy_of);
                                    if let Some(&arr_target) = len_to_arr.get(&resolved_right) {
                                        dom_checks.push((*left, arr_target));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !dom_checks.is_empty() {
                let block = &mut f.blocks[block_idx];
                for inst in &mut block.instructions {
                    if let Inst::Call { func, args, .. } = inst {
                        if (func == "datara_rt_list_get" || func == "datara_rt_list_set")
                            && args.len() >= 2
                        {
                            let arr_vid = resolve_vid(args[0], &copy_of);
                            let idx_vid = resolve_vid(args[1], &copy_of);
                            let is_safe = dom_checks.iter().any(|(chk_idx, chk_arr)| {
                                resolve_vid(*chk_idx, &copy_of) == idx_vid
                                    && resolve_vid(*chk_arr, &copy_of) == arr_vid
                            });
                            if is_safe {
                                *func = format!("{}_unchecked", func);
                                eliminated += 1;
                                dominator_eliminated += 1;
                            }
                        }
                    }
                }
            }
        }

        if dominator_eliminated > 0 {
            trace.record(
                "ConditionDominator:BCE",
                &format!("{}:dominator", f.name),
                "Applied",
                &format!("+{} BCE proven unchecked access", dominator_eliminated),
                "0",
                &format!(
                    "BCE proven: 0 <= idx < len(arr) verified by dominating condition for {} accesses",
                    dominator_eliminated
                ),
            );
        }

        // --- Loop Induction Variable Range Analysis ---
        let mut loop_eliminated = 0;
        for lp in &cfg.loops {
            let header_block = match f.get_block(lp.header) {
                Some(b) => b,
                None => continue,
            };

            // Canonical header: `cond = (i < bound)`. <= is rejected: it
            // allows `idx == bound`, which is out of range when len == bound.
            let (induction_var, bound_val) = match &header_block.terminator {
                Terminator::CondBranch { cond, .. } => {
                    let mut found = None;
                    for inst in &header_block.instructions {
                        if let Inst::BinOp {
                            dest,
                            op,
                            left,
                            right,
                            ..
                        } = inst
                            && dest == cond
                            && op == "<"
                        {
                            found = Some((*left, *right));
                        }
                    }
                    match found {
                        Some(v) => v,
                        None => continue,
                    }
                }
                _ => continue,
            };

            // Check if induction variable is an SSA block parameter OR a named counter
            let ssa_param_idx = header_block
                .params
                .iter()
                .position(|p| p.val == induction_var);
            let counter_name = resolve_name_deep(induction_var);

            let loop_blocks: HashSet<_> = lp.blocks.iter().copied().collect();

            let mut step_block: Option<BasicBlockId> = None;
            let mut step_idx: Option<usize> = None;
            let mut step_vid: Option<ValueId> = None;

            if let Some(param_idx) = ssa_param_idx {
                // SSA block parameter induction variable
                let entry_preds: Vec<(BasicBlockId, ValueId)> = f
                    .blocks
                    .iter()
                    .filter(|b| !loop_blocks.contains(&b.id))
                    .filter_map(|b| {
                        let arg = match &b.terminator {
                            Terminator::Branch { target, args } if *target == lp.header => {
                                args.get(param_idx).copied()
                            }
                            Terminator::CondBranch {
                                then_block,
                                then_args,
                                else_block,
                                else_args,
                                ..
                            } => {
                                if *then_block == lp.header {
                                    then_args.get(param_idx).copied()
                                } else if *else_block == lp.header {
                                    else_args.get(param_idx).copied()
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        arg.map(|a| (b.id, a))
                    })
                    .collect();

                let entry_zero = !entry_preds.is_empty()
                    && entry_preds.iter().all(|(_, arg)| {
                        matches!(
                            resolve(*arg, &copy_of, &list_len, &consts),
                            LenVal::Const(0)
                        ) || consts.get(arg) == Some(&0)
                    });

                if !entry_zero || lp.back_edges.is_empty() {
                    continue;
                }

                // Latches (loop blocks branching to header) must pass induction_var + 1
                let mut all_latches_step_one = true;
                let mut latch_step_vid = None;
                for &latch_id in &lp.back_edges {
                    let latch_blk = match f.get_block(latch_id) {
                        Some(b) => b,
                        None => {
                            all_latches_step_one = false;
                            break;
                        }
                    };
                    let arg = match &latch_blk.terminator {
                        Terminator::Branch { target, args } if *target == lp.header => {
                            args.get(param_idx).copied()
                        }
                        Terminator::CondBranch {
                            then_block,
                            then_args,
                            else_block,
                            else_args,
                            ..
                        } => {
                            if *then_block == lp.header {
                                then_args.get(param_idx).copied()
                            } else if *else_block == lp.header {
                                else_args.get(param_idx).copied()
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    let s_vid = match arg {
                        Some(v) => v,
                        None => {
                            all_latches_step_one = false;
                            break;
                        }
                    };

                    let mut is_step = false;
                    for b in &f.blocks {
                        for inst in &b.instructions {
                            let is_add = match inst {
                                Inst::BinOp {
                                    dest,
                                    op,
                                    left,
                                    right,
                                    ..
                                } if *dest == s_vid && (op == "+" || op == "wrapping_+") => {
                                    Some((*left, *right))
                                }
                                Inst::Call {
                                    func, args, dest, ..
                                } if *dest == s_vid
                                    && func == "datara_rt_checked_add"
                                    && args.len() == 2 =>
                                {
                                    Some((args[0], args[1]))
                                }
                                _ => None,
                            };
                            if let Some((left, right)) = is_add {
                                let left_res = resolve_vid(left, &copy_of);
                                let right_res = resolve_vid(right, &copy_of);
                                let left_const = resolve(left, &copy_of, &list_len, &consts);
                                let right_const = resolve(right, &copy_of, &list_len, &consts);
                                let iv_res = resolve_vid(induction_var, &copy_of);
                                let iv_name = resolve_name_deep(induction_var);
                                let l_matches = left_res == iv_res
                                    || (iv_name.is_some() && resolve_name_deep(left) == iv_name);
                                let r_matches = right_res == iv_res
                                    || (iv_name.is_some() && resolve_name_deep(right) == iv_name);
                                let r_is_one = right_const == LenVal::Const(1)
                                    || consts.get(&right) == Some(&1);
                                let l_is_one =
                                    left_const == LenVal::Const(1) || consts.get(&left) == Some(&1);
                                if (l_matches && r_is_one) || (r_matches && l_is_one) {
                                    is_step = true;
                                    break;
                                }
                            }
                        }
                        if is_step {
                            break;
                        }
                    }

                    if !is_step {
                        all_latches_step_one = false;
                        break;
                    }
                    latch_step_vid = Some(s_vid);
                }

                if !all_latches_step_one {
                    continue;
                }
                step_vid = latch_step_vid;
            } else if let Some(cname) = &counter_name {
                let mut init_zero = false;
                let mut in_loop_steps = 0;
                let mut rebound = false;
                for block in &f.blocks {
                    let in_loop = loop_blocks.contains(&block.id);
                    for (inst_idx, inst) in block.instructions.iter().enumerate() {
                        match inst {
                            Inst::AssignVar { name, value } if name == cname => {
                                let is_step = if in_loop {
                                    match Self::binop_add_one_source(f, *value) {
                                        Some(lhs) => {
                                            resolve_name(lhs, &copy_of, &val_to_name).as_deref()
                                                == Some(cname.as_str())
                                        }
                                        None => false,
                                    }
                                } else {
                                    false
                                };
                                if is_step {
                                    in_loop_steps += 1;
                                    step_block = Some(block.id);
                                    step_idx = Some(inst_idx);
                                    step_vid = Some(*value);
                                } else if in_loop {
                                    rebound = true;
                                } else if const_val(*value) == Some(0) {
                                    if cfg.dominates(block.id, lp.header) {
                                        init_zero = true;
                                    }
                                } else {
                                    rebound = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if !init_zero || in_loop_steps != 1 || rebound {
                    continue;
                }

                // Step block must be a latch
                let step_is_latch = step_block.map_or(false, |sb| {
                    f.get_block(sb).map_or(false, |b| {
                        matches!(&b.terminator, Terminator::Branch { target, .. } if *target == lp.header)
                    })
                });
                if !step_is_latch {
                    continue;
                }
            } else {
                continue;
            }

            // Safe induction step optimization: rewrite `i + 1` to `wrapping_+`
            if let Some(vid) = step_vid {
                for b in &mut f.blocks {
                    for inst in &mut b.instructions {
                        if let Inst::BinOp { dest, op, .. } = inst {
                            if *dest == vid && op == "+" {
                                *op = "wrapping_+".to_string();
                            }
                        } else if let Inst::Call {
                            func,
                            args,
                            dest,
                            ty,
                        } = inst
                        {
                            if *dest == vid && func == "datara_rt_checked_add" && args.len() == 2 {
                                *inst = Inst::BinOp {
                                    dest: *dest,
                                    op: "wrapping_+".to_string(),
                                    left: args[0],
                                    right: args[1],
                                    ty: ty.clone(),
                                };
                            }
                        }
                    }
                }
            }

            // Prove header bound <= indexed list's length
            let bound_len = resolve(bound_val, &copy_of, &list_len, &consts);
            let resolved_bound_vid = resolve_vid(bound_val, &copy_of);
            let bound_arr_vid = len_to_arr.get(&resolved_bound_vid).copied();
            let bound_name = resolve_name_deep(bound_val);

            for &block_id in &lp.blocks {
                if let Some(block) = f.get_block_mut(block_id) {
                    for (inst_idx, inst) in block.instructions.iter_mut().enumerate() {
                        if let Inst::Call { func, args, .. } = inst
                            && (func == "datara_rt_list_get" || func == "datara_rt_list_set")
                            && args.len() >= 2
                        {
                            if let Some(sb) = step_block {
                                if let Some(si) = step_idx {
                                    if block_id == sb && inst_idx >= si {
                                        continue;
                                    }
                                }
                            }

                            // The index must be the counter variable
                            let iv_name = resolve_name_deep(induction_var);
                            let iv_res = resolve_vid(induction_var, &copy_of);
                            let idx_res = resolve_vid(args[1], &copy_of);
                            let idx_name = resolve_name_deep(args[1]);
                            let matches_index = idx_res == iv_res
                                || (iv_name.is_some() && idx_name == iv_name)
                                || (counter_name.is_some() && idx_name == counter_name);

                            if !matches_index {
                                continue;
                            }

                            let mut list_len_val = resolve(args[0], &copy_of, &list_len, &consts);
                            let resolved_arr_vid = resolve_vid(args[0], &copy_of);
                            let arr_name = resolve_name_deep(args[0]);

                            if let Some(an) = &arr_name {
                                if let Some(&vl) = var_len.get(an) {
                                    if list_len_val == LenVal::Vid(args[0])
                                        || list_len_val == LenVal::Vid(resolved_arr_vid)
                                    {
                                        list_len_val = match vl {
                                            LenVal::Const(c) => LenVal::Const(c),
                                            LenVal::Vid(v) => {
                                                resolve(v, &copy_of, &list_len, &consts)
                                            }
                                        };
                                    }
                                }
                            }

                            let mut proven = match (bound_len, list_len_val) {
                                (LenVal::Vid(a), LenVal::Vid(b)) => {
                                    let a_res = resolve_vid(a, &copy_of);
                                    let b_res = resolve_vid(b, &copy_of);
                                    a_res == b_res
                                        || (resolve_name_deep(a).is_some()
                                            && resolve_name_deep(a) == resolve_name_deep(b))
                                }
                                (LenVal::Const(a), LenVal::Const(b)) => a <= b,
                                _ => false,
                            };

                            if !proven {
                                if let Some(target_arr) = bound_arr_vid {
                                    if resolve_vid(target_arr, &copy_of) == resolved_arr_vid {
                                        proven = true;
                                    }
                                }
                            }

                            if !proven {
                                if let (Some(bn), Some(an)) = (&bound_name, &arr_name) {
                                    if var_len_of_arr.get(bn) == Some(an) {
                                        proven = true;
                                    }
                                }
                            }

                            if proven {
                                *func = format!("{}_unchecked", func);
                                eliminated += 1;
                                loop_eliminated += 1;
                            }
                        }
                    }
                }
            }
        }

        if loop_eliminated > 0 {
            trace.record(
                "BCE",
                &format!("{}:loop_bounds", f.name),
                "Applied",
                &format!("+{} BCE proven unchecked access", loop_eliminated),
                "0",
                &format!(
                    "BCE proven: idx < len for {} accesses (canonical 0-based +1 loop bound tied to allocation length)",
                    loop_eliminated
                ),
            );
        }

        // --- 2D Row-Major Affine Index Bounds Check Elimination ---
        // Proves: 0 <= (I * N + J) < len(arr) when I < N, J < N, and len(arr) >= N * N.
        // Applicable to all matrix operations, 2D convolutions, image processing, and grids.
        let mut loop_bounds: Vec<(
            ValueId,
            Option<String>,
            ValueId,
            Option<String>,
            HashSet<BasicBlockId>,
        )> = Vec::new();
        for lp in &cfg.loops {
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
                            let left_res = resolve_vid(*left, &copy_of);
                            let right_res = resolve_vid(*right, &copy_of);
                            let left_name = resolve_name(*left, &copy_of, &val_to_name);
                            let right_name = resolve_name(*right, &copy_of, &val_to_name);
                            let loop_blocks: HashSet<_> = lp.blocks.iter().copied().collect();
                            loop_bounds.push((
                                left_res,
                                left_name,
                                right_res,
                                right_name,
                                loop_blocks,
                            ));
                        }
                    }
                }
            }
        }

        let mut affine_2d_eliminated = 0;
        for blk in &mut f.blocks {
            let blk_id = blk.id;
            for inst in &mut blk.instructions {
                if let Inst::Call { func, args, .. } = inst {
                    if (func == "datara_rt_list_get" || func == "datara_rt_list_set")
                        && args.len() >= 2
                    {
                        let arr_vid = resolve_vid(args[0], &copy_of);
                        let arr_name = resolve_name_deep(args[0]);
                        let idx_vid = resolve_vid(args[1], &copy_of);

                        if let Some(&(add_l, add_r)) = add_map.get(&idx_vid) {
                            let pairs = [(add_l, add_r), (add_r, add_l)];
                            for (mul_cand, off_cand) in pairs {
                                if let Some(resolved_mul) = find_mul(mul_cand) {
                                    if let Some(&(m1, m2)) = mul_map.get(&resolved_mul) {
                                        let factor_choices = [(m1, m2), (m2, m1)];
                                        for (i_val, n_val) in factor_choices {
                                            let j_val = off_cand;
                                            let is_bounded =
                                                |val: ValueId, bound: ValueId| -> bool {
                                                    let v_name = resolve_name_deep(val);
                                                    let b_name = resolve_name_deep(bound);
                                                    let v_res = resolve_vid(val, &copy_of);
                                                    let b_res = resolve_vid(bound, &copy_of);
                                                    loop_bounds.iter().any(
                                                        |(lv, ln, rv, rn, blocks)| {
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
                                                        },
                                                    )
                                                };

                                            if is_bounded(i_val, n_val) && is_bounded(j_val, n_val)
                                            {
                                                let arr_len_match = {
                                                    let mut ok = false;
                                                    let check_len_vid = |len_v: ValueId| -> bool {
                                                        if let Some(resolved_len) = find_mul(len_v)
                                                        {
                                                            if let Some(&(ml, mr)) =
                                                                mul_map.get(&resolved_len)
                                                            {
                                                                let ml_res =
                                                                    resolve_vid(ml, &copy_of);
                                                                let mr_res =
                                                                    resolve_vid(mr, &copy_of);
                                                                let n_res =
                                                                    resolve_vid(n_val, &copy_of);
                                                                let ml_name = resolve_name_deep(ml);
                                                                let mr_name = resolve_name_deep(mr);
                                                                let n_name =
                                                                    resolve_name_deep(n_val);
                                                                let m_ok =
                                                                    |r, n: &Option<String>| {
                                                                        r == n_res
                                                                            || (n.is_some()
                                                                                && n == &n_name)
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
                                                        list_len.get(&arr_vid).or_else(|| {
                                                            arr_name
                                                                .as_ref()
                                                                .and_then(|an| var_len.get(an))
                                                        });
                                                    if let Some(&LenVal::Vid(v)) = len_cand {
                                                        ok = check_len_vid(v);
                                                    }
                                                    ok
                                                };

                                                if arr_len_match {
                                                    *func = format!("{}_unchecked", func);
                                                    eliminated += 1;
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

        eliminated
    }
}
