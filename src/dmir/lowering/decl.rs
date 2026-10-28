use crate::ast::*;
use crate::dmir::ir::*;
use std::collections::HashMap;

use super::Lowering;

/// Full recursive TypeNode → DataraType for parameter records. The flat
/// name-match used before collapsed nested generics: a
/// `List<Outcome<Int>>` parameter was recorded as `List(Int)`, so `l[0]`
/// lowered to a list_get with ty "Int" and the `?`-unwrap GetField
/// `is_success` failed codegen with E0944 (refusing to guess the field
/// offset on a non-class value).
pub(crate) fn type_node_datara(tn: &TypeNode) -> crate::types::DataraType {
    use crate::types::DataraType;
    match tn.name.as_str() {
        "Int" | "Int64" | "i64" | "int" | "isize" => DataraType::Int,
        "Float" | "Float64" | "f64" | "float" => DataraType::Float,
        "Float32" | "f32" => DataraType::Float32,
        "Bool" | "bool" => DataraType::Bool,
        "Str" | "String" | "str" => DataraType::String,
        "List" => DataraType::List(Box::new(match tn.generic_args.first() {
            Some(arg) => type_node_datara(arg),
            None => DataraType::Int,
        })),
        "Map" => DataraType::Map(
            Box::new(match tn.generic_args.first() {
                Some(arg) => type_node_datara(arg),
                None => DataraType::String,
            }),
            Box::new(match tn.generic_args.get(1) {
                Some(arg) => type_node_datara(arg),
                None => DataraType::Int,
            }),
        ),
        other => {
            if other == "Val" || other == "any" {
                // M7 TSU: the dynamic type itself — a List<Val> parameter
                // must record DataraType::Val (not Class("Val")) so `is`
                // checks and val methods route to the NaN-box path.
                DataraType::Val
            } else if tn.generic_args.is_empty() {
                DataraType::Class(other.to_string())
            } else {
                DataraType::GenericInstance {
                    name: other.to_string(),
                    args: tn.generic_args.iter().map(type_node_datara).collect(),
                }
            }
        }
    }
}

impl<'a> Lowering<'a> {
    pub(crate) fn lower_function(&mut self, f: &FunctionDecl) -> Function {
        self.val_counter = 0;
        self.current_fn_name = f.name.clone();
        self.local_var_types.clear();
        self.current_blocks.clear();
        self.block_counter = 0;
        self.symbol_values.clear();
        self.current_line_spans.clear();

        let mut entry_id = self.create_block("entry");

        let mut params = Vec::new();
        let mut param_refinements = Vec::new();
        for p in &f.params {
            let p_val = self.next_val();
            let ty_str = p
                .type_node
                .as_ref()
                .map(|t| t.full_type_name())
                .unwrap_or_else(|| "Int".into());
            if let Some(ref tn) = p.type_node {
                if tn.name == "List" && !tn.generic_args.is_empty() {
                    let elem = type_node_datara(&tn.generic_args[0]);
                    self.local_var_types.insert(
                        p.name.clone(),
                        crate::types::DataraType::List(Box::new(elem)),
                    );
                    self.class_field_types
                        .insert(p.name.clone(), ty_str.clone());
                } else if tn.name == "Float32" {
                    // v1.4.5 W2: keep the f32 width on parameters too.
                    self.local_var_types
                        .insert(p.name.clone(), crate::types::DataraType::Float32);
                    self.class_field_types
                        .insert(p.name.clone(), "Float32".into());
                } else if tn.name == "Float" || tn.name == "Float64" {
                    self.local_var_types
                        .insert(p.name.clone(), crate::types::DataraType::Float);
                    self.class_field_types
                        .insert(p.name.clone(), "Float".into());
                } else if tn.name == "String" || tn.name == "Str" {
                    self.local_var_types
                        .insert(p.name.clone(), crate::types::DataraType::String);
                } else if tn.name == "Bool" {
                    self.local_var_types
                        .insert(p.name.clone(), crate::types::DataraType::Bool);
                } else if tn.name == "Map" {
                    // Typed map parameters keep BOTH arguments. The old
                    // fallback below only ran for parameters WITHOUT a type
                    // node, so `fn f(m: Map<Int, Int>)` was recorded as
                    // Map<Str, Int> and every `m.get(int)` dispatched to the
                    // string ABI (miss -> `or` fallback, or worse).
                    let parse = |t: Option<&crate::ast::TypeNode>| match t {
                        Some(tn) => match tn.name.as_str() {
                            "Int" | "Int64" | "Int32" | "Int16" | "Int8" => {
                                crate::types::DataraType::Int
                            }
                            "Float" | "Float64" | "f64" | "Float32" | "f32" => {
                                crate::types::DataraType::Float
                            }
                            "Bool" => crate::types::DataraType::Bool,
                            _ => crate::types::DataraType::String,
                        },
                        None => crate::types::DataraType::Int,
                    };
                    let k_ty = parse(tn.generic_args.first());
                    let v_ty = parse(tn.generic_args.get(1));
                    self.local_var_types.insert(
                        p.name.clone(),
                        crate::types::DataraType::Map(Box::new(k_ty), Box::new(v_ty)),
                    );
                }
            } else if ty_str == "Float" {
                self.class_field_types
                    .insert(p.name.clone(), "Float".into());
            } else if ty_str == "Float32" {
                self.class_field_types
                    .insert(p.name.clone(), "Float32".into());
            } else if ty_str == "Int8" {
                self.local_var_types
                    .insert(p.name.clone(), crate::types::DataraType::Int8);
            } else if ty_str == "Int16" {
                self.local_var_types
                    .insert(p.name.clone(), crate::types::DataraType::Int16);
            } else if ty_str == "Int32" {
                self.local_var_types
                    .insert(p.name.clone(), crate::types::DataraType::Int32);
            } else if ty_str == "UInt8" || ty_str == "Byte" {
                self.local_var_types
                    .insert(p.name.clone(), crate::types::DataraType::UInt8);
            } else if ty_str == "UInt16" {
                self.local_var_types
                    .insert(p.name.clone(), crate::types::DataraType::UInt16);
            } else if ty_str == "UInt32" {
                self.local_var_types
                    .insert(p.name.clone(), crate::types::DataraType::UInt32);
            } else if ty_str == "UInt" || ty_str == "UInt64" {
                self.local_var_types
                    .insert(p.name.clone(), crate::types::DataraType::UInt64);
            } else if let Some(rest) = ty_str.strip_prefix("Map<")
                && let Some(inner) = rest.strip_suffix('>')
            {
                // v1.5.0: Map<Str, Str> parameters must keep their value
                // type in local_var_types — `m.get(k)` keys the Outcome
                // payload tagging (Int vs bitcast-Float vs Str pointer)
                // off the receiver's value type, and without this entry
                // the repr fell back to Int (raw pointer printed as a
                // number, E0944-class silent wrong data).
                let key_repr = inner.split(',').next().unwrap_or("Str").trim().to_string();
                let key_ty = match key_repr.as_str() {
                    "Int" | "Int64" | "Int32" | "Int16" | "Int8" => crate::types::DataraType::Int,
                    _ => crate::types::DataraType::String,
                };
                let val_repr = inner.rsplit(',').next().unwrap_or("Int").trim().to_string();
                let val_ty = match val_repr.as_str() {
                    "Float" | "Float64" | "f64" => crate::types::DataraType::Float,
                    "Float32" | "f32" => crate::types::DataraType::Float32,
                    "Bool" => crate::types::DataraType::Bool,
                    "Int" | "Int64" | "Int32" | "Int16" | "Int8" => crate::types::DataraType::Int,
                    _ => crate::types::DataraType::String,
                };
                self.local_var_types.insert(
                    p.name.clone(),
                    crate::types::DataraType::Map(Box::new(key_ty), Box::new(val_ty)),
                );
            }
            params.push((p.name.clone(), ty_str, p_val));
            let refn = p.type_node.as_ref().and_then(|t| {
                if let Some(r) = &t.refinement {
                    Some(r.clone())
                } else if let Some(td) = self.resolver.type_aliases.get(&t.name) {
                    td.base_type.refinement.clone()
                } else {
                    None
                }
            });
            param_refinements.push((p.name.clone(), refn));
            self.symbol_values.insert(p.name.clone(), p_val);
        }

        if f.name == "main" {
            let globals = std::mem::take(&mut self.program_globals);
            for g in &globals {
                if let Some(init_val) = self.lower_expr(&g.init, &mut entry_id) {
                    self.symbol_values.insert(g.name.clone(), init_val);
                    self.get_block_mut(entry_id)
                        .instructions
                        .push(Inst::AssignVar {
                            name: g.name.clone(),
                            value: init_val,
                        });
                }
            }
            self.program_globals = globals;
        }

        for req in &f.requires {
            if is_contract_statically_true(&req.condition, &param_refinements) {
                continue;
            }
            if let Some(cond_val) = self.lower_expr(&req.condition, &mut entry_id) {
                let msg_val = self.next_val();
                let user_msg = req.message.as_deref().unwrap_or("Precondition failed");
                let prefix = if user_msg.starts_with("CONTRACT_VIOLATION") {
                    ""
                } else {
                    "CONTRACT_VIOLATION: "
                };
                let msg_str = format!("{}{}", prefix, user_msg);
                self.get_block_mut(entry_id)
                    .instructions
                    .push(Inst::ConstStr {
                        dest: msg_val,
                        value: msg_str,
                    });
                let dest = self.next_val();
                self.get_block_mut(entry_id).instructions.push(Inst::Call {
                    dest,
                    func: "datara_rt_assert".into(),
                    args: vec![cond_val, msg_val],
                    ty: "Unit".into(),
                });
            }
        }

        let (mut cur_block, ret_val) = self.lower_stmt_cfg(&f.body, entry_id);

        if !f.ensures.is_empty() {
            if let Some(rv) = ret_val {
                self.symbol_values.insert("result".into(), rv);
            }
            for ens in &f.ensures {
                if is_contract_statically_true(&ens.condition, &[]) {
                    continue;
                }
                if let Some(cond_val) = self.lower_expr(&ens.condition, &mut cur_block) {
                    let msg_val = self.next_val();
                    let user_msg = ens.message.as_deref().unwrap_or("Postcondition failed");
                    let prefix = if user_msg.starts_with("CONTRACT_VIOLATION") {
                        ""
                    } else {
                        "CONTRACT_VIOLATION: "
                    };
                    let msg_str = format!("{}{}", prefix, user_msg);
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: msg_val,
                            value: msg_str,
                        });
                    let dest = self.next_val();
                    self.get_block_mut(cur_block).instructions.push(Inst::Call {
                        dest,
                        func: "datara_rt_assert".into(),
                        args: vec![cond_val, msg_val],
                        ty: "Unit".into(),
                    });
                }
            }
            self.symbol_values.remove("result");
        }
        self.symbol_values.clear();

        let cur = self.get_block_mut(cur_block);
        if matches!(
            cur.terminator,
            Terminator::Unreachable | Terminator::Return { value: None }
        ) {
            cur.terminator = Terminator::Return { value: ret_val };
        }
        if !cur
            .instructions
            .iter()
            .any(|i| matches!(i, Inst::Return { .. }))
            && ret_val.is_some()
        {
            cur.instructions.push(Inst::Return { value: ret_val });
        }

        Function {
            name: f.name.clone(),
            params,
            param_refinements,
            requires: f.requires.clone(),
            return_type: f
                .return_type
                .as_ref()
                .map(Self::repr_type_string)
                .unwrap_or_else(|| "Unit".into()),
            entry_block: entry_id,
            blocks: std::mem::take(&mut self.current_blocks),
            // v1.3.4: malformed inline attributes are rejected by the
            // driver-level validation pass before lowering runs, so only
            // the classified hint is carried here.
            inline_hint: InlineHint::parse_from_attrs(&f.attributes).0,
            // v1.4.0: allocator tier (@arena/@pool) and the asm marker.
            alloc_hint: ArenaHint::parse_from_attrs(&f.attributes).0,
            has_inline_asm: stmt_contains_structured_asm(&f.body),
            proven_no_overflow: std::collections::BTreeSet::new(),
        }
    }

    pub(crate) fn lower_class(&mut self, c: &ClassDecl, program: &Program, module: &mut Module) {
        for item in &c.body_items {
            if let ClassItem::Method(m) = item {
                let lowered = self.lower_method(m, &c.name);
                let fn_name = format!("{}_{}", c.name, m.name);
                self.symbol_table
                    .define(&fn_name, "method", Some(m.span.clone()));
                module.functions.insert(fn_name.clone(), lowered);
                module
                    .function_spans
                    .insert(fn_name.clone(), m.span.clone());
                module
                    .function_line_spans
                    .insert(fn_name, self.current_line_spans.clone());
            }
        }
        for item in &c.body_items {
            if let ClassItem::Using(other_name, _) = item {
                for decl in &program.declarations {
                    let other_methods: Vec<&MethodDecl> = match decl {
                        Decl::Class(oc) if oc.name == *other_name => oc
                            .body_items
                            .iter()
                            .filter_map(|it| match it {
                                ClassItem::Method(m) => Some(m),
                                _ => None,
                            })
                            .collect(),
                        Decl::Behavior(b) if b.target_type == *other_name => b
                            .body_items
                            .iter()
                            .filter_map(|it| match it {
                                ClassItem::Method(m) => Some(m),
                                _ => None,
                            })
                            .collect(),
                        _ => Vec::new(),
                    };

                    for m in other_methods {
                        let is_overridden = c.body_items.iter().any(|it| {
                            matches!(it, ClassItem::Method(my_m) if my_m.name == m.name)
                        }) || program.declarations.iter().any(|d| {
                            if let Decl::Behavior(my_b) = d && my_b.target_type == c.name {
                                my_b.body_items.iter().any(
                                    |it| matches!(it, ClassItem::Method(my_m) if my_m.name == m.name),
                                )
                            } else {
                                false
                            }
                        });

                        if !is_overridden {
                            let lowered = self.lower_method(m, &c.name);
                            let fn_name = format!("{}_{}", c.name, m.name);
                            self.symbol_table
                                .define(&fn_name, "method", Some(m.span.clone()));
                            module.functions.insert(fn_name.clone(), lowered);
                            module
                                .function_spans
                                .insert(fn_name.clone(), m.span.clone());
                            module
                                .function_line_spans
                                .insert(fn_name, self.current_line_spans.clone());
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn lower_behavior(&mut self, b: &BehaviorDecl, module: &mut Module) {
        for item in &b.body_items {
            if let ClassItem::Method(m) = item {
                let lowered = self.lower_method(m, &b.target_type);
                let fn_name = format!("{}_{}", b.target_type, m.name);
                let b_name = format!("B_{}_{}", b.target_type, m.name);
                self.symbol_table
                    .define(&fn_name, "method", Some(m.span.clone()));
                self.symbol_table
                    .define(&b_name, "method", Some(m.span.clone()));
                module.functions.insert(fn_name.clone(), lowered.clone());
                module.functions.insert(b_name.clone(), lowered);
                module
                    .function_spans
                    .insert(fn_name.clone(), m.span.clone());
                module.function_spans.insert(b_name.clone(), m.span.clone());
                module
                    .function_line_spans
                    .insert(fn_name, self.current_line_spans.clone());
                module
                    .function_line_spans
                    .insert(b_name, self.current_line_spans.clone());
            }
        }
    }

    pub(crate) fn lower_impl(&mut self, i: &ImplBlock, module: &mut Module) {
        for m in &i.methods {
            let lowered = self.lower_impl_function(m, &i.target_type);
            let fn_name = format!("{}_{}", i.target_type, m.name);
            self.symbol_table
                .define(&fn_name, "method", Some(m.span.clone()));
            module.functions.insert(fn_name.clone(), lowered.clone());
            module
                .function_spans
                .insert(fn_name.clone(), m.span.clone());
            module
                .function_line_spans
                .insert(fn_name.clone(), self.current_line_spans.clone());

            if let Some(ref tr) = i.trait_name {
                let i_name = format!("I_{}_{}_{}", tr, i.target_type, m.name);
                self.symbol_table
                    .define(&i_name, "method", Some(m.span.clone()));
                module.functions.insert(i_name.clone(), lowered.clone());
                module.function_spans.insert(i_name.clone(), m.span.clone());
                module
                    .function_line_spans
                    .insert(i_name, self.current_line_spans.clone());
            }

            if !module.functions.contains_key(&m.name) {
                self.symbol_table
                    .define(&m.name, "method", Some(m.span.clone()));
                module.functions.insert(m.name.clone(), lowered);
                module.function_spans.insert(m.name.clone(), m.span.clone());
                module
                    .function_line_spans
                    .insert(m.name.clone(), self.current_line_spans.clone());
            }
        }
    }

    pub(crate) fn lower_impl_function(&mut self, f: &FunctionDecl, target_type: &str) -> Function {
        self.val_counter = 0;
        self.current_fn_name = format!("{}_{}", target_type, f.name);
        self.local_var_types.clear();
        self.symbol_values.clear();
        self.current_blocks.clear();
        self.block_counter = 0;
        self.current_line_spans.clear();

        let mut entry_id = self.create_block("entry");

        let mut params = Vec::new();
        let mut param_refinements = Vec::new();

        let has_self = f
            .params
            .first()
            .map(|p| {
                p.name == "self" || p.name == "&self" || p.name == "mut self" || p.name == "this"
            })
            .unwrap_or(false);

        if !has_self {
            let this_val = self.next_val();
            params.push(("this".to_string(), target_type.to_string(), this_val));
            param_refinements.push(("this".to_string(), None));
            self.symbol_values.insert("this".to_string(), this_val);
            self.symbol_values.insert("self".to_string(), this_val);
        }

        for p in &f.params {
            let p_val = self.next_val();
            let is_self_param =
                p.name == "self" || p.name == "&self" || p.name == "mut self" || p.name == "this";
            let ty_str = if is_self_param {
                target_type.to_string()
            } else {
                p.type_node
                    .as_ref()
                    .map(|t| t.full_type_name())
                    .unwrap_or_else(|| "Int".into())
            };
            if ty_str.contains("Float") {
                self.class_field_types
                    .insert(p.name.clone(), "Float".into());
            }
            params.push((p.name.clone(), ty_str, p_val));
            let refn = p.type_node.as_ref().and_then(|t| {
                if let Some(r) = &t.refinement {
                    Some(r.clone())
                } else if let Some(td) = self.resolver.type_aliases.get(&t.name) {
                    td.base_type.refinement.clone()
                } else {
                    None
                }
            });
            param_refinements.push((p.name.clone(), refn));
            self.symbol_values.insert(p.name.clone(), p_val);
            if is_self_param {
                self.symbol_values.insert("this".to_string(), p_val);
                self.symbol_values.insert("self".to_string(), p_val);
            }
        }

        for req in &f.requires {
            if is_contract_statically_true(&req.condition, &param_refinements) {
                continue;
            }
            if let Some(cond_val) = self.lower_expr(&req.condition, &mut entry_id) {
                let msg_val = self.next_val();
                let user_msg = req.message.as_deref().unwrap_or("Precondition failed");
                let prefix = if user_msg.starts_with("CONTRACT_VIOLATION") {
                    ""
                } else {
                    "CONTRACT_VIOLATION: "
                };
                let msg_str = format!("{}{}", prefix, user_msg);
                self.get_block_mut(entry_id)
                    .instructions
                    .push(Inst::ConstStr {
                        dest: msg_val,
                        value: msg_str,
                    });
                let dest = self.next_val();
                self.get_block_mut(entry_id).instructions.push(Inst::Call {
                    dest,
                    func: "datara_rt_assert".into(),
                    args: vec![cond_val, msg_val],
                    ty: "Unit".into(),
                });
            }
        }

        let (mut cur_block, ret_val) = self.lower_stmt_cfg(&f.body, entry_id);

        if !f.ensures.is_empty() {
            if let Some(rv) = ret_val {
                self.symbol_values.insert("result".into(), rv);
            }
            for ens in &f.ensures {
                if is_contract_statically_true(&ens.condition, &[]) {
                    continue;
                }
                if let Some(cond_val) = self.lower_expr(&ens.condition, &mut cur_block) {
                    let msg_val = self.next_val();
                    let user_msg = ens.message.as_deref().unwrap_or("Postcondition failed");
                    let prefix = if user_msg.starts_with("CONTRACT_VIOLATION") {
                        ""
                    } else {
                        "CONTRACT_VIOLATION: "
                    };
                    let msg_str = format!("{}{}", prefix, user_msg);
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: msg_val,
                            value: msg_str,
                        });
                    let dest = self.next_val();
                    self.get_block_mut(cur_block).instructions.push(Inst::Call {
                        dest,
                        func: "datara_rt_assert".into(),
                        args: vec![cond_val, msg_val],
                        ty: "Unit".into(),
                    });
                }
            }
            self.symbol_values.remove("result");
        }
        self.symbol_values.clear();

        let cur = self.get_block_mut(cur_block);
        if matches!(
            cur.terminator,
            Terminator::Unreachable | Terminator::Return { value: None }
        ) {
            cur.terminator = Terminator::Return { value: ret_val };
        }

        let return_type = f
            .return_type
            .as_ref()
            .map(Self::repr_type_string)
            .unwrap_or_else(|| "Unit".into());

        Function {
            name: format!("{}_{}", target_type, f.name),
            params,
            param_refinements,
            requires: f.requires.clone(),
            return_type,
            entry_block: entry_id,
            blocks: std::mem::take(&mut self.current_blocks),
            inline_hint: InlineHint::parse_from_attrs(&f.attributes).0,
            alloc_hint: ArenaHint::parse_from_attrs(&f.attributes).0,
            has_inline_asm: stmt_contains_structured_asm(&f.body),
            proven_no_overflow: std::collections::BTreeSet::new(),
        }
    }

    pub(crate) fn specialize_function_decl(
        &self,
        f: &FunctionDecl,
        mangled_name: &str,
        type_substs: &HashMap<String, String>,
    ) -> FunctionDecl {
        let mut cloned = f.clone();
        cloned.name = mangled_name.to_string();
        cloned.generic_params.clear();
        cloned.generic_constraints.clear();

        for p in &mut cloned.params {
            if let Some(tn) = &mut p.type_node {
                Self::subst_type_node(tn, type_substs);
            }
        }

        if let Some(rt) = &mut cloned.return_type {
            Self::subst_type_node(rt, type_substs);
        }

        Self::subst_stmt(&mut cloned.body, type_substs);
        cloned
    }

    pub(crate) fn subst_type_node(tn: &mut TypeNode, type_substs: &HashMap<String, String>) {
        if let Some(concrete) = type_substs.get(&tn.name) {
            tn.name = concrete.clone();
        }
        for arg in &mut tn.generic_args {
            Self::subst_type_node(arg, type_substs);
        }
    }

    pub(crate) fn subst_stmt(stmt: &mut Stmt, type_substs: &HashMap<String, String>) {
        match stmt {
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    Self::subst_stmt(s, type_substs);
                }
            }
            Stmt::Let {
                type_node, init, ..
            }
            | Stmt::Mut {
                type_node, init, ..
            }
            | Stmt::Const {
                type_node, init, ..
            }
            | Stmt::Val {
                type_node, init, ..
            } => {
                if let Some(tn) = type_node {
                    Self::subst_type_node(tn, type_substs);
                }
                Self::subst_expr(init, type_substs);
            }
            Stmt::CompactBind { init, .. } => {
                Self::subst_expr(init, type_substs);
            }
            Stmt::Assign { value, .. } => {
                Self::subst_expr(value, type_substs);
            }
            Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => {
                Self::subst_expr(e, type_substs);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                Self::subst_expr(condition, type_substs);
                Self::subst_stmt(then_branch, type_substs);
                if let Some(eb) = else_branch {
                    Self::subst_stmt(eb, type_substs);
                }
            }
            Stmt::For { iterable, body, .. } | Stmt::ParallelFor { iterable, body, .. } => {
                Self::subst_expr(iterable, type_substs);
                Self::subst_stmt(body, type_substs);
            }
            Stmt::While {
                condition, body, ..
            } => {
                Self::subst_expr(condition, type_substs);
                Self::subst_stmt(body, type_substs);
            }
            Stmt::Loop { body, .. } => {
                Self::subst_stmt(body, type_substs);
            }
            Stmt::Parallel(s, _) | Stmt::Unsafe { body: s, .. } => {
                Self::subst_stmt(s, type_substs);
            }
            Stmt::With { init, body, .. } => {
                Self::subst_expr(init, type_substs);
                Self::subst_stmt(body, type_substs);
            }
            Stmt::Return(Some(e), _) => {
                Self::subst_expr(e, type_substs);
            }
            _ => {}
        }
    }

    pub(crate) fn subst_expr(expr: &mut Expr, type_substs: &HashMap<String, String>) {
        match expr {
            Expr::Binary { left, right, .. } => {
                Self::subst_expr(left, type_substs);
                Self::subst_expr(right, type_substs);
            }
            Expr::Unary { expr, .. } => {
                Self::subst_expr(expr, type_substs);
            }
            Expr::Call { callee, args, .. } => {
                Self::subst_expr(callee, type_substs);
                for a in args {
                    Self::subst_expr(a, type_substs);
                }
            }
            Expr::MemberAccess { object, .. } => {
                Self::subst_expr(object, type_substs);
            }
            Expr::IndexAccess { object, index, .. } => {
                Self::subst_expr(object, type_substs);
                Self::subst_expr(index, type_substs);
            }
            Expr::InterpolatedString { expressions, .. } => {
                for e in expressions {
                    Self::subst_expr(e, type_substs);
                }
            }
            Expr::ObjectInit {
                generic_args,
                fields,
                ..
            } => {
                for ga in generic_args {
                    Self::subst_type_node(ga, type_substs);
                }
                for (_, f_expr) in fields {
                    Self::subst_expr(f_expr, type_substs);
                }
            }
            Expr::ListLiteral(elements, _) => {
                for e in elements {
                    Self::subst_expr(e, type_substs);
                }
            }
            Expr::MapLiteral(entries, _) => {
                for (k, v) in entries {
                    Self::subst_expr(k, type_substs);
                    Self::subst_expr(v, type_substs);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn lower_method(&mut self, m: &MethodDecl, class_name: &str) -> Function {
        self.val_counter = 0;
        self.current_fn_name = format!("{}_{}", class_name, m.name);
        self.local_var_types.clear();
        self.symbol_values.clear();
        self.current_blocks.clear();
        self.block_counter = 0;
        self.current_line_spans.clear();

        let mut entry_id = self.create_block("entry");

        let this_val = self.next_val();
        let mut params = vec![("this".to_string(), class_name.to_string(), this_val)];
        let mut param_refinements = vec![("this".to_string(), None)];
        self.symbol_values.insert("this".to_string(), this_val);
        self.symbol_values.insert("self".to_string(), this_val);

        for (i, p) in m.params.iter().enumerate() {
            if i == 0 && (p.name == "self" || p.name == "this") {
                continue;
            }
            let p_val = self.next_val();
            let ty_str = p
                .type_node
                .as_ref()
                .map(|t| t.full_type_name())
                .unwrap_or_else(|| "Int".into());
            if ty_str.contains("Float") {
                self.class_field_types
                    .insert(p.name.clone(), "Float".into());
            }
            params.push((p.name.clone(), ty_str, p_val));
            let refn = p.type_node.as_ref().and_then(|t| {
                if let Some(r) = &t.refinement {
                    Some(r.clone())
                } else if let Some(td) = self.resolver.type_aliases.get(&t.name) {
                    td.base_type.refinement.clone()
                } else {
                    None
                }
            });
            param_refinements.push((p.name.clone(), refn));
            self.symbol_values.insert(p.name.clone(), p_val);
        }

        for req in &m.requires {
            if is_contract_statically_true(&req.condition, &param_refinements) {
                continue;
            }
            if let Some(cond_val) = self.lower_expr(&req.condition, &mut entry_id) {
                let msg_val = self.next_val();
                let user_msg = req.message.as_deref().unwrap_or("Precondition failed");
                let prefix = if user_msg.starts_with("CONTRACT_VIOLATION") {
                    ""
                } else {
                    "CONTRACT_VIOLATION: "
                };
                let msg_str = format!("{}{}", prefix, user_msg);
                self.get_block_mut(entry_id)
                    .instructions
                    .push(Inst::ConstStr {
                        dest: msg_val,
                        value: msg_str,
                    });
                let dest = self.next_val();
                self.get_block_mut(entry_id).instructions.push(Inst::Call {
                    dest,
                    func: "datara_rt_assert".into(),
                    args: vec![cond_val, msg_val],
                    ty: "Unit".into(),
                });
            }
        }

        let (mut cur_block, ret_val) = if let Some(body) = &m.body {
            self.lower_stmt_cfg(body, entry_id)
        } else {
            (entry_id, None)
        };

        if !m.ensures.is_empty() {
            if let Some(rv) = ret_val {
                self.symbol_values.insert("result".into(), rv);
            }
            for ens in &m.ensures {
                if is_contract_statically_true(&ens.condition, &[]) {
                    continue;
                }
                if let Some(cond_val) = self.lower_expr(&ens.condition, &mut cur_block) {
                    let msg_val = self.next_val();
                    let user_msg = ens.message.as_deref().unwrap_or("Postcondition failed");
                    let prefix = if user_msg.starts_with("CONTRACT_VIOLATION") {
                        ""
                    } else {
                        "CONTRACT_VIOLATION: "
                    };
                    let msg_str = format!("{}{}", prefix, user_msg);
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: msg_val,
                            value: msg_str,
                        });
                    let dest = self.next_val();
                    self.get_block_mut(cur_block).instructions.push(Inst::Call {
                        dest,
                        func: "datara_rt_assert".into(),
                        args: vec![cond_val, msg_val],
                        ty: "Unit".into(),
                    });
                }
            }
            self.symbol_values.remove("result");
        }
        self.symbol_values.clear();

        let cur = self.get_block_mut(cur_block);
        if matches!(
            cur.terminator,
            Terminator::Unreachable | Terminator::Return { value: None }
        ) {
            cur.terminator = Terminator::Return { value: ret_val };
        }
        if !cur
            .instructions
            .iter()
            .any(|i| matches!(i, Inst::Return { .. }))
            && ret_val.is_some()
        {
            cur.instructions.push(Inst::Return { value: ret_val });
        }

        Function {
            name: format!("{}_{}", class_name, m.name),
            params,
            param_refinements,
            requires: m.requires.clone(),
            return_type: m
                .return_type
                .as_ref()
                .map(Self::repr_type_string)
                .unwrap_or_else(|| "Unit".into()),
            entry_block: entry_id,
            blocks: std::mem::take(&mut self.current_blocks),
            inline_hint: InlineHint::parse_from_attrs(&m.attributes).0,
            alloc_hint: ArenaHint::parse_from_attrs(&m.attributes).0,
            has_inline_asm: m
                .body
                .as_ref()
                .map(|b| stmt_contains_structured_asm(b))
                .unwrap_or(false),
            proven_no_overflow: std::collections::BTreeSet::new(),
        }
    }
}
