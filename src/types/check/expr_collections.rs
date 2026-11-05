//! Collection-literal and index-access type checking, extracted from
//! expr.rs to keep that file under the release size gate (60 KB,
//! test_release_file_size_limits).

use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use crate::types::{DataraType, TypeChecker};

impl<'a> TypeChecker<'a> {
    /// Type-checks the collection-shaped expression variants
    /// (List/Map literals, index access, array-repeat). Returns `Some(type)`
    /// when `expr` is one of the handled variants and was fully checked;
    /// `None` leaves the decision to the caller's match.
    pub(crate) fn check_collection_expr(
        &mut self,
        expr: &Expr,
        diag: &mut DiagnosticEngine,
    ) -> Option<DataraType> {
        match expr {
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
                Some(DataraType::List(Box::new(elem_ty)))
            }
            Expr::MapLiteral(entries, span) => {
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
                let key_ty = key.unwrap_or(DataraType::Val);
                // v1.5.0 map key contract: the runtime has exactly two key
                // layouts (owned Str and raw Int). A Float/Bool/class key used
                // to be accepted by the checker and then reinterpreted as a
                // pointer at run time — fail at compile time instead.
                if !matches!(
                    key_ty,
                    DataraType::String | DataraType::Int | DataraType::Val
                ) {
                    diag.error_with_help(
                        ErrorCode::TypeMismatch,
                        format!("unsupported map key type '{}'", key_ty),
                        Some(span.clone()),
                        Some(
                            "map keys must be 'Str' or 'Int' (convert with int_to_str/str_to_int)"
                                .to_string(),
                        ),
                    );
                }
                Some(DataraType::Map(
                    Box::new(key_ty),
                    Box::new(value.unwrap_or(DataraType::Val)),
                ))
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
                    Some(obj_ty)
                } else {
                    Some(match &obj_ty {
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
                    })
                }
            }
            Expr::ArrayRepeatLiteral { elem, .. } => {
                let elem_ty = self.check_expr(elem, diag);
                Some(DataraType::GenericInstance {
                    name: "Array".to_string(),
                    args: vec![elem_ty],
                })
            }
            _ => None,
        }
    }
}
