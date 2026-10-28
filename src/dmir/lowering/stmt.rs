use crate::ast::*;
use crate::dmir::ir::*;
use crate::types::DataraType;

use super::Lowering;

impl<'a> Lowering<'a> {
    /// M5 TMM Layer 3: the @arc class a variable holds, if any. Mirrors
    /// lookup_var_type but also resolves globals of user-class type —
    /// global_vars stores the raw type string, which lookup_var_type
    /// collapses to Int for classes.
    pub(super) fn arc_class_of_var(&self, var_name: &str) -> Option<String> {
        if let Some(DataraType::Class(c)) = self.lookup_var_type(var_name)
            && self.arc_classes_lower.contains(&c)
        {
            return Some(c);
        }
        if let Some((ty_str, _)) = self.global_vars.get(var_name)
            && self.arc_classes_lower.contains(ty_str)
        {
            return Some(ty_str.clone());
        }
        None
    }

    pub fn lower_stmt_cfg(
        &mut self,
        stmt: &Stmt,
        mut cur_block: BasicBlockId,
    ) -> (BasicBlockId, Option<ValueId>) {
        let sp = stmt.span();
        if sp.start_line > 0 {
            self.current_line_spans.push(sp.clone());
        }
        match stmt {
            Stmt::Block(stmts, _) => {
                let mut last = None;
                for s in stmts {
                    let (next_b, val) = self.lower_stmt_cfg(s, cur_block);
                    cur_block = next_b;
                    last = val;
                }
                (cur_block, last)
            }
            Stmt::Let { name, init, .. }
            | Stmt::Mut { name, init, .. }
            | Stmt::Const { name, init, .. }
            | Stmt::Val { name, init, .. }
            | Stmt::CompactBind { name, init, .. } => {
                let type_node = match stmt {
                    Stmt::Let { type_node, .. }
                    | Stmt::Mut { type_node, .. }
                    | Stmt::Const { type_node, .. }
                    | Stmt::Val { type_node, .. } => type_node.as_ref(),
                    _ => None,
                };
                if let Some(tn) = type_node {
                    if tn.name == "List" {
                        let elem_ty = match tn.generic_args.first().map(|a| a.name.as_str()) {
                            Some("Float" | "Float64" | "f64" | "Float32" | "f32") => {
                                DataraType::Float
                            }
                            Some("String" | "Str") => DataraType::String,
                            Some("Bool") => DataraType::Bool,
                            Some("Int" | "Int64" | "Int32" | "Int16" | "Int8" | "Byte") => {
                                DataraType::Int
                            }
                            // Struct elements keep their class name: an
                            // annotated List<Body> coerced to List<Int>
                            // made `bodies[k].x` lose the receiver class
                            // and field-offset resolution fall to guessing
                            // (E0944).
                            Some(other) => DataraType::Class(other.to_string()),
                            None => DataraType::Int,
                        };
                        self.local_var_types
                            .insert(name.clone(), DataraType::List(Box::new(elem_ty.clone())));
                        if elem_ty == DataraType::Float {
                            self.class_field_types.insert(name.clone(), "Float".into());
                        }
                    } else if tn.name == "Map" {
                        // Annotated maps keep BOTH type arguments: keying every
                        // map as Map<Str, _> routed `Map<Int, Int>` inserts
                        // through the string ABI (segfault). Only Str/Int keys
                        // exist at runtime; the checker rejects the rest.
                        let parse = |t: Option<&crate::ast::TypeNode>| match t {
                            Some(tn) => match tn.name.as_str() {
                                "Int" | "Int64" | "Int32" | "Int16" | "Int8" => DataraType::Int,
                                "Float" | "Float64" | "f64" | "Float32" | "f32" => {
                                    DataraType::Float
                                }
                                "Bool" => DataraType::Bool,
                                "String" | "Str" => DataraType::String,
                                other => DataraType::Class(other.to_string()),
                            },
                            None => DataraType::Int,
                        };
                        let k_ty = parse(tn.generic_args.first());
                        let v_ty = parse(tn.generic_args.get(1));
                        self.local_var_types.insert(
                            name.clone(),
                            DataraType::Map(Box::new(k_ty), Box::new(v_ty)),
                        );
                    } else if matches!(tn.name.as_str(), "Float" | "Float64" | "f64") {
                        self.local_var_types.insert(name.clone(), DataraType::Float);
                        self.class_field_types.insert(name.clone(), "Float".into());
                    } else if matches!(tn.name.as_str(), "Float32" | "f32") {
                        // v1.4.5 W2: Float32 keeps its own width — collapsing it
                        // to Float turned annotated f32 vars into f64 ops.
                        self.local_var_types
                            .insert(name.clone(), DataraType::Float32);
                        self.class_field_types
                            .insert(name.clone(), "Float32".into());
                    } else if matches!(tn.name.as_str(), "Int8" | "i8") {
                        self.local_var_types.insert(name.clone(), DataraType::Int8);
                    } else if matches!(tn.name.as_str(), "Int16" | "i16") {
                        self.local_var_types.insert(name.clone(), DataraType::Int16);
                    } else if matches!(tn.name.as_str(), "Int32" | "i32") {
                        self.local_var_types.insert(name.clone(), DataraType::Int32);
                    } else if matches!(tn.name.as_str(), "UInt8" | "Byte" | "u8") {
                        self.local_var_types.insert(name.clone(), DataraType::UInt8);
                    } else if matches!(tn.name.as_str(), "UInt16" | "u16") {
                        self.local_var_types
                            .insert(name.clone(), DataraType::UInt16);
                    } else if matches!(tn.name.as_str(), "UInt32" | "u32") {
                        self.local_var_types
                            .insert(name.clone(), DataraType::UInt32);
                    } else if matches!(tn.name.as_str(), "UInt" | "UInt64" | "u64" | "usize") {
                        self.local_var_types
                            .insert(name.clone(), DataraType::UInt64);
                    } else if matches!(tn.name.as_str(), "String" | "Str") {
                        self.local_var_types
                            .insert(name.clone(), DataraType::String);
                    } else if tn.name == "Bool" {
                        self.local_var_types.insert(name.clone(), DataraType::Bool);
                    }
                }
                // v1.4.5 W1/W2: the legacy `symbol_types` map collapses
                // narrow types (Int8→Int, Float32→Float). It must NOT
                // overwrite a type already recorded from the explicit type
                // annotation above, or `let a: Float32` reverts to f64 ops.
                if !self.local_var_types.contains_key(name)
                    && let Some(ty) = self.types.symbol_types.get(name)
                {
                    self.local_var_types.insert(name.clone(), ty.clone());
                }
                // List-protocol call inits (`val s = l.sort()`) keep the
                // element type: record_var_type erases the checked type to
                // Class("List"), and element reads `s.get(0)` then fell
                // back to Int — a raw char* pointer printed instead of the
                // string. Refine ONLY that erased form; an explicit
                // annotation or a richer symbol_types entry wins.
                if let Some(DataraType::List(elem)) = self.infer_expr_datara_type(init)
                    && matches!(
                        self.local_var_types.get(name),
                        Some(DataraType::Class(c))
                            if c == "List"
                    )
                {
                    self.local_var_types
                        .insert(name.clone(), DataraType::List(elem));
                }
                if let Expr::ObjectInit { class_name, .. } = init {
                    self.local_var_types
                        .insert(name.clone(), DataraType::Class(class_name.clone()));
                } else if let Expr::MapLiteral(entries, _) = init {
                    // An explicit annotation wins; only infer for unannotated
                    // literals. Key kind comes from the first key (Int or Str —
                    // the checker rejects every other key type).
                    if !self.local_var_types.contains_key(name) {
                        let int_keys = entries
                            .first()
                            .map(|(k, _)| self.is_expr_int(k))
                            .unwrap_or(false);
                        let mut is_flt = false;
                        for (_, v) in entries {
                            if self.is_expr_float(v) {
                                is_flt = true;
                                break;
                            }
                        }
                        let val_ty = if is_flt {
                            DataraType::Float
                        } else {
                            DataraType::Int
                        };
                        let key_ty = if int_keys {
                            DataraType::Int
                        } else {
                            DataraType::String
                        };
                        self.local_var_types.insert(
                            name.clone(),
                            DataraType::Map(Box::new(key_ty), Box::new(val_ty)),
                        );
                    }
                } else if let Expr::ListLiteral(elements, _) = init {
                    if !self.local_var_types.contains_key(name) {
                        // Element type must consider strings first: a list
                        // literal of strings lowered as List<Int> made any
                        // inline `parts[i]` read route to out_int as a raw
                        // pointer instead of the string value (E0944-class
                        // silent wrong data).
                        // M7 TSU: element kinds are computed per element; a
                        // literal MIXING kinds (or containing a Val-typed
                        // element) records List(Val) so indexing and `is`
                        // checks route to the NaN-box dynamic path.
                        let mut saw_int = false;
                        let mut saw_flt = false;
                        let mut saw_str = false;
                        let mut saw_bool = false;
                        for e in elements {
                            if self.is_expr_str(e) {
                                saw_str = true;
                            } else if self.is_expr_float(e) {
                                saw_flt = true;
                            } else if self.is_expr_bool(e) {
                                saw_bool = true;
                            } else if self.is_expr_int(e) {
                                saw_int = true;
                            }
                        }
                        let kinds =
                            saw_int as u32 + saw_flt as u32 + saw_str as u32 + saw_bool as u32;
                        let val_ty = if kinds > 1 {
                            DataraType::Val
                        } else if saw_str {
                            DataraType::String
                        } else if saw_flt {
                            DataraType::Float
                        } else if saw_bool {
                            DataraType::Bool
                        } else {
                            DataraType::Int
                        };
                        self.local_var_types
                            .insert(name.clone(), DataraType::List(Box::new(val_ty)));
                    }
                } else if self.is_expr_str(init) {
                    self.local_var_types
                        .insert(name.clone(), DataraType::String);
                } else if self.is_expr_float(init) && !self.local_var_types.contains_key(name) {
                    // v1.4.5 W2: an explicit annotation wins over init-based
                    // inference — `let a: Float32 = 0.1` must stay Float32,
                    // not be re-collapsed to Float by the f64 literal.
                    self.local_var_types.insert(name.clone(), DataraType::Float);
                } else if self.is_expr_bool(init) {
                    self.local_var_types.insert(name.clone(), DataraType::Bool);
                }
                if let Expr::Lambda { params, body, .. } = init {
                    self.local_lambdas
                        .insert(name.clone(), (params.clone(), *body.clone()));
                    // Record the statically inferred return type so a call to
                    // this lambda dispatches correctly downstream (map/list
                    // indexing, checked map.get interception). The value side
                    // defaults to Int for untyped lambda params — same
                    // historical default as Map.new() — and an explicit
                    // annotation on the binding still wins.
                    if let Some(ret) = self.infer_lambda_ret_type(body) {
                        self.local_lambda_ret.insert(name.clone(), ret);
                    } else {
                        self.local_lambda_ret.remove(name.as_str());
                    }
                    // By-value capture snapshot: record the free variables of
                    // the body with their CURRENT SSA values. Every inline
                    // call re-binds these names to the snapshot (with
                    // shadow-restore), so reads see the registration-time
                    // values and writes inside the body never leak back into
                    // the enclosing scope.
                    let enclosing: std::collections::HashSet<String> =
                        self.symbol_values.keys().cloned().collect();
                    let captured = crate::ast::infer_captures(params, body, &enclosing, true);
                    // M5 TMM Layer 3: a capture snapshot survives the
                    // registering frame — it is re-bound at every inline
                    // call, including calls made after the enclosing scope
                    // is gone. When the captured variable holds an @arc
                    // object the snapshot must share ownership, so a strong
                    // reference is acquired for the snapshot itself (the
                    // release of a superseded snapshot belongs to the
                    // dataflow layer, M6). The Call keeps the CLASS repr so
                    // GetField/method dispatch on the captured name inside
                    // the body keeps resolving through the real layout.
                    let mut snapshots: Vec<(String, ValueId)> = Vec::new();
                    for c in &captured {
                        if let Some(v) = self.symbol_values.get(&c.name).copied() {
                            let snap_v = if let Some(class) = self.arc_class_of_var(&c.name) {
                                let acq = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(Inst::Call {
                                    dest: acq,
                                    func: "datara_rt_arc_acquire".into(),
                                    args: vec![v],
                                    ty: class,
                                });
                                acq
                            } else {
                                v
                            };
                            snapshots.push((c.name.clone(), snap_v));
                        }
                    }
                    if !snapshots.is_empty() {
                        self.lambda_captures.insert(name.clone(), snapshots);
                    }
                }
                if !self.local_var_types.contains_key(name) {
                    if let Some(inferred) = self.infer_expr_datara_type(init) {
                        self.local_var_types.insert(name.clone(), inferred);
                    }
                }
                if !self.local_var_types.contains_key(name)
                    && let Some(ty) = self.lookup_var_type(name)
                {
                    self.local_var_types.insert(name.clone(), ty);
                }
                let val = self.lower_expr(init, &mut cur_block);
                if let Some(v) = val {
                    self.symbol_values.insert(name.clone(), v);
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::AssignVar {
                            name: name.clone(),
                            value: v,
                        });
                }
                (cur_block, val)
            }
            Stmt::Assign { target, value, .. } => {
                let val = self.lower_expr(value, &mut cur_block);
                if let Some(v) = val {
                    match target {
                        Expr::Identifier(name, _) => {
                            // M5 TMM Layer 2: assigning an @arc object into an
                            // existing `mut` slot is a strong acquire — the
                            // slot now shares ownership. The lowering does not
                            // know the previous slot contents, so the release
                            // of the replaced object belongs to the dataflow
                            // layer (M6); the acquire is correct regardless.
                            let src_class = self
                                .infer_expr_datara_type(value)
                                .map(|t| match t {
                                    DataraType::Class(c) => c,
                                    other => other.to_string(),
                                })
                                .or_else(|| match value {
                                    Expr::Identifier(src, _) => {
                                        self.local_var_types.get(src).map(|t| match t {
                                            DataraType::Class(c) => c.clone(),
                                            other => other.to_string(),
                                        })
                                    }
                                    _ => None,
                                });
                            if let Some(c) = src_class
                                && (self.arc_classes_lower.contains(&c) || c.starts_with("RawPtr"))
                            {
                                // Class check only: RawPtr values coming from
                                // arc_init/acquire are also refcounted. The
                                // acquire is idempotent for non-ARC pointers
                                // guarded by the static TMM check (only @arc
                                // types reach this path).
                                let acq = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(Inst::Call {
                                    dest: acq,
                                    func: "datara_rt_arc_acquire".into(),
                                    args: vec![v],
                                    ty: "RawPtr".into(),
                                });
                                self.symbol_values.insert(name.clone(), acq);
                                self.get_block_mut(cur_block)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: name.clone(),
                                        value: acq,
                                    });
                            } else {
                                self.symbol_values.insert(name.clone(), v);
                                self.get_block_mut(cur_block)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: name.clone(),
                                        value: v,
                                    });
                            }
                        }
                        // `this.field = v` / `obj.field = v`.
                        //
                        // This is the only place a field store is produced.
                        // It used to be missing entirely: `Stmt::Assign` only
                        // matched `Expr::Identifier`, so every assignment whose
                        // target was a member access was silently dropped on
                        // the floor. Mutating methods compiled to no-ops and
                        // the field kept its initial value forever — with no
                        // diagnostic, because the statement *was* visited, it
                        // just produced no instruction.
                        Expr::MemberAccess { object, member, .. } => {
                            if let Some((base_addr, fields)) =
                                self.find_mmio_for_member(object, member)
                            {
                                if let Some(&(offset, ref ty_name)) = fields.get(member) {
                                    let addr_val = self.next_val();
                                    self.get_block_mut(cur_block).instructions.push(
                                        Inst::ConstInt {
                                            dest: addr_val,
                                            value: (base_addr + offset) as i64,
                                        },
                                    );
                                    self.get_block_mut(cur_block).instructions.push(
                                        Inst::VolatileStore {
                                            addr: addr_val,
                                            value: v,
                                            ty: ty_name.clone(),
                                        },
                                    );
                                }
                            } else if let Some(obj_val) = self.lower_expr(object, &mut cur_block) {
                                self.get_block_mut(cur_block)
                                    .instructions
                                    .push(Inst::SetField {
                                        object: obj_val,
                                        field: member.clone(),
                                        value: v,
                                    });
                            }
                        }
                        Expr::IndexAccess { object, index, .. } => {
                            if let Some(obj_val) = self.lower_expr(object, &mut cur_block)
                                && let Some(idx_val) = self.lower_expr(index, &mut cur_block)
                            {
                                let is_map = self.is_expr_str(index)
                                    || match &**object {
                                        Expr::Identifier(name, _) => matches!(
                                            self.lookup_var_type(name),
                                            Some(DataraType::Map(..))
                                        ),
                                        _ => false,
                                    };
                                let (func_name, ty) = if is_map {
                                    ("datara_rt_map_insert", "Map")
                                } else {
                                    ("datara_rt_list_set", "List")
                                };
                                let ret_val = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(Inst::Call {
                                    dest: ret_val,
                                    func: func_name.into(),
                                    args: vec![obj_val, idx_val, v],
                                    ty: ty.into(),
                                });
                                if is_map && let Expr::Identifier(var_name, _) = &**object {
                                    self.get_block_mut(cur_block).instructions.push(
                                        Inst::AssignVar {
                                            name: var_name.clone(),
                                            value: ret_val,
                                        },
                                    );
                                    self.symbol_values.insert(var_name.clone(), ret_val);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                (cur_block, val)
            }
            Stmt::Expr(e, _) => {
                let val = self.lower_expr(e, &mut cur_block);
                if let Expr::Call { callee, .. } = e
                    && let Expr::MemberAccess { object, member, .. } = &**callee
                    && matches!(
                        member.as_str(),
                        "push" | "append" | "add" | "set" | "insert" | "insert_at"
                    )
                    && let Expr::Identifier(var_name, _) = &**object
                {
                    let is_collection = if let Some(ty) = self.lookup_var_type(var_name) {
                        match ty {
                            crate::types::DataraType::List(_)
                            | crate::types::DataraType::Map(..) => true,
                            crate::types::DataraType::Class(name) => {
                                name == "List" || name == "Array" || name == "Map"
                            }
                            crate::types::DataraType::GenericInstance { name, .. } => {
                                name == "List" || name == "Array" || name == "Map"
                            }
                            _ => false,
                        }
                    } else {
                        false
                    };
                    if is_collection && let Some(v) = val {
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::AssignVar {
                                name: var_name.clone(),
                                value: v,
                            });
                        self.symbol_values.insert(var_name.clone(), v);
                    }
                }
                (cur_block, val)
            }
            Stmt::Out(e, _) => self.lower_out_stmt(e, cur_block),
            Stmt::Err(e, _) => self.lower_err_stmt(e, cur_block),
            Stmt::Return(opt_e, _) => {
                // `return x?` in a Result/Option-returning function: the
                // propagate-lowering's error path already returns the failed
                // object, and its success path yields the payload — which
                // must be re-wrapped in the enclosing constructor
                // (semantics: `let v = x?; return Outcome.ok(v)`). Without
                // the wrap the function would return the bare payload in an
                // Outcome-returning frame.
                let wrap_ctor: Option<bool> = match opt_e {
                    Some(Expr::ErrorPropagate(..)) => {
                        let ret = self
                            .function_return_types
                            .get(self.current_fn_name.as_str())
                            .cloned()
                            .unwrap_or_default();
                        if ret.starts_with("Outcome") || ret.starts_with("Result") {
                            Some(true)
                        } else if ret.starts_with("Maybe") || ret.starts_with("Option") {
                            Some(false)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                let val = if let Some(is_outcome) = wrap_ctor {
                    let payload = match self.lower_expr(opt_e.as_ref().unwrap(), &mut cur_block) {
                        Some(p) => p,
                        None => ValueId(0),
                    };
                    let dest = self.next_val();
                    let flag = self.next_val();
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::ConstBool {
                            dest: flag,
                            value: true,
                        });
                    if is_outcome {
                        let empty = self.next_val();
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::ConstStr {
                                dest: empty,
                                value: String::new(),
                            });
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::StructInit {
                                dest,
                                class_name: "Outcome".into(),
                                fields: vec![
                                    ("is_success".into(), flag),
                                    ("value".into(), payload),
                                    ("error_msg".into(), empty),
                                ],
                            });
                    } else {
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::StructInit {
                                dest,
                                class_name: "Maybe".into(),
                                fields: vec![("is_some".into(), flag), ("value".into(), payload)],
                            });
                    }
                    Some(dest)
                } else if let Some(e) = opt_e {
                    self.lower_expr(e, &mut cur_block)
                } else {
                    None
                };
                // Inline-return frame: a `return e` inside a lambda body that
                // is being lowered IN PLACE must exit the inlined body with
                // value e, not return from the enclosing function (the old
                // behavior silently killed the rest of the caller — e.g. a
                // block-bodied lambda with `if { return x }` returned from
                // `main` mid-body). Store the value in the frame's synthetic
                // slot and branch to the frame's join block; the inline call
                // site loads the slot there as the lambda-call result.
                if let Some(frame) = self.inline_ret_frames.last_mut() {
                    frame.used = true;
                }
                let join = if let Some(frame) = self.inline_ret_frames.last() {
                    frame.join
                } else {
                    None
                };
                if let Some(frame_idx) = self.inline_ret_frames.len().checked_sub(1) {
                    let join = match join {
                        Some(j) => j,
                        None => {
                            let j = self.create_block("inline_lam_join");
                            self.inline_ret_frames[frame_idx].join = Some(j);
                            j
                        }
                    };
                    let slot = self.inline_ret_frames[frame_idx].ret_slot.clone();
                    let b = self.get_block_mut(cur_block);
                    if let Some(v) = val {
                        b.instructions.push(Inst::AssignVar {
                            name: slot,
                            value: v,
                        });
                    }
                    b.instructions.push(Inst::Return { value: None });
                    b.terminator = Terminator::Branch {
                        target: join,
                        args: Vec::new(),
                    };
                    return (cur_block, None);
                }
                let b = self.get_block_mut(cur_block);
                b.instructions.push(Inst::Return { value: val });
                b.terminator = Terminator::Return { value: val };
                (cur_block, val)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                // M7 TSU narrowing: `if x is T` re-binds x to the UNBOXED
                // value for the whole then-branch (checked read — a tag
                // mismatch panics instead of reinterpreting bits), restoring
                // the dynamic binding afterwards. Only identifier operands
                // narrow; index/call results re-evaluate per read.
                let narrow = match condition {
                    Expr::IsCheck {
                        operand, ty_name, ..
                    } => match (&**operand, ty_name.as_str()) {
                        (Expr::Identifier(v, _), "Int") => {
                            Some((v.clone(), "datara_rt_val_to_int_checked", "Int".to_string()))
                        }
                        (Expr::Identifier(v, _), "Float") => Some((
                            v.clone(),
                            "datara_rt_val_to_float_checked",
                            "Float".to_string(),
                        )),
                        (Expr::Identifier(v, _), "Str" | "String") => {
                            Some((v.clone(), "datara_val_to_str", "String".to_string()))
                        }
                        (Expr::Identifier(v, _), "Bool") => {
                            Some((v.clone(), "datara_val_to_bool", "Bool".to_string()))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let cond_val = self
                    .lower_expr(condition, &mut cur_block)
                    .unwrap_or(ValueId(0));
                let then_id = self.create_block("if_then");
                let else_id = self.create_block("if_else");
                let merge_id = self.create_block("if_merge");

                self.get_block_mut(cur_block).terminator = Terminator::CondBranch {
                    cond: cond_val,
                    then_block: then_id,
                    then_args: Vec::new(),
                    else_block: else_id,
                    else_args: Vec::new(),
                };

                // Narrow re-binding inside the then-block.
                let narrow_shadow = narrow.map(|(var, func, ty)| {
                    let raw = self.symbol_values.get(&var).copied().unwrap_or(ValueId(0));
                    let unboxed = self.next_val();
                    self.get_block_mut(then_id).instructions.push(Inst::Call {
                        dest: unboxed,
                        func: func.into(),
                        args: vec![raw],
                        ty: ty.clone(),
                    });
                    let old_val = self.symbol_values.get(&var).copied();
                    let old_ty = self.local_var_types.get(&var).cloned();
                    self.symbol_values.insert(var.clone(), unboxed);
                    self.local_var_types.insert(
                        var.clone(),
                        match ty.as_str() {
                            "Float" => DataraType::Float,
                            "Bool" => DataraType::Bool,
                            "String" | "Str" => DataraType::String,
                            _ => DataraType::Int,
                        },
                    );
                    self.get_block_mut(then_id)
                        .instructions
                        .push(Inst::AssignVar {
                            name: var.clone(),
                            value: unboxed,
                        });
                    (var, old_val, old_ty)
                });

                let (then_end, _) = self.lower_stmt_cfg(then_branch, then_id);
                if let Some((var, old_val, old_ty)) = &narrow_shadow {
                    let restore_val = *old_val;
                    let restore_ty = old_ty.clone();
                    let var = var.clone();
                    match restore_val {
                        Some(ov) => {
                            self.symbol_values.insert(var.clone(), ov);
                        }
                        None => {
                            self.symbol_values.remove(var.as_str());
                        }
                    }
                    match restore_ty {
                        Some(t) => {
                            self.local_var_types.insert(var.clone(), t);
                        }
                        None => {
                            self.local_var_types.remove(var.as_str());
                        }
                    }
                    let b = self.get_block_mut(then_end);
                    if let Some(ov) = restore_val {
                        b.instructions.push(Inst::AssignVar {
                            name: var,
                            value: ov,
                        });
                    }
                }
                if matches!(
                    self.get_block_mut(then_end).terminator,
                    Terminator::Unreachable
                ) {
                    self.get_block_mut(then_end).terminator = Terminator::Branch {
                        target: merge_id,
                        args: Vec::new(),
                    };
                }

                let (else_end, _) = if let Some(eb) = else_branch {
                    self.lower_stmt_cfg(eb, else_id)
                } else {
                    (else_id, None)
                };
                if matches!(
                    self.get_block_mut(else_end).terminator,
                    Terminator::Unreachable
                ) {
                    self.get_block_mut(else_end).terminator = Terminator::Branch {
                        target: merge_id,
                        args: Vec::new(),
                    };
                }

                (merge_id, None)
            }
            Stmt::While {
                condition, body, ..
            } => {
                let mut header_id = self.create_block("while_header");
                // The back edge must re-enter the loop at the FIRST block of the
                // condition, not wherever condition lowering left off: a
                // short-circuiting condition ("a && b") spans several blocks and
                // every one of them has to run again on the next iteration.
                let loop_header_id = header_id;
                let body_id = self.create_block("while_body");
                let exit_id = self.create_block("while_exit");

                self.get_block_mut(cur_block).terminator = Terminator::Branch {
                    target: header_id,
                    args: Vec::new(),
                };

                let cond_val = self
                    .lower_expr(condition, &mut header_id)
                    .unwrap_or(ValueId(0));
                self.get_block_mut(header_id).terminator = Terminator::CondBranch {
                    cond: cond_val,
                    then_block: body_id,
                    then_args: Vec::new(),
                    else_block: exit_id,
                    else_args: Vec::new(),
                };

                // `continue` re-enters the loop at the header so the full
                // (possibly short-circuiting) condition chain re-runs;
                // `break` leaves at the exit block.
                self.loop_stack.push((loop_header_id, exit_id));
                let (body_end, _) = self.lower_stmt_cfg(body, body_id);
                self.loop_stack.pop();
                self.set_back_edge(body_end, loop_header_id);

                // No compound `WhileLoop` snapshot is emitted here. The legacy
                // node duplicated the header/body instructions inside a single
                // instruction: the backend ignored it, and under strict SSA
                // verification its duplicate definitions and out-of-dominance
                // uses are illegal. The real CFG blocks above are the loop.

                (exit_id, None)
            }
            Stmt::For {
                var_name,
                iterable,
                body,
                ..
            } => {
                // `for v in start..end` desugars into a counted loop:
                //     v = start
                //     while v < end { body; v = v + 1 }
                //
                // Iterating a non-range value (a list, map or string) is not
                // implemented: the runtime exposes no iterator protocol yet, so
                // the expression is evaluated and the body runs once, exactly as
                // before. See docs/AUDIT_OPTIMIZATION_FIXES.md.
                match iterable {
                    Expr::Range { start, end, .. } => {
                        let start_val = self.lower_expr(start, &mut cur_block);
                        let end_val = self.lower_expr(end, &mut cur_block);

                        let header_id = self.create_block("for_header");
                        let body_id = self.create_block("for_body");
                        let exit_id = self.create_block("for_exit");

                        if let Some(sv) = start_val {
                            self.get_block_mut(cur_block)
                                .instructions
                                .push(Inst::AssignVar {
                                    name: var_name.clone(),
                                    value: sv,
                                });
                            // Registering the name is what makes `Expr::Identifier`
                            // inside the body emit a `LoadVar` for the induction variable.
                            self.symbol_values.insert(var_name.clone(), sv);
                        }
                        self.get_block_mut(cur_block).terminator = Terminator::Branch {
                            target: header_id,
                            args: Vec::new(),
                        };

                        // Header: cond = (v < end)
                        let cur = self.next_val();
                        self.get_block_mut(header_id)
                            .instructions
                            .push(Inst::LoadVar {
                                dest: cur,
                                name: var_name.clone(),
                            });
                        let cond = self.next_val();
                        match end_val {
                            Some(ev) => {
                                self.get_block_mut(header_id)
                                    .instructions
                                    .push(Inst::BinOp {
                                        dest: cond,
                                        op: "<".into(),
                                        left: cur,
                                        right: ev,
                                        ty: "Int".into(),
                                    })
                            }
                            None => {
                                self.get_block_mut(header_id)
                                    .instructions
                                    .push(Inst::ConstBool {
                                        dest: cond,
                                        value: false,
                                    })
                            }
                        }
                        self.get_block_mut(header_id).terminator = Terminator::CondBranch {
                            cond,
                            then_block: body_id,
                            then_args: Vec::new(),
                            else_block: exit_id,
                            else_args: Vec::new(),
                        };

                        // The increment lives in its own block so `continue`
                        // can jump to it: a continued iteration must still
                        // advance the induction variable before re-testing
                        // the loop condition.
                        let incr_id = self.create_block("for_incr");
                        self.loop_stack.push((incr_id, exit_id));
                        let (body_end, _) = self.lower_stmt_cfg(body, body_id);
                        self.loop_stack.pop();

                        // A body that ends in `return` or `break` has no
                        // fall-through edge, so it never reaches the increment.
                        if self.block_falls_through(body_end) {
                            self.set_back_edge(body_end, incr_id);
                        }
                        // Increment: v = v + 1
                        let one = self.next_val();
                        self.get_block_mut(incr_id)
                            .instructions
                            .push(Inst::ConstInt {
                                dest: one,
                                value: 1,
                            });
                        let loaded = self.next_val();
                        self.get_block_mut(incr_id)
                            .instructions
                            .push(Inst::LoadVar {
                                dest: loaded,
                                name: var_name.clone(),
                            });
                        let next = self.next_val();
                        self.get_block_mut(incr_id).instructions.push(Inst::BinOp {
                            dest: next,
                            op: "+".into(),
                            left: loaded,
                            right: one,
                            ty: "Int".into(),
                        });
                        self.get_block_mut(incr_id)
                            .instructions
                            .push(Inst::AssignVar {
                                name: var_name.clone(),
                                value: next,
                            });
                        self.get_block_mut(incr_id).terminator = Terminator::Branch {
                            target: header_id,
                            args: Vec::new(),
                        };

                        (exit_id, None)
                    }
                    _ if self.is_expr_str(iterable) => {
                        let str_val = self.lower_expr(iterable, &mut cur_block);
                        match str_val {
                            Some(sv) => {
                                let offset_name = format!("__for_stroff_{}", self.next_val().0);
                                let zero = self.next_val();
                                self.get_block_mut(cur_block)
                                    .instructions
                                    .push(Inst::ConstInt {
                                        dest: zero,
                                        value: 0,
                                    });
                                self.get_block_mut(cur_block)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: offset_name.clone(),
                                        value: zero,
                                    });
                                let len_val = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(Inst::Call {
                                    dest: len_val,
                                    func: "datara_rt_str_len".into(),
                                    args: vec![sv],
                                    ty: "Int".into(),
                                });

                                let header_id = self.create_block("for_str_header");
                                let body_id = self.create_block("for_str_body");
                                let exit_id = self.create_block("for_str_exit");
                                self.get_block_mut(cur_block).terminator = Terminator::Branch {
                                    target: header_id,
                                    args: Vec::new(),
                                };

                                let cur_off = self.next_val();
                                self.get_block_mut(header_id)
                                    .instructions
                                    .push(Inst::LoadVar {
                                        dest: cur_off,
                                        name: offset_name.clone(),
                                    });
                                let cond = self.next_val();
                                self.get_block_mut(header_id)
                                    .instructions
                                    .push(Inst::BinOp {
                                        dest: cond,
                                        op: "<".into(),
                                        left: cur_off,
                                        right: len_val,
                                        ty: "Int".into(),
                                    });
                                self.get_block_mut(header_id).terminator = Terminator::CondBranch {
                                    cond,
                                    then_block: body_id,
                                    then_args: Vec::new(),
                                    else_block: exit_id,
                                    else_args: Vec::new(),
                                };

                                let fetch_off = self.next_val();
                                self.get_block_mut(body_id)
                                    .instructions
                                    .push(Inst::LoadVar {
                                        dest: fetch_off,
                                        name: offset_name.clone(),
                                    });
                                let scalar_val = self.next_val();
                                self.get_block_mut(body_id).instructions.push(Inst::Call {
                                    dest: scalar_val,
                                    func: "datara_rt_str_scalar_at".into(),
                                    args: vec![sv, fetch_off],
                                    ty: "String".into(),
                                });
                                self.get_block_mut(body_id)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: var_name.clone(),
                                        value: scalar_val,
                                    });
                                self.symbol_values.insert(var_name.clone(), scalar_val);

                                let incr_id = self.create_block("for_str_incr");
                                self.loop_stack.push((incr_id, exit_id));
                                let (body_end, _) = self.lower_stmt_cfg(body, body_id);
                                self.loop_stack.pop();

                                if self.block_falls_through(body_end) {
                                    self.set_back_edge(body_end, incr_id);
                                }
                                let body_off = self.next_val();
                                self.get_block_mut(incr_id)
                                    .instructions
                                    .push(Inst::LoadVar {
                                        dest: body_off,
                                        name: offset_name.clone(),
                                    });
                                let next_off = self.next_val();
                                self.get_block_mut(incr_id).instructions.push(Inst::Call {
                                    dest: next_off,
                                    func: "datara_rt_str_next_offset".into(),
                                    args: vec![sv, body_off],
                                    ty: "Int".into(),
                                });
                                self.get_block_mut(incr_id)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: offset_name.clone(),
                                        value: next_off,
                                    });
                                self.get_block_mut(incr_id).terminator = Terminator::Branch {
                                    target: header_id,
                                    args: Vec::new(),
                                };

                                (exit_id, None)
                            }
                            None => (cur_block, None),
                        }
                    }
                    _ => {
                        // `for item in <list>`: lower into a counted loop over
                        // the runtime list protocol:
                        //     idx = 0; len = list_len(list)
                        //     while idx < len {
                        //         item = list_get(list, idx)
                        //         body
                        //         idx = idx + 1
                        //     }
                        // The list pointer is materialised once, before the
                        // loop, and the header reads only the induction var.
                        let list_val = self.lower_expr(iterable, &mut cur_block);
                        match list_val {
                            Some(lv) => {
                                let idx_name = format!("__for_idx_{}", self.next_val().0);
                                let zero = self.next_val();
                                self.get_block_mut(cur_block)
                                    .instructions
                                    .push(Inst::ConstInt {
                                        dest: zero,
                                        value: 0,
                                    });
                                self.get_block_mut(cur_block)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: idx_name.clone(),
                                        value: zero,
                                    });
                                let len_val = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(Inst::Call {
                                    dest: len_val,
                                    func: "datara_rt_list_len".into(),
                                    args: vec![lv],
                                    ty: "Int".into(),
                                });

                                let header_id = self.create_block("for_header");
                                let body_id = self.create_block("for_body");
                                let exit_id = self.create_block("for_exit");
                                self.get_block_mut(cur_block).terminator = Terminator::Branch {
                                    target: header_id,
                                    args: Vec::new(),
                                };

                                let cur_idx = self.next_val();
                                self.get_block_mut(header_id)
                                    .instructions
                                    .push(Inst::LoadVar {
                                        dest: cur_idx,
                                        name: idx_name.clone(),
                                    });
                                let cond = self.next_val();
                                self.get_block_mut(header_id)
                                    .instructions
                                    .push(Inst::BinOp {
                                        dest: cond,
                                        op: "<".into(),
                                        left: cur_idx,
                                        right: len_val,
                                        ty: "Int".into(),
                                    });
                                self.get_block_mut(header_id).terminator = Terminator::CondBranch {
                                    cond,
                                    then_block: body_id,
                                    then_args: Vec::new(),
                                    else_block: exit_id,
                                    else_args: Vec::new(),
                                };

                                // Fetch the element and bind it before the
                                // user statements run. Registering the loop
                                // var in symbol_values makes every
                                // `Expr::Identifier` inside the body emit a
                                // `LoadVar`, which is correct across
                                // iterations (same pattern as the counted
                                // `for i in a..b` loop above).
                                let fetch_idx = self.next_val();
                                self.get_block_mut(body_id)
                                    .instructions
                                    .push(Inst::LoadVar {
                                        dest: fetch_idx,
                                        name: idx_name.clone(),
                                    });
                                // The element representation must match the
                                // loop variable's checked type: the backend
                                // keys string/bool/float printing and concat
                                // off the Call's `ty` field, so a `List<Str>`
                                // loop must fetch with `ty: "String"`, not the
                                // integer default.
                                let elem_type =
                                    self.lookup_var_type(var_name).or_else(|| match iterable {
                                        Expr::Identifier(name, _) => {
                                            match self.lookup_var_type(name) {
                                                Some(DataraType::List(inner)) => Some(*inner),
                                                _ => None,
                                            }
                                        }
                                        _ => None,
                                    });
                                if let Some(ref et) = elem_type {
                                    self.local_var_types.insert(var_name.clone(), et.clone());
                                }
                                let elem_repr = match elem_type.as_ref() {
                                    Some(DataraType::String) | Some(DataraType::Char) => "String",
                                    Some(DataraType::Bool) => "Bool",
                                    Some(DataraType::Float) => "Float",
                                    Some(DataraType::List(_)) => "List",
                                    Some(DataraType::Map(..)) => "Map",
                                    _ => "Int",
                                };
                                let item_val = self.next_val();
                                self.get_block_mut(body_id).instructions.push(Inst::Call {
                                    dest: item_val,
                                    func: "datara_rt_list_get_unchecked".into(),
                                    args: vec![lv, fetch_idx],
                                    ty: elem_repr.into(),
                                });
                                self.get_block_mut(body_id)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: var_name.clone(),
                                        value: item_val,
                                    });
                                self.symbol_values.insert(var_name.clone(), item_val);

                                let incr_id = self.create_block("for_incr");
                                self.loop_stack.push((incr_id, exit_id));
                                let (body_end, _) = self.lower_stmt_cfg(body, body_id);
                                self.loop_stack.pop();

                                if self.block_falls_through(body_end) {
                                    self.set_back_edge(body_end, incr_id);
                                }
                                // Increment: idx = idx + 1
                                let one = self.next_val();
                                self.get_block_mut(incr_id)
                                    .instructions
                                    .push(Inst::ConstInt {
                                        dest: one,
                                        value: 1,
                                    });
                                let loaded = self.next_val();
                                self.get_block_mut(incr_id)
                                    .instructions
                                    .push(Inst::LoadVar {
                                        dest: loaded,
                                        name: idx_name.clone(),
                                    });
                                let next = self.next_val();
                                self.get_block_mut(incr_id).instructions.push(Inst::BinOp {
                                    dest: next,
                                    op: "+".into(),
                                    left: loaded,
                                    right: one,
                                    ty: "Int".into(),
                                });
                                self.get_block_mut(incr_id)
                                    .instructions
                                    .push(Inst::AssignVar {
                                        name: idx_name.clone(),
                                        value: next,
                                    });
                                self.get_block_mut(incr_id).terminator = Terminator::Branch {
                                    target: header_id,
                                    args: Vec::new(),
                                };

                                (exit_id, None)
                            }
                            None => self.lower_stmt_cfg(body, cur_block),
                        }
                    }
                }
            }
            Stmt::Loop { body, .. } => {
                // `loop { .. }` is `while true { .. }`. The exit block only
                // exists so the value after the loop is well formed; it is
                // unreachable unless the body returns.
                let header_id = self.create_block("loop_header");
                let body_id = self.create_block("loop_body");
                let exit_id = self.create_block("loop_exit");

                self.get_block_mut(cur_block).terminator = Terminator::Branch {
                    target: header_id,
                    args: Vec::new(),
                };

                let true_val = self.next_val();
                self.get_block_mut(header_id)
                    .instructions
                    .push(Inst::ConstInt {
                        dest: true_val,
                        value: 1,
                    });
                self.get_block_mut(header_id).terminator = Terminator::CondBranch {
                    cond: true_val,
                    then_block: body_id,
                    then_args: Vec::new(),
                    else_block: exit_id,
                    else_args: Vec::new(),
                };

                self.loop_stack.push((header_id, exit_id));
                let (body_end, _) = self.lower_stmt_cfg(body, body_id);
                self.loop_stack.pop();
                self.set_back_edge(body_end, header_id);

                (exit_id, None)
            }
            Stmt::Break(_) => {
                // The type checker guarantees a surrounding loop (E0312);
                // defensively fall through to the next statement otherwise.
                if let Some((_, break_id)) = self.loop_stack.last().copied() {
                    self.get_block_mut(cur_block).terminator = Terminator::Branch {
                        target: break_id,
                        args: Vec::new(),
                    };
                    // Statements after `break` in the same block are dead
                    // code; route them into a fresh unreachable block that
                    // the DCE pass removes.
                    cur_block = self.create_block("after_break");
                }
                (cur_block, None)
            }
            Stmt::Continue(_) => {
                if let Some((continue_id, _)) = self.loop_stack.last().copied() {
                    self.get_block_mut(cur_block).terminator = Terminator::Branch {
                        target: continue_id,
                        args: Vec::new(),
                    };
                    cur_block = self.create_block("after_continue");
                }
                (cur_block, None)
            }
            Stmt::Parallel(body, _) => {
                if let Stmt::Block(stmts, _) = body.as_ref() {
                    let get_call_info = |s: &Stmt| -> Option<(String, Option<Expr>)> {
                        match s {
                            Stmt::Expr(Expr::Call { callee, args, .. }, _) => {
                                if let Expr::Identifier(fn_name, _) = callee.as_ref() {
                                    if args.is_empty() {
                                        return Some((fn_name.clone(), None));
                                    } else if args.len() == 1 {
                                        return Some((fn_name.clone(), Some(args[0].clone())));
                                    }
                                }
                            }
                            Stmt::Block(inner, _) if inner.len() == 1 => {
                                if let Stmt::Expr(Expr::Call { callee, args, .. }, _) = &inner[0]
                                    && let Expr::Identifier(fn_name, _) = callee.as_ref()
                                {
                                    if args.is_empty() {
                                        return Some((fn_name.clone(), None));
                                    } else if args.len() == 1 {
                                        return Some((fn_name.clone(), Some(args[0].clone())));
                                    }
                                }
                            }
                            _ => {}
                        }
                        None
                    };

                    let mut call_infos = Vec::new();
                    for s in stmts {
                        if let Some(info) = get_call_info(s) {
                            call_infos.push(info);
                        }
                    }

                    if call_infos.len() == stmts.len() && call_infos.len() >= 2 {
                        for chunk in call_infos.chunks(2) {
                            if chunk.len() == 2 {
                                let (ref fn1_name, ref arg1) = chunk[0];
                                let (ref fn2_name, ref arg2) = chunk[1];
                                let ctx1 = if let Some(arg) = arg1 {
                                    self.lower_expr(arg, &mut cur_block).unwrap_or_else(|| {
                                        let z = self.next_val();
                                        self.get_block_mut(cur_block)
                                            .instructions
                                            .push(Inst::ConstInt { dest: z, value: 0 });
                                        z
                                    })
                                } else {
                                    let z = self.next_val();
                                    self.get_block_mut(cur_block)
                                        .instructions
                                        .push(Inst::ConstInt { dest: z, value: 0 });
                                    z
                                };

                                let ctx2 = if let Some(arg) = arg2 {
                                    self.lower_expr(arg, &mut cur_block).unwrap_or_else(|| {
                                        let z = self.next_val();
                                        self.get_block_mut(cur_block)
                                            .instructions
                                            .push(Inst::ConstInt { dest: z, value: 0 });
                                        z
                                    })
                                } else {
                                    let z = self.next_val();
                                    self.get_block_mut(cur_block)
                                        .instructions
                                        .push(Inst::ConstInt { dest: z, value: 0 });
                                    z
                                };

                                let fn1_addr = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(
                                    Inst::GetFuncAddr {
                                        dest: fn1_addr,
                                        func_name: fn1_name.clone(),
                                    },
                                );

                                let fn2_addr = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(
                                    Inst::GetFuncAddr {
                                        dest: fn2_addr,
                                        func_name: fn2_name.clone(),
                                    },
                                );

                                let dummy = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(Inst::Call {
                                    dest: dummy,
                                    func: "datara_rt_parallel_invoke".into(),
                                    args: vec![fn1_addr, ctx1, fn2_addr, ctx2],
                                    ty: "Unit".into(),
                                });
                            } else {
                                let (ref fn_name, ref arg) = chunk[0];
                                let call_args = if let Some(a) = arg {
                                    let ctx =
                                        self.lower_expr(a, &mut cur_block).unwrap_or_else(|| {
                                            let z = self.next_val();
                                            self.get_block_mut(cur_block)
                                                .instructions
                                                .push(Inst::ConstInt { dest: z, value: 0 });
                                            z
                                        });
                                    vec![ctx]
                                } else {
                                    vec![]
                                };
                                let dummy = self.next_val();
                                self.get_block_mut(cur_block).instructions.push(Inst::Call {
                                    dest: dummy,
                                    func: fn_name.clone(),
                                    args: call_args,
                                    ty: "Unit".into(),
                                });
                            }
                        }
                        return (cur_block, None);
                    }
                }
                self.lower_stmt_cfg(body, cur_block)
            }
            Stmt::ParallelFor {
                var_name,
                iterable,
                body,
                span,
            } => {
                if let Expr::Range { start, end, .. } = iterable {
                    let worker_opt = match body.as_ref() {
                        Stmt::Expr(Expr::Call { callee, args, .. }, _) => {
                            if let Expr::Identifier(fn_name, _) = callee.as_ref() {
                                if args.len() == 1 {
                                    if let Expr::Identifier(arg_name, _) = &args[0] {
                                        if arg_name == var_name {
                                            Some(fn_name.clone())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        Stmt::Block(stmts, _) if stmts.len() == 1 => {
                            if let Stmt::Expr(Expr::Call { callee, args, .. }, _) = &stmts[0] {
                                if let Expr::Identifier(fn_name, _) = callee.as_ref() {
                                    if args.len() == 1 {
                                        if let Expr::Identifier(arg_name, _) = &args[0] {
                                            if arg_name == var_name {
                                                Some(fn_name.clone())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };

                    if let Some(fn_name) = worker_opt
                        && let (Some(s_val), Some(e_val)) = (
                            self.lower_expr(start, &mut cur_block),
                            self.lower_expr(end, &mut cur_block),
                        )
                    {
                        let fn_addr = self.next_val();
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::GetFuncAddr {
                                dest: fn_addr,
                                func_name: fn_name,
                            });
                        let zero = self.next_val();
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::ConstInt {
                                dest: zero,
                                value: 0,
                            });
                        let dummy = self.next_val();
                        self.get_block_mut(cur_block).instructions.push(Inst::Call {
                            dest: dummy,
                            func: "datara_rt_parallel_for".into(),
                            args: vec![s_val, e_val, fn_addr, zero],
                            ty: "Unit".into(),
                        });
                        return (cur_block, None);
                    }
                }
                let for_stmt = Stmt::For {
                    var_name: var_name.clone(),
                    iterable: iterable.clone(),
                    body: body.clone(),
                    span: span.clone(),
                };
                self.lower_stmt_cfg(&for_stmt, cur_block)
            }
            Stmt::With {
                resource_name,
                init,
                body,
                ..
            } => {
                let prev_sym = self.symbol_values.get(resource_name).cloned();
                if let Some(init_val) = self.lower_expr(init, &mut cur_block) {
                    self.symbol_values.insert(resource_name.clone(), init_val);
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::AssignVar {
                            name: resource_name.clone(),
                            value: init_val,
                        });
                }
                let (body_end, ret_val) = self.lower_stmt_cfg(body, cur_block);
                if self.block_falls_through(body_end) {
                    let res_val = self.next_val();
                    self.get_block_mut(body_end)
                        .instructions
                        .push(Inst::LoadVar {
                            dest: res_val,
                            name: resource_name.clone(),
                        });
                    let close_dest = self.next_val();
                    self.get_block_mut(body_end)
                        .instructions
                        .push(Inst::MethodCall {
                            dest: close_dest,
                            object: res_val,
                            method: "close".into(),
                            args: Vec::new(),
                            ty: "Unit".into(),
                        });
                }
                match prev_sym {
                    Some(v) => self.symbol_values.insert(resource_name.clone(), v),
                    None => self.symbol_values.remove(resource_name),
                };
                (body_end, ret_val)
            }
            Stmt::Unsafe { body, .. } => self.lower_stmt_cfg(body, cur_block),
            Stmt::Asm {
                instructions,
                options,
                structured,
                ..
            } => {
                if !structured.is_empty() {
                    super::asm::lower_structured_asm(self, structured, cur_block)
                } else {
                    let template = instructions.join("; ");
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::InlineAsm {
                            template,
                            outputs: Vec::new(),
                            inputs: Vec::new(),
                            clobbers: Vec::new(),
                            options: options.clone(),
                        });
                    (cur_block, None)
                }
            }
            Stmt::Simd(body, _) => self.lower_stmt_cfg(body, cur_block),
        }
    }
}
