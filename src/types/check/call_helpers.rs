//! Free helper functions for call type-checking, split out of call.rs to
//! keep that file under the release size gate (60 KB, test_release_file_size_limits).
use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::types::DataraType;
use std::collections::HashMap;


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

pub(crate) fn subst_type_params(ty: &DataraType, bindings: &HashMap<String, DataraType>) -> DataraType {
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
