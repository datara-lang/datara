use super::*;
use crate::dmir::{Function, Inst, ValueId};
use std::collections::HashMap;

impl Optimizer {
    pub(crate) fn constant_fold(&mut self, f: &mut Function) -> bool {
        let mut changed = false;
        let mut int_constants: HashMap<ValueId, i64> = HashMap::new();
        let mut float_constants: HashMap<ValueId, f64> = HashMap::new();
        let mut str_constants: HashMap<ValueId, String> = HashMap::new();
        let mut bool_constants: HashMap<ValueId, bool> = HashMap::new();
        let mut float_binops: HashMap<ValueId, (String, ValueId, ValueId)> = HashMap::new();
        let mut fresh_vid = crate::optimizer::loops::engine_v2::LoopEngineV2::max_vid(f) + 1;

        for block in &mut f.blocks {
            let mut block_var_ints: HashMap<String, i64> = HashMap::new();
            let mut block_var_floats: HashMap<String, f64> = HashMap::new();
            let mut block_var_strs: HashMap<String, String> = HashMap::new();
            let mut block_var_bools: HashMap<String, bool> = HashMap::new();
            let mut new_instructions = Vec::new();

            for inst in &block.instructions {
                match inst {
                    Inst::ConstInt { dest, value } => {
                        int_constants.insert(*dest, *value);
                        new_instructions.push(inst.clone());
                    }
                    Inst::ConstFloat { dest, value } => {
                        float_constants.insert(*dest, *value);
                        new_instructions.push(inst.clone());
                    }
                    Inst::ConstStr { dest, value } => {
                        str_constants.insert(*dest, value.clone());
                        new_instructions.push(inst.clone());
                    }
                    Inst::ConstBool { dest, value } => {
                        bool_constants.insert(*dest, *value);
                        new_instructions.push(inst.clone());
                    }
                    Inst::AssignVar { name, value } => {
                        if !self.module_mutable_globals.contains(name) {
                            if let Some(v) = int_constants.get(value) {
                                block_var_ints.insert(name.clone(), *v);
                            } else {
                                block_var_ints.remove(name);
                            }
                            if let Some(v) = float_constants.get(value) {
                                block_var_floats.insert(name.clone(), *v);
                            } else {
                                block_var_floats.remove(name);
                            }
                            if let Some(v) = str_constants.get(value) {
                                block_var_strs.insert(name.clone(), v.clone());
                            } else {
                                block_var_strs.remove(name);
                            }
                            if let Some(v) = bool_constants.get(value) {
                                block_var_bools.insert(name.clone(), *v);
                            } else {
                                block_var_bools.remove(name);
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::LoadVar { dest, name } => {
                        if !self.module_mutable_globals.contains(name) {
                            if let Some(v) = block_var_ints.get(name) {
                                int_constants.insert(*dest, *v);
                            }
                            if let Some(v) = block_var_floats.get(name) {
                                float_constants.insert(*dest, *v);
                            }
                            if let Some(v) = block_var_strs.get(name) {
                                str_constants.insert(*dest, v.clone());
                            }
                            if let Some(v) = block_var_bools.get(name) {
                                bool_constants.insert(*dest, *v);
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::UnOp {
                        dest,
                        op,
                        operand,
                        ty: _,
                    } => {
                        if op == "copy" {
                            if let Some(i) = int_constants.get(operand).copied() {
                                int_constants.insert(*dest, i);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: i,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(f) = float_constants.get(operand).copied() {
                                float_constants.insert(*dest, f);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: f,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(s) = str_constants.get(operand).cloned() {
                                str_constants.insert(*dest, s.clone());
                                new_instructions.push(Inst::ConstStr {
                                    dest: *dest,
                                    value: s,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(b) = bool_constants.get(operand).copied() {
                                bool_constants.insert(*dest, b);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: b,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        } else if op == "-" {
                            if let Some(i) = int_constants.get(operand).copied() {
                                let res = i.wrapping_neg();
                                int_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(f) = float_constants.get(operand).copied() {
                                let res = -f;
                                float_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        } else if op == "!" || op == "not" {
                            if let Some(b) = bool_constants.get(operand).copied() {
                                let res = !b;
                                bool_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::Cast {
                        dest,
                        value,
                        from_ty,
                        to_ty,
                    } => {
                        // A cast of a known constant folds to the target family's
                        // constant. Integer casts preserve the i64 bit pattern in
                        // Datara's universal Int64 carrier; float->int truncates;
                        // int->float widens.
                        let is_f = |t: &str| t == "Float" || t == "Float32" || t == "Dec64";
                        let is_i = |t: &str| {
                            matches!(
                                t,
                                "Int" | "Int8" | "Int16" | "Int32" | "Int64" | "UInt"
                                    | "UInt8" | "UInt16" | "UInt32" | "UInt64" | "Bool"
                                    | "Char"
                            )
                        };
                        if is_f(from_ty) && is_i(to_ty) {
                            if let Some(f) = float_constants.get(value).copied() {
                                int_constants.insert(*dest, f as i64);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: f as i64,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        } else if is_i(from_ty) && is_f(to_ty) {
                            if let Some(i) = int_constants.get(value).copied() {
                                let res = i as f64;
                                float_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        } else if is_i(from_ty) && is_i(to_ty) {
                            if let Some(i) = int_constants.get(value).copied() {
                                int_constants.insert(*dest, i);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: i,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::BinOp {
                        dest,
                        op,
                        left,
                        right,
                        ty,
                    } => {
                        if let (Some(l_val), Some(r_val)) =
                            (int_constants.get(left), int_constants.get(right))
                        {
                            // Shift amounts are validated here as well: DMIR
                            // constants can reach this point through variables
                            // (`mut s = 64; x << s`), not only through the
                            // literals already rejected by the type checker.
                            // Fail closed: an out-of-range constant shift has
                            // no defined result, so reject the compilation.
                            // v1.4.5 W1: shift-count validation follows the
                            // operand width (`ty`), not hard-coded 64.
                            let shift_w = crate::dmir::ir::dm_repr_shift_width(ty);
                            if matches!(op.as_str(), "<<" | ">>") && (*r_val < 0 || *r_val >= shift_w) {
                                let diag = crate::diagnostics::Diagnostic::error(
                                    crate::diagnostics::ErrorCode::RangeViolation,
                                    format!(
                                        "[E0947] Shift amount {} is out of range for a {}-bit shift ({}): the shift count must be in 0..{}",
                                        r_val, shift_w, ty, shift_w
                                    ),
                                    None,
                                );
                                self.diagnostics.push(diag);
                            }
                            // v1.4.5 W1: narrow types wrap at their own width
                            // (SPEC overflow semantics), so constexpr folding
                            // must not reject the very programs the backend
                            // would run — fold with wrapping at `ty`'s width.
                            // WIDE Int/Int64 is DIFFERENT: default `+ - *` keeps
                            // the runtime overflow trap (SPEC v1 semantics, test
                            // test_spec_integer_default_overflow_traps), so an
                            // overflowing wide constexpr must NOT fold to the
                            // wrapped value — leave the instruction alone and
                            // let the backend emit sadd_overflow/trapnz.
                            let narrow_ty = crate::dmir::ir::dm_repr_is_narrow_int(ty);
                            let sat = crate::dmir::ir::dm_repr_sat_bounds(ty);
                            let wrap_to_width = |v: i64| crate::dmir::ir::dm_wrap_i64_to_repr(ty, v);
                            let folded = match op.as_str() {
                                "+" | "-" | "*" if !narrow_ty => {
                                    let (raw, _wrapped) = match op.as_str() {
                                        "+" => (
                                            l_val.checked_add(*r_val),
                                            l_val.wrapping_add(*r_val),
                                        ),
                                        "-" => (
                                            l_val.checked_sub(*r_val),
                                            l_val.wrapping_sub(*r_val),
                                        ),
                                        _ => (
                                            l_val.checked_mul(*r_val),
                                            l_val.wrapping_mul(*r_val),
                                        ),
                                    };
                                    match raw {
                                        Some(v) => Some(v),
                                        // Overflowing wide constexpr: unfoldable.
                                        // (The backend will trap at runtime, which
                                        // is exactly the SPEC-mandated behavior.)
                                        None => None,
                                    }
                                }
                                "+" | "wrapping_+" => Some(wrap_to_width(l_val.wrapping_add(*r_val))),
                                "-" | "wrapping_-" => Some(wrap_to_width(l_val.wrapping_sub(*r_val))),
                                "*" | "wrapping_*" => Some(wrap_to_width(l_val.wrapping_mul(*r_val))),
                                "saturating_+" => Some(
                                    l_val.saturating_add(*r_val),
                                ),
                                "saturating_-" => Some(
                                    l_val.saturating_sub(*r_val),
                                ),
                                "saturating_*" => Some(
                                    l_val.saturating_mul(*r_val),
                                ),
                                "&" => Some(l_val & r_val),
                                "|" => Some(l_val | r_val),
                                "^" => Some(l_val ^ r_val),
                                // The range guard above reports out-of-range
                                // shift counts; these arms still re-check so
                                // an invalid count falls through to `_ => None`
                                // instead of panicking on a negative shift.
                                "<<" if *r_val >= 0 && *r_val < shift_w => {
                                    Some(wrap_to_width(l_val.wrapping_shl(*r_val as u32)))
                                }
                                ">>" if *r_val >= 0 && *r_val < shift_w => {
                                    Some(l_val >> *r_val)
                                }
                                "/" if *r_val != 0 && !(*l_val == i64::MIN && *r_val == -1) => {
                                    l_val.checked_div(*r_val).map(wrap_to_width)
                                }
                                "%" if *r_val != 0 && !(*l_val == i64::MIN && *r_val == -1) => {
                                    l_val.checked_rem(*r_val).map(wrap_to_width)
                                }
                                _ => None,
                            };
                            // Saturating ops clamp to the width's bounds
                            // instead of wrapping.
                            let folded = if let (true, Some((lo, hi))) = (
                                op.starts_with("saturating_"),
                                sat,
                            ) {
                                folded.map(|v| v.clamp(lo, hi))
                            } else {
                                folded
                            };
                            if let Some(res) = folded {
                                int_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }

                            let bool_folded = match op.as_str() {
                                "<" => Some(l_val < r_val),
                                "<=" => Some(l_val <= r_val),
                                ">" => Some(l_val > r_val),
                                ">=" => Some(l_val >= r_val),
                                "==" => Some(l_val == r_val),
                                "!=" => Some(l_val != r_val),
                                _ => None,
                            };
                            if let Some(res) = bool_folded {
                                bool_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }

                        if let (Some(l_val), Some(r_val)) =
                            (float_constants.get(left), float_constants.get(right))
                        {
                            // v1.4.5 W2: Float32 constexpr folds at f32
                            // precision (each op rounds to f32, matching the
                            // backend's fdemote path). A wide Float fold stays
                            // f64. Otherwise `0.1f32 + 0.2f32` would fold to
                            // the f64 artifact 0.30000000000000004 before the
                            // backend ever sees the operands.
                            let is_f32_fold = ty == "Float32" || ty == "f32";
                            let fold_op =
                                |a: f64, b: f64| -> f64 {
                                    match op.as_str() {
                                        "+" => a + b,
                                        "-" => a - b,
                                        "*" => a * b,
                                        "/" if b != 0.0 => a / b,
                                        _ => f64::NAN,
                                    }
                                };
                            let folded = if is_f32_fold {
                                // Fold at f32: convert, compute, round back.
                                let r = fold_op(*l_val as f32 as f64, *r_val as f32 as f64);
                                if r.is_nan() {
                                    None
                                } else {
                                    Some((r as f32) as f64)
                                }
                            } else {
                                let r = fold_op(*l_val, *r_val);
                                if r.is_nan() {
                                    None
                                } else {
                                    Some(r)
                                }
                            };
                            if let Some(res) = folded {
                                float_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }

                            let bool_folded = match op.as_str() {
                                "<" => Some(l_val < r_val),
                                "<=" => Some(l_val <= r_val),
                                ">" => Some(l_val > r_val),
                                ">=" => Some(l_val >= r_val),
                                "==" => Some(l_val == r_val),
                                "!=" => Some(l_val != r_val),
                                _ => None,
                            };
                            if let Some(res) = bool_folded {
                                bool_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }

                        if ty == "Float" || ty == "f64" {
                            if op == "-" {
                                if let Some(c2) = float_constants.get(right).copied() {
                                    if let Some((prev_op, a, c1_vid)) =
                                        float_binops.get(left).cloned()
                                    {
                                        if prev_op == "+" {
                                            if let Some(c1) = float_constants.get(&c1_vid).copied()
                                            {
                                                let new_c = c1 - c2;
                                                let c_vid = ValueId(fresh_vid);
                                                fresh_vid += 1;
                                                float_constants.insert(c_vid, new_c);
                                                new_instructions.push(Inst::ConstFloat {
                                                    dest: c_vid,
                                                    value: new_c,
                                                });
                                                new_instructions.push(Inst::BinOp {
                                                    dest: *dest,
                                                    op: "+".to_string(),
                                                    left: a,
                                                    right: c_vid,
                                                    ty: ty.clone(),
                                                });
                                                float_binops
                                                    .insert(*dest, ("+".to_string(), a, c_vid));
                                                self.report.constants_folded += 1;
                                                changed = true;
                                                continue;
                                            }
                                        }
                                    }
                                }
                            } else if op == "/" {
                                if let Some(c) = float_constants.get(right).copied() {
                                    if c != 0.0 && c.is_finite() && c != 1.0 {
                                        let recip = 1.0 / c;
                                        let recip_vid = ValueId(fresh_vid);
                                        fresh_vid += 1;
                                        float_constants.insert(recip_vid, recip);
                                        new_instructions.push(Inst::ConstFloat {
                                            dest: recip_vid,
                                            value: recip,
                                        });
                                        new_instructions.push(Inst::BinOp {
                                            dest: *dest,
                                            op: "*".to_string(),
                                            left: *left,
                                            right: recip_vid,
                                            ty: ty.clone(),
                                        });
                                        float_binops
                                            .insert(*dest, ("*".to_string(), *left, recip_vid));
                                        self.report.constants_folded += 1;
                                        changed = true;
                                        continue;
                                    }
                                }
                            }
                            float_binops.insert(*dest, (op.clone(), *left, *right));
                        }

                        if let (Some(l_val), Some(r_val)) =
                            (bool_constants.get(left), bool_constants.get(right))
                        {
                            let bool_folded = match op.as_str() {
                                "==" => Some(l_val == r_val),
                                "!=" => Some(l_val != r_val),
                                "&&" | "and" => Some(*l_val && *r_val),
                                "||" | "or" => Some(*l_val || *r_val),
                                _ => None,
                            };
                            if let Some(res) = bool_folded {
                                bool_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }

                        if let (Some(l_val), Some(r_val)) =
                            (str_constants.get(left), str_constants.get(right))
                        {
                            let bool_folded = match op.as_str() {
                                "==" => Some(l_val == r_val),
                                "!=" => Some(l_val != r_val),
                                _ => None,
                            };
                            if let Some(res) = bool_folded {
                                bool_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::Decide {
                        dest,
                        arms,
                        else_val,
                        ty: _,
                    } => {
                        let mut resolved: Option<ValueId> = None;
                        let mut can_resolve = true;
                        for (cond, val) in arms {
                            if let Some(b) = bool_constants.get(cond) {
                                if *b {
                                    resolved = Some(*val);
                                    break;
                                }
                            } else if let Some(i) = int_constants.get(cond) {
                                if *i != 0 {
                                    resolved = Some(*val);
                                    break;
                                }
                            } else {
                                can_resolve = false;
                                break;
                            }
                        }
                        if resolved.is_none() && can_resolve {
                            resolved = *else_val;
                        }

                        if let Some(res_vid) = resolved {
                            if let Some(ival) = int_constants.get(&res_vid).copied() {
                                int_constants.insert(*dest, ival);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: ival,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(fval) = float_constants.get(&res_vid).copied() {
                                float_constants.insert(*dest, fval);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: fval,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(sval) = str_constants.get(&res_vid).cloned() {
                                str_constants.insert(*dest, sval.clone());
                                new_instructions.push(Inst::ConstStr {
                                    dest: *dest,
                                    value: sval,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(bval) = bool_constants.get(&res_vid).copied() {
                                bool_constants.insert(*dest, bval);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: bval,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::Select {
                        dest,
                        cond,
                        then_val,
                        else_val,
                        ty: _,
                    } => {
                        if let Some(b) = bool_constants.get(cond) {
                            let chosen = if *b { *then_val } else { *else_val };
                            if let Some(ival) = int_constants.get(&chosen).copied() {
                                int_constants.insert(*dest, ival);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: ival,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(fval) = float_constants.get(&chosen).copied() {
                                float_constants.insert(*dest, fval);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: fval,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(sval) = str_constants.get(&chosen).cloned() {
                                str_constants.insert(*dest, sval.clone());
                                new_instructions.push(Inst::ConstStr {
                                    dest: *dest,
                                    value: sval,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(bval) = bool_constants.get(&chosen).copied() {
                                bool_constants.insert(*dest, bval);
                                new_instructions.push(Inst::ConstBool {
                                    dest: *dest,
                                    value: bval,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }
                        new_instructions.push(inst.clone());
                    }                        Inst::FormatStr {
                            dest,
                            parts,
                            values,
                            value_tys: _,
                        } => {
                        let all_known = values.iter().all(|v| {
                            int_constants.contains_key(v)
                                || float_constants.contains_key(v)
                                || str_constants.contains_key(v)
                                || bool_constants.contains_key(v)
                        });
                        if all_known {
                            let mut res_str = String::new();
                            for (i, p) in parts.iter().enumerate() {
                                res_str.push_str(p);
                                if i < values.len() {
                                    let v_id = &values[i];
                                    if let Some(c) = int_constants.get(v_id) {
                                        res_str.push_str(&c.to_string());
                                    } else if let Some(c) = float_constants.get(v_id) {
                                        res_str.push_str(&c.to_string());
                                    } else if let Some(c) = str_constants.get(v_id) {
                                        res_str.push_str(c);
                                    } else if let Some(c) = bool_constants.get(v_id) {
                                        res_str.push_str(if *c { "true" } else { "false" });
                                    }
                                }
                            }
                            str_constants.insert(*dest, res_str.clone());
                            new_instructions.push(Inst::ConstStr {
                                dest: *dest,
                                value: res_str,
                            });
                            self.report.constants_folded += 1;
                            changed = true;
                            continue;
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::Call {
                        dest,
                        func,
                        args,
                        ty: _,
                    } => {
                        let int_args: Vec<Option<i64>> =
                            args.iter().map(|a| int_constants.get(a).copied()).collect();
                        let all_int_consts =
                            !int_args.is_empty() && int_args.iter().all(|a| a.is_some());

                        if all_int_consts {
                            let vals: Vec<i64> = int_args.into_iter().map(|a| a.unwrap()).collect();
                            let folded: Option<i64> = match func.as_str() {
                                "abs" | "math_abs" if vals.len() == 1 => vals[0].checked_abs(),
                                "min" if vals.len() == 2 => Some(vals[0].min(vals[1])),
                                "max" if vals.len() == 2 => Some(vals[0].max(vals[1])),
                                _ => None,
                            };

                            if let Some(res) = folded {
                                int_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }

                        let float_args: Vec<Option<f64>> = args
                            .iter()
                            .map(|a| float_constants.get(a).copied())
                            .collect();
                        let all_float_consts =
                            !float_args.is_empty() && float_args.iter().all(|a| a.is_some());

                        if all_float_consts {
                            let vals: Vec<f64> =
                                float_args.into_iter().map(|a| a.unwrap()).collect();
                            let folded: Option<f64> = match func.as_str() {
                                "abs" | "math_abs" if vals.len() == 1 => Some(vals[0].abs()),
                                "sqrt" | "math_sqrt" if vals.len() == 1 && vals[0] >= 0.0 => {
                                    Some(vals[0].sqrt())
                                }
                                "sin" | "math_sin" if vals.len() == 1 => Some(vals[0].sin()),
                                "cos" | "math_cos" if vals.len() == 1 => Some(vals[0].cos()),
                                "tan" | "math_tan" if vals.len() == 1 => Some(vals[0].tan()),
                                "floor" | "math_floor" if vals.len() == 1 => Some(vals[0].floor()),
                                "ceil" | "math_ceil" if vals.len() == 1 => Some(vals[0].ceil()),
                                "round" | "math_round" if vals.len() == 1 => Some(vals[0].round()),
                                "min" | "math_min" if vals.len() == 2 => Some(vals[0].min(vals[1])),
                                "max" | "math_max" if vals.len() == 2 => Some(vals[0].max(vals[1])),
                                _ => None,
                            };

                            if let Some(res) = folded {
                                float_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    Inst::MethodCall {
                        dest,
                        object,
                        method,
                        args,
                        ty: _,
                    } if (method == "to_float" || method == "to_int") && args.is_empty() => {
                        // Numeric conversion intrinsics (Gate 7): a constant
                        // receiver folds to a constant result instead of a
                        // runtime conversion. `to_int` folds only when the
                        // float is finite and inside the i64 range, matching
                        // the in-range fptosi the hardware path executes.
                        if method == "to_float" {
                            if let Some(i) = int_constants.get(object).copied() {
                                let res = i as f64;
                                float_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            } else if let Some(f) = float_constants.get(object).copied() {
                                float_constants.insert(*dest, f);
                                new_instructions.push(Inst::ConstFloat {
                                    dest: *dest,
                                    value: f,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        } else if let Some(i) = int_constants.get(object).copied() {
                            int_constants.insert(*dest, i);
                            new_instructions.push(Inst::ConstInt {
                                dest: *dest,
                                value: i,
                            });
                            self.report.constants_folded += 1;
                            changed = true;
                            continue;
                        } else if let Some(f) = float_constants.get(object).copied() {
                            let in_i64_range = f.is_finite()
                                && (-9223372036854775808.0..9223372036854775808.0).contains(&f);
                            if in_i64_range {
                                let res = f as i64;
                                int_constants.insert(*dest, res);
                                new_instructions.push(Inst::ConstInt {
                                    dest: *dest,
                                    value: res,
                                });
                                self.report.constants_folded += 1;
                                changed = true;
                                continue;
                            }
                        }
                        new_instructions.push(inst.clone());
                    }
                    _ => {
                        new_instructions.push(inst.clone());
                    }
                }
            }

            block.instructions = new_instructions;
        }

        changed
    }
}
