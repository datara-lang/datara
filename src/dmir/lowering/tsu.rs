//! M7 TSU lowering: the `is` type test and the mixed-literal NaN-boxing.
//! Split out of expr_composite.rs for the 60 KB release size gate.

use super::Lowering;
use crate::ast::Expr;
use crate::dmir::ir::{BasicBlockId, Inst, ValueId};
use crate::types::DataraType;

impl<'a> Lowering<'a> {
    pub(crate) fn lower_is_check(
        &mut self,
        operand: &Expr,
        ty_name: &str,
        cur_block: &mut BasicBlockId,
    ) -> Option<ValueId> {
        // M7 TSU: `x is T`. A dynamically-typed operand (Val) lowers
        // to the NaN-box tag predicate; a statically-typed operand
        // makes the check a compile-time constant — the degenerate
        // TypeSet compiles to zero runtime checks.
        let op_ty = self.infer_expr_datara_type(operand);
        let is_val_operand = matches!(op_ty, Some(DataraType::Val))
            || matches!(&op_ty, Some(DataraType::List(b)) if matches!(&**b, DataraType::Val));
        if !is_val_operand {
            let dest = self.next_val();
            let matches_static = match (&op_ty, ty_name) {
                (Some(DataraType::Int), "Int")
                | (Some(DataraType::Float), "Float")
                | (Some(DataraType::Bool), "Bool")
                | (Some(DataraType::String), "Str" | "String")
                | (Some(DataraType::List(..)), "List")
                | (Some(DataraType::Map(..)), "Map") => true,
                (Some(DataraType::Class(c)), other) => c == other,
                _ => false,
            };
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::ConstBool {
                    dest,
                    value: matches_static,
                });
            return Some(dest);
        }
        let v = self.lower_expr(operand, cur_block)?;
        let func = match ty_name {
            "Int" => "datara_val_is_int",
            "Float" => "datara_val_is_float",
            "Bool" => "datara_val_is_bool",
            "Str" | "String" => "datara_val_is_str",
            _ => {
                return None;
            }
        };
        let dest = self.next_val();
        self.get_block_mut(*cur_block)
            .instructions
            .push(Inst::Call {
                dest,
                func: func.into(),
                args: vec![v],
                ty: "Bool".into(),
            });
        Some(dest)
    }
}

/// The Outcome member convention (repro #13): is_success -> Bool,
/// value -> the payload repr, error_msg -> Str (ALWAYS — the error
/// channel is fixed Str per types/mod.rs:488, even when the generic
/// template for the class is not registered in this compilation).
/// Returns the repr string, or None when `name` is not Outcome or the
/// member is not an Outcome slot.
pub(crate) fn outcome_member_repr(name: &str, member: &str, args: &[DataraType]) -> Option<String> {
    if name != "Outcome" {
        return None;
    }
    match member {
        "is_success" | "is_ok" | "is_err" => Some("Bool".into()),
        "value" | "ok" => Some(
            args.first()
                .map(|t| t.to_string())
                .unwrap_or_else(|| "Val".into()),
        ),
        "error_msg" | "error" | "err" => Some("Str".into()),
        _ => None,
    }
}

/// Same convention over DataraType args (for the infer path). Returns
/// the member's DataraType, or None when not an Outcome slot.
pub(crate) fn outcome_member_datara(
    name: &str,
    member: &str,
    args: &[DataraType],
) -> Option<DataraType> {
    if name != "Outcome" {
        return None;
    }
    match member {
        "is_success" | "is_ok" | "is_err" => Some(DataraType::Bool),
        "value" | "ok" => Some(args.first().cloned().unwrap_or(DataraType::Val)),
        "error_msg" | "error" | "err" => Some(DataraType::String),
        _ => None,
    }
}
