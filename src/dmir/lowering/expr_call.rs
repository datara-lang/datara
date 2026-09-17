use crate::ast::*;
use crate::dmir::ir::*;
use crate::types::DataraType;

use super::Lowering;

impl<'a> Lowering<'a> {
    /// Runtime element representation of a list-valued expression. Runtime
    /// calls whose semantics depend on the element layout (sort's elem_kind,
    /// the Outcome contract of pop/first/last) key off this repr: string
    /// elements are char* pointers, floats travel as IEEE bit patterns,
    /// everything else compares as i64. Mirrors the for-in loop's element
    /// derivation (stmt.rs): the variable's checked element type wins,
    /// map/filter chains inherit their source list's element, anything
    /// opaque falls back to "Int".
    fn list_elem_repr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier(name, _) => match self.lookup_var_type(name) {
                Some(DataraType::List(inner)) => Self::repr_of_datara_type(&inner),
                _ => "Int".into(),
            },
            Expr::Call { callee, args, .. } => match &**callee {
                // Chained member form: `xs.map(f)` / `xs.filter(p)`.
                Expr::MemberAccess { object, member, .. }
                    if (member == "map" || member == "filter") && self.is_expr_list(expr) =>
                {
                    self.list_elem_repr(object)
                }
                // Free function form: `map(xs, f)` / `filter(xs, p)`.
                Expr::Identifier(fn_name, _)
                    if (fn_name == "map" || fn_name == "filter")
                        && !args.is_empty()
                        && self.is_expr_list(expr) =>
                {
                    self.list_elem_repr(&args[0])
                }
                _ => "Int".into(),
            },
            _ => "Int".into(),
        }
    }

    /// Checked element type -> backend repr string (same mapping as the
    /// for-in loop's elem_repr in stmt.rs).
    fn repr_of_datara_type(t: &DataraType) -> String {
        match t {
            DataraType::String | DataraType::Char => "String".into(),
            DataraType::Bool => "Bool".into(),
            DataraType::Float => "Float".into(),
            DataraType::List(_) => "List".into(),
            DataraType::Map(..) => "Map".into(),
            DataraType::Class(c) => c.clone(),
            _ => "Int".into(),
        }
    }

    pub(crate) fn lower_expr_call(
        &mut self,
        _expr: &Expr,
        callee: &Expr,
        args: &[Expr],
        cur_block: &mut BasicBlockId,
    ) -> Option<ValueId> {
        if let Expr::Lambda { params, body, .. } = callee {
            let mut arg_vals = Vec::new();
            for a in args {
                if let Some(av) = self.lower_expr(a, cur_block) {
                    arg_vals.push(av);
                }
            }
            for (p, aval) in params.iter().zip(arg_vals) {
                self.symbol_values.insert(p.name.clone(), aval);
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::AssignVar {
                        name: p.name.clone(),
                        value: aval,
                    });
            }
            return self.lower_expr(body, cur_block);
        }
        if let Expr::Identifier(fn_name, _) = callee {
            if let Some((params, body)) = self.local_lambdas.get(fn_name).cloned() {
                let mut arg_vals = Vec::new();
                for a in args {
                    if let Some(av) = self.lower_expr(a, cur_block) {
                        arg_vals.push(av);
                    }
                }
                // By-value captures: bind the registration-time snapshot of
                // every free variable, shadow-saving the caller's slots so
                // writes inside the body are rolled back afterwards.
                let captures = self
                    .lambda_captures
                    .get(fn_name)
                    .cloned()
                    .unwrap_or_default();
                let mut shadowed_captures: Vec<(String, Option<ValueId>)> = Vec::new();
                for (cname, cval) in &captures {
                    let old_val = self.symbol_values.get(cname).copied();
                    shadowed_captures.push((cname.clone(), old_val));
                    self.symbol_values.insert(cname.clone(), *cval);
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::AssignVar {
                            name: cname.clone(),
                            value: *cval,
                        });
                }
                for (p, aval) in params.iter().zip(arg_vals) {
                    self.symbol_values.insert(p.name.clone(), aval);
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::AssignVar {
                            name: p.name.clone(),
                            value: aval,
                        });
                }
                let res = self.lower_expr(&body, cur_block);
                for (cname, old_val) in shadowed_captures {
                    if let Some(ov) = old_val {
                        self.symbol_values.insert(cname.clone(), ov);
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::AssignVar {
                                name: cname,
                                value: ov,
                            });
                    } else {
                        self.symbol_values.remove(&cname);
                    }
                }
                return res;
            }
            if (fn_name == "view" || fn_name == "borrow" || fn_name == "clone" || fn_name == "move")
                && args.len() == 1
            {
                return self.lower_expr(&args[0], cur_block);
            }
            if (fn_name == "destroy" || fn_name == "drop") && args.len() == 1 {
                let arg_val = self.lower_expr(&args[0], cur_block);
                let dest = self.next_val();
                let mut call_args = Vec::new();
                if let Some(v) = arg_val {
                    call_args.push(v);
                }
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: fn_name.clone(),
                        args: call_args,
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "println" || fn_name == "print" {
                if args.is_empty() {
                    let dest = self.next_val();
                    if fn_name == "println" {
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::Call {
                                dest,
                                func: "datara_rt_print_newline".into(),
                                args: vec![],
                                ty: "Unit".into(),
                            });
                    } else {
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::ConstInt { dest, value: 0 });
                    }
                    return Some(dest);
                }

                for (idx, arg) in args.iter().enumerate() {
                    if idx > 0 {
                        let sp_dest = self.next_val();
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::Call {
                                dest: sp_dest,
                                func: "datara_rt_print_space".into(),
                                args: vec![],
                                ty: "Unit".into(),
                            });
                    }

                    let arg_val = self.lower_expr(arg, cur_block)?;
                    let print_func = if self.is_expr_str(arg) {
                        "datara_rt_print_str"
                    } else if self.is_expr_float(arg) {
                        "datara_rt_print_float"
                    } else if self.is_expr_bool(arg) {
                        "datara_rt_print_bool"
                    } else if self.is_expr_list(arg) {
                        "datara_rt_print_list"
                    } else {
                        "datara_rt_print_int"
                    };

                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: print_func.into(),
                            args: vec![arg_val],
                            ty: "Unit".into(),
                        });
                }

                let final_dest = self.next_val();
                if fn_name == "println" {
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: final_dest,
                            func: "datara_rt_print_newline".into(),
                            args: vec![],
                            ty: "Unit".into(),
                        });
                } else {
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: final_dest,
                            func: "datara_rt_flush".into(),
                            args: vec![],
                            ty: "Unit".into(),
                        });
                }

                return Some(final_dest);
            }
            if fn_name == "eprintln" && args.len() == 1 {
                let arg_val = self.lower_expr(&args[0], cur_block)?;
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Err { value: arg_val });
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::ConstInt { dest, value: 0 });
                return Some(dest);
            }
            if fn_name == "len" && args.len() == 1 {
                let arg_val = self.lower_expr(&args[0], cur_block)?;
                let dest = self.next_val();
                let func = if self.is_expr_str(&args[0]) {
                    "datara_rt_len"
                } else {
                    "datara_rt_list_len"
                };
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: func.into(),
                        args: vec![arg_val],
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "now" && args.is_empty() {
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_now_ms".into(),
                        args: vec![],
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "panic" && args.len() == 1 {
                let arg_val = self.lower_expr(&args[0], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_panic".into(),
                        args: vec![arg_val],
                        ty: "Never".into(),
                    });
                return Some(dest);
            }
            if (fn_name == "wrapping" || fn_name == "wrapping_mode") && args.len() == 1 {
                let prev = self.in_wrapping_mode;
                self.in_wrapping_mode = true;
                let res = self.lower_expr(&args[0], cur_block);
                self.in_wrapping_mode = prev;
                return res;
            }
            if (fn_name == "saturating" || fn_name == "saturating_mode") && args.len() == 1 {
                let prev = self.in_saturating_mode;
                self.in_saturating_mode = true;
                let res = self.lower_expr(&args[0], cur_block);
                self.in_saturating_mode = prev;
                return res;
            }
            if (fn_name == "map"
                || fn_name == "filter"
                || fn_name == "find"
                || fn_name == "any"
                || fn_name == "all")
                && args.len() == 2
                && self.is_expr_list(&args[0])
                && self.is_callable_expr(&args[1])
            {
                let list_val = self.lower_expr(&args[0], cur_block)?;
                return self.lower_list_higher_order(list_val, fn_name, &args[1..], cur_block);
            }
            if fn_name == "reduce"
                && args.len() == 3
                && self.is_expr_list(&args[0])
                && self.is_callable_expr(&args[2])
            {
                let list_val = self.lower_expr(&args[0], cur_block)?;
                return self.lower_list_higher_order(list_val, fn_name, &args[1..], cur_block);
            }
            // v1.4.1: fold(xs, init, acc_fn) -- same shape as reduce.
            if fn_name == "fold"
                && args.len() == 3
                && self.is_expr_list(&args[0])
                && self.is_callable_expr(&args[2])
            {
                let list_val = self.lower_expr(&args[0], cur_block)?;
                return self.lower_list_higher_order(list_val, fn_name, &args[1..], cur_block);
            }
            // collect(xs) materializes the pipeline sink; map/filter already
            // build eager result lists, so the list itself is the value.
            if fn_name == "collect" && args.len() == 1 && self.is_expr_list(&args[0]) {
                return self.lower_expr(&args[0], cur_block);
            }
            if fn_name == "wrapping_add" && args.len() == 2 {
                let l = self.lower_expr(&args[0], cur_block)?;
                let r = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::BinOp {
                        dest,
                        op: "wrapping_+".into(),
                        left: l,
                        right: r,
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "wrapping_sub" && args.len() == 2 {
                let l = self.lower_expr(&args[0], cur_block)?;
                let r = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::BinOp {
                        dest,
                        op: "wrapping_-".into(),
                        left: l,
                        right: r,
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "wrapping_mul" && args.len() == 2 {
                let l = self.lower_expr(&args[0], cur_block)?;
                let r = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::BinOp {
                        dest,
                        op: "wrapping_*".into(),
                        left: l,
                        right: r,
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "saturating_add" && args.len() == 2 {
                let l = self.lower_expr(&args[0], cur_block)?;
                let r = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::BinOp {
                        dest,
                        op: "saturating_+".into(),
                        left: l,
                        right: r,
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "saturating_sub" && args.len() == 2 {
                let l = self.lower_expr(&args[0], cur_block)?;
                let r = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::BinOp {
                        dest,
                        op: "saturating_-".into(),
                        left: l,
                        right: r,
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "saturating_mul" && args.len() == 2 {
                let l = self.lower_expr(&args[0], cur_block)?;
                let r = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::BinOp {
                        dest,
                        op: "saturating_*".into(),
                        left: l,
                        right: r,
                        ty: "Int".into(),
                    });
                return Some(dest);
            }
            if fn_name == "assert" && !args.is_empty() {
                let cond_val = self.lower_expr(&args[0], cur_block)?;
                let msg_val = if args.len() >= 2 {
                    self.lower_expr(&args[1], cur_block)?
                } else {
                    let m = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: m,
                            value: "Assertion failed".into(),
                        });
                    m
                };
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_assert".into(),
                        args: vec![cond_val, msg_val],
                        ty: "Unit".into(),
                    });
                return Some(dest);
            }
            if fn_name == "exit" && args.len() == 1 {
                let arg_val = self.lower_expr(&args[0], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_exit".into(),
                        args: vec![arg_val],
                        ty: "Never".into(),
                    });
                return Some(dest);
            }
            if (fn_name == "input"
                || fn_name == "read_line"
                || fn_name == "input_int"
                || fn_name == "read_int"
                || fn_name == "fast_read_int"
                || fn_name == "input_float"
                || fn_name == "read_float"
                || fn_name == "fast_read_float")
                && args.len() <= 1
            {
                if (fn_name == "read_int" || fn_name == "fast_read_int") && args.is_empty() {
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fast_read_int".into(),
                            args: vec![],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                if (fn_name == "read_float" || fn_name == "fast_read_float") && args.is_empty() {
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fast_read_float".into(),
                            args: vec![],
                            ty: "Float".into(),
                        });
                    return Some(dest);
                }
                let prompt_val = if !args.is_empty() {
                    self.lower_expr(&args[0], cur_block)?
                } else {
                    let empty = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: empty,
                            value: "".into(),
                        });
                    empty
                };
                let (target_func, ret_ty) = if fn_name == "input_int"
                    || fn_name == "read_int"
                    || fn_name == "fast_read_int"
                {
                    ("datara_rt_input_int", "Int")
                } else if fn_name == "input_float"
                    || fn_name == "read_float"
                    || fn_name == "fast_read_float"
                {
                    ("datara_rt_input_float", "Float")
                } else {
                    ("datara_rt_input", "String")
                };
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: target_func.into(),
                        args: vec![prompt_val],
                        ty: ret_ty.into(),
                    });
                return Some(dest);
            }
            if fn_name == "socket_set_timeout" && args.len() == 2 {
                let sock = self.lower_expr(&args[0], cur_block)?;
                let ms = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_socket_set_timeout".into(),
                        args: vec![sock, ms],
                        ty: "Outcome".into(),
                    });
                return Some(dest);
            }
            if fn_name == "socket_nonblocking" && args.len() == 2 {
                let sock = self.lower_expr(&args[0], cur_block)?;
                let on = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_socket_nonblocking".into(),
                        args: vec![sock, on],
                        ty: "Outcome".into(),
                    });
                return Some(dest);
            }
            if fn_name == "socket_recv_outcome" && args.len() == 2 {
                let sock = self.lower_expr(&args[0], cur_block)?;
                let max_bytes = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_socket_recv_outcome".into(),
                        args: vec![sock, max_bytes],
                        ty: "Outcome".into(),
                    });
                return Some(dest);
            }
            if (fn_name == "py_eval_batch" || fn_name == "datara_py_eval_batch") && args.len() == 1
            {
                let json_expr = self.lower_expr(&args[0], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_py_eval_batch".into(),
                        args: vec![json_expr],
                        ty: "String".into(),
                    });
                return Some(dest);
            }
            if (fn_name == "str_to_float" || fn_name == "datara_rt_str_to_float") && args.len() == 1
            {
                let arg_val = self.lower_expr(&args[0], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_str_to_float".into(),
                        args: vec![arg_val],
                        ty: "Float".into(),
                    });
                return Some(dest);
            }

            if let Some(&tag) = self.enum_variant_tags.get(fn_name) {
                let tag_val = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::ConstInt {
                        dest: tag_val,
                        value: tag,
                    });
                let mut fields = vec![("__tag".to_string(), tag_val)];
                for (idx, a) in args.iter().enumerate() {
                    if let Some(av) = self.lower_expr(a, cur_block) {
                        fields.push((format!("f{}", idx), av));
                    }
                }
                let slots = self.enum_slots.get(fn_name).cloned().unwrap_or_default();
                for (idx, s_ty) in slots.iter().enumerate().skip(args.len()) {
                    let pad_dest = self.next_val();
                    if s_ty.contains("Float") {
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::ConstFloat {
                                dest: pad_dest,
                                value: 0.0,
                            });
                    } else {
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::ConstInt {
                                dest: pad_dest,
                                value: 0,
                            });
                    }
                    fields.push((format!("f{}", idx), pad_dest));
                }
                let dest = self.next_val();
                let class_name = self
                    .enum_variant_names
                    .get(&tag)
                    .cloned()
                    .unwrap_or_else(|| fn_name.clone());
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::StructInit {
                        dest,
                        class_name,
                        fields,
                    });
                return Some(dest);
            }
        }

        if let Expr::MemberAccess { object, member, .. } = callee {
            // v1.3.3 namespace-qualified calls: `alias.func(args)` where
            // `alias` names an imported module lowers to a plain call of
            // the module's function (modules are inlined at load time).
            if let Expr::Identifier(ns_name, _) = &**object {
                if self
                    .module_alias_functions
                    .get(ns_name)
                    .map(|funcs| funcs.contains(member))
                    .unwrap_or(false)
                {
                    let mut arg_vals = Vec::new();
                    for a in args {
                        if let Some(av) = self.lower_expr(a, cur_block) {
                            arg_vals.push(av);
                        }
                    }
                    if let Some(dest) =
                        self.lower_bridge_call(ns_name, member, &arg_vals, cur_block)
                    {
                        return Some(dest);
                    }
                    let dest = self.next_val();
                    let ret_ty = self.infer_fn_ret_ty(member);
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: member.clone(),
                            args: arg_vals,
                            ty: ret_ty,
                        });
                    return Some(dest);
                }
            }
            // Outcome.ok(v) / Outcome.err(msg) constructors: the receiver is
            // the generic class name itself, never a value, so this must be
            // intercepted before the static-method and enum-variant paths
            // below can claim it.
            //
            // Representation: a StructInit of the stdlib `Outcome<T>` class
            // (fields is_success: Bool, value: T, error_msg: Str) — the exact
            // same 3-slot generic-class layout used by `Outcome<T> { ... }`
            // literals, `?` propagation, and the checked-I/O runtime builtins
            // (file_read_checked / env_get_checked). No new ABI shape.
            //
            // Two-state convention: ok carries (true, v, "") and err carries
            // (false, 0, msg); `err()` on an ok object therefore yields ""
            // (empty when ok is legal).
            if let Expr::Identifier(cls_name, _) = &**object
                && cls_name == "Outcome"
                && (member == "ok" || member == "err")
            {
                if args.len() != 1 {
                    return None;
                }
                let arg_val = self.lower_expr(&args[0], cur_block)?;
                let dest = self.next_val();
                let (flag_val, value_val, msg_val): (ValueId, ValueId, ValueId) = if member == "ok"
                {
                    let t = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstBool {
                            dest: t,
                            value: true,
                        });
                    let empty = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: empty,
                            value: String::new(),
                        });
                    (t, arg_val, empty)
                } else {
                    let f = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstBool {
                            dest: f,
                            value: false,
                        });
                    let zero = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstInt {
                            dest: zero,
                            value: 0,
                        });
                    (f, zero, arg_val)
                };
                // Field order matches the stdlib class declaration order
                // (is_success, value, error_msg); StructInit stores by the
                // declared field offsets, so this order is ABI-visible.
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::StructInit {
                        dest,
                        class_name: "Outcome".into(),
                        fields: vec![
                            ("is_success".into(), flag_val),
                            ("value".into(), value_val),
                            ("error_msg".into(), msg_val),
                        ],
                    });
                return Some(dest);
            }
            if let Expr::Identifier(class_name, _) = &**object {
                let enum_key = format!("{}.{}", class_name, member);
                if let Some(&tag) = self.enum_variant_tags.get(&enum_key) {
                    let tag_val = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstInt {
                            dest: tag_val,
                            value: tag,
                        });
                    let mut fields = vec![("__tag".to_string(), tag_val)];
                    for (idx, a) in args.iter().enumerate() {
                        if let Some(av) = self.lower_expr(a, cur_block) {
                            fields.push((format!("f{}", idx), av));
                        }
                    }
                    let slots = self.enum_slots.get(&enum_key).cloned().unwrap_or_default();
                    for (idx, s_ty) in slots.iter().enumerate().skip(args.len()) {
                        let pad_dest = self.next_val();
                        if s_ty.contains("Float") {
                            self.get_block_mut(*cur_block)
                                .instructions
                                .push(Inst::ConstFloat {
                                    dest: pad_dest,
                                    value: 0.0,
                                });
                        } else {
                            self.get_block_mut(*cur_block)
                                .instructions
                                .push(Inst::ConstInt {
                                    dest: pad_dest,
                                    value: 0,
                                });
                        }
                        fields.push((format!("f{}", idx), pad_dest));
                    }
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::StructInit {
                            dest,
                            class_name: format!("{}_{}", class_name, member),
                            fields,
                        });
                    return Some(dest);
                }

                let static_func_name = format!("{}_{}", class_name, member);
                // Arity guard: the static-call ABI passes a dummy `this`
                // first, so the target must have exactly 1 + args.len()
                // parameters. Without this check, `Class.zero_arg_method(x)`
                // compiled into a 2-arg call against a 1-param function and
                // tripped a Cranelift ABI assertion instead of a diagnostic.
                let static_arity_ok = match self.types.function_signatures.get(&static_func_name) {
                    Some((param_types, _, _)) => param_types.len() == args.len() + 1,
                    None => true,
                };
                if static_arity_ok && self.function_return_types.contains_key(&static_func_name) {
                    let dummy_this = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::ConstInt {
                            dest: dummy_this,
                            value: 0,
                        });
                    let mut call_args = vec![dummy_this];
                    for a in args {
                        if let Some(av) = self.lower_expr(a, cur_block) {
                            call_args.push(av);
                        }
                    }
                    let dest = self.next_val();
                    let ret_ty = self
                        .function_return_types
                        .get(&static_func_name)
                        .cloned()
                        .unwrap_or_else(|| "Unit".into());
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: static_func_name,
                            args: call_args,
                            ty: ret_ty,
                        });
                    return Some(dest);
                }
            }

            if member == "view" && args.is_empty() {
                return self.lower_expr(object, cur_block);
            }
            if (member == "grant_readonly" || member == "grant_readwrite" || member == "open")
                && args.len() == 1
            {
                return self.lower_expr(&args[0], cur_block);
            }
            if member == "read_all" {
                let dest = self.next_val();
                let arg = if args.is_empty() {
                    self.lower_expr(object, cur_block)?
                } else {
                    self.lower_expr(&args[0], cur_block)?
                };
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_file_read".into(),
                        args: vec![arg],
                        ty: "String".into(),
                    });
                return Some(dest);
            }
            let obj_val = self.lower_expr(object, cur_block)?;
            if self.is_expr_list(object) {
                if (member == "map"
                    || member == "filter"
                    || member == "find"
                    || member == "any"
                    || member == "all")
                    && args.len() == 1
                    && self.is_callable_expr(&args[0])
                {
                    return self.lower_list_higher_order(obj_val, member, args, cur_block);
                }
                if member == "reduce" && args.len() == 2 && self.is_callable_expr(&args[1]) {
                    return self.lower_list_higher_order(obj_val, member, args, cur_block);
                }
                if member == "fold" && args.len() == 2 && self.is_callable_expr(&args[1]) {
                    return self.lower_list_higher_order(obj_val, member, args, cur_block);
                }
                // collect() materializes the pipeline: map/filter already
                // build eager result lists, so the sink list is the value.
                if member == "collect" && args.is_empty() {
                    return Some(obj_val);
                }
                // v1.4.1 checked accessors: pop/first/last return an
                // Outcome<T> object in the stdlib Outcome<T> layout, so `?`,
                // unwrap/unwrap_or/is_ok/is_err/err all work on them exactly
                // like on file_read_checked(). The element repr keys the
                // Outcome payload tagging downstream (Float bit patterns,
                // Str pointers).
                if member == "pop" && args.is_empty() {
                    let elem_repr = self.list_elem_repr(object);
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_list_pop_outcome".into(),
                            args: vec![obj_val],
                            ty: format!("Outcome<{}>", elem_repr),
                        });
                    return Some(dest);
                }
                if member == "first" && args.is_empty() {
                    let elem_repr = self.list_elem_repr(object);
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_list_first".into(),
                            args: vec![obj_val],
                            ty: format!("Outcome<{}>", elem_repr),
                        });
                    return Some(dest);
                }
                if member == "last" && args.is_empty() {
                    let elem_repr = self.list_elem_repr(object);
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_list_last".into(),
                            args: vec![obj_val],
                            ty: format!("Outcome<{}>", elem_repr),
                        });
                    return Some(dest);
                }
                if member == "slice" && args.len() == 2 {
                    let start = self.lower_expr(&args[0], cur_block)?;
                    let end = self.lower_expr(&args[1], cur_block)?;
                    let elem_repr = self.list_elem_repr(object);
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_list_slice".into(),
                            args: vec![obj_val, start, end],
                            ty: format!("List<{}>", elem_repr),
                        });
                    return Some(dest);
                }
            }
            if member == "insert" && args.len() == 2 && self.is_expr_map(object) {
                let k = self.lower_expr(&args[0], cur_block)?;
                let v = self.lower_expr(&args[1], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_map_insert".into(),
                        args: vec![obj_val, k, v],
                        ty: "Map".into(),
                    });
                return Some(dest);
            }
            if ((member == "contains" && self.is_expr_map(object))
                || member == "has_key"
                || member == "contains_key")
                && args.len() == 1
            {
                let k = self.lower_expr(&args[0], cur_block)?;
                let dest = self.next_val();
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::Call {
                        dest,
                        func: "datara_rt_map_contains".into(),
                        args: vec![obj_val, k],
                        ty: "Bool".into(),
                    });
                return Some(dest);
            }
            let mut arg_vals = Vec::new();
            for a in args {
                if let Some(av) = self.lower_expr(a, cur_block) {
                    arg_vals.push(av);
                }
            }
            let mut method_ty = if member == "to_float" || member == "to_int" {
                // Numeric conversion intrinsics (Gate 7 explicit casts):
                // the result type comes from the method name, never from
                // the name heuristics below.
                if member == "to_float" {
                    "Float".to_string()
                } else {
                    "Int".to_string()
                }
            } else {
                self.function_return_types
                    .get(member)
                    .or_else(|| self.class_field_types.get(member))
                    .cloned()
                    .unwrap_or_else(|| {
                        if member.contains("float") || member.contains("flt") {
                            "Float".into()
                        } else if member.contains("string")
                            || member.contains("to_str")
                            || member.starts_with("str_")
                            || member.ends_with("_str")
                            || member.contains("render")
                            || member.contains("format")
                            || member.contains("quote")
                            || member.starts_with("wrap")
                        {
                            "String".into()
                        } else {
                            "Int".into()
                        }
                    })
            };
            if member == "unwrap" || member == "unwrap_or" {
                let obj_ty = match &**object {
                    Expr::Identifier(name, _) => self.lookup_var_type(name),
                    _ => None,
                };
                if let Some(DataraType::GenericInstance { name, args }) = &obj_ty {
                    if name == "Outcome" && !args.is_empty() {
                        method_ty = match &args[0] {
                            DataraType::Float => "Float".into(),
                            DataraType::String => "String".into(),
                            DataraType::Bool => "Bool".into(),
                            DataraType::Int => "Int".into(),
                            DataraType::Class(c) => c.clone(),
                            _ => "Int".into(),
                        };
                    }
                } else if let Some(DataraType::Result(ok, _)) = &obj_ty {
                    method_ty = match &**ok {
                        DataraType::Float => "Float".into(),
                        DataraType::String => "String".into(),
                        DataraType::Bool => "Bool".into(),
                        DataraType::Int => "Int".into(),
                        DataraType::Class(c) => c.clone(),
                        _ => "Int".into(),
                    };
                }
            } else if member == "is_ok" || member == "is_err" {
                method_ty = "Bool".into();
            } else if member == "err" {
                // Outcome<T>.err() returns the error channel (Str, "" when
                // ok); only special-cased on Outcome/Result receivers so a
                // user class's own `err` method keeps its declared type.
                let obj_ty = match &**object {
                    Expr::Identifier(name, _) => self.lookup_var_type(name),
                    _ => None,
                };
                let is_outcome_receiver = match &obj_ty {
                    Some(DataraType::GenericInstance { name, .. }) => name == "Outcome",
                    Some(DataraType::Result(..)) => true,
                    _ => false,
                };
                if is_outcome_receiver {
                    method_ty = "String".into();
                }
            } else if member == "await" {
                let obj_ty = match &**object {
                    Expr::Identifier(name, _) => self.lookup_var_type(name),
                    _ => None,
                };
                if let Some(DataraType::GenericInstance { name, args }) = &obj_ty {
                    if (name == "Future" || name == "Task") && !args.is_empty() {
                        method_ty = match &args[0] {
                            DataraType::Float => "Float".into(),
                            DataraType::String => "String".into(),
                            DataraType::Bool => "Bool".into(),
                            DataraType::Int => "Int".into(),
                            DataraType::Class(c) => c.clone(),
                            _ => "Int".into(),
                        };
                    }
                } else if let Some(DataraType::Class(c)) = &obj_ty
                    && (c == "Future" || c == "Task")
                {
                    method_ty = "String".into();
                }
            }
            // v1.4.1: remaining List protocol methods reach the backends'
            // list dispatch tables as MethodCall. Their result repr must not
            // fall through to the name-heuristic default: contains/is_empty
            // are Bool, remove_at/remove_value/index_of are Int, and the
            // in-place mutators plus slice return the list itself (keyed
            // for list tagging and the backends' elem_kind injection).
            if self.is_expr_list(object) {
                let elem_repr = self.list_elem_repr(object);
                match member.as_str() {
                    "sort" | "reverse" | "clear" | "insert_at" | "slice" => {
                        method_ty = format!("List<{}>", elem_repr);
                    }
                    "contains" | "is_empty" => {
                        method_ty = "Bool".into();
                    }
                    "remove_at" | "remove_value" | "index_of" => {
                        method_ty = "Int".into();
                    }
                    _ => {}
                }
            }
            let dest = self.next_val();
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::MethodCall {
                    dest,
                    object: obj_val,
                    method: member.clone(),
                    args: arg_vals,
                    ty: method_ty,
                });
            return Some(dest);
        }

        let mut arg_vals = Vec::new();
        for a in args {
            if let Some(av) = self.lower_expr(a, cur_block) {
                arg_vals.push(av);
            }
        }
        let mut func_name = if let Expr::Identifier(fn_name, _) = callee {
            fn_name.clone()
        } else {
            "func".into()
        };

        // Lambda-argument static dispatch. When a call passes a lambda (a
        // literal, or a local lambda variable) to a function whose body is a
        // single expression (`return <expr>` / `=> expr`), the body is
        // inlined at the call site with the lambda bound to the parameter
        // name -- the same inline machinery `list.map(f)` uses. This is the
        // documented static-dispatch path for closures as arguments; bodies
        // with control flow are not eligible (they were never indexed in
        // `inlineable_fns`) and fall through to a real call.
        let mut has_lambda_arg = false;
        for a in args {
            match a {
                Expr::Lambda { .. } => {
                    has_lambda_arg = true;
                    break;
                }
                Expr::Identifier(name, _) if self.local_lambdas.contains_key(name) => {
                    has_lambda_arg = true;
                    break;
                }
                _ => {}
            }
        }
        if has_lambda_arg
            && let Some((params, inline_stmts, tail_expr)) =
                self.inlineable_fns.get(&func_name).cloned()
        {
            let mut arg_vals = Vec::new();
            for a in args {
                if let Some(av) = self.lower_expr(a, cur_block) {
                    arg_vals.push(av);
                }
            }
            // Shadow-save every binding the inline touches (parameters, and
            // the lambda name in local_lambdas) so the caller's scope is
            // restored afterwards -- same discipline as
            // lower_inline_closure_call.
            let mut shadowed_symbols: Vec<(String, Option<ValueId>)> = Vec::new();
            let mut shadowed_lambdas: Vec<String> = Vec::new();
            for (i, p) in params.iter().enumerate() {
                let Some(&aval) = arg_vals.get(i) else {
                    break;
                };
                let old_val = self.symbol_values.get(&p.name).copied();
                shadowed_symbols.push((p.name.clone(), old_val));
                self.symbol_values.insert(p.name.clone(), aval);
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::AssignVar {
                        name: p.name.clone(),
                        value: aval,
                    });
                // A lambda argument binds its value to the parameter name in
                // local_lambdas so `<param>(x)` inside the body inlines it.
                // Positional lookup goes through `args`, not `arg_vals`,
                // because a failed lowering can shorten arg_vals.
                let lambda = match args.get(i) {
                    Some(Expr::Lambda { params, body, .. }) => {
                        Some((params.clone(), (**body).clone()))
                    }
                    Some(Expr::Identifier(name, _)) => self.local_lambdas.get(name).cloned(),
                    _ => None,
                };
                if let Some((lp, lb)) = lambda {
                    shadowed_lambdas.push(p.name.clone());
                    self.local_lambdas.insert(p.name.clone(), (lp, lb));
                }
            }
            let res = {
                for s in &inline_stmts {
                    let (next_b, _val) = self.lower_stmt_cfg(s, *cur_block);
                    *cur_block = next_b;
                }
                match &tail_expr {
                    Some(e) => self.lower_expr(e, cur_block),
                    None => {
                        let zero = self.next_val();
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::ConstInt {
                                dest: zero,
                                value: 0,
                            });
                        Some(zero)
                    }
                }
            };
            for name in shadowed_lambdas {
                self.local_lambdas.remove(&name);
            }
            for (name, old_val) in shadowed_symbols {
                if let Some(ov) = old_val {
                    self.symbol_values.insert(name.clone(), ov);
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::AssignVar { name, value: ov });
                } else {
                    self.symbol_values.remove(&name);
                }
            }
            return res;
        }

        if let Some(specs) = self.types.generic_specializations.get(&func_name) {
            let mut candidate_mangled = None;
            if let Some(first_arg) = args.first() {
                let arg_ty_opt = match first_arg {
                    Expr::Identifier(var_name, _) => self.lookup_var_type(var_name),
                    Expr::ObjectInit { class_name, .. } => {
                        Some(DataraType::Class(class_name.clone()))
                    }
                    _ => None,
                };
                if let Some(arg_ty) = arg_ty_opt {
                    let type_str = match &arg_ty {
                        DataraType::Class(c) => c.clone(),
                        DataraType::Int => "Int".to_string(),
                        DataraType::Float => "Float".to_string(),
                        DataraType::String => "String".to_string(),
                        DataraType::Bool => "Bool".to_string(),
                        other => other.to_string(),
                    };
                    let candidate = format!("{}_{}", func_name, type_str);
                    if self.function_return_types.contains_key(&candidate) {
                        candidate_mangled = Some(candidate);
                    }
                }
            }
            if candidate_mangled.is_none()
                && specs.len() == 1
                && let Some(first_spec) = specs.iter().next()
            {
                let s_names: Vec<String> = first_spec
                    .iter()
                    .map(|t| match t {
                        DataraType::Class(c) => c.clone(),
                        DataraType::Int => "Int".to_string(),
                        DataraType::Float => "Float".to_string(),
                        DataraType::String => "String".to_string(),
                        DataraType::Bool => "Bool".to_string(),
                        other => other.to_string(),
                    })
                    .collect();
                let candidate = format!("{}_{}", func_name, s_names.join("_"));
                if self.function_return_types.contains_key(&candidate) {
                    candidate_mangled = Some(candidate);
                }
            }
            if let Some(mangled) = candidate_mangled {
                func_name = mangled;
            }
        }

        if func_name == "require" {
            let zero = self.next_val();
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::ConstInt {
                    dest: zero,
                    value: 0,
                });
            return Some(zero);
        }

        let dest = self.next_val();
        let ret_ty = self.infer_fn_ret_ty(&func_name);
        self.get_block_mut(*cur_block)
            .instructions
            .push(Inst::Call {
                dest,
                func: func_name,
                args: arg_vals,
                ty: ret_ty,
            });
        Some(dest)
    }
}
