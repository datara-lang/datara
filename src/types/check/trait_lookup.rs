//! Trait/role method lookup (split out of check/expr.rs for the 60 KB
//! release size gate).

use crate::ast::SourceSpan;
use crate::ast::{Expr, TraitMethodSignature};
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use crate::types::{DataraType, TypeChecker};
use std::collections::HashSet;

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

    pub fn check_is_check(
        &mut self,
        operand: &Expr,
        ty_name: &str,
        span: &SourceSpan,
        diag: &mut DiagnosticEngine,
    ) -> DataraType {
        // M7 TSU: `x is T` — a runtime type test on a dynamic value.
        // The operand may be `Val` (the interesting case) or a
        // statically-typed value (the check is compile-time known;
        // narrowing still refines the branch). Result: Bool.
        let op_ty = self.check_expr(operand, diag);
        let known = matches!(
            ty_name,
            "Int" | "Float" | "Bool" | "Str" | "String" | "List" | "Map" | "Val" | "any"
        ) || self.resolver.classes.contains_key(ty_name);
        if !known {
            diag.error(
                ErrorCode::ResolveUnknownType,
                format!("'is' check against unknown type '{}'", ty_name),
                Some(span.clone()),
            );
        }
        let _ = op_ty;
        DataraType::Bool
    }
}
