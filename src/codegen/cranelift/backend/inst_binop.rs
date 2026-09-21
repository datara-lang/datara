use crate::dmir::ValueId;
use cranelift_codegen::ir::{InstBuilder, types as clif_types};
use cranelift_module::Module as ClifModule;

use super::types::FunctionCompileCtx;

pub fn compile_binop<M: ClifModule>(
    ctx: &mut FunctionCompileCtx<'_, '_, M>,
    dest: &ValueId,
    op: &str,
    left: &ValueId,
    right: &ValueId,
    ty: &str,
) -> Result<(), String> {
    // v1.4.5 W1: `ty` carries the exact operand type name (Int8, UInt32,
    // Float32, ...). Narrow-width arithmetic is done in I64 (sign- or
    // zero-extended per signedness) and the RESULT is then re-wrapped to
    // the operand width (wrapping semantics; checked overflow is a
    // typechecker concern — see types/check/binary.rs).
    let repr = ty;
    let is_u32 = repr == "UInt32";
    let is_u16 = repr == "UInt16";
    let is_u8 = repr == "UInt8";
    let is_u64 = repr == "UInt" || repr == "UInt64" || repr == "u64" || repr == "usize";
    let is_unsigned = is_u32 || is_u16 || is_u8 || is_u64;
    let _is_i8 = repr == "Int8" || repr == "i8";
    let _is_i16 = repr == "Int16" || repr == "i16";
    let _is_i32 = repr == "Int32" || repr == "i32";
    let is_f32 = repr == "Float32" || repr == "f32";

    let raw_lv = ctx
        .val_map
        .get(left)
        .copied()
        .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
    let raw_rv = ctx
        .val_map
        .get(right)
        .copied()
        .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
    let lv_ty = ctx.builder.func.dfg.value_type(raw_lv);
    let rv_ty = ctx.builder.func.dfg.value_type(raw_rv);
    let left_is_str = ctx.string_vids.contains(left);
    let right_is_str = ctx.string_vids.contains(right);
    let is_string =
        op == "+" && (left_is_str || right_is_str || ty == "String" || ty.contains("Str"));

    if is_string {
        let conv_ref = ctx
            .module
            .declare_func_in_func(ctx.runtime.rt_int_to_str_id, ctx.builder.func);
        let bool_to_str_ref = ctx
            .module
            .declare_func_in_func(ctx.runtime.rt_bool_to_str_id, ctx.builder.func);
        let flt_to_str_ref = ctx
            .module
            .declare_func_in_func(ctx.runtime.rt_flt_to_str_id, ctx.builder.func);
        let l_str = if left_is_str {
            raw_lv
        } else if ctx.bool_vids.contains(left) {
            let call = ctx.builder.ins().call(bool_to_str_ref, &[raw_lv]);
            ctx.builder.inst_results(call)[0]
        } else if lv_ty == clif_types::F64 {
            let call = ctx.builder.ins().call(flt_to_str_ref, &[raw_lv]);
            ctx.builder.inst_results(call)[0]
        } else {
            let call = ctx.builder.ins().call(conv_ref, &[raw_lv]);
            ctx.builder.inst_results(call)[0]
        };
        let r_str = if right_is_str {
            raw_rv
        } else if ctx.bool_vids.contains(right) {
            let call = ctx.builder.ins().call(bool_to_str_ref, &[raw_rv]);
            ctx.builder.inst_results(call)[0]
        } else if rv_ty == clif_types::F64 {
            let call = ctx.builder.ins().call(flt_to_str_ref, &[raw_rv]);
            ctx.builder.inst_results(call)[0]
        } else {
            let call = ctx.builder.ins().call(conv_ref, &[raw_rv]);
            ctx.builder.inst_results(call)[0]
        };
        let fn_ref = ctx
            .module
            .declare_func_in_func(ctx.runtime.rt_concat_id, ctx.builder.func);
        let call_inst = ctx.builder.ins().call(fn_ref, &[l_str, r_str]);
        let res = ctx.builder.inst_results(call_inst)[0];
        ctx.val_map.insert(*dest, res);
        ctx.string_vids.insert(*dest);
        return Ok(());
    }

    let is_float =
        lv_ty == clif_types::F64 || rv_ty == clif_types::F64 || is_f32;

    let (lv, rv) = if is_float {
        if is_f32 {
            // v1.4.5 W2: true Float32 arithmetic. Operands may arrive as F64
            // (literal ConstFloat slots or widened loads); demote them to F32
            // so the math runs at f32 precision (0.1f + 0.2f == 0.3f, not
            // 0.30000000000000004). Demotion is the assignment conversion a
            // Float32 local would have applied anyway.
            let flv = if lv_ty == clif_types::F64 {
                ctx.builder.ins().fdemote(clif_types::F32, raw_lv)
            } else {
                raw_lv
            };
            let frv = if rv_ty == clif_types::F64 {
                ctx.builder.ins().fdemote(clif_types::F32, raw_rv)
            } else {
                raw_rv
            };
            (flv, frv)
        } else {
            let flv = if lv_ty == clif_types::I64 {
                ctx.builder.ins().fcvt_from_sint(clif_types::F64, raw_lv)
            } else {
                raw_lv
            };
            let frv = if rv_ty == clif_types::I64 {
                ctx.builder.ins().fcvt_from_sint(clif_types::F64, raw_rv)
            } else {
                raw_rv
            };
            (flv, frv)
        }
    } else {
        // Narrow/unsigned integers live in I64 slots, stored as the
        // canonical representation chosen at load sites: signed narrow ->
        // sign-extended, unsigned narrow -> zero-extended. Here we only
        // normalise an operand that arrived with a narrower Cranelift
        // type (e.g. I8 from a VolatileLoad that did not widen).
        let ilv = if lv_ty != clif_types::I64 {
            match (lv_ty, is_unsigned) {
                (clif_types::I8, true) => ctx.builder.ins().uextend(clif_types::I64, raw_lv),
                (clif_types::I8, false) => ctx.builder.ins().sextend(clif_types::I64, raw_lv),
                (clif_types::I16, true) => ctx.builder.ins().uextend(clif_types::I64, raw_lv),
                (clif_types::I16, false) => ctx.builder.ins().sextend(clif_types::I64, raw_lv),
                (clif_types::I32, true) => ctx.builder.ins().uextend(clif_types::I64, raw_lv),
                (clif_types::I32, false) => ctx.builder.ins().sextend(clif_types::I64, raw_lv),
                _ => raw_lv,
            }
        } else {
            raw_lv
        };
        let irv = if rv_ty != clif_types::I64 {
            match (rv_ty, is_unsigned) {
                (clif_types::I8, true) => ctx.builder.ins().uextend(clif_types::I64, raw_rv),
                (clif_types::I8, false) => ctx.builder.ins().sextend(clif_types::I64, raw_rv),
                (clif_types::I16, true) => ctx.builder.ins().uextend(clif_types::I64, raw_rv),
                (clif_types::I16, false) => ctx.builder.ins().sextend(clif_types::I64, raw_rv),
                (clif_types::I32, true) => ctx.builder.ins().uextend(clif_types::I64, raw_rv),
                (clif_types::I32, false) => ctx.builder.ins().sextend(clif_types::I64, raw_rv),
                _ => raw_rv,
            }
        } else {
            raw_rv
        };
        (ilv, irv)
    };

    let c_left = ctx
        .const_float_map
        .get(left)
        .copied()
        .or_else(|| ctx.const_int_map.get(left).map(|&v| v as f64));
    let c_right = ctx
        .const_float_map
        .get(right)
        .copied()
        .or_else(|| ctx.const_int_map.get(right).map(|&v| v as f64));

    let res = if is_float {
        if let (Some(l_val), Some(r_val)) = (c_left, c_right) {
            // v1.4.5 W2: constexpr float folding must match the operand
            // width — Float32 folds at f32 (round after each op), wide
            // Float stays f64. Folding wide first demote-later produced
            // f64 artifacts like 0.30000000000000004 for 0.1f + 0.2f.
            let folded = if is_f32 {
                let l32 = l_val as f32;
                let r32 = r_val as f32;
                let r = match op {
                    "+" => l32 + r32,
                    "-" => l32 - r32,
                    "*" => l32 * r32,
                    "/" if r32 != 0.0 => l32 / r32,
                    _ => f32::NAN,
                };
                if r.is_nan() { None } else { Some(r as f64) }
            } else {
                match op {
                    "+" => Some(l_val + r_val),
                    "-" => Some(l_val - r_val),
                    "*" => Some(l_val * r_val),
                    "/" if r_val != 0.0 => Some(l_val / r_val),
                    _ => None,
                }
            };
            if let Some(val) = folded {
                ctx.const_float_map.insert(*dest, val);
            }
        }

        match op {
            "+" => {
                if let Some(c) = c_right {
                    if c == 0.0 {
                        lv
                    } else {
                        ctx.builder.ins().fadd(lv, rv)
                    }
                } else if let Some(c) = c_left {
                    if c == 0.0 {
                        rv
                    } else {
                        ctx.builder.ins().fadd(lv, rv)
                    }
                } else {
                    ctx.builder.ins().fadd(lv, rv)
                }
            }
            "-" => {
                if let Some(c) = c_right {
                    if c == 0.0 {
                        lv
                    } else {
                        ctx.builder.ins().fsub(lv, rv)
                    }
                } else if let Some(c) = c_left {
                    if c == 0.0 {
                        ctx.builder.ins().fneg(rv)
                    } else {
                        ctx.builder.ins().fsub(lv, rv)
                    }
                } else {
                    ctx.builder.ins().fsub(lv, rv)
                }
            }
            "*" => {
                if let Some(c) = c_right {
                    if c == 1.0 {
                        lv
                    } else if c == 2.0 {
                        ctx.builder.ins().fadd(lv, lv)
                    } else {
                        ctx.builder.ins().fmul(lv, rv)
                    }
                } else if let Some(c) = c_left {
                    if c == 1.0 {
                        rv
                    } else if c == 2.0 {
                        ctx.builder.ins().fadd(rv, rv)
                    } else {
                        ctx.builder.ins().fmul(lv, rv)
                    }
                } else {
                    ctx.builder.ins().fmul(lv, rv)
                }
            }
            "/" => {
                if lv_ty == clif_types::F32X4 || rv_ty == clif_types::F32X4 {
                    let v1 = super::simd::ensure_f32x4(ctx, raw_lv);
                    let v2 = super::simd::ensure_f32x4(ctx, raw_rv);
                    let e0_a = ctx.builder.ins().extractlane(v1, 0);
                    let e0_b = ctx.builder.ins().extractlane(v2, 0);
                    let d0 = ctx.builder.ins().fdiv(e0_a, e0_b);

                    let e1_a = ctx.builder.ins().extractlane(v1, 1);
                    let e1_b = ctx.builder.ins().extractlane(v2, 1);
                    let d1 = ctx.builder.ins().fdiv(e1_a, e1_b);

                    let e2_a = ctx.builder.ins().extractlane(v1, 2);
                    let e2_b = ctx.builder.ins().extractlane(v2, 2);
                    let d2 = ctx.builder.ins().fdiv(e2_a, e2_b);

                    let e3_a = ctx.builder.ins().extractlane(v1, 3);
                    let e3_b = ctx.builder.ins().extractlane(v2, 3);
                    let d3 = ctx.builder.ins().fdiv(e3_a, e3_b);

                    let mut v = ctx.builder.ins().splat(clif_types::F32X4, d0);
                    v = ctx.builder.ins().insertlane(v, d1, 1);
                    v = ctx.builder.ins().insertlane(v, d2, 2);
                    ctx.builder.ins().insertlane(v, d3, 3)
                } else if let Some(c) = c_right {
                    if c == 1.0 {
                        lv
                    } else {
                        ctx.builder.ins().fdiv(lv, rv)
                    }
                } else {
                    ctx.builder.ins().fdiv(lv, rv)
                }
            }
            "<" => {
                let c = ctx.builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::LessThan,
                    lv,
                    rv,
                );
                ctx.builder.ins().uextend(clif_types::I64, c)
            }
            "<=" => {
                let c = ctx.builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
                    lv,
                    rv,
                );
                ctx.builder.ins().uextend(clif_types::I64, c)
            }
            ">" => {
                let c = ctx.builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
                    lv,
                    rv,
                );
                ctx.builder.ins().uextend(clif_types::I64, c)
            }
            ">=" => {
                let c = ctx.builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
                    lv,
                    rv,
                );
                ctx.builder.ins().uextend(clif_types::I64, c)
            }
            "==" => {
                let c = ctx.builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::Equal,
                    lv,
                    rv,
                );
                ctx.builder.ins().uextend(clif_types::I64, c)
            }
            "!=" => {
                let c = ctx.builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::NotEqual,
                    lv,
                    rv,
                );
                ctx.builder.ins().uextend(clif_types::I64, c)
            }
            // Used to be a silent `fadd` fallback, which
            // turned any unrecognised operator into
            // addition and produced wrong answers with
            // no diagnostic. Fail loudly instead.
            other => {
                return Err(format!(
                    "No Cranelift lowering for float operator '{}'. \
                                         Refusing to fall back to 'fadd' and silently \
                                         compute the wrong result.",
                    other
                ));
            }
        }
    } else {
        // v1.3.4 tiered overflow-check strategy: the optimizer's
        // `ovf_elide` pass marks the dest of an induction-variable
        // increment whose trip range is statically proven to keep the
        // result inside i64 (see optimizer/loops/ovf_elide.rs). For exactly
        // those instructions the `trapnz(INTEGER_OVERFLOW)` gate is
        // dropped; every other integer op — in particular all user body
        // arithmetic — keeps its checked lowering. `wrapping_`/`saturating_`
        // forms are unaffected (they never trapped in the first place).
        let proven_safe = ctx.current_func.proven_no_overflow.contains(dest);
        match op {
            "+" => {
                if proven_safe {
                    ctx.builder.ins().iadd(lv, rv)
                } else {
                    let (res, ovf) = ctx.builder.ins().sadd_overflow(lv, rv);
                    ctx.builder
                        .ins()
                        .trapnz(ovf, cranelift_codegen::ir::TrapCode::INTEGER_OVERFLOW);
                    res
                }
            }
            "-" => {
                if proven_safe {
                    ctx.builder.ins().isub(lv, rv)
                } else {
                    let (res, ovf) = ctx.builder.ins().ssub_overflow(lv, rv);
                    ctx.builder
                        .ins()
                        .trapnz(ovf, cranelift_codegen::ir::TrapCode::INTEGER_OVERFLOW);
                    res
                }
            }
            "*" => {
                if proven_safe {
                    ctx.builder.ins().imul(lv, rv)
                } else {
                    let (res, ovf) = ctx.builder.ins().smul_overflow(lv, rv);
                    ctx.builder
                        .ins()
                        .trapnz(ovf, cranelift_codegen::ir::TrapCode::INTEGER_OVERFLOW);
                    res
                }
            }
            "wrapping_+" => ctx.builder.ins().iadd(lv, rv),
            "wrapping_-" => ctx.builder.ins().isub(lv, rv),
            "wrapping_*" => {
                let const_mult = ctx
                    .const_int_map
                    .get(right)
                    .copied()
                    .or_else(|| ctx.const_int_map.get(left).copied());
                let var_val = if ctx.const_int_map.contains_key(right) {
                    lv
                } else {
                    rv
                };
                if let Some(c) = const_mult {
                    if c == 0 {
                        ctx.builder.ins().iconst(clif_types::I64, 0)
                    } else if c == 1 {
                        var_val
                    } else if c == 2 {
                        let shift = ctx.builder.ins().iconst(clif_types::I64, 1);
                        ctx.builder.ins().ishl(var_val, shift)
                    } else if c == 3 {
                        let shift = ctx.builder.ins().iconst(clif_types::I64, 1);
                        let shifted = ctx.builder.ins().ishl(var_val, shift);
                        ctx.builder.ins().iadd(shifted, var_val)
                    } else if c == 4 {
                        let shift = ctx.builder.ins().iconst(clif_types::I64, 2);
                        ctx.builder.ins().ishl(var_val, shift)
                    } else if c == 5 {
                        let shift = ctx.builder.ins().iconst(clif_types::I64, 2);
                        let shifted = ctx.builder.ins().ishl(var_val, shift);
                        ctx.builder.ins().iadd(shifted, var_val)
                    } else if c == 8 {
                        let shift = ctx.builder.ins().iconst(clif_types::I64, 3);
                        ctx.builder.ins().ishl(var_val, shift)
                    } else if c == 9 {
                        let shift = ctx.builder.ins().iconst(clif_types::I64, 3);
                        let shifted = ctx.builder.ins().ishl(var_val, shift);
                        ctx.builder.ins().iadd(shifted, var_val)
                    } else if c > 0 && (c & (c - 1)) == 0 {
                        let shift = ctx
                            .builder
                            .ins()
                            .iconst(clif_types::I64, c.trailing_zeros() as i64);
                        ctx.builder.ins().ishl(var_val, shift)
                    } else {
                        ctx.builder.ins().imul(lv, rv)
                    }
                } else {
                    ctx.builder.ins().imul(lv, rv)
                }
            }
            "saturating_+" => {
                let (res, ovf) = ctx.builder.ins().sadd_overflow(lv, rv);
                let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                let is_pos = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    lv,
                    zero,
                );
                let max_val = ctx.builder.ins().iconst(clif_types::I64, i64::MAX);
                let min_val = ctx.builder.ins().iconst(clif_types::I64, i64::MIN);
                let sat_val = ctx.builder.ins().select(is_pos, max_val, min_val);
                ctx.builder.ins().select(ovf, sat_val, res)
            }
            "saturating_-" => {
                let (res, ovf) = ctx.builder.ins().ssub_overflow(lv, rv);
                let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                let is_pos = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    lv,
                    zero,
                );
                let max_val = ctx.builder.ins().iconst(clif_types::I64, i64::MAX);
                let min_val = ctx.builder.ins().iconst(clif_types::I64, i64::MIN);
                let sat_val = ctx.builder.ins().select(is_pos, max_val, min_val);
                ctx.builder.ins().select(ovf, sat_val, res)
            }
            "saturating_*" => {
                let (res, ovf) = ctx.builder.ins().smul_overflow(lv, rv);
                let xor_val = ctx.builder.ins().bxor(lv, rv);
                let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                let is_pos = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    xor_val,
                    zero,
                );
                let max_val = ctx.builder.ins().iconst(clif_types::I64, i64::MAX);
                let min_val = ctx.builder.ins().iconst(clif_types::I64, i64::MIN);
                let sat_val = ctx.builder.ins().select(is_pos, max_val, min_val);
                ctx.builder.ins().select(ovf, sat_val, res)
            }
            "/" => {
                let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                let is_zero = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    rv,
                    zero,
                );
                ctx.builder.ins().trapnz(
                    is_zero,
                    cranelift_codegen::ir::TrapCode::INTEGER_DIVISION_BY_ZERO,
                );

                let min_val = ctx.builder.ins().iconst(clif_types::I64, i64::MIN);
                let is_min = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    lv,
                    min_val,
                );
                let neg_one = ctx.builder.ins().iconst(clif_types::I64, -1);
                let is_neg_one = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    rv,
                    neg_one,
                );
                let is_ovf = ctx.builder.ins().band(is_min, is_neg_one);
                ctx.builder
                    .ins()
                    .trapnz(is_ovf, cranelift_codegen::ir::TrapCode::INTEGER_OVERFLOW);

                if let Some(&c) = ctx.const_int_map.get(right) {
                    if c > 1 && (c & (c - 1)) == 0 && c <= (1 << 62) {
                        // Signed truncating division by 2^k:
                        let k = c.trailing_zeros();
                        let sign = ctx.builder.ins().sshr_imm_s(lv, 63);
                        let bias = ctx.builder.ins().ushr_imm_u(sign, (64 - k) as i64);
                        let sum = ctx.builder.ins().iadd(lv, bias);
                        ctx.builder.ins().sshr_imm_s(sum, k as i64)
                    } else {
                        ctx.builder.ins().sdiv(lv, rv)
                    }
                } else {
                    ctx.builder.ins().sdiv(lv, rv)
                }
            }
            "%" => {
                let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                let is_zero = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    rv,
                    zero,
                );
                ctx.builder.ins().trapnz(
                    is_zero,
                    cranelift_codegen::ir::TrapCode::INTEGER_DIVISION_BY_ZERO,
                );

                let min_val = ctx.builder.ins().iconst(clif_types::I64, i64::MIN);
                let is_min = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    lv,
                    min_val,
                );
                let neg_one = ctx.builder.ins().iconst(clif_types::I64, -1);
                let is_neg_one = ctx.builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    rv,
                    neg_one,
                );
                let is_ovf = ctx.builder.ins().band(is_min, is_neg_one);
                ctx.builder
                    .ins()
                    .trapnz(is_ovf, cranelift_codegen::ir::TrapCode::INTEGER_OVERFLOW);

                if let Some(&c) = ctx.const_int_map.get(right) {
                    if c > 1 && (c & (c - 1)) == 0 && c <= (1 << 62) {
                        let k = c.trailing_zeros();
                        let sign = ctx.builder.ins().sshr_imm_s(lv, 63);
                        let bias = ctx.builder.ins().ushr_imm_u(sign, (64 - k) as i64);
                        let sum = ctx.builder.ins().iadd(lv, bias);
                        let q = ctx.builder.ins().sshr_imm_s(sum, k as i64);
                        let scaled = ctx.builder.ins().ishl_imm_s(q, k as i64);
                        ctx.builder.ins().isub(lv, scaled)
                    } else {
                        ctx.builder.ins().srem(lv, rv)
                    }
                } else {
                    ctx.builder.ins().srem(lv, rv)
                }
            }
            "<" => {
                if ctx.string_vids.contains(left)
                    || ctx.string_vids.contains(right)
                    || ty == "String"
                    || ty.contains("Str")
                {
                    let cmp_ref = ctx
                        .module
                        .declare_func_in_func(ctx.runtime.rt_str_cmp_id, ctx.builder.func);
                    let call_inst = ctx.builder.ins().call(cmp_ref, &[lv, rv]);
                    let cmp_res = ctx.builder.inst_results(call_inst)[0];
                    let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                        cmp_res,
                        zero,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                } else {
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                        lv,
                        rv,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                }
            }
            "<=" => {
                if ctx.string_vids.contains(left)
                    || ctx.string_vids.contains(right)
                    || ty == "String"
                    || ty.contains("Str")
                {
                    let cmp_ref = ctx
                        .module
                        .declare_func_in_func(ctx.runtime.rt_str_cmp_id, ctx.builder.func);
                    let call_inst = ctx.builder.ins().call(cmp_ref, &[lv, rv]);
                    let cmp_res = ctx.builder.inst_results(call_inst)[0];
                    let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedLessThanOrEqual,
                        cmp_res,
                        zero,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                } else {
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedLessThanOrEqual,
                        lv,
                        rv,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                }
            }
            ">" => {
                if ctx.string_vids.contains(left)
                    || ctx.string_vids.contains(right)
                    || ty == "String"
                    || ty.contains("Str")
                {
                    let cmp_ref = ctx
                        .module
                        .declare_func_in_func(ctx.runtime.rt_str_cmp_id, ctx.builder.func);
                    let call_inst = ctx.builder.ins().call(cmp_ref, &[lv, rv]);
                    let cmp_res = ctx.builder.inst_results(call_inst)[0];
                    let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThan,
                        cmp_res,
                        zero,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                } else {
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThan,
                        lv,
                        rv,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                }
            }
            ">=" => {
                if ctx.string_vids.contains(left)
                    || ctx.string_vids.contains(right)
                    || ty == "String"
                    || ty.contains("Str")
                {
                    let cmp_ref = ctx
                        .module
                        .declare_func_in_func(ctx.runtime.rt_str_cmp_id, ctx.builder.func);
                    let call_inst = ctx.builder.ins().call(cmp_ref, &[lv, rv]);
                    let cmp_res = ctx.builder.inst_results(call_inst)[0];
                    let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                        cmp_res,
                        zero,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                } else {
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                        lv,
                        rv,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                }
            }
            "==" => {
                if ctx.string_vids.contains(left)
                    || ctx.string_vids.contains(right)
                    || ty == "String"
                    || ty.contains("Str")
                {
                    let eq_ref = ctx
                        .module
                        .declare_func_in_func(ctx.runtime.rt_str_eq_id, ctx.builder.func);
                    let call_inst = ctx.builder.ins().call(eq_ref, &[lv, rv]);
                    ctx.builder.inst_results(call_inst)[0]
                } else {
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::Equal,
                        lv,
                        rv,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                }
            }
            "!=" => {
                if ctx.string_vids.contains(left)
                    || ctx.string_vids.contains(right)
                    || ty == "String"
                    || ty.contains("Str")
                {
                    let eq_ref = ctx
                        .module
                        .declare_func_in_func(ctx.runtime.rt_str_eq_id, ctx.builder.func);
                    let call_inst = ctx.builder.ins().call(eq_ref, &[lv, rv]);
                    let res = ctx.builder.inst_results(call_inst)[0];
                    let one = ctx.builder.ins().iconst(clif_types::I64, 1);
                    ctx.builder.ins().bxor(res, one)
                } else {
                    let c = ctx.builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                        lv,
                        rv,
                    );
                    ctx.builder.ins().uextend(clif_types::I64, c)
                }
            }
            "&" | "&&" => ctx.builder.ins().band(lv, rv),
            "|" | "||" => ctx.builder.ins().bor(lv, rv),
            "^" => ctx.builder.ins().bxor(lv, rv),
            // Shift counts are masked to the operand width (Int8 -> 0..7,
            // Int32 -> 0..31, Int/Int64 -> 0..63): Cranelift leaves ishl/sshr
            // results undefined for out-of-range counts, and the mask makes
            // the runtime behavior deterministic and identical to the x86
            // native semantics and the WASM i64.shl / i64.shr_s rules.
            // v1.4.5 W1: the mask width follows the operand type from `ty`.
            "<<" => {
                let shift_mask: i64 = match repr {
                    "Int8" | "i8" | "UInt8" | "Byte" | "u8" => 7,
                    "Int16" | "i16" | "UInt16" | "u16" => 15,
                    "Int32" | "i32" | "UInt32" | "u32" => 31,
                    _ => 63,
                };
                let mask = ctx.builder.ins().iconst(clif_types::I64, shift_mask);
                let count = ctx.builder.ins().band(rv, mask);
                ctx.builder.ins().ishl(lv, count)
            }
            ">>" => {
                let shift_mask: i64 = match repr {
                    "Int8" | "i8" | "UInt8" | "Byte" | "u8" => 7,
                    "Int16" | "i16" | "UInt16" | "u16" => 15,
                    "Int32" | "i32" | "UInt32" | "u32" => 31,
                    _ => 63,
                };
                let mask = ctx.builder.ins().iconst(clif_types::I64, shift_mask);
                let count = ctx.builder.ins().band(rv, mask);
                if is_unsigned {
                    // Logical right shift for unsigned types.
                    let is_neg = ctx.builder.ins().icmp_imm_s(
                        cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                        lv,
                        0,
                    );
                    let shr_s = ctx.builder.ins().sshr(lv, count);
                    let shr_u = ctx.builder.ins().ushr(lv, count);
                    ctx.builder.ins().select(is_neg, shr_u, shr_s)
                } else {
                    ctx.builder.ins().sshr(lv, count)
                }
            }
            // Used to be a silent `iadd` fallback. That is
            // how `a && b` compiled to `a + b`: the operator
            // had no arm here, so it silently became
            // addition. Fail loudly instead.
            other => {
                return Err(format!(
                    "No Cranelift lowering for integer operator '{}'. \
                                         Refusing to fall back to 'iadd' and silently \
                                         compute the wrong result.",
                    other
                ));
            }
        }
    };

    // v1.4.5 W1: wrap arithmetic results back into the operand width.
    // Semantics (fixed in SPEC): release = wrapping, and the typechecker
    // enforces constant overflow at compile time. Integer narrowing uses
    // `ireduce` (truncating wrap); the value stays in an I64 slot as the
    // canonical extension of the narrowed result (sign/zero per type).
    // Float32 results are computed in F32 but stored BACK in an F64 slot
    // (fpromote) — the same canonical-slot discipline narrow ints use with
    // I64. This keeps every Cranelift `Variable` monomorphic: a `mut t:
    // Float32` initialized from an f64 literal and later assigned f32
    // arithmetic must not flip the variable's declared type (panics in
    // cranelift-frontend). The f32-ness lives in the DMIR `ty` metadata,
    // and `Inst::Out` demotes on print.
    let res = if is_float && is_f32 {
        match ctx.builder.func.dfg.value_type(res) {
            // Math ran through the F64 pipe (mixed-width operands).
            clif_types::F64 => {
                let demoted = ctx.builder.ins().fdemote(clif_types::F32, res);
                ctx.builder.ins().fpromote(clif_types::F64, demoted)
            }
            // Math already ran in F32 (both operands demoted): promote the
            // f32 result back into the canonical F64 slot.
            clif_types::F32 => ctx.builder.ins().fpromote(clif_types::F64, res),
            // Comparisons yield Bools (I64): untouched.
            _ => res,
        }
    } else if !is_float
        && !matches!(op, "<" | "<=" | ">" | ">=" | "==" | "!=" | "&&" | "||" | "<<" | ">>")
    {
        // Shift counts must NOT be wrapped (they are widths, not values);
        // comparisons are Bools; everything else is a value op.
        match repr {
            "Int8" | "i8" => {
                let red = ctx.builder.ins().ireduce(clif_types::I8, res);
                ctx.builder.ins().sextend(clif_types::I64, red)
            }
            "Int16" | "i16" => {
                let red = ctx.builder.ins().ireduce(clif_types::I16, res);
                ctx.builder.ins().sextend(clif_types::I64, red)
            }
            "Int32" | "i32" => {
                let red = ctx.builder.ins().ireduce(clif_types::I32, res);
                ctx.builder.ins().sextend(clif_types::I64, red)
            }
            "UInt8" | "Byte" | "u8" => {
                let red = ctx.builder.ins().ireduce(clif_types::I8, res);
                ctx.builder.ins().uextend(clif_types::I64, red)
            }
            "UInt16" | "u16" => {
                let red = ctx.builder.ins().ireduce(clif_types::I16, res);
                ctx.builder.ins().uextend(clif_types::I64, red)
            }
            "UInt32" | "u32" => {
                let red = ctx.builder.ins().ireduce(clif_types::I32, res);
                ctx.builder.ins().uextend(clif_types::I64, red)
            }
            _ => res,
        }
    } else {
        res
    };

    // v1.4.5 W1: unsigned comparisons must be unsigned — the operands are
    // zero-extended into I64 slots, so a signed `icmp` would rank a small
    // UInt8 value above a large one once the high bit is involved.
    if is_unsigned
        && matches!(op, "<" | "<=" | ">" | ">=" | "==" | "!=")
        && !ctx.string_vids.contains(left)
        && !ctx.string_vids.contains(right)
    {
        // Re-emit the comparison with the unsigned condition code. The
        // generic arm already produced a signed one; compute the correct
        // one and overwrite the mapping below via `res` reassignment is
        // not possible (moved), so patch the value in place.
        let cc = match op {
            "<" => cranelift_codegen::ir::condcodes::IntCC::UnsignedLessThan,
            "<=" => cranelift_codegen::ir::condcodes::IntCC::UnsignedLessThanOrEqual,
            ">" => cranelift_codegen::ir::condcodes::IntCC::UnsignedGreaterThan,
            ">=" => cranelift_codegen::ir::condcodes::IntCC::UnsignedGreaterThanOrEqual,
            _ => cranelift_codegen::ir::condcodes::IntCC::Equal,
        };
        let mask: i64 = match repr {
            "UInt8" | "Byte" | "u8" => 0xFF,
            "UInt16" | "u16" => 0xFFFF,
            "UInt32" | "u32" => 0xFFFF_FFFF,
            _ => -1, // UInt64: full width, no mask needed
        };
        let lm = if mask == -1 { lv } else { ctx.builder.ins().band_imm_u(lv, mask) };
        let rm = if mask == -1 { rv } else { ctx.builder.ins().band_imm_u(rv, mask) };
        let c = ctx.builder.ins().icmp(cc, lm, rm);
        let res_u = ctx.builder.ins().uextend(clif_types::I64, c);
        ctx.val_map.insert(*dest, res_u);
        ctx.bool_vids.insert(*dest);
        return Ok(());
    }

    ctx.val_map.insert(*dest, res);
    // Comparisons and logical operators always produce
    // a Bool; the lowering labels them `ty: "Int"`
    // because that is their machine representation.
    if matches!(op, "<" | "<=" | ">" | ">=" | "==" | "!=" | "&&" | "||") || ty == "Bool" {
        ctx.bool_vids.insert(*dest);
    }

    Ok(())
}

pub fn compile_unop<M: ClifModule>(
    ctx: &mut FunctionCompileCtx<'_, '_, M>,
    dest: &ValueId,
    op: &str,
    operand: &ValueId,
    ty: &str,
) -> Result<(), String> {
    let raw_v = ctx
        .val_map
        .get(operand)
        .copied()
        .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
    let v_ty = ctx.builder.func.dfg.value_type(raw_v);
    let res = if v_ty == clif_types::F64 {
        if op == "-" {
            let zero = ctx.builder.ins().f64const(0.0);
            ctx.builder.ins().fsub(zero, raw_v)
        } else {
            raw_v
        }
    } else {
        let v = raw_v;
        if op == "-" {
            ctx.builder.ins().ineg(v)
        } else if op == "!" {
            if ctx.bool_vids.contains(operand) {
                // Logical NOT: `!true` must be `false`,
                // not bnot(1) = -2 (still truthy).
                let is_zero = ctx.builder.ins().icmp_imm_s(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    v,
                    0,
                );
                ctx.builder.ins().uextend(clif_types::I64, is_zero)
            } else {
                ctx.builder.ins().bnot(v)
            }
        } else {
            v
        }
    };
    ctx.val_map.insert(*dest, res);
    if ctx.string_vids.contains(operand) {
        ctx.string_vids.insert(*dest);
    }
    if op == "!" {
        ctx.bool_vids.insert(*dest);
    }
    // SROA field forwarding emits `copy` for
    // `struct.field` reads. A copied Bool must stay a
    // Bool or `out m.is_some` prints "1" instead of
    // "true" (the UnOp result loses the flag the
    // original GetField carried in its `ty`).
    if (op == "copy" || op == "await") && ty == "Bool" {
        ctx.bool_vids.insert(*dest);
    }
    if let Some(c) = ctx.val_to_class.get(operand) {
        ctx.val_to_class.insert(*dest, c.clone());
    }

    Ok(())
}
