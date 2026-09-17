use super::*;
use crate::ast::{Expr, LiteralValue};
use crate::dmir::{Function, Inst, ValueId};

impl LoopOptimizer {
    /// If `vid` is the dest of `a + 1` (BinOp), return the left operand.
    pub(crate) fn binop_add_one_source(f: &Function, vid: ValueId) -> Option<ValueId> {
        for block in &f.blocks {
            for inst in &block.instructions {
                if let Inst::BinOp {
                    dest,
                    op,
                    left,
                    right,
                    ..
                } = inst
                    && *dest == vid
                    && (op == "+" || op == "wrapping_+")
                    && Self::const_int_value(f, *right) == Some(1)
                {
                    return Some(*left);
                }
                if let Inst::Call {
                    func, args, dest, ..
                } = inst
                    && *dest == vid
                    && func == "datara_rt_checked_add"
                    && args.len() == 2
                    && Self::const_int_value(f, args[1]) == Some(1)
                {
                    return Some(args[0]);
                }
            }
        }
        None
    }

    pub(crate) fn is_zero_expr(expr: &Expr) -> bool {
        matches!(expr, Expr::Literal(LiteralValue::Int(0), _))
    }

    pub(crate) fn extract_len_target(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Call { callee, args, .. } if args.is_empty() => {
                if let Expr::MemberAccess { object, member, .. } = callee.as_ref()
                    && (member == "len" || member == "length")
                    && let Expr::Identifier(arr_name, _) = object.as_ref()
                {
                    return Some(arr_name.clone());
                }
                if let Expr::Identifier(fn_name, _) = callee.as_ref()
                    && (fn_name == "len" || fn_name == "length")
                    && let Some(Expr::Identifier(arr_name, _)) = args.first()
                {
                    return Some(arr_name.clone());
                }
                None
            }
            Expr::Call { callee, args, .. } if args.len() == 1 => {
                if let Expr::Identifier(fn_name, _) = callee.as_ref()
                    && (fn_name == "len" || fn_name == "length")
                    && let Some(Expr::Identifier(arr_name, _)) = args.first()
                {
                    return Some(arr_name.clone());
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn extract_predicate_len_target(var_name: &str, predicate: &Expr) -> Option<String> {
        match predicate {
            Expr::Binary {
                op, left, right, ..
            } if op == "&&" => {
                let left_ok = Self::is_non_negative_check(var_name, left);
                let right_target = Self::is_less_than_len_check(var_name, right);
                if left_ok && right_target.is_some() {
                    return right_target;
                }
                let right_ok = Self::is_non_negative_check(var_name, right);
                let left_target = Self::is_less_than_len_check(var_name, left);
                if right_ok && left_target.is_some() {
                    return left_target;
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn is_non_negative_check(var_name: &str, expr: &Expr) -> bool {
        match expr {
            Expr::Binary {
                op, left, right, ..
            } => {
                if op == ">=" {
                    if let Expr::Identifier(name, _) = left.as_ref()
                        && name == var_name
                        && Self::is_zero_expr(right)
                    {
                        return true;
                    }
                } else if op == "<="
                    && let Expr::Identifier(name, _) = right.as_ref()
                    && name == var_name
                    && Self::is_zero_expr(left)
                {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    pub(crate) fn is_less_than_len_check(var_name: &str, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Binary {
                op, left, right, ..
            } if op == "<" => {
                if let Expr::Identifier(name, _) = left.as_ref()
                    && name == var_name
                {
                    return Self::extract_len_target(right);
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn extract_index_bound_from_contract(expr: &Expr) -> Option<(String, String)> {
        match expr {
            Expr::Binary {
                op, left, right, ..
            } if op == "&&" => {
                if let (Some(idx1), Some((idx2, arr))) = (
                    Self::extract_non_negative_var(left),
                    Self::extract_less_than_len(right),
                ) && idx1 == idx2
                {
                    return Some((idx1, arr));
                }
                if let (Some((idx1, arr)), Some(idx2)) = (
                    Self::extract_less_than_len(left),
                    Self::extract_non_negative_var(right),
                ) && idx1 == idx2
                {
                    return Some((idx1, arr));
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn extract_non_negative_var(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Binary {
                op, left, right, ..
            } => {
                if op == ">=" {
                    if let Expr::Identifier(name, _) = left.as_ref()
                        && Self::is_zero_expr(right)
                    {
                        return Some(name.clone());
                    }
                } else if op == "<="
                    && let Expr::Identifier(name, _) = right.as_ref()
                    && Self::is_zero_expr(left)
                {
                    return Some(name.clone());
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn extract_less_than_len(expr: &Expr) -> Option<(String, String)> {
        match expr {
            Expr::Binary {
                op, left, right, ..
            } if op == "<" => {
                if let Expr::Identifier(idx_name, _) = left.as_ref()
                    && let Some(arr_name) = Self::extract_len_target(right)
                {
                    return Some((idx_name.clone(), arr_name));
                }
                None
            }
            _ => None,
        }
    }
}
