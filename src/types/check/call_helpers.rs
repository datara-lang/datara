//! Free helper functions for call type-checking, split out of call.rs to
//! keep that file under the release size gate (60 KB, test_release_file_size_limits).
use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::types::DataraType;
use std::collections::{HashMap, HashSet};

/// v1.4.1 E-TYPE-008 gate for the List combinators: a wrong arity or a
/// definitively non-callable argument is a hard type error instead of the
/// old silent "returns the receiver list" fallback. Named functions surface
/// as RawPtr and lambdas as Function, so only concrete primitives are
/// rejected as non-callable here.
pub(crate) fn check_list_combinator_args(
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

pub(crate) fn bind_type_params(
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

/// True when `name` occurs anywhere inside `ty` (recursively through
/// List/Map/GenericInstance arguments). Used by the callback-binding
/// heuristic at generic call sites.
pub(crate) fn contains_type_param(ty: &DataraType, name: &str) -> bool {
    match ty {
        DataraType::TypeParam(p) => p == name,
        DataraType::List(inner) => contains_type_param(inner, name),
        DataraType::Map(k, v) => contains_type_param(k, name) || contains_type_param(v, name),
        DataraType::GenericInstance { args, .. } => {
            args.iter().any(|a| contains_type_param(a, name))
        }
        DataraType::Function {
            params,
            return_type,
            ..
        } => {
            params.iter().any(|a| contains_type_param(a, name))
                || contains_type_param(return_type, name)
        }
        DataraType::Result(ok, err) => {
            contains_type_param(ok, name) || contains_type_param(err, name)
        }
        _ => false,
    }
}

/// Bare-generic callback binding: a generic parameter bound to a lambda AND
/// appearing bare inside the generic fn's return type is a callback whose
/// RESULT flows out (`fn iter_map<T, U>(items: List<T>, f: U) -> List<U>`).
/// Bind U to the lambda's RETURN type, not its Function type — `f(x)` call
/// sites inside the body type via the declared return type (lambda-argument
/// static dispatch), so the old binding made every result `(Int) -> Int`-
/// shaped ("expected List, got List"). Parameters NOT referenced in the
/// return type keep the Function binding (the callback is only consumed,
/// not its result). Returns the overridden parameter names.
pub(crate) fn apply_callback_binding_overrides<'a, I>(
    params_args: I,
    ret_type: &DataraType,
    type_bindings: &mut HashMap<String, DataraType>,
) -> Vec<String>
where
    I: Iterator<Item = (&'a DataraType, &'a DataraType)>,
{
    let mut callback_bound: Vec<String> = Vec::new();
    for (p_ty, a_ty) in params_args {
        if let (DataraType::TypeParam(p), DataraType::Function { return_type, .. }) = (p_ty, a_ty)
            && contains_type_param(ret_type, p)
        {
            type_bindings.insert(p.clone(), (**return_type).clone());
            callback_bound.push(p.clone());
        }
    }
    callback_bound
}

pub(crate) fn subst_type_params(
    ty: &DataraType,
    bindings: &HashMap<String, DataraType>,
) -> DataraType {
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

/// Validates the key argument of `Map<K, V>.insert/get/contains` against the
/// receiver's key type. Before v1.5.0 the checker accepted any key, so e.g.
/// `m.insert(1, 10)` on a `Map<Str, Int>` reached the C runtime and was
/// reinterpreted as a `char*` (segfault). A clear compile-time error is the
/// only sound answer: the runtime key layouts are not interchangeable.
pub(crate) fn check_map_key_arg(
    diag: &mut DiagnosticEngine,
    member: &str,
    key_ty: &DataraType,
    arg_types: &[DataraType],
    span: &SourceSpan,
    resolver: &crate::resolver::Resolver,
) {
    let Some(arg) = arg_types.first() else {
        return;
    };
    if arg.is_compatible_with_args(key_ty, Some(resolver)) {
        return;
    }
    let help = match (key_ty, arg) {
        (DataraType::String, DataraType::Int) => {
            "convert the key with int_to_str(...) before inserting/looking it up".to_string()
        }
        (DataraType::Int, DataraType::String) => {
            "convert the key with text_parse_int(...) / str_to_int(...) (an Outcome — use `or`)"
                .to_string()
        }
        _ => format!("this map is keyed by '{}'", key_ty),
    };
    diag.error_with_help(
        ErrorCode::TypeMismatch,
        format!(
            "map.{} key type mismatch: expected '{}', got '{}'",
            member, key_ty, arg
        ),
        Some(span.clone()),
        Some(help),
    );
}

pub(crate) fn is_comptime_constant(expr: &Expr, resolver: &crate::resolver::Resolver) -> bool {
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

/// Trait-bound satisfaction for the concrete types a generic call bound
/// (moved out of call.rs for the 60 KB release size gate). Walks impls and
/// composition/base-class closure; a TypeParam bound through the CALLER's
/// own bounds passes through.
pub(crate) fn check_generic_trait_bounds(
    tc: &super::super::TypeChecker,
    fn_name: &str,
    type_bindings: &HashMap<String, DataraType>,
    span: &SourceSpan,
    diag: &mut DiagnosticEngine,
) {
    for (param_name, concrete_ty) in type_bindings {
        let bounds = tc
            .trait_bounds
            .get(&format!("{}:{}", fn_name, param_name))
            .or_else(|| tc.trait_bounds.get(param_name));
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
                let mut satisfied = tc.impls.contains_key(&(tr_name.clone(), type_str.clone()));
                if !satisfied {
                    if let DataraType::TypeParam(p) = concrete_ty {
                        let caller_bounds = tc
                            .current_fn_name
                            .as_ref()
                            .and_then(|fn_n| tc.trait_bounds.get(&format!("{}:{}", fn_n, p)))
                            .or_else(|| tc.trait_bounds.get(p));
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
                        if tc.impls.contains_key(&(tr_name.clone(), cls_name.clone())) {
                            satisfied = true;
                            break;
                        }
                        if let Some(cls_sym) = tc.resolver.classes.get(&cls_name) {
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
}
