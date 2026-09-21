use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use crate::types::{DataraType, MutabilityKind, PropagationKind, PropagationSite, TypeChecker};
use std::collections::{HashMap, HashSet};

impl<'a> TypeChecker<'a> {
    pub fn find_trait_method(
        &self,
        tr_name: &str,
        method_name: &str,
        visited: &mut HashSet<String>,
    ) -> Option<TraitMethodSignature> {
        if !visited.insert(tr_name.to_string()) {
            return None;
        }
        if let Some(tr) = self.traits.get(tr_name) {
            if let Some(tm) = tr.methods.iter().find(|m| m.name == method_name) {
                return Some(tm.clone());
            }
            for super_tr in &tr.super_traits {
                if let Some(tm) = self.find_trait_method(super_tr, method_name, visited) {
                    return Some(tm);
                }
            }
        }
        if let Some(r) = self.roles.get(tr_name) {
            if let Some(rm) = r.methods.iter().find(|m| m.name == method_name) {
                return Some(TraitMethodSignature {
                    name: rm.name.clone(),
                    generic_params: rm.generic_params.clone(),
                    params: rm.params.clone(),
                    return_type: rm.return_type.clone(),
                    default_body: rm.body.clone(),
                    span: rm.span.clone(),
                });
            }
        }
        None
    }

    pub fn check_expr(&mut self, expr: &Expr, diag: &mut DiagnosticEngine) -> DataraType {
        // Guard against stack overflow from pathologically-nested input.
        // Increment on entry, decrement on exit via a guard value.
        self.expr_depth += 1;
        if self.expr_depth > crate::types::MAX_EXPR_DEPTH {
            self.expr_depth -= 1;
            let span = match expr {
                Expr::Binary { span, .. }
                | Expr::Unary { span, .. }
                | Expr::Call { span, .. }
                | Expr::MemberAccess { span, .. }
                | Expr::IndexAccess { span, .. }
                | Expr::Literal(_, span)
                | Expr::Identifier(_, span) => Some(span.clone()),
                _ => None,
            };
            diag.error(
                ErrorCode::RecursionLimitExceeded,
                format!(
                    "expression nesting depth exceeds limit of {} -- \
                     simplify deeply-nested expressions",
                    crate::types::MAX_EXPR_DEPTH
                ),
                span,
            );
            return DataraType::Unit;
        }
        let result = self.check_expr_inner(expr, diag);
        self.expr_depth -= 1;
        result
    }

    fn check_expr_inner(&mut self, expr: &Expr, diag: &mut DiagnosticEngine) -> DataraType {
        match expr {
            Expr::Literal(lit, _) => match lit {
                LiteralValue::Int(_) => DataraType::Int,
                LiteralValue::Float(_) => DataraType::Float,
                LiteralValue::Dec64(_) => DataraType::Dec64,
                LiteralValue::String(_) => DataraType::String,
                LiteralValue::Bool(_) => DataraType::Bool,
                LiteralValue::Char(_) => DataraType::Char,
                LiteralValue::None => DataraType::Option(Box::new(DataraType::Unit)),
            },
            Expr::Identifier(name, _) => {
                let lookup_name = if name == "Self" {
                    self.current_target_type.as_deref().unwrap_or(name)
                } else {
                    name.as_str()
                };
                if let Some(t) = self.symbol_types.get(lookup_name) {
                    return t.clone();
                }
                if let Some(DataraType::Class(this_cls)) = self.symbol_types.get("this") {
                    if let Some(cls_sym) = self.resolver.classes.get(this_cls) {
                        if let Some(field_sym) = cls_sym.fields.get(lookup_name) {
                            if let Some(tn) = &field_sym.type_node {
                                return self.resolve_type_node(tn, diag);
                            }
                            return DataraType::Int;
                        }
                    }
                }
                if self.resolver.classes.contains_key(lookup_name) {
                    return DataraType::Class(lookup_name.to_string());
                }
                if self.resolver.functions.contains_key(lookup_name)
                    || self.resolver.extern_functions.contains_key(lookup_name)
                {
                    return DataraType::RawPtr;
                }
                // Genuinely unknown identifier. The resolver has already
                // reported an undefined-symbol error for it, so do not spam a
                // second diagnostic here; `Unit` fails loudly downstream
                // (e.g. in arithmetic/conditions) instead of silently
                // pretending to be an `Int`.
                DataraType::Unit
            }
            Expr::InterpolatedString { expressions, .. } => {
                for e in expressions {
                    let ty = self.check_expr(e, diag);
                    if !self.is_printable_type(&ty) {
                        let help = match &ty {
                            DataraType::Class(c) => {
                                format!("add '@derive(Display)' to '{}' or use 'to_str(...)'", c)
                            }
                            DataraType::List(_) => {
                                "format list elements via a loop, e.g. 'for x in xs { out x }', or serialize with a custom formatter".to_string()
                            }
                            DataraType::Map { .. } => {
                                "format entries via a loop, or convert keys/values to Str".to_string()
                            }
                            _ => "provide an explicit conversion to Str".to_string(),
                        };
                        diag.error_with_help(
                            ErrorCode::UnprintableType,
                            format!(
                                "Cannot interpolate value of unprintable type '{}' into string",
                                ty
                            ),
                            Some(e.span().clone()),
                            Some(help),
                        );
                    }
                }
                DataraType::String
            }
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => self.check_binary(op, left, right, span, diag),
            Expr::Unary { op, expr, .. } => {
                let inner = self.check_expr(expr, diag);
                if op == "!" {
                    DataraType::Bool
                } else if op == "await" {
                    match &inner {
                        DataraType::GenericInstance { name, args }
                            if (name == "Future" || name == "Task") =>
                        {
                            args.first().cloned().unwrap_or(DataraType::String)
                        }
                        DataraType::Class(name) if (name == "Future" || name == "Task") => {
                            DataraType::String
                        }
                        DataraType::Result(ok, _) => (**ok).clone(),
                        DataraType::Option(val) => (**val).clone(),
                        _ => inner,
                    }
                } else {
                    inner
                }
            }
            Expr::Call { callee, args, span } => self.check_call(callee, args, span, diag),
            Expr::MemberAccess {
                object,
                member,
                span,
            } => {
                let obj_type = self.check_expr(object, diag);
                match &obj_type {
                    DataraType::Class(cls_name) => {
                        if self.resolver.mmio_classes.contains_key(cls_name) {
                            if !self.in_unsafe && !self.allowed_devices.contains(cls_name) {
                                diag.error(
                                    ErrorCode::MmioRequiresUnsafe,
                                    format!(
                                        "MMIO access to '{}' requires an 'unsafe' block or declaration in datara.toml [devices]",
                                        cls_name
                                    ),
                                    Some(span.clone()),
                                );
                            }
                            if let Some(mmio) = self.resolver.mmio_classes.get(cls_name) {
                                if let Some((_, ty_name)) = mmio.fields.get(member) {
                                    return match ty_name.as_str() {
                                        "Float" | "Float64" | "Float32" | "f64" | "f32" => {
                                            DataraType::Float
                                        }
                                        "Bool" => DataraType::Bool,
                                        _ => DataraType::Int,
                                    };
                                }
                            }
                        }
                        let full_name = format!("{}.{}", cls_name, member);
                        if let Some(t) = self.symbol_types.get(&full_name) {
                            return t.clone();
                        }
                        if cls_name == "SystemCapabilities" {
                            if member == "files" {
                                return DataraType::Class("FileCapabilityProvider".into());
                            }
                            if member == "net" {
                                return DataraType::Class("NetCapabilityProvider".into());
                            }
                            if member == "proc" {
                                return DataraType::Class("ProcessCapabilityProvider".into());
                            }
                        }
                        if member == "view" || member == "clone" || member == "mut_view" {
                            return DataraType::Class(cls_name.clone());
                        }
                        if let Some(pkt) = self.resolver.packets.get(cls_name)
                            && pkt.fields.iter().any(|f| &f.name == member)
                        {
                            return DataraType::Int;
                        }
                        if let Some(fields) = self.class_fields.get(cls_name)
                            && let Some(f_type) = fields.get(member)
                        {
                            return f_type.clone();
                        }
                        if let Some((_, t_fields)) = self.generic_templates.get(cls_name)
                            && let Some(f_type) = t_fields.get(member)
                        {
                            return f_type.clone();
                        }
                        if let Some(methods) = self.class_methods.get(cls_name)
                            && methods.contains_key(member)
                        {
                            return DataraType::Unit;
                        }
                        let mut known_fields: Vec<&str> = Vec::new();
                        if let Some(fields) = self.class_fields.get(cls_name) {
                            known_fields.extend(fields.keys().map(|s| s.as_str()));
                        }
                        if let Some((_, t_fields)) = self.generic_templates.get(cls_name) {
                            known_fields.extend(t_fields.keys().map(|s| s.as_str()));
                        }
                        if let Some(methods) = self.class_methods.get(cls_name) {
                            known_fields.extend(methods.keys().map(|s| s.as_str()));
                        }
                        known_fields.sort_unstable();
                        let help_msg = if let Some(similar) =
                            crate::diagnostics::suggestions::find_best_match(member, known_fields)
                        {
                            format!(
                                "class '{}' has a field or method with a similar name: '{}'",
                                cls_name, similar
                            )
                        } else {
                            format!(
                                "class '{}' does not define field or method '{}'",
                                cls_name, member
                            )
                        };
                        diag.error_with_help(
                            ErrorCode::TypeInvalidMemberAccess,
                            format!(
                                "Class '{}' has no field or method named '{}'",
                                cls_name, member
                            ),
                            Some(span.clone()),
                            Some(help_msg),
                        );
                    }
                    DataraType::GenericInstance { name, args } => {
                        if member == "view" || member == "clone" || member == "mut_view" {
                            return DataraType::GenericInstance {
                                name: name.clone(),
                                args: args.clone(),
                            };
                        }
                        let full_name = format!("{}.{}", name, member);
                        if let Some(t) = self.symbol_types.get(&full_name) {
                            if let DataraType::TypeParam(p) = t {
                                if let Some((params, _)) = self.generic_templates.get(name) {
                                    if let Some(pos) = params.iter().position(|param| param == p) {
                                        if let Some(concrete) = args.get(pos) {
                                            return concrete.clone();
                                        }
                                    }
                                }
                                if let Some(first) = args.first() {
                                    return first.clone();
                                }
                            }
                            return t.clone();
                        }
                        if let Some((params, t_fields)) = self.generic_templates.get(name)
                            && let Some(field_type) = t_fields.get(member)
                        {
                            if let DataraType::TypeParam(p) = field_type
                                && let Some(idx) = params.iter().position(|param| param == p)
                                && idx < args.len()
                            {
                                return args[idx].clone();
                            }
                            return field_type.clone();
                        }
                        if let Some(fields) = self.class_fields.get(name)
                            && let Some(f_type) = fields.get(member)
                        {
                            return f_type.clone();
                        }
                    }
                    DataraType::Result(ok, err) => match member.as_str() {
                        "is_success" | "is_ok" | "is_err" => return DataraType::Bool,
                        "value" | "ok" => return (**ok).clone(),
                        "error_msg" | "error" | "err" => return (**err).clone(),
                        _ => {}
                    },
                    DataraType::Option(val) => match member.as_str() {
                        "is_some" | "is_none" => return DataraType::Bool,
                        "value" | "val" => return (**val).clone(),
                        _ => {}
                    },
                    DataraType::TypeParam(p) => {
                        let bounds = self
                            .current_fn_name
                            .as_ref()
                            .and_then(|fn_name| {
                                self.trait_bounds.get(&format!("{}:{}", fn_name, p))
                            })
                            .or_else(|| self.trait_bounds.get(p));
                        if let Some(trait_names) = bounds {
                            for tr_name in trait_names {
                                let mut visited = HashSet::new();
                                if let Some(tm) =
                                    self.find_trait_method(tr_name, member, &mut visited)
                                {
                                    let ret = tm
                                        .return_type
                                        .as_ref()
                                        .map(|t| self.resolve_type_node(t, diag))
                                        .unwrap_or(DataraType::Unit);
                                    return ret;
                                }
                            }
                        }
                        let bound_str = bounds
                            .map(|b| b.join(" + "))
                            .unwrap_or_else(|| "none".to_string());
                        diag.error(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Member '{}' not found for type parameter '{}' with bounds '{}'",
                                member, p, bound_str
                            ),
                            Some(span.clone()),
                        );
                        return DataraType::Unit;
                    }
                    _ => {}
                }
                // MemberAccess on a non-class value has no meaningful static
                // type; `Unit` fails loudly downstream instead of silently
                // pretending to be a `Str`.
                DataraType::Unit
            }
            Expr::ObjectInit {
                class_name,
                generic_args,
                fields,
                ..
            } => {
                let effective_class_name = if class_name == "Self" {
                    self.current_target_type
                        .clone()
                        .unwrap_or_else(|| class_name.clone())
                } else {
                    class_name.clone()
                };

                let mut inferred_args = Vec::new();
                for g in generic_args {
                    inferred_args.push(self.resolve_type_node(g, diag));
                }

                let mut field_types = HashMap::new();
                for (fname, val) in fields {
                    let ft = self.check_expr(val, diag);
                    field_types.insert(fname.clone(), ft);
                }

                if let Some(cls_sym) = self.resolver.classes.get(&effective_class_name) {
                    for (fname, val) in fields {
                        if let Some(fld_sym) = cls_sym.fields.get(fname) {
                            if let Some(expected_tn) = &fld_sym.type_node {
                                let expected_ty = self.resolve_type_node(expected_tn, diag);
                                if let Some(actual_ty) = field_types.get(fname) {
                                    // v1.4.5 W1: an int literal adopts the field's
                                    // narrow width when its value fits (same
                                    // compile-time labeling as declarations).
                                    let lit_fits =
                                        if let Expr::Literal(LiteralValue::Int(n), _) = val {
                                            DataraType::narrow_literal_fits(&expected_ty, *n)
                                        } else {
                                            false
                                        };
                                    if !lit_fits
                                        && !matches!(expected_ty, DataraType::TypeParam(_))
                                        && !actual_ty.is_compatible_with_args(
                                            &expected_ty,
                                            Some(self.resolver),
                                        )
                                    {
                                        diag.error(
                                            ErrorCode::TypeMismatch,
                                            format!(
                                                "Field '{}' of '{}' expects type '{}', got '{}'",
                                                fname, effective_class_name, expected_ty, actual_ty
                                            ),
                                            Some(val.span().clone()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some((params, t_fields)) = self.generic_templates.get(&effective_class_name)
                {
                    if inferred_args.is_empty() && !params.is_empty() {
                        // Infer type parameters deterministically: bind each
                        // template type parameter from the initializer's
                        // field values, walking the initializer's fields in
                        // declaration order (never arbitrary HashMap order).
                        let param_index: HashMap<&String, usize> =
                            params.iter().enumerate().map(|(i, p)| (p, i)).collect();
                        let mut bindings: Vec<Option<DataraType>> = vec![None; params.len()];
                        for (fname, _) in fields {
                            if let (Some(DataraType::TypeParam(p)), Some(v_ty)) =
                                (t_fields.get(fname), field_types.get(fname))
                                && let Some(&i) = param_index.get(p)
                                && bindings[i].is_none()
                            {
                                bindings[i] = Some(v_ty.clone());
                            }
                        }

                        if bindings.iter().all(Option::is_some) {
                            for b in bindings.into_iter().flatten() {
                                inferred_args.push(b);
                            }
                        } else if params.len() == 1 {
                            // Fallback for a single parameter with no
                            // directly-matching template field: use the first
                            // initializer value in declaration order.
                            if let Some((fname, _)) = fields.first()
                                && let Some(v_ty) = field_types.get(fname)
                            {
                                inferred_args.push(v_ty.clone());
                            }
                        }
                    }

                    if !inferred_args.is_empty() {
                        self.generic_specializations
                            .entry(effective_class_name.to_string())
                            .or_default()
                            .insert(inferred_args.clone());

                        return DataraType::GenericInstance {
                            name: effective_class_name.to_string(),
                            args: inferred_args,
                        };
                    }
                }

                DataraType::Class(effective_class_name.to_string())
            }
            Expr::Pipeline { stages, .. } => {
                if stages.is_empty() {
                    return DataraType::Unit;
                }
                let mut current = self.check_expr(&stages[0], diag);
                for stage in &stages[1..] {
                    match stage {
                        Expr::Identifier(fn_name, _) => {
                            if let Some((_, ret_ty, _)) = self.function_signatures.get(fn_name) {
                                current = ret_ty.clone();
                            } else if let Some(func_sym) = self.resolver.functions.get(fn_name) {
                                current = func_sym
                                    .return_type
                                    .as_ref()
                                    .map(|tn| self.resolve_type_node(tn, diag))
                                    .unwrap_or(DataraType::Unit);
                            } else {
                                current = self.check_expr(stage, diag);
                            }
                        }
                        Expr::Call { callee, .. } => {
                            if let Expr::Identifier(fn_name, _) = &**callee {
                                if let Some((_, ret_ty, _)) = self.function_signatures.get(fn_name)
                                {
                                    current = ret_ty.clone();
                                } else if let Some(func_sym) = self.resolver.functions.get(fn_name)
                                {
                                    current = func_sym
                                        .return_type
                                        .as_ref()
                                        .map(|tn| self.resolve_type_node(tn, diag))
                                        .unwrap_or(DataraType::Unit);
                                } else {
                                    current = self.check_expr(stage, diag);
                                }
                            } else {
                                current = self.check_expr(stage, diag);
                            }
                        }
                        _ => {
                            current = self.check_expr(stage, diag);
                        }
                    }
                }
                current
            }
            Expr::Decide {
                arms,
                else_arm,
                span,
            } => {
                let mut unified: Option<DataraType> = None;
                for arm in arms {
                    let cond_ty = self.check_expr(&arm.condition, diag);
                    if !cond_ty.is_compatible_with_args(&DataraType::Bool, Some(self.resolver)) {
                        diag.error(
                            ErrorCode::TypeMismatch,
                            format!("Condition in decide arm must be Bool, got '{}'", cond_ty),
                            Some(arm.condition.span().clone()),
                        );
                    }
                    let body_ty = self.check_expr(&arm.body, diag);
                    if let Some(ref u) = unified {
                        if !body_ty.is_compatible_with_args(u, Some(self.resolver))
                            && !u.is_compatible_with_args(&body_ty, Some(self.resolver))
                        {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "Incompatible types in decide arms: '{}' and '{}'",
                                    u, body_ty
                                ),
                                Some(arm.body.span().clone()),
                            );
                        }
                    } else {
                        unified = Some(body_ty);
                    }
                }
                if let Some(eb) = else_arm {
                    let else_ty = self.check_expr(eb, diag);
                    if let Some(ref u) = unified {
                        if !else_ty.is_compatible_with_args(u, Some(self.resolver))
                            && !u.is_compatible_with_args(&else_ty, Some(self.resolver))
                        {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "Incompatible types in decide else branch: '{}' and '{}'",
                                    u, else_ty
                                ),
                                Some(eb.span().clone()),
                            );
                        }
                    } else {
                        unified = Some(else_ty);
                    }
                }
                let res_ty = unified.unwrap_or(DataraType::Unit);
                self.check_decide_exhaustiveness(arms, else_arm.as_deref(), &res_ty, span, diag);
                res_ty
            }
            Expr::Match { value, arms, span } => {
                let val_ty = self.check_expr(value, diag);
                self.check_match_exhaustiveness(&val_ty, arms, span, diag);
                let mut unified: Option<DataraType> = None;
                for arm in arms {
                    let mut bound = Vec::new();
                    match &arm.pattern {
                        Pattern::Identifier(name, _) if name != "_" => {
                            let prev = self.symbol_types.insert(name.clone(), val_ty.clone());
                            let prev_mut = self
                                .symbol_mutability
                                .insert(name.clone(), MutabilityKind::Immutable);
                            bound.push((name.clone(), prev, prev_mut));
                        }
                        Pattern::Variant {
                            variant_name,
                            bindings,
                            ..
                        } => {
                            for (i, b) in bindings.iter().enumerate() {
                                let bind_ty = match &val_ty {
                                    DataraType::Option(inner)
                                        if (variant_name == "Some") && i == 0 =>
                                    {
                                        (**inner).clone()
                                    }
                                    DataraType::Result(ok, _)
                                        if (variant_name == "Ok") && i == 0 =>
                                    {
                                        (**ok).clone()
                                    }
                                    DataraType::Result(_, err)
                                        if (variant_name == "Err") && i == 0 =>
                                    {
                                        (**err).clone()
                                    }
                                    DataraType::Class(cls) => {
                                        if let Some(e) = self.resolver.enums.get(cls) {
                                            if let Some(v) = e
                                                .variants
                                                .iter()
                                                .find(|var| &var.name == variant_name)
                                            {
                                                if let Some(f_tn) = v.fields.get(i) {
                                                    self.resolve_type_node(f_tn, diag)
                                                } else {
                                                    val_ty.clone()
                                                }
                                            } else {
                                                val_ty.clone()
                                            }
                                        } else {
                                            val_ty.clone()
                                        }
                                    }
                                    DataraType::GenericInstance { name, .. } => {
                                        if let Some(e) = self.resolver.enums.get(name) {
                                            if let Some(v) = e
                                                .variants
                                                .iter()
                                                .find(|var| &var.name == variant_name)
                                            {
                                                if let Some(f_tn) = v.fields.get(i) {
                                                    self.resolve_type_node(f_tn, diag)
                                                } else {
                                                    val_ty.clone()
                                                }
                                            } else {
                                                val_ty.clone()
                                            }
                                        } else {
                                            val_ty.clone()
                                        }
                                    }
                                    _ => val_ty.clone(),
                                };
                                let prev = self.symbol_types.insert(b.clone(), bind_ty);
                                let prev_mut = self
                                    .symbol_mutability
                                    .insert(b.clone(), MutabilityKind::Immutable);
                                bound.push((b.clone(), prev, prev_mut));
                            }
                        }
                        _ => {}
                    }
                    if let Some(g) = &arm.guard {
                        self.check_expr(g, diag);
                    }
                    let body_ty = self.check_expr(&arm.body, diag);
                    for (name, prev, prev_mut) in bound {
                        if let Some(p) = prev {
                            self.symbol_types.insert(name.clone(), p);
                        } else {
                            self.symbol_types.remove(&name);
                        }
                        if let Some(m) = prev_mut {
                            self.symbol_mutability.insert(name, m);
                        } else {
                            self.symbol_mutability.remove(&name);
                        }
                    }
                    if let Some(ref u) = unified {
                        if !body_ty.is_compatible_with_args(u, Some(self.resolver))
                            && !u.is_compatible_with_args(&body_ty, Some(self.resolver))
                        {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "Incompatible types in match arms: '{}' and '{}'",
                                    u, body_ty
                                ),
                                Some(arm.body.span().clone()),
                            );
                        }
                    } else {
                        unified = Some(body_ty);
                    }
                }
                unified.unwrap_or(DataraType::Unit)
            }
            Expr::Select { arms, else_arm, .. } => {
                let mut unified: Option<DataraType> = None;
                for arm in arms {
                    self.check_expr(&arm.condition, diag);
                    let body_ty = self.check_expr(&arm.body, diag);
                    if let Some(ref u) = unified {
                        if !body_ty.is_compatible_with_args(u, Some(self.resolver))
                            && !u.is_compatible_with_args(&body_ty, Some(self.resolver))
                        {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "Incompatible types in select arms: '{}' and '{}'",
                                    u, body_ty
                                ),
                                Some(arm.body.span().clone()),
                            );
                        }
                    } else {
                        unified = Some(body_ty);
                    }
                }
                if let Some(eb) = else_arm {
                    let else_ty = self.check_expr(eb, diag);
                    if let Some(ref u) = unified {
                        if !else_ty.is_compatible_with_args(u, Some(self.resolver))
                            && !u.is_compatible_with_args(&else_ty, Some(self.resolver))
                        {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "Incompatible types in select else branch: '{}' and '{}'",
                                    u, else_ty
                                ),
                                Some(eb.span().clone()),
                            );
                        }
                    } else {
                        unified = Some(else_ty);
                    }
                }
                unified.unwrap_or(DataraType::Unit)
            }
            Expr::Lambda { params, body, .. } => {
                let mut bound = Vec::new();
                let mut param_types = Vec::new();
                for p in params {
                    let p_ty = p
                        .type_node
                        .as_ref()
                        .map(|t| self.resolve_type_node(t, diag))
                        .unwrap_or(DataraType::Int);
                    param_types.push(p_ty.clone());
                    let prev = self.symbol_types.insert(p.name.clone(), p_ty);
                    let prev_mut = self
                        .symbol_mutability
                        .insert(p.name.clone(), MutabilityKind::Immutable);
                    bound.push((p.name.clone(), prev, prev_mut));
                }
                let ret_ty = self.check_expr(body, diag);
                for (name, prev, prev_mut) in bound {
                    if let Some(p) = prev {
                        self.symbol_types.insert(name.clone(), p);
                    } else {
                        self.symbol_types.remove(&name);
                    }
                    if let Some(m) = prev_mut {
                        self.symbol_mutability.insert(name, m);
                    } else {
                        self.symbol_mutability.remove(&name);
                    }
                }
                DataraType::Function {
                    params: param_types,
                    return_type: Box::new(ret_ty),
                }
            }
            Expr::ListLiteral(items, _) => {
                // Unify element types: all elements of one type -> List(T);
                // mixed or unknown elements -> List(Val) (the dynamic type),
                // never a silent Int default.
                let mut elem: Option<DataraType> = None;
                for item in items {
                    let t = self.check_expr(item, diag);
                    if t == DataraType::Unit {
                        continue;
                    }
                    match &elem {
                        None => elem = Some(t),
                        Some(prev) if *prev == t => {}
                        Some(_) => elem = Some(DataraType::Val),
                    }
                }
                let elem_ty = elem.unwrap_or(DataraType::Val);
                self.last_list_element = Some(elem_ty.clone());
                DataraType::List(Box::new(elem_ty))
            }
            Expr::MapLiteral(entries, _) => {
                // Unify key/value types the same way list elements are
                // unified: one type -> Map(K, V); mixed/empty -> Val.
                let mut key: Option<DataraType> = None;
                let mut value: Option<DataraType> = None;
                for (k, v) in entries {
                    let kt = self.check_expr(k, diag);
                    let vt = self.check_expr(v, diag);
                    match &key {
                        None => key = Some(kt),
                        Some(prev) if *prev == kt => {}
                        Some(_) => key = Some(DataraType::Val),
                    }
                    match &value {
                        None => value = Some(vt),
                        Some(prev) if *prev == vt => {}
                        Some(_) => value = Some(DataraType::Val),
                    }
                }
                DataraType::Map(
                    Box::new(key.unwrap_or(DataraType::Val)),
                    Box::new(value.unwrap_or(DataraType::Val)),
                )
            }
            Expr::IndexAccess { object, index, .. } => {
                let obj_ty = self.check_expr(object, diag);
                let idx_ty = self.check_expr(index, diag);

                // Static negative index check and Range bounds verification
                let static_idx: Option<i128> = match &**index {
                    Expr::Literal(LiteralValue::Int(n), _) => Some(*n as i128),
                    Expr::Unary { op, expr, .. } if op == "-" => {
                        if let Expr::Literal(LiteralValue::Int(n), _) = &**expr {
                            Some(-(*n as i128))
                        } else {
                            Some(-1)
                        }
                    }
                    _ => None,
                };

                if let Some(n) = static_idx {
                    if n < 0 {
                        diag.error_with_help(
                            ErrorCode::RangeViolation,
                            format!("Array index cannot be negative: {}", n),
                            Some(index.span().clone()),
                            Some("Array indices in Datara must be non-negative (>= 0)".to_string()),
                        );
                    }
                } else if let DataraType::Range { min, .. } = &idx_ty
                    && *min < 0
                {
                    diag.error_with_help(
                        ErrorCode::RangeViolation,
                        format!(
                            "Array index range allows negative values (minimum is {})",
                            min
                        ),
                        Some(index.span().clone()),
                        Some(
                            "Constrain index range to be non-negative: e.g. UInt or Int<0..>"
                                .to_string(),
                        ),
                    );
                }

                if let Expr::Identifier(name, _) = &**object
                    && let Some(arr_len) = self.var_array_lengths.get(name).copied()
                {
                    if let Some(n) = static_idx {
                        if n >= 0 && n >= arr_len as i128 {
                            diag.error_with_help(
                                ErrorCode::RangeViolation,
                                format!(
                                    "Index {} is out of bounds for array '{}' of length {}",
                                    n, name, arr_len
                                ),
                                Some(index.span().clone()),
                                Some(format!(
                                    "Valid indices are 0..{}",
                                    arr_len.saturating_sub(1)
                                )),
                            );
                        }
                    } else if let DataraType::Range { max, .. } = &idx_ty
                        && *max >= arr_len as i128
                    {
                        diag.error_with_help(
                            ErrorCode::RangeViolation,
                            format!(
                                "Index range maximum {} may exceed array '{}' bound of length {}",
                                max, name, arr_len
                            ),
                            Some(index.span().clone()),
                            Some(format!(
                                "Constrain index range to 0..{}",
                                arr_len.saturating_sub(1)
                            )),
                        );
                    }
                }

                if idx_ty == DataraType::Class("Range".into()) {
                    obj_ty
                } else {
                    match &obj_ty {
                        // Parametric collections carry their element types:
                        // `l[i]` -> T, `m[k]` -> V.
                        DataraType::List(elem) => (**elem).clone(),
                        DataraType::Map(_, val) => (**val).clone(),
                        DataraType::GenericInstance { name, args }
                            if (name == "List" || name == "Array") && !args.is_empty() =>
                        {
                            args[0].clone()
                        }
                        DataraType::GenericInstance { name, args }
                            if name == "Map" && args.len() == 2 =>
                        {
                            args[1].clone()
                        }
                        // Erased collections: fall back to the recorded
                        // element type, else the dynamic type `Val` — never
                        // a silent `Int` default.
                        DataraType::Class(c) if c == "List" || c == "Array" => {
                            if let Expr::Identifier(name, _) = &**object
                                && let Some(elem) = self.var_element_types.get(name)
                            {
                                elem.clone()
                            } else {
                                self.last_list_element.clone().unwrap_or(DataraType::Val)
                            }
                        }
                        DataraType::Class(c) if c == "Map" => DataraType::Val,
                        // Indexing a non-collection has no statically known
                        // result; `Val` fails loudly in typed contexts instead
                        // of silently pretending to be an `Int`.
                        _ => DataraType::Val,
                    }
                }
            }
            Expr::Range { start, end, .. } => {
                self.check_expr(start, diag);
                self.check_expr(end, diag);
                DataraType::Class("Range".into())
            }
            Expr::Tuple(exprs, _) => {
                let types = exprs.iter().map(|e| self.check_expr(e, diag)).collect();
                DataraType::Tuple(types)
            }
            Expr::ErrorPropagate(inner, span) => {
                let t = self.check_expr(inner, diag);
                // `?` is real error propagation, not a silent no-op. Two hard
                // rules keep it predictable:
                //   1. the operand must be Result-like (`T!E`, `Result<T,E>`,
                //      `Outcome<T>`) or Option-like (`T?`, `Option<T>`, `Maybe<T>`);
                //   2. the enclosing function/method must return the same kind
                //      with a compatible payload, otherwise there is nothing to
                //      propagate the error into.
                let (kind, payload) = if let Some((ok, _err)) = t.result_like() {
                    (PropagationKind::Outcome, ok)
                } else if let Some(inner_ty) = t.option_like() {
                    (PropagationKind::Maybe, inner_ty)
                } else {
                    diag.error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "'?' requires a Result ('T!E') or Option ('T?') operand, got '{}'",
                            t
                        ),
                        Some(span.clone()),
                    );
                    return t;
                };

                let expected_payload = match kind {
                    PropagationKind::Outcome => self
                        .current_return_type
                        .as_ref()
                        .and_then(|r| r.result_like())
                        .map(|(ok, _)| ok),
                    PropagationKind::Maybe => self
                        .current_return_type
                        .as_ref()
                        .and_then(|r| r.option_like()),
                };
                match expected_payload {
                    None => {
                        // v1.4.1: dedicated E-TYPE-010 for a `?` whose error
                        // has nowhere to propagate. The message text is kept
                        // byte-identical to the previous generic TypeMismatch
                        // emission so existing diagnostics-based tests keep
                        // matching.
                        diag.error(
                            ErrorCode::QuestionPropagation,
                            format!(
                                "'?' propagates a {} but the enclosing function returns '{}'; the function must return the same Result/Option type to propagate",
                                match kind {
                                    PropagationKind::Outcome => "Result error",
                                    PropagationKind::Maybe => "None",
                                },
                                self.current_return_type
                                    .as_ref()
                                    .map(|r| r.to_string())
                                    .unwrap_or_else(|| "Unit".into()),
                            ),
                            Some(span.clone()),
                        );
                    }
                    Some(expected) => {
                        // For Maybe (Option<T>), the inner payload type must match.
                        // For Outcome (Result<T, Str>), error propagation returns the failed object
                        // (is_success = 0, error_msg) unchanged without touching the payload slot,
                        // so an Outcome<Str> can propagate out of a function returning Outcome<Int>.
                        if kind == PropagationKind::Maybe
                            && !payload.is_compatible_with_args(&expected, Some(self.resolver))
                        {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "'?' unwraps '{}' but the enclosing function returns a Result/Option of '{}'",
                                    payload, expected
                                ),
                                Some(span.clone()),
                            );
                        }
                    }
                }

                self.propagation_sites.push(PropagationSite {
                    span: span.clone(),
                    kind,
                    payload_repr: payload.to_string(),
                });
                payload
            }
            Expr::OrRecovery { expr, arms, .. } => {
                let expr_ty = self.check_expr(expr, diag);
                let inner_ty = if let Some((ok, _err)) = expr_ty.result_like() {
                    ok
                } else if let Some(inner) = expr_ty.option_like() {
                    inner
                } else {
                    expr_ty
                };
                for arm in arms {
                    let _ = self.check_expr(&arm.body, diag);
                }
                inner_ty
            }
            Expr::ArrayRepeatLiteral { elem, .. } => {
                let elem_ty = self.check_expr(elem, diag);
                DataraType::GenericInstance {
                    name: "Array".to_string(),
                    args: vec![elem_ty],
                }
            }
            Expr::Comptime { expr, .. } => self.check_expr(expr, diag),
            Expr::Wrapping(expr, _) | Expr::Saturating(expr, _) => self.check_expr(expr, diag),
            Expr::Cast {
                expr: inner,
                target_ty,
                span,
            } => {
                let src_ty = self.check_expr(inner, diag);
                let dst_ty = self.resolve_type_node(
                    &TypeNode {
                        name: target_ty.clone(),
                        generic_args: Vec::new(),
                        is_option: false,
                        error_type: None,
                        refinement: None,
                        span: span.clone(),
                    },
                    diag,
                );
                // Gate 7: casts are the sanctioned explicit conversions. Numeric
                // families interconvert freely (narrowing is value-checked by
                // the runtime); Str <-> numeric and Bool <-> numeric are also
                // defined. Class/pointer casts are rejected as unsound.
                let numeric = |t: &DataraType| {
                    matches!(
                        t,
                        DataraType::Int
                            | DataraType::UInt
                            | DataraType::Int8
                            | DataraType::Int16
                            | DataraType::Int32
                            | DataraType::UInt8
                            | DataraType::UInt16
                            | DataraType::UInt32
                            | DataraType::UInt64
                            | DataraType::Float
                            | DataraType::Float32
                            | DataraType::Dec64
                    )
                };
                let src_ok = numeric(&src_ty)
                    || matches!(
                        src_ty,
                        DataraType::String | DataraType::Bool | DataraType::Char
                    );
                let dst_ok = numeric(&dst_ty)
                    || matches!(
                        dst_ty,
                        DataraType::String | DataraType::Bool | DataraType::Char
                    );
                if !src_ok || !dst_ok {
                    diag.error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "Cast from {:?} to {:?} is not supported; use explicit conversion methods for non-numeric types",
                            src_ty, target_ty
                        ),
                        Some(span.clone()),
                    );
                    return DataraType::Unit;
                }
                dst_ty
            }
            Expr::Block(stmts, value, _) => {
                // Lexical scope: declarations inside the block must not leak
                // into sibling arms or the enclosing scope (same pattern as
                // Stmt::Block in check_stmt).
                let saved_types = self.symbol_types.clone();
                let saved_mut = self.symbol_mutability.clone();
                let saved_elem = self.var_element_types.clone();
                let saved_refinements = self.var_refinements.clone();
                let saved_lengths = self.var_array_lengths.clone();
                for s in stmts {
                    self.check_stmt(s, diag);
                }
                let result = match value {
                    Some(v) => self.check_expr(v, diag),
                    None => DataraType::Unit,
                };
                self.symbol_types = saved_types;
                self.symbol_mutability = saved_mut;
                self.var_element_types = saved_elem;
                self.var_refinements = saved_refinements;
                self.var_array_lengths = saved_lengths;
                result
            }
        }
    }
}
