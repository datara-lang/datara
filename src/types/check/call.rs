use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::types::{DataraType, TypeChecker};
use std::collections::{HashMap, HashSet};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_call(
        &mut self,
        callee: &Box<Expr>,
        args: &[Expr],
        span: &SourceSpan,
        diag: &mut DiagnosticEngine,
    ) -> DataraType {
        let mut arg_types = Vec::new();
        for a in args {
            arg_types.push(self.check_expr(a, diag));
        }

        if let Expr::Identifier(fn_name, _) = &**callee {
            if fn_name == "view" || fn_name == "mut_view" || fn_name == "mutView" {
                return arg_types.first().cloned().unwrap_or(DataraType::Unit);
            }
            if fn_name == "destroy" || fn_name == "unsafe_op" {
                return DataraType::Unit;
            }
            if fn_name == "println" || fn_name == "eprintln" {
                let replacement = if fn_name == "eprintln" { "err" } else { "out" };
                diag.warning(
                    ErrorCode::DeprecatedPrintFunction,
                    format!(
                        "Function '{}' is deprecated: use '{}' statement instead (e.g. '{} \"text\"' or '{} fmt\"a {{x}}\"')",
                        fn_name, replacement, replacement, replacement
                    ),
                    Some(span.clone()),
                );
                return DataraType::Unit;
            }
            if fn_name == "print" {
                if let Some(arg_ty) = arg_types.first() {
                    if !self.is_printable_type(arg_ty) {
                        let help = match arg_ty {
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
                            format!("Cannot print value of unprintable type '{}'", arg_ty),
                            Some(span.clone()),
                            Some(help),
                        );
                    }
                }
                return DataraType::Unit;
            }

            if fn_name == "input_int" || fn_name == "read_int" || fn_name == "fast_read_int" {
                return DataraType::Int;
            }
            if fn_name == "input_float" || fn_name == "read_float" || fn_name == "fast_read_float" {
                return DataraType::Float;
            }
            if fn_name == "len" || fn_name == "now" {
                return DataraType::Int;
            }
            if fn_name == "map" || fn_name == "filter" {
                return arg_types
                    .first()
                    .cloned()
                    .unwrap_or(DataraType::Class("List".into()));
            }
            if fn_name == "reduce" {
                return arg_types.get(1).cloned().unwrap_or(DataraType::Int);
            }
            if fn_name == "find" {
                if let Some(DataraType::List(elem)) = arg_types.first() {
                    return (**elem).clone();
                }
                return DataraType::Int;
            }
            if fn_name == "any" || fn_name == "all" {
                return DataraType::Bool;
            }
            if fn_name == "panic" || fn_name == "exit" {
                return DataraType::Never;
            }
            if fn_name == "assert" || fn_name == "require" {
                if fn_name == "require" {
                    if let Some(cond) = args.first() {
                        self.active_requires.push(cond.clone());
                    }
                }
                return DataraType::Unit;
            }
            if fn_name == "input" || fn_name == "read_line" {
                return DataraType::String;
            }
            if fn_name == "str_to_float" {
                return DataraType::Float;
            }
            if fn_name == "float4"
                || fn_name == "datara_rt_float4"
                || fn_name == "f32x4"
                || fn_name == "datara_rt_f32x4"
                || fn_name == "min4"
                || fn_name == "max4"
                || fn_name.starts_with("f32x4_add")
                || fn_name.starts_with("f32x4_sub")
                || fn_name.starts_with("f32x4_mul")
                || fn_name.starts_with("f32x4_div")
                || fn_name.starts_with("f32x4_cross")
                || fn_name.starts_with("f32x4_min")
                || fn_name.starts_with("f32x4_max")
                || fn_name.starts_with("f32x4_lerp")
                || fn_name.starts_with("f32x4_normalize")
            {
                return DataraType::SimdF32x4;
            }
            if fn_name == "f32x8"
                || fn_name == "datara_rt_f32x8"
                || fn_name.starts_with("f32x8_add")
                || fn_name.starts_with("f32x8_sub")
                || fn_name.starts_with("f32x8_mul")
                || fn_name.starts_with("f32x8_div")
                || fn_name.starts_with("f32x8_min")
                || fn_name.starts_with("f32x8_max")
                || fn_name.starts_with("f32x8_lerp")
                || fn_name.starts_with("f32x8_normalize")
            {
                return DataraType::Class("f32x8".to_string());
            }
            if fn_name == "f32x16"
                || fn_name == "datara_rt_f32x16"
                || fn_name.starts_with("f32x16_add")
                || fn_name.starts_with("f32x16_sub")
                || fn_name.starts_with("f32x16_mul")
                || fn_name.starts_with("f32x16_div")
                || fn_name.starts_with("f32x16_min")
                || fn_name.starts_with("f32x16_max")
                || fn_name.starts_with("f32x16_lerp")
                || fn_name.starts_with("f32x16_normalize")
            {
                return DataraType::Class("f32x16".to_string());
            }
            if fn_name == "int4"
                || fn_name == "datara_rt_int4"
                || fn_name == "i32x4"
                || fn_name == "datara_rt_i32x4"
                || fn_name.starts_with("i32x4_add")
                || fn_name.starts_with("i32x4_sub")
                || fn_name.starts_with("i32x4_mul")
                || fn_name.starts_with("i32x4_div")
                || fn_name.starts_with("i32x4_min")
                || fn_name.starts_with("i32x4_max")
            {
                return DataraType::SimdI32x4;
            }
            if fn_name == "i32x8"
                || fn_name == "datara_rt_i32x8"
                || fn_name.starts_with("i32x8_add")
                || fn_name.starts_with("i32x8_sub")
                || fn_name.starts_with("i32x8_mul")
                || fn_name.starts_with("i32x8_div")
                || fn_name.starts_with("i32x8_min")
                || fn_name.starts_with("i32x8_max")
            {
                return DataraType::Class("i32x8".to_string());
            }
            if fn_name == "f64x2"
                || fn_name == "datara_rt_f64x2"
                || fn_name.starts_with("f64x2_add")
                || fn_name.starts_with("f64x2_sub")
                || fn_name.starts_with("f64x2_mul")
                || fn_name.starts_with("f64x2_div")
                || fn_name.starts_with("f64x2_min")
                || fn_name.starts_with("f64x2_max")
                || fn_name.starts_with("f64x2_lerp")
                || fn_name.starts_with("f64x2_normalize")
            {
                return DataraType::Class("f64x2".to_string());
            }
            if fn_name == "f64x4"
                || fn_name == "datara_rt_f64x4"
                || fn_name.starts_with("f64x4_add")
                || fn_name.starts_with("f64x4_sub")
                || fn_name.starts_with("f64x4_mul")
                || fn_name.starts_with("f64x4_div")
                || fn_name.starts_with("f64x4_min")
                || fn_name.starts_with("f64x4_max")
                || fn_name.starts_with("f64x4_lerp")
                || fn_name.starts_with("f64x4_normalize")
            {
                return DataraType::Class("f64x4".to_string());
            }
            if fn_name == "dot"
                || fn_name == "datara_rt_float4_dot"
                || fn_name.starts_with("float4_")
                || fn_name.starts_with("f32x4_dot")
                || fn_name.starts_with("f32x4_horizontal_add")
                || fn_name.starts_with("f32x4_distance")
                || fn_name.starts_with("f32x8_dot")
                || fn_name.starts_with("f32x8_horizontal_add")
                || fn_name.starts_with("f32x8_distance")
                || fn_name.starts_with("f32x16_dot")
                || fn_name.starts_with("f32x16_horizontal_add")
                || fn_name.starts_with("f32x16_distance")
                || fn_name.starts_with("f64x2_dot")
                || fn_name.starts_with("f64x2_horizontal_add")
                || fn_name.starts_with("f64x2_distance")
                || fn_name.starts_with("f64x4_dot")
                || fn_name.starts_with("f64x4_horizontal_add")
                || fn_name.starts_with("f64x4_distance")
                || fn_name == "dot_f32_array"
                || fn_name == "datara_rt_dot_f32_array"
                || fn_name.contains("ray_sphere_intersect")
                || fn_name.starts_with("lane")
                || fn_name == "fma"
                || fn_name == "fmaf"
                || fn_name == "datara_rt_fma"
                || fn_name == "datara_rt_fmaf"
            {
                return DataraType::Float;
            }
            if fn_name == "aabb_intersects" || fn_name == "datara_rt_aabb_intersects" {
                return DataraType::Int;
            }
            if fn_name.starts_with("int4_")
                || fn_name.starts_with("i32x4_dot")
                || fn_name.starts_with("i32x4_horizontal_add")
                || fn_name.starts_with("i32x8_dot")
                || fn_name.starts_with("i32x8_horizontal_add")
            {
                return DataraType::Int;
            }

            if self.resolver.comptime_functions.contains(fn_name) {
                for a in args {
                    if !is_comptime_constant(a, self.resolver) {
                        diag.error(
                            ErrorCode::ComptimeNonConstArg,
                            format!(
                                "Arguments to comptime function must be compile-time constants: '{}'",
                                fn_name
                            ),
                            Some(a.span().clone()),
                        );
                    }
                }
            }

            if let Some(param_nodes) = self.function_param_nodes.get(fn_name).cloned() {
                for (arg, p_node) in args.iter().zip(param_nodes.iter()) {
                    if let Some(tn) = p_node {
                        self.check_refinement(tn, arg, arg.span(), diag);
                    }
                }
            }

            if let Some((param_types, ret_type, gen_params)) =
                self.function_signatures.get(fn_name).cloned()
            {
                let mut type_bindings: HashMap<String, DataraType> = HashMap::new();

                for (p_ty, a_ty) in param_types.iter().zip(arg_types.iter()) {
                    bind_type_params(p_ty, a_ty, &mut type_bindings);
                }

                for (idx, (p_ty, a_ty)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                    let expected_ty = subst_type_params(p_ty, &type_bindings);
                    if !a_ty.is_compatible_with_refined_with_args(&expected_ty, Some(self.resolver))
                    {
                        if self.bridge_functions.contains(fn_name) {
                            diag.error(
                                ErrorCode::BridgeTypeMismatch,
                                format!(
                                    "Bridge argument type mismatch for argument {}: expected '{}', got '{}'",
                                    idx + 1,
                                    expected_ty,
                                    a_ty
                                ),
                                Some(span.clone()),
                            );
                        } else {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "Type mismatch for argument {}: expected '{}', got '{}'",
                                    idx + 1,
                                    expected_ty,
                                    a_ty
                                ),
                                Some(span.clone()),
                            );
                        }
                    }
                }

                for (param_name, concrete_ty) in &type_bindings {
                    let bounds = self
                        .trait_bounds
                        .get(&format!("{}:{}", fn_name, param_name))
                        .or_else(|| self.trait_bounds.get(param_name));
                    if let Some(trait_names) = bounds {
                        for tr_name in trait_names {
                            let type_str = match concrete_ty {
                                DataraType::Class(c) => c.clone(),
                                DataraType::Int => "Int".to_string(),
                                DataraType::Float => "Float".to_string(),
                                DataraType::String => "String".to_string(),
                                DataraType::Bool => "Bool".to_string(),
                                DataraType::Unit => "Unit".to_string(),
                                DataraType::GenericInstance { name, .. } => name.clone(),
                                other => other.to_string(),
                            };
                            let mut satisfied = self
                                .impls
                                .contains_key(&(tr_name.clone(), type_str.clone()));
                            if !satisfied {
                                if let DataraType::TypeParam(p) = concrete_ty {
                                    let caller_bounds = self
                                        .current_fn_name
                                        .as_ref()
                                        .and_then(|fn_n| {
                                            self.trait_bounds.get(&format!("{}:{}", fn_n, p))
                                        })
                                        .or_else(|| self.trait_bounds.get(p));
                                    if let Some(tb) = caller_bounds {
                                        if tb.contains(tr_name) {
                                            satisfied = true;
                                        }
                                    }
                                }
                            }
                            if !satisfied {
                                let mut queue = vec![type_str.clone()];
                                let mut visited = HashSet::new();
                                while let Some(cls_name) = queue.pop() {
                                    if !visited.insert(cls_name.clone()) {
                                        continue;
                                    }
                                    if self
                                        .impls
                                        .contains_key(&(tr_name.clone(), cls_name.clone()))
                                    {
                                        satisfied = true;
                                        break;
                                    }
                                    if let Some(cls_sym) = self.resolver.classes.get(&cls_name) {
                                        if cls_sym.compositions.iter().any(|comp| comp == tr_name) {
                                            satisfied = true;
                                            break;
                                        }
                                        for comp in &cls_sym.compositions {
                                            queue.push(comp.clone());
                                        }
                                        if let Some(base) = &cls_sym.base_type {
                                            queue.push(base.clone());
                                        }
                                    }
                                }
                            }
                            if !satisfied {
                                diag.error(
                                            ErrorCode::TypeMismatch,
                                            format!(
                                                "Type '{}' does not satisfy trait bound '{}' for parameter '{}' in function '{}'",
                                                concrete_ty, tr_name, param_name, fn_name
                                            ),
                                            Some(span.clone()),
                                        );
                            }
                        }
                    }
                }

                if !type_bindings.is_empty() {
                    // A generic parameter bound to a Function type means a
                    // lambda argument (lambda-argument static dispatch): the
                    // call site is inline-lowered by the compiler, so no
                    // monomorphized instance is recorded for it.
                    let binds_function = type_bindings
                        .values()
                        .any(|t| matches!(t, DataraType::Function { .. }));
                    if !binds_function {
                        let mut spec_args = Vec::new();
                        for gp in &gen_params {
                            if let Some(concrete) = type_bindings.get(gp) {
                                spec_args.push(concrete.clone());
                            }
                        }
                        if !spec_args.is_empty() {
                            self.generic_specializations
                                .entry(fn_name.clone())
                                .or_default()
                                .insert(spec_args);
                        }
                    }
                }

                return subst_type_params(&ret_type, &type_bindings);
            }
        }

        if let Expr::MemberAccess { object, member, .. } = &**callee {
            // Outcome.ok(v) / Outcome.err(msg) static constructors: the
            // receiver is the generic class name itself, never a value, so
            // this is a constructor call rather than a method dispatch.
            // Checked before the receiver is resolved as an expression so
            // the bare class name never produces an unknown-symbol
            // diagnostic. The result is the language Outcome type (the
            // checker-level Result(T, Str) / GenericInstance representation,
            // matching `?` propagation and the checked-I/O builtins).
            if let Expr::Identifier(cls_name, _) = &**object
                && cls_name == "Outcome"
                && (member == "ok" || member == "err")
            {
                if args.len() != 1 {
                    diag.error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "Outcome.{}() takes exactly 1 argument, got {}",
                            member,
                            args.len()
                        ),
                        Some(span.clone()),
                    );
                    return DataraType::Unit;
                }
                if member == "err" {
                    // The error channel is always Str; anything else would
                    // put a non-pointer into the error_msg slot.
                    let arg_ok = matches!(
                        arg_types.first(),
                        Some(DataraType::String) | Some(DataraType::TypeParam(_))
                    );
                    if !arg_ok {
                        diag.error(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Outcome.err() expects a 'Str' message, got '{}'",
                                arg_types.first().map(|t| t.to_string()).unwrap_or_default()
                            ),
                            Some(span.clone()),
                        );
                        return DataraType::Unit;
                    }
                    return DataraType::GenericInstance {
                        name: "Outcome".to_string(),
                        args: vec![DataraType::String],
                    };
                }
                let payload_ty = arg_types.first().cloned().unwrap_or(DataraType::Int);
                return DataraType::GenericInstance {
                    name: "Outcome".to_string(),
                    args: vec![payload_ty],
                };
            }
            if let Expr::Identifier(mod_alias, _) = &**object {
                if self
                    .program_module_aliases
                    .get(mod_alias)
                    .map(|funcs| funcs.contains(member))
                    .unwrap_or(false)
                {
                    let qualified = format!("{}.{}", mod_alias, member);
                    let sig_opt = self
                        .function_signatures
                        .get(&qualified)
                        .or_else(|| self.function_signatures.get(member))
                        .cloned();

                    if let Some((param_types, ret_ty, _)) = sig_opt {
                        let is_bridge = self.bridge_functions.contains(&qualified)
                            || self.bridge_functions.contains(member);

                        for (idx, (p_ty, a_ty)) in
                            param_types.iter().zip(arg_types.iter()).enumerate()
                        {
                            if !a_ty.is_compatible_with_refined_with_args(p_ty, Some(self.resolver))
                            {
                                if is_bridge {
                                    diag.error(
                                        ErrorCode::BridgeTypeMismatch,
                                        format!(
                                            "Bridge argument type mismatch for argument {}: expected '{}', got '{}'",
                                            idx + 1,
                                            p_ty,
                                            a_ty
                                        ),
                                        Some(span.clone()),
                                    );
                                } else {
                                    diag.error(
                                        ErrorCode::TypeMismatch,
                                        format!(
                                            "Type mismatch for argument {}: expected '{}', got '{}'",
                                            idx + 1,
                                            p_ty,
                                            a_ty
                                        ),
                                        Some(span.clone()),
                                    );
                                }
                            }
                        }
                        return ret_ty;
                    }
                }
            }
            let obj_type = self.check_expr(object, diag);
            if let DataraType::GenericInstance { name, args } = &obj_type
                && name == "Capability"
            {
                let cap_kind = args.first().map(|a| a.to_string()).unwrap_or_default();
                if cap_kind == "FileRead" {
                    if member == "open" {
                        return DataraType::Class("FileHandle".into());
                    }
                    if member == "read_all" {
                        return DataraType::String;
                    }
                } else if cap_kind == "FileWrite" {
                    if member == "open" {
                        return DataraType::Class("FileWriteHandle".into());
                    }
                    if member == "write" || member == "write_all" {
                        return DataraType::Int;
                    }
                } else if cap_kind == "NetworkConnect" && member == "connect" {
                    return DataraType::Int;
                }
            }
            // Parametric collection methods: the receiver's element
            // types flow into the result (e.g. `Map<Str, Int>.get`
            // returns `Int`, not a hardcoded `Int` for every map).
            match &obj_type {
                DataraType::SimdF32x4 => match member.as_str() {
                    "sum" => return DataraType::Float,
                    "dot" => {
                        if let Some(arg_ty) = arg_types.first() {
                            if *arg_ty != DataraType::SimdF32x4 {
                                diag.error_with_help(
                                    ErrorCode::TypeIncomparableOperands,
                                    format!("SIMD vector type mismatch: 'dot' expects 'simd_f32x4', got '{}'", arg_ty),
                                    Some(span.clone()),
                                    Some("Use explicit conversion '.to_f32()' before calling '.dot()'.".to_string()),
                                );
                            }
                        }
                        return DataraType::Float;
                    }
                    "min" | "max" => return DataraType::Float,
                    "to_i32" => return DataraType::SimdI32x4,
                    "to_f32" => return DataraType::SimdF32x4,
                    _ => {}
                },
                DataraType::SimdI32x4 => match member.as_str() {
                    "sum" => return DataraType::Int,
                    "dot" => {
                        if let Some(arg_ty) = arg_types.first() {
                            if *arg_ty != DataraType::SimdI32x4 {
                                diag.error_with_help(
                                    ErrorCode::TypeIncomparableOperands,
                                    format!("SIMD vector type mismatch: 'dot' expects 'simd_i32x4', got '{}'", arg_ty),
                                    Some(span.clone()),
                                    Some("Use explicit conversion '.to_i32()' before calling '.dot()'.".to_string()),
                                );
                            }
                        }
                        return DataraType::Int;
                    }
                    "min" | "max" => return DataraType::Int,
                    "to_f32" => return DataraType::SimdF32x4,
                    "to_i32" => return DataraType::SimdI32x4,
                    _ => {}
                },
                DataraType::List(elem) => match member.as_str() {
                    "length" | "count" | "len" => return DataraType::Int,
                    "get" => return (**elem).clone(),
                    // v1.4.1: pop/first/last return the checked Outcome<T>
                    // object, not a bare element -- an empty list must be
                    // observable through is_err()/err()/`?`, never guessed.
                    // Checker-level representation is Result(T, Str),
                    // matching the checked-I/O builtins and `?` propagation.
                    "pop" => {
                        self.note_list_len_mutation(object, false);
                        return DataraType::Result(
                            Box::new((**elem).clone()),
                            Box::new(DataraType::String),
                        );
                    }
                    "first" | "last" => {
                        return DataraType::Result(
                            Box::new((**elem).clone()),
                            Box::new(DataraType::String),
                        );
                    }
                    "set" | "push" | "append" => {
                        // `push`/`append` grow the receiver in place; `set`
                        // replaces an element and leaves the length alone.
                        if member != "set" {
                            self.note_list_len_mutation(object, true);
                        }
                        return DataraType::List(elem.clone());
                    }
                    "insert_at" => {
                        // May grow the receiver (idx == count appends).
                        self.note_list_len_mutation(object, true);
                        return DataraType::List(elem.clone());
                    }
                    "remove_at" | "remove_value" => {
                        // Honest "nothing removed" (Int 0) on a miss; the
                        // length can only shrink.
                        self.note_list_len_mutation(object, false);
                        return DataraType::Int;
                    }
                    "clear" => {
                        self.note_list_len_mutation(object, false);
                        return DataraType::List(elem.clone());
                    }
                    "sort" | "reverse" | "slice" => {
                        return DataraType::List(elem.clone());
                    }
                    "contains" | "is_empty" => return DataraType::Bool,
                    "index_of" => return DataraType::Int,
                    "map" => {
                        check_list_combinator_args(diag, member, &arg_types, 1, span);
                        if let Some(first_arg) = arg_types.first() {
                            if let DataraType::Function { return_type, .. } = first_arg {
                                return DataraType::List(return_type.clone());
                            }
                        }
                        return DataraType::List(elem.clone());
                    }
                    "filter" => {
                        check_list_combinator_args(diag, member, &arg_types, 1, span);
                        return DataraType::List(elem.clone());
                    }
                    "reduce" | "fold" => {
                        check_list_combinator_args(diag, member, &arg_types, 2, span);
                        if let Some(init_ty) = arg_types.first() {
                            return init_ty.clone();
                        }
                        return DataraType::Int;
                    }
                    "collect" => {
                        check_list_combinator_args(diag, member, &arg_types, 0, span);
                        return DataraType::List(elem.clone());
                    }
                    "find" => return (**elem).clone(),
                    "any" | "all" => return DataraType::Bool,
                    _ => {}
                },
                DataraType::Map(key, val) => match member.as_str() {
                    "get" => return (**val).clone(),
                    "insert" => {
                        return DataraType::Map(key.clone(), val.clone());
                    }
                    "length" | "count" | "len" => return DataraType::Int,
                    _ => {}
                },
                DataraType::Result(ok, err) => match member.as_str() {
                    "is_ok" | "is_err" => return DataraType::Bool,
                    "unwrap" | "ok" | "unwrap_or" => return (**ok).clone(),
                    "unwrap_err" | "err" => return (**err).clone(),
                    _ => {}
                },
                DataraType::Option(val) => match member.as_str() {
                    "is_some" | "is_none" => return DataraType::Bool,
                    "unwrap" | "unwrap_or" => return (**val).clone(),
                    _ => {}
                },
                _ => {}
            }
            if member == "await" {
                match &obj_type {
                    DataraType::GenericInstance { name, args }
                        if (name == "Future" || name == "Task") =>
                    {
                        return args.first().cloned().unwrap_or(DataraType::String);
                    }
                    DataraType::Class(name) if (name == "Future" || name == "Task") => {
                        return DataraType::String;
                    }
                    DataraType::Result(ok, _) => return (**ok).clone(),
                    DataraType::Option(val) => return (**val).clone(),
                    _ => return obj_type,
                }
            }
            // Numeric conversion intrinsics (Gate 7 explicit casts):
            // `.to_float()` and `.to_int()` on numeric receivers are
            // compiler intrinsics lowered to a single conversion
            // instruction, never a method dispatch.
            if matches!(
                &obj_type,
                DataraType::Int | DataraType::Float | DataraType::Dec64 | DataraType::Dec128
            ) && matches!(member.as_str(), "to_float" | "to_int")
            {
                if !arg_types.is_empty() {
                    diag.error(
                        ErrorCode::TypeMismatch,
                        format!("Conversion method '{}' takes no arguments", member),
                        Some(span.clone()),
                    );
                    return DataraType::Unit;
                }
                return if member == "to_float" {
                    DataraType::Float
                } else {
                    DataraType::Int
                };
            }
            let (cls_opt, gen_args_opt) = match &obj_type {
                DataraType::Class(cls) => (Some(cls.as_str()), None),
                DataraType::GenericInstance { name, args } => (Some(name.as_str()), Some(args)),
                _ => (None, None),
            };
            if let Some(cls) = cls_opt {
                let full_name = format!("{}.{}", cls, member);
                if let Some(t) = self.symbol_types.get(&full_name) {
                    if let (DataraType::TypeParam(p), Some(args)) = (t, gen_args_opt) {
                        if let Some((params, _)) = self.generic_templates.get(cls) {
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
                if cls == "List" {
                    match member.as_str() {
                        "length" | "count" | "len" => return DataraType::Int,
                        "get" => {
                            // Element type recorded from the initializer
                            // when the receiver is a named variable;
                            // otherwise the dynamic type, never Int.
                            if let Expr::Identifier(name, _) = &**object
                                && let Some(elem) = self.var_element_types.get(name)
                            {
                                return elem.clone();
                            }
                            return DataraType::Val;
                        }
                        // v1.4.1: checked accessors wrap the (recorded or
                        // dynamic) element type in Outcome -- checker-level
                        // Result(T, Str), like the checked-I/O builtins.
                        "pop" => {
                            self.note_list_len_mutation(object, false);
                            let payload = if let Expr::Identifier(name, _) = &**object
                                && let Some(elem) = self.var_element_types.get(name)
                            {
                                elem.clone()
                            } else {
                                DataraType::Val
                            };
                            return DataraType::Result(
                                Box::new(payload),
                                Box::new(DataraType::String),
                            );
                        }
                        "first" | "last" => {
                            let payload = if let Expr::Identifier(name, _) = &**object
                                && let Some(elem) = self.var_element_types.get(name)
                            {
                                elem.clone()
                            } else {
                                DataraType::Val
                            };
                            return DataraType::Result(
                                Box::new(payload),
                                Box::new(DataraType::String),
                            );
                        }
                        "set" | "push" | "append" | "map" | "filter" | "insert_at" | "sort"
                        | "reverse" | "clear" | "slice" | "collect" => {
                            return DataraType::Class("List".into());
                        }
                        "remove_at" | "remove_value" | "reduce" | "fold" | "find" | "index_of" => {
                            return DataraType::Int;
                        }
                        "any" | "all" | "contains" | "is_empty" => {
                            return DataraType::Bool;
                        }
                        _ => {}
                    }
                }
                if cls == "Map" {
                    match member.as_str() {
                        "get" => {
                            if let Expr::Identifier(name, _) = &**object
                                && let Some(elem) = self.var_element_types.get(name)
                            {
                                return elem.clone();
                            }
                            return DataraType::Val;
                        }
                        "insert" => {
                            return DataraType::Class("Map".into());
                        }
                        "length" | "count" | "len" => return DataraType::Int,
                        _ => {}
                    }
                }
                if let Some(m_type) = self.class_methods.get(cls).and_then(|m| m.get(member)) {
                    if let (DataraType::TypeParam(p), Some(args)) = (m_type, gen_args_opt) {
                        if let Some((params, _)) = self.generic_templates.get(cls) {
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
                    return m_type.clone();
                }
                let specialized = format!("{}_{}", cls, member);
                if let Some((_, ret_ty, _)) = self.function_signatures.get(&specialized) {
                    if let (DataraType::TypeParam(p), Some(args)) = (ret_ty, gen_args_opt) {
                        if let Some((params, _)) = self.generic_templates.get(cls) {
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
                    return ret_ty.clone();
                }
                if let Some(cls_sym) = self.resolver.classes.get(cls) {
                    for comp in &cls_sym.compositions {
                        if let Some(m_type) =
                            self.class_methods.get(comp).and_then(|m| m.get(member))
                        {
                            return m_type.clone();
                        }
                        let specialized = format!("{}_{}", comp, member);
                        if let Some((_, ret_ty, _)) = self.function_signatures.get(&specialized) {
                            return ret_ty.clone();
                        }
                    }
                }
            }
            if let DataraType::TypeParam(p) = &obj_type {
                let bounds = self
                    .current_fn_name
                    .as_ref()
                    .and_then(|fn_name| self.trait_bounds.get(&format!("{}:{}", fn_name, p)))
                    .or_else(|| self.trait_bounds.get(p));
                if let Some(trait_names) = bounds {
                    for tr_name in trait_names {
                        let mut visited = HashSet::new();
                        if let Some(tm) = self.find_trait_method(tr_name, member, &mut visited) {
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
                        "Method '{}' not found for type parameter '{}' with bounds '{}'",
                        member, p, bound_str
                    ),
                    Some(span.clone()),
                );
                return DataraType::Unit;
            }
            // Method-call fallback: only prelude builtins may be
            // resolved as methods on arbitrary receivers. Resolving
            // user-defined global functions here would let any
            // `obj.foo()` silently call the global `foo`.
            if !self.resolver.functions.contains_key(member)
                && !self.resolver.extern_functions.contains_key(member)
                && let Some((_, ret_ty, _)) = self.function_signatures.get(member)
            {
                return ret_ty.clone();
            }
            // Borrow-system semantic markers: `view` / `clone` /
            // `mut_view` produce a borrowed view of the receiver instead
            // of a dispatched call. This mirrors the MemberAccess rule in
            // `check_expr`, which resolves the same markers to the
            // receiver's own type.
            if matches!(member.as_str(), "view" | "clone" | "mut_view") {
                return obj_type.clone();
            }
            // UFCS: a method call that names a module-level function (or
            // extern) resolves to that function; the native dispatch
            // falls back to the bare name in its function table, so the
            // call is real, not a silent fallback.
            if self.resolver.functions.contains_key(member)
                || self.resolver.extern_functions.contains_key(member)
            {
                if let Some((_, ret_ty, _)) = self.function_signatures.get(member) {
                    return ret_ty.clone();
                }
                return obj_type.clone();
            }
            // Unknown-method gate (E-TYPE-009): the receiver type is
            // fully known and no class method, specialized function,
            // trait method, prelude builtin or conversion intrinsic
            // resolved the call. The diagnostic keeps unknown methods
            // from reaching code generation, where a dynamic dispatch
            // fallback once emitted calls to unresolved symbols.
            let receiver_known = cls_opt.is_some()
                || matches!(
                    &obj_type,
                    DataraType::Int
                        | DataraType::Float
                        | DataraType::Bool
                        | DataraType::String
                        | DataraType::Char
                        | DataraType::Dec64
                        | DataraType::Dec128
                        | DataraType::List(_)
                        | DataraType::Map(_, _)
                        | DataraType::Result(_, _)
                        | DataraType::Option(_)
                );
            if receiver_known {
                let help = if matches!(
                    &obj_type,
                    DataraType::Int | DataraType::Float | DataraType::Dec64 | DataraType::Dec128
                ) {
                    Some(
                        "numeric receivers provide the conversion methods '.to_float()' and '.to_int()'"
                            .to_string(),
                    )
                } else {
                    None
                };
                diag.error_with_help(
                    ErrorCode::TypeUnknownMethod,
                    format!("Unknown method '{}' on type '{}'", member, obj_type),
                    Some(span.clone()),
                    help,
                );
                return DataraType::Unit;
            }
        }

        let callee_ty = self.check_expr(callee, diag);
        if let DataraType::Function { return_type, .. } = callee_ty {
            return *return_type;
        }
        if matches!(callee_ty, DataraType::TypeParam(_)) {
            // Lambda-argument static dispatch: calling a generic parameter
            // (`g(v)` inside `fn apply<T>(g: T, v: Int) -> Int`) types as the
            // enclosing function's declared return type. Such call sites are
            // inlined by the lowering (inlineable_fns) with the lambda bound
            // to the parameter, so the declared return type is the concrete
            // result type.
            if let Some(fn_name) = &self.current_fn_name
                && let Some((_, ret_type, _)) = self.function_signatures.get(fn_name)
            {
                return ret_type.clone();
            }
        }
        DataraType::Unit
    }

    /// Track the in-place length change of `List.push` / `append` / `pop` on a
    /// plain-identifier receiver.
    ///
    /// These methods mutate the receiver in place at run time, so the
    /// statically tracked array length must move with them. Without this a
    /// following `xs[i]` is compared against the pre-mutation length and
    /// `E0947` rejects an index that is in bounds. Receivers that are not a
    /// plain identifier, and variables whose length was never tracked, are
    /// left untouched so the real out-of-bounds catches keep firing.
    fn note_list_len_mutation(&mut self, object: &Expr, grow: bool) {
        let Expr::Identifier(name, _) = object else {
            return;
        };
        let Some(len) = self.var_array_lengths.get(name).copied() else {
            return;
        };
        let updated = if grow { len + 1 } else { len.saturating_sub(1) };
        self.var_array_lengths.insert(name.clone(), updated);
    }
}

/// v1.4.1 E-TYPE-008 gate for the List combinators: a wrong arity or a
/// definitively non-callable argument is a hard type error instead of the
/// old silent "returns the receiver list" fallback. Named functions surface
/// as RawPtr and lambdas as Function, so only concrete primitives are
/// rejected as non-callable here.
fn check_list_combinator_args(
    diag: &mut DiagnosticEngine,
    member: &str,
    arg_types: &[DataraType],
    expected_arity: usize,
    span: &SourceSpan,
) {
    let hint = match member {
        "collect" => "collect() takes no arguments",
        "reduce" | "fold" => {
            "reduce/fold expect (initial_value, accumulator_fn) where the accumulator is a lambda or named function of arity 2"
        }
        _ => "map/filter expect a lambda or named function of arity 1",
    };
    if arg_types.len() != expected_arity {
        diag.error(
            ErrorCode::TypeIncomparableOperands,
            format!(
                "'{}' on a List expects {} argument(s): {}",
                member, expected_arity, hint
            ),
            Some(span.clone()),
        );
        return;
    }
    if expected_arity > 0 {
        let callable_idx = expected_arity - 1;
        let definitely_not_callable = matches!(
            arg_types.get(callable_idx),
            Some(DataraType::Int)
                | Some(DataraType::Float)
                | Some(DataraType::Bool)
                | Some(DataraType::String)
                | Some(DataraType::Char)
        );
        if definitely_not_callable {
            diag.error(
                ErrorCode::TypeIncomparableOperands,
                format!(
                    "'{}' expects a lambda or named function as its last argument, got '{}': {}",
                    member, arg_types[callable_idx], hint
                ),
                Some(span.clone()),
            );
        }
    }
}

fn bind_type_params(
    p_ty: &DataraType,
    a_ty: &DataraType,
    bindings: &mut HashMap<String, DataraType>,
) {
    match (p_ty, a_ty) {
        (DataraType::TypeParam(param_name), _) => {
            bindings
                .entry(param_name.clone())
                .or_insert_with(|| a_ty.clone());
        }
        (DataraType::List(p_in), DataraType::List(a_in)) => {
            bind_type_params(p_in, a_in, bindings);
        }
        (DataraType::Map(pk, pv), DataraType::Map(ak, av)) => {
            bind_type_params(pk, ak, bindings);
            bind_type_params(pv, av, bindings);
        }
        (
            DataraType::GenericInstance {
                name: pn,
                args: pargs,
            },
            DataraType::GenericInstance {
                name: an,
                args: aargs,
            },
        ) if pn == an && pargs.len() == aargs.len() => {
            for (p, a) in pargs.iter().zip(aargs.iter()) {
                bind_type_params(p, a, bindings);
            }
        }
        _ => {}
    }
}

fn subst_type_params(ty: &DataraType, bindings: &HashMap<String, DataraType>) -> DataraType {
    match ty {
        DataraType::TypeParam(p) => bindings.get(p).cloned().unwrap_or_else(|| ty.clone()),
        DataraType::List(inner) => DataraType::List(Box::new(subst_type_params(inner, bindings))),
        DataraType::Map(k, v) => DataraType::Map(
            Box::new(subst_type_params(k, bindings)),
            Box::new(subst_type_params(v, bindings)),
        ),
        DataraType::GenericInstance { name, args } => DataraType::GenericInstance {
            name: name.clone(),
            args: args
                .iter()
                .map(|a| subst_type_params(a, bindings))
                .collect(),
        },
        _ => ty.clone(),
    }
}

fn is_comptime_constant(expr: &Expr, resolver: &crate::resolver::Resolver) -> bool {
    match expr {
        Expr::Literal(..) => true,
        Expr::ListLiteral(items, _) => items.iter().all(|it| is_comptime_constant(it, resolver)),
        Expr::MapLiteral(entries, _) => entries
            .iter()
            .all(|(k, v)| is_comptime_constant(k, resolver) && is_comptime_constant(v, resolver)),
        Expr::Binary { left, right, .. } => {
            is_comptime_constant(left, resolver) && is_comptime_constant(right, resolver)
        }
        Expr::Unary { expr, .. } => is_comptime_constant(expr, resolver),
        Expr::Wrapping(inner, _)
        | Expr::Saturating(inner, _)
        | Expr::Comptime { expr: inner, .. } => is_comptime_constant(inner, resolver),
        Expr::Call { callee, args, .. } => {
            if let Expr::Identifier(name, _) = &**callee {
                resolver.comptime_functions.contains(name)
                    && args.iter().all(|a| is_comptime_constant(a, resolver))
            } else {
                false
            }
        }
        _ => false,
    }
}
