use crate::dmir::{ArenaHint, ValueId};
use cranelift_codegen::ir::{BlockArg, InstBuilder, types as clif_types};
use cranelift_module::Module as ClifModule;

use super::types::FunctionCompileCtx;

pub fn compile_call<M: ClifModule>(
    ctx: &mut FunctionCompileCtx<'_, '_, M>,
    dest: &ValueId,
    func: &str,
    args: &[ValueId],
    ty: &str,
) -> Result<(), String> {
    if func == "destroy" || func == "drop" {
        ctx.val_map
            .insert(*dest, ctx.builder.ins().iconst(clif_types::I64, 0));
        return Ok(());
    }
    // Hidden sret return-slot ABI (Microsoft x64): a C function returning a
    // by-value struct larger than one machine word takes a caller-allocated
    // buffer whose pointer is passed as the first argument. Allocate the
    // buffer as a regular Datara heap object (one 8-byte slot per field, the
    // same layout StructInit produces), pass it as the hidden first argument,
    // and let the Datara result value be that pointer: subsequent field reads
    // then load directly from the buffer the C callee filled.
    if let Some(&sret_size) = ctx.dmir_module.extern_sret.get(func) {
        return compile_sret_call(ctx, dest, func, args, ty, sret_size);
    }
    // SysV AMD64 register-pair ABI (v1.3.3, linux-x86_64): a C function
    // returning a 16-byte layout-compatible struct hands the two eightbytes
    // back in registers (INTEGER -> RAX then RDX, SSE -> XMM0 then XMM1).
    // The call receives both register values and writes them into a fresh
    // Datara heap object, so the result is shaped exactly like the sret
    // result: field reads load the 8-byte slot at `index * 8`.
    if let Some(&classes) = ctx.dmir_module.extern_sysv.get(func) {
        return compile_sysv_register_call(ctx, dest, func, args, ty, classes);
    }
    // First-Class Hardware SIMD Lowering (F32X4 / I32X4 Native Vector Registers)
    if super::simd::try_compile_simd_call(ctx, dest, func, args, ty)? {
        return Ok(());
    }

    if (func == "fma" || func == "datara_rt_fma") && args.len() == 3 {
        let mut f_operands = Vec::with_capacity(3);
        for arg in &args[0..3] {
            let val = ctx
                .val_map
                .get(arg)
                .copied()
                .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
            let ty = ctx.builder.func.dfg.value_type(val);
            let fval = if ty == clif_types::I64
                || ty == clif_types::I32
                || ty == clif_types::I16
                || ty == clif_types::I8
            {
                ctx.builder.ins().fcvt_from_sint(clif_types::F64, val)
            } else if ty == clif_types::F32 {
                ctx.builder.ins().fpromote(clif_types::F64, val)
            } else {
                val
            };
            f_operands.push(fval);
        }
        let prod = ctx.builder.ins().fmul(f_operands[0], f_operands[1]);
        let res = ctx.builder.ins().fadd(prod, f_operands[2]);
        ctx.val_map.insert(*dest, res);
        return Ok(());
    }

    // List literals: the lowering emits
    // datara_rt_list_create_N for the exact literal
    // length, so a fixed set of runtime symbols can
    // never cover every arity. Build the
    // DataraListHeader + [count, e0..eN-1] block
    // inline for any N, mirroring the C runtime
    // layout (append/free read the header).
    if let Some(rest) = func.strip_prefix("datara_rt_list_create_")
        && let Ok(n) = rest.parse::<usize>()
    {
        const LIST_HEADER_SIZE: i64 = 16; // { capacity: i64, magic: i64 }
        const LIST_MAGIC: i64 = 0x4441544C49535430; // DATARA_LIST_MAGIC
        // v1.4.0: in @arena functions the header is bump-allocated from the
        // frame arena so the return-time checkpoint restore reclaims it.
        let list_alloc_id = if ctx.current_func.alloc_hint == ArenaHint::Arena {
            ctx.runtime.rt_arena_alloc_id
        } else {
            ctx.runtime.malloc_id
        };
        let malloc_ref = ctx
            .module
            .declare_func_in_func(list_alloc_id, ctx.builder.func);
        let size_val = ctx.builder.ins().iconst(
            clif_types::I64,
            LIST_HEADER_SIZE.saturating_add((n.saturating_add(1) as i64).saturating_mul(8)),
        );
        let call_inst = ctx.builder.ins().call(malloc_ref, &[size_val]);
        let hdr_addr = ctx.builder.inst_results(call_inst)[0];
        let slot_addr = ctx.builder.ins().iadd_imm_s(hdr_addr, LIST_HEADER_SIZE);
        let flags = cranelift_codegen::ir::MachMemFlags::new();
        let cap_val = ctx.builder.ins().iconst(clif_types::I64, n as i64);
        let magic_val = ctx.builder.ins().iconst(clif_types::I64, LIST_MAGIC);
        ctx.builder.ins().store(flags, cap_val, hdr_addr, 0);
        ctx.builder.ins().store(flags, magic_val, hdr_addr, 8);
        let count_val = ctx.builder.ins().iconst(clif_types::I64, n as i64);
        ctx.builder.ins().store(flags, count_val, slot_addr, 0);
        for (i, a) in args.iter().enumerate() {
            let elem = ctx
                .val_map
                .get(a)
                .copied()
                .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
            ctx.builder
                .ins()
                .store(flags, elem, slot_addr, ((i + 1) * 8) as i32);
        }
        ctx.val_map.insert(*dest, slot_addr);
        ctx.list_vids.insert(*dest);
        return Ok(());
    }
    if let Some(rest) = func.strip_prefix("datara_rt_tuple_create_")
        && let Ok(n) = rest.parse::<usize>()
    {
        let malloc_ref = ctx
            .module
            .declare_func_in_func(ctx.runtime.malloc_id, ctx.builder.func);
        let size_val = ctx.builder.ins().iconst(
            clif_types::I64,
            (n.saturating_add(1) as i64).saturating_mul(8),
        );
        let call_inst = ctx.builder.ins().call(malloc_ref, &[size_val]);
        let slot_addr = ctx.builder.inst_results(call_inst)[0];
        let flags = cranelift_codegen::ir::MachMemFlags::new();
        let count_val = ctx.builder.ins().iconst(clif_types::I64, n as i64);
        ctx.builder.ins().store(flags, count_val, slot_addr, 0);
        for (i, a) in args.iter().enumerate() {
            let elem = ctx
                .val_map
                .get(a)
                .copied()
                .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
            ctx.builder
                .ins()
                .store(flags, elem, slot_addr, ((i + 1) * 8) as i32);
        }
        ctx.val_map.insert(*dest, slot_addr);
        return Ok(());
    }
    // Direct Hardware Math Intrinsics (sqrt, abs)
    if (func == "math_sqrt" || func == "sqrt" || func == "datara_rt_math_sqrt") && args.len() == 1 {
        let arg = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let arg_ty = ctx.builder.func.dfg.value_type(arg);
        let f64_val = if arg_ty == clif_types::F64 {
            arg
        } else if arg_ty == clif_types::I64 {
            ctx.builder.ins().fcvt_from_sint(clif_types::F64, arg)
        } else if arg_ty == clif_types::F32 {
            ctx.builder.ins().fpromote(clif_types::F64, arg)
        } else {
            arg
        };
        let res = ctx.builder.ins().sqrt(f64_val);
        ctx.val_map.insert(*dest, res);
        return Ok(());
    }

    if (func == "math_abs" || func == "abs" || func == "datara_rt_math_abs") && args.len() == 1 {
        let arg = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let arg_ty = ctx.builder.func.dfg.value_type(arg);
        if arg_ty == clif_types::F64 {
            let res = ctx.builder.ins().fabs(arg);
            ctx.val_map.insert(*dest, res);
            return Ok(());
        }
    }

    if (func == "math_floor" || func == "floor" || func == "datara_rt_math_floor")
        && args.len() == 1
    {
        let arg = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let arg_ty = ctx.builder.func.dfg.value_type(arg);
        let f64_val = if arg_ty == clif_types::F64 {
            arg
        } else if arg_ty == clif_types::I64 {
            ctx.builder.ins().fcvt_from_sint(clif_types::F64, arg)
        } else if arg_ty == clif_types::F32 {
            ctx.builder.ins().fpromote(clif_types::F64, arg)
        } else {
            arg
        };
        let res = ctx.builder.ins().floor(f64_val);
        ctx.val_map.insert(*dest, res);
        return Ok(());
    }

    if (func == "math_ceil" || func == "ceil" || func == "datara_rt_math_ceil") && args.len() == 1 {
        let arg = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let arg_ty = ctx.builder.func.dfg.value_type(arg);
        let f64_val = if arg_ty == clif_types::F64 {
            arg
        } else if arg_ty == clif_types::I64 {
            ctx.builder.ins().fcvt_from_sint(clif_types::F64, arg)
        } else if arg_ty == clif_types::F32 {
            ctx.builder.ins().fpromote(clif_types::F64, arg)
        } else {
            arg
        };
        let res = ctx.builder.ins().ceil(f64_val);
        ctx.val_map.insert(*dest, res);
        return Ok(());
    }

    if (func == "math_round" || func == "round" || func == "datara_rt_math_round")
        && args.len() == 1
    {
        let arg = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let arg_ty = ctx.builder.func.dfg.value_type(arg);
        let f64_val = if arg_ty == clif_types::F64 {
            arg
        } else if arg_ty == clif_types::I64 {
            ctx.builder.ins().fcvt_from_sint(clif_types::F64, arg)
        } else if arg_ty == clif_types::F32 {
            ctx.builder.ins().fpromote(clif_types::F64, arg)
        } else {
            arg
        };
        let res = ctx.builder.ins().nearest(f64_val);
        ctx.val_map.insert(*dest, res);
        return Ok(());
    }

    if (func == "math_min" || func == "min" || func == "datara_rt_math_min") && args.len() == 2 {
        let a = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let b = ctx
            .val_map
            .get(&args[1])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let a_ty = ctx.builder.func.dfg.value_type(a);
        let b_ty = ctx.builder.func.dfg.value_type(b);
        if a_ty == clif_types::F64 && b_ty == clif_types::F64 {
            let res = ctx.builder.ins().fmin(a, b);
            ctx.val_map.insert(*dest, res);
            return Ok(());
        }
    }

    if (func == "math_max" || func == "max" || func == "datara_rt_math_max") && args.len() == 2 {
        let a = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let b = ctx
            .val_map
            .get(&args[1])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().f64const(0.0));
        let a_ty = ctx.builder.func.dfg.value_type(a);
        let b_ty = ctx.builder.func.dfg.value_type(b);
        if a_ty == clif_types::F64 && b_ty == clif_types::F64 {
            let res = ctx.builder.ins().fmax(a, b);
            ctx.val_map.insert(*dest, res);
            return Ok(());
        }
    }

    // Direct unchecked list get/set: only DMIR sites proven safe
    // by static analysis (induction variable within bounds or
    // bounds-check-elimination pass, which must prove the trip
    // count) may bypass the runtime bounds check. The checked
    // variants always go through the real runtime call below.
    if (func == "datara_rt_list_get_unchecked" || func == "datara_rt_list_get_f64_unchecked")
        && args.len() == 2
    {
        let list_ptr = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let idx_val = ctx
            .val_map
            .get(&args[1])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let idx_scaled = ctx.builder.ins().ishl_imm_u(idx_val, 3);
        let offset = ctx.builder.ins().iadd_imm_s(idx_scaled, 8);
        let addr = ctx.builder.ins().iadd(list_ptr, offset);
        let flags = cranelift_codegen::ir::MachMemFlags::new();
        let is_float = ty == "Float" || ty == "f64" || func == "datara_rt_list_get_f64_unchecked";
        let elem = if is_float {
            ctx.builder.ins().load(clif_types::F64, flags, addr, 0)
        } else {
            ctx.builder.ins().load(clif_types::I64, flags, addr, 0)
        };
        ctx.val_map.insert(*dest, elem);
        // Record the element's class so GetField on the result resolves
        // against the element's own layout (E0944 contract).
        if let Some(c) = super::types::class_type_name(ty) {
            ctx.val_to_class.insert(*dest, c);
        }
        if ty == "String" || ty == "Str" {
            ctx.string_vids.insert(*dest);
        } else if ty == "Bool" {
            ctx.bool_vids.insert(*dest);
        } else if ty.starts_with("List") || ty.starts_with('[') {
            ctx.list_vids.insert(*dest);
        } else if ty.starts_with("Map") {
            ctx.map_vids.insert(*dest);
        }
        return Ok(());
    }
    if (func == "datara_rt_list_set_unchecked" || func == "datara_rt_list_set_f64_unchecked")
        && args.len() == 3
    {
        let list_ptr = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let idx_val = ctx
            .val_map
            .get(&args[1])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let val = ctx
            .val_map
            .get(&args[2])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let idx_scaled = ctx.builder.ins().ishl_imm_u(idx_val, 3);
        let offset = ctx.builder.ins().iadd_imm_s(idx_scaled, 8);
        let addr = ctx.builder.ins().iadd(list_ptr, offset);
        let flags = cranelift_codegen::ir::MachMemFlags::new();
        let val_ty = ctx.builder.func.dfg.value_type(val);
        if val_ty == clif_types::F64 {
            ctx.builder.ins().store(flags, val, addr, 0);
        } else {
            ctx.builder.ins().store(flags, val, addr, 0);
        }
        ctx.val_map.insert(*dest, list_ptr);
        return Ok(());
    }
    if (func == "datara_rt_list_get" || func == "datara_rt_list_get_f64") && args.len() == 2 {
        let list_ptr = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let idx_val = ctx
            .val_map
            .get(&args[1])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let flags = cranelift_codegen::ir::MachMemFlags::new();

        let check_block = ctx.builder.create_block();
        let load_block = ctx.builder.create_block();
        let else_block = ctx.builder.create_block();
        let merge_block = ctx.builder.create_block();

        let is_float = ty == "Float" || ty == "f64" || func == "datara_rt_list_get_f64";
        let res_ty = if is_float {
            clif_types::F64
        } else {
            clif_types::I64
        };
        ctx.builder.append_block_param(merge_block, res_ty);

        let is_non_null = ctx.builder.ins().icmp_imm_u(
            cranelift_codegen::ir::condcodes::IntCC::NotEqual,
            list_ptr,
            0,
        );
        ctx.builder
            .ins()
            .brif(is_non_null, check_block, &[], else_block, &[]);

        ctx.builder.switch_to_block(check_block);
        ctx.builder.seal_block(check_block);
        let list_len = ctx.builder.ins().load(clif_types::I64, flags, list_ptr, 0);
        let in_bounds = ctx.builder.ins().icmp(
            cranelift_codegen::ir::condcodes::IntCC::UnsignedLessThan,
            idx_val,
            list_len,
        );
        ctx.builder
            .ins()
            .brif(in_bounds, load_block, &[], else_block, &[]);

        ctx.builder.switch_to_block(load_block);
        ctx.builder.seal_block(load_block);
        let idx_scaled = ctx.builder.ins().ishl_imm_u(idx_val, 3);
        let offset = ctx.builder.ins().iadd_imm_s(idx_scaled, 8);
        let addr = ctx.builder.ins().iadd(list_ptr, offset);
        let elem = ctx.builder.ins().load(res_ty, flags, addr, 0);
        ctx.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(elem)]);

        ctx.builder.switch_to_block(else_block);
        ctx.builder.seal_block(else_block);
        let zero = if is_float {
            ctx.builder.ins().f64const(0.0)
        } else {
            ctx.builder.ins().iconst(clif_types::I64, 0)
        };
        ctx.builder
            .ins()
            .jump(merge_block, &[BlockArg::Value(zero)]);

        ctx.builder.switch_to_block(merge_block);
        ctx.builder.seal_block(merge_block);
        let res = ctx.builder.block_params(merge_block)[0];
        ctx.val_map.insert(*dest, res);
        // Record the element's class so GetField on the result resolves
        // against the element's own layout (E0944 contract).
        if let Some(c) = super::types::class_type_name(ty) {
            ctx.val_to_class.insert(*dest, c);
        }
        if ty == "String" || ty == "Str" {
            ctx.string_vids.insert(*dest);
        } else if ty == "Bool" {
            ctx.bool_vids.insert(*dest);
        } else if ty.starts_with("List") || ty.starts_with('[') {
            ctx.list_vids.insert(*dest);
        } else if ty.starts_with("Map") {
            ctx.map_vids.insert(*dest);
        }
        return Ok(());
    }
    if (func == "datara_rt_list_set" || func == "datara_rt_list_set_f64") && args.len() == 3 {
        let list_ptr = ctx
            .val_map
            .get(&args[0])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let idx_val = ctx
            .val_map
            .get(&args[1])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let val = ctx
            .val_map
            .get(&args[2])
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let flags = cranelift_codegen::ir::MachMemFlags::new();

        let check_block = ctx.builder.create_block();
        let store_block = ctx.builder.create_block();
        let merge_block = ctx.builder.create_block();

        let is_non_null = ctx.builder.ins().icmp_imm_u(
            cranelift_codegen::ir::condcodes::IntCC::NotEqual,
            list_ptr,
            0,
        );
        ctx.builder
            .ins()
            .brif(is_non_null, check_block, &[], merge_block, &[]);

        ctx.builder.switch_to_block(check_block);
        ctx.builder.seal_block(check_block);
        let list_len = ctx.builder.ins().load(clif_types::I64, flags, list_ptr, 0);
        let in_bounds = ctx.builder.ins().icmp(
            cranelift_codegen::ir::condcodes::IntCC::UnsignedLessThan,
            idx_val,
            list_len,
        );
        ctx.builder
            .ins()
            .brif(in_bounds, store_block, &[], merge_block, &[]);

        ctx.builder.switch_to_block(store_block);
        ctx.builder.seal_block(store_block);
        let idx_scaled = ctx.builder.ins().ishl_imm_u(idx_val, 3);
        let offset = ctx.builder.ins().iadd_imm_s(idx_scaled, 8);
        let addr = ctx.builder.ins().iadd(list_ptr, offset);
        let val_ty = ctx.builder.func.dfg.value_type(val);
        if val_ty == clif_types::F64 {
            ctx.builder.ins().store(flags, val, addr, 0);
        } else {
            ctx.builder.ins().store(flags, val, addr, 0);
        }
        ctx.builder.ins().jump(merge_block, &[]);

        ctx.builder.switch_to_block(merge_block);
        ctx.builder.seal_block(merge_block);
        ctx.val_map.insert(*dest, list_ptr);
        return Ok(());
    }
    let (callee_id, callee_name) = match ctx.func_ids.get(func) {
        Some(v) => (v.0, func.to_string()),
        None => {
            let matched = if let Some(first_arg) = args.first() {
                if let Some(c) = ctx.val_to_class.get(first_arg) {
                    let specialized = format!("{}_{}", c, func);
                    if let Some(target) = ctx.func_ids.get(&specialized) {
                        Some((target.0, specialized))
                    } else {
                        let base_c = c.split('_').next().unwrap_or(c);
                        let base_spec = format!("{}_{}", base_c, func);
                        ctx.func_ids
                            .get(&base_spec)
                            .map(|target| (target.0, base_spec))
                    }
                } else {
                    None
                }
            } else {
                None
            };
            matched
                .or_else(|| {
                    // Collect all suffix matches and pick the
                    // smallest name deterministically; a bare
                    // HashMap-order `find` made the dispatch
                    // target vary run to run.
                    let mut candidates: Vec<&String> = ctx
                        .func_ids
                        .keys()
                        .filter(|k| k.split_once('_').map(|(_, m)| m == func).unwrap_or(false))
                        .collect();
                    candidates.sort();
                    candidates
                        .first()
                        .map(|k| (ctx.func_ids[*k].0, (*k).clone()))
                })
                .unwrap_or({
                    if std::env::var("DATARA_CODEGEN_TRACE").is_ok() {
                        eprintln!(
                            "[datara-codegen] UNRESOLVED CALL: {} in {}",
                            func, ctx.current_func.name
                        );
                    }
                    (cranelift_module::FuncId::from_u32(0), String::new())
                })
        }
    };
    if callee_name.is_empty() {
        return Err(format!(
            "Code generation failed: unresolved function call '{}' in function '{}'",
            func, ctx.current_func.name
        ));
    }

    let mut resolved_callee_id = callee_id;
    if func.starts_with("datara_rt_print_")
        && args.len() == 1
        && let Some(&first_arg_id) = args.first()
        && let Some(&av) = ctx.val_map.get(&first_arg_id)
    {
        let v_ty = ctx.builder.func.dfg.value_type(av);
        if v_ty == clif_types::F64 {
            if let Some(target) = ctx.func_ids.get("datara_rt_print_float") {
                resolved_callee_id = target.0;
            }
        } else if ctx.string_vids.contains(&first_arg_id) {
            if let Some(target) = ctx.func_ids.get("datara_rt_print_str") {
                resolved_callee_id = target.0;
            }
        } else if ctx.bool_vids.contains(&first_arg_id) {
            if let Some(target) = ctx.func_ids.get("datara_rt_print_bool") {
                resolved_callee_id = target.0;
            }
        } else if ctx.list_vids.contains(&first_arg_id)
            && let Some(target) = ctx.func_ids.get("datara_rt_print_list")
        {
            resolved_callee_id = target.0;
        }
    }

    let callee_ref = ctx
        .module
        .declare_func_in_func(resolved_callee_id, ctx.builder.func);
    ctx.builder.func.dfg.ext_funcs[callee_ref].colocated = true;
    let is_str_concat = callee_name.starts_with("datara_rt_str_concat");
    let conv_ref = ctx
        .module
        .declare_func_in_func(ctx.runtime.rt_int_to_str_id, ctx.builder.func);
    let bool_to_str_ref = ctx
        .module
        .declare_func_in_func(ctx.runtime.rt_bool_to_str_id, ctx.builder.func);
    let flt_to_str_ref = ctx
        .module
        .declare_func_in_func(ctx.runtime.rt_flt_to_str_id, ctx.builder.func);
    let mut arg_vals = Vec::new();
    for a in args {
        // Preserve arity: a missing value must still
        // occupy its argument slot in the signature.
        let av = ctx
            .val_map
            .get(a)
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        let final_av = if is_str_concat && !ctx.string_vids.contains(a) {
            let call = if ctx.bool_vids.contains(a) {
                ctx.builder.ins().call(bool_to_str_ref, &[av])
            } else if ctx.builder.func.dfg.value_type(av) == clif_types::F64 {
                ctx.builder.ins().call(flt_to_str_ref, &[av])
            } else {
                ctx.builder.ins().call(conv_ref, &[av])
            };
            ctx.builder.inst_results(call)[0]
        } else {
            av
        };
        arg_vals.push(final_av);
    }
    // http_get historically had a zero-arg builtin
    // signature; keep `http_get()` calls compilable by
    // padding a null URL argument.
    if callee_name.ends_with("http_get") && arg_vals.is_empty() {
        arg_vals.push(ctx.builder.ins().iconst(clif_types::I64, 0));
    }
    // The runtime ABI is all-I64 (collections store
    // floats as their IEEE bit pattern). An F64 value
    // passed against an I64 param fails Cranelift
    // verification, so bitcast per the declared sig.
    if let Some((_, callee_sig)) = ctx.func_ids.get(&callee_name) {
        for (i, av) in arg_vals.iter_mut().enumerate() {
            if ctx.builder.func.dfg.value_type(*av) == clif_types::F64
                && callee_sig
                    .params
                    .get(i)
                    .map(|p| p.value_type == clif_types::I64)
                    .unwrap_or(false)
            {
                *av = ctx.builder.ins().bitcast(
                    clif_types::I64,
                    cranelift_codegen::ir::MemFlagsData::new(),
                    *av,
                );
            }
        }
    }
    let call_inst = ctx.builder.ins().call(callee_ref, &arg_vals);
    let results = ctx.builder.inst_results(call_inst);
    let ret_ty = ctx
        .dmir_module
        .functions
        .get(&callee_name)
        .or_else(|| ctx.dmir_module.functions.get(func))
        .map(|f| f.return_type.as_str())
        .or_else(|| {
            ctx.dmir_module
                .extern_functions
                .get(&callee_name)
                .or_else(|| ctx.dmir_module.extern_functions.get(func))
                .map(|(_, ret)| ret.as_str())
        })
        .unwrap_or("");
    if let Some(&r) = results.first() {
        let mut r_val = r;
        // Checked-I/O builtins return Outcome<Str> objects (stdlib Outcome<T>
        // layout). The generic `contains("Str")` heuristics below would
        // mis-tag the object pointer as a string value, so the Outcome
        // prefix suppresses the string/list/map tagging; the class tagging
        // at the end of this block still records the receiver class (with
        // the generic suffix, which method dispatch strips).
        let is_outcome_obj = ty.starts_with("Outcome<") || ret_ty.starts_with("Outcome<");
        // Generic behavior methods (e.g. Outcome<T>.unwrap, which IPO
        // devirtualization rewrites from a MethodCall into this plain
        // Inst::Call) declare return type "T" in the DMIR function table,
        // so neither `ty` nor `ret_ty` names the payload. A devirtualized
        // call passes the receiver as the first argument and the receiver's
        // tracked class ("Outcome<Float>") names the payload: specialize
        // exactly like compile_method_call does. Static-dispatch calls pass
        // a dummy 0 `this` with no tracked class and stay generic.
        let receiver_class = if ret_ty == "T" {
            args.first().and_then(|a| ctx.val_to_class.get(a)).cloned()
        } else {
            None
        };
        let generic_payload: Option<String> = receiver_class.as_ref().and_then(|c| {
            let start = c.find('<')?;
            let end = c.rfind('>')?;
            if start < end {
                Some(c[start + 1..end].trim().to_string())
            } else {
                None
            }
        });
        let generic_is_float = generic_payload
            .as_ref()
            .map(|p| p == "Float" || p == "Float64" || p == "f64")
            .unwrap_or(false);
        let generic_is_str = generic_payload
            .as_ref()
            .map(|p| p == "Str" || p == "String")
            .unwrap_or(false);
        let generic_is_bool = generic_payload
            .as_ref()
            .map(|p| p == "Bool")
            .unwrap_or(false);
        let generic_is_list = generic_payload
            .as_ref()
            .map(|p| p.starts_with("List") || p.starts_with('['))
            .unwrap_or(false);
        let generic_is_map = generic_payload
            .as_ref()
            .map(|p| p.starts_with("Map"))
            .unwrap_or(false);
        if (ty == "Float" || ret_ty == "Float" || generic_is_float)
            && ctx.builder.func.dfg.value_type(r) == clif_types::I64
        {
            r_val = ctx.builder.ins().bitcast(
                clif_types::F64,
                cranelift_codegen::ir::MemFlagsData::new(),
                r,
            );
        }
        ctx.val_map.insert(*dest, r_val);
        let first_arg_is_str_class = args
            .first()
            .and_then(|a| ctx.val_to_class.get(a))
            .map(|c| c.ends_with("String") || c.ends_with("Str"))
            .unwrap_or(false);
        if !is_outcome_obj
            && (ty == "String"
                || ty.contains("Str")
                || ret_ty == "String"
                || ret_ty == "Str"
                || (ret_ty == "T" && first_arg_is_str_class)
                || generic_is_str
                || func == "datara_rt_range_str"
                || func == "datara_rt_int_to_str"
                || func == "datara_rt_str_concat"
                || func == "datara_rt_str_char_at"
                || func == "str_char_at"
                || func == "char_at"
                || ctx.string_return_funcs.contains(func)
                || ctx.string_return_funcs.contains(&callee_name))
        {
            ctx.string_vids.insert(*dest);
        }
        if ty == "Bool" || ret_ty == "Bool" || generic_is_bool {
            ctx.bool_vids.insert(*dest);
        }
        if !is_outcome_obj
            && (ty.contains("List")
                || ret_ty.contains("List")
                || ty.starts_with('[')
                || ret_ty.starts_with('[')
                || generic_is_list
                || func.starts_with("datara_rt_list_create")
                || func == "datara_rt_list_append")
        {
            ctx.list_vids.insert(*dest);
        }
        if !is_outcome_obj
            && (ty.contains("Map")
                || ret_ty.contains("Map")
                || generic_is_map
                || func.starts_with("datara_rt_map_create"))
        {
            ctx.map_vids.insert(*dest);
        }
        let class_to_record = if ret_ty == "T" {
            match &generic_payload {
                Some(p) => p.clone(),
                None => ret_ty.to_string(),
            }
        } else if !ret_ty.is_empty() {
            ret_ty.to_string()
        } else {
            ty.to_string()
        };
        let stripped = class_to_record
            .split('<')
            .next()
            .unwrap_or(&class_to_record);
        if !stripped.is_empty()
            && stripped != "Int"
            && stripped != "Float"
            && stripped != "Bool"
            && stripped != "Str"
            && stripped != "String"
            && stripped != "List"
            && stripped != "Map"
            && stripped != "Unit"
            && !stripped.starts_with('[')
        {
            ctx.val_to_class.insert(*dest, class_to_record.to_string());
        }
    }

    Ok(())
}

pub use super::inst_method_call::compile_method_call;

/// Microsoft x64 convention: the caller allocates the return buffer and
/// passes its pointer as the FIRST integer argument (RCX; user arguments
/// shift into RDX/R8/R9/stack), and the callee echoes that pointer in RAX.
/// The buffer is allocated with the same runtime `malloc` the StructInit
/// lowering uses, so the returned value is an ordinary Datara heap object:
/// field reads load the 8-byte slot at `index * 8`, which the cimport gate
/// guarantees matches the C layout.
fn compile_sret_call<M: ClifModule>(
    ctx: &mut FunctionCompileCtx<'_, '_, M>,
    dest: &ValueId,
    func: &str,
    args: &[ValueId],
    ty: &str,
    sret_size: usize,
) -> Result<(), String> {
    let Some((callee_id, callee_sig)) = ctx.func_ids.get(func).map(|(id, sig)| (*id, sig)) else {
        return Err(format!(
            "Code generation failed: unresolved function call '{}' in function '{}'",
            func, ctx.current_func.name
        ));
    };
    if callee_sig.call_conv != cranelift_codegen::isa::CallConv::WindowsFastcall {
        return Err(format!(
            "Code generation failed: C function '{}' returns a by-value struct of {} bytes through the hidden sret return slot, which the native backend implements only for the Microsoft x64 calling convention (target uses {:?}). Use an out-pointer parameter (e.g. `void f(T* out)`).",
            func, sret_size, callee_sig.call_conv
        ));
    }

    // Mirror the StructInit allocation: heap object with one 8-byte slot per
    // field, minimum 16 bytes. CRT `malloc` returns memory aligned for any
    // fundamental type (16 bytes on x64), so the buffer satisfies every
    // struct alignment the cimport gate admits.
    let malloc_ref = ctx
        .module
        .declare_func_in_func(ctx.runtime.malloc_id, ctx.builder.func);
    let size_val = ctx
        .builder
        .ins()
        .iconst(clif_types::I64, sret_size.max(16) as i64);
    let alloc_inst = ctx.builder.ins().call(malloc_ref, &[size_val]);
    let sret_buf = ctx.builder.inst_results(alloc_inst)[0];

    let callee_ref = ctx.module.declare_func_in_func(callee_id, ctx.builder.func);
    let mut arg_vals = vec![sret_buf];
    for (i, a) in args.iter().enumerate() {
        // Preserve arity: a missing value must still occupy its argument
        // slot in the signature (index shifted by the hidden sret pointer).
        let mut av = ctx
            .val_map
            .get(a)
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        if ctx.builder.func.dfg.value_type(av) == clif_types::F64
            && callee_sig
                .params
                .get(i + 1)
                .map(|p| p.value_type == clif_types::I64)
                .unwrap_or(false)
        {
            av = ctx.builder.ins().bitcast(
                clif_types::I64,
                cranelift_codegen::ir::MemFlagsData::new(),
                av,
            );
        }
        arg_vals.push(av);
    }
    ctx.builder.ins().call(callee_ref, &arg_vals);
    ctx.val_map.insert(*dest, sret_buf);
    let class = if ty.is_empty() {
        ctx.dmir_module
            .extern_functions
            .get(func)
            .map(|(_, ret)| ret.clone())
            .unwrap_or_default()
    } else {
        ty.to_string()
    };
    if !class.is_empty() {
        ctx.val_to_class.insert(*dest, class);
    }
    Ok(())
}

/// Lower a call to an imported C function that returns a 16-byte
/// layout-compatible struct through the SysV AMD64 register pair (v1.3.3).
///
/// System V AMD64 convention: each eightbyte of the aggregate is classified
/// independently, INTEGER eightbytes ride RAX then RDX and SSE eightbytes
/// ride XMM0 then XMM1 (independent per-class sequences). The extern
/// declaration carries the two field-order classes, so Cranelift's
/// multi-value return semantics hand back exactly the register pair the C
/// callee produced. The values are written into a runtime `malloc` buffer
/// with the Datara object layout (field `i` at `i * 8`), and the Datara
/// result value is that buffer — identical to the sret path's result shape,
/// so field reads need no translation.
fn compile_sysv_register_call<M: ClifModule>(
    ctx: &mut FunctionCompileCtx<'_, '_, M>,
    dest: &ValueId,
    func: &str,
    args: &[ValueId],
    ty: &str,
    classes: [crate::ast::SysVClass; 2],
) -> Result<(), String> {
    use crate::ast::SysVClass;

    let Some((callee_id, callee_sig)) = ctx.func_ids.get(func).map(|(id, sig)| (*id, sig)) else {
        return Err(format!(
            "Code generation failed: unresolved function call '{}' in function '{}'",
            func, ctx.current_func.name
        ));
    };
    if callee_sig.call_conv != cranelift_codegen::isa::CallConv::SystemV {
        return Err(format!(
            "Code generation failed: C function '{}' returns a 16-byte struct through the SysV AMD64 register pair (RAX/RDX, XMM0/XMM1), which the native backend implements only for the SystemV calling convention (target uses {:?}). Use an out-pointer parameter (e.g. `void f(T* out)`).",
            func, callee_sig.call_conv
        ));
    }
    let expected: Vec<clif_types::Type> = classes
        .iter()
        .map(|c| match c {
            SysVClass::Integer => clif_types::I64,
            SysVClass::Sse => clif_types::F64,
        })
        .collect();
    if callee_sig.returns.len() != 2
        || callee_sig
            .returns
            .iter()
            .zip(&expected)
            .any(|(p, e)| p.value_type != *e)
    {
        return Err(format!(
            "Code generation failed: C function '{}' is declared with a return signature that does not match its SysV register classes {:?}/{:?}; refusing to misread the return registers. Use an out-pointer parameter (e.g. `void f(T* out)`).",
            func, classes[0], classes[1]
        ));
    }

    // Mirror the StructInit/sret allocation: one 8-byte slot per field. CRT
    // `malloc` returns memory aligned for any fundamental type (16 bytes on
    // x86-64), so the buffer satisfies both fields' alignment.
    let malloc_ref = ctx
        .module
        .declare_func_in_func(ctx.runtime.malloc_id, ctx.builder.func);
    let size_val = ctx.builder.ins().iconst(clif_types::I64, 16);
    let alloc_inst = ctx.builder.ins().call(malloc_ref, &[size_val]);
    let ret_buf = ctx.builder.inst_results(alloc_inst)[0];

    let callee_ref = ctx.module.declare_func_in_func(callee_id, ctx.builder.func);
    let mut arg_vals = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        // Preserve arity: a missing value must still occupy its argument
        // slot in the signature. The runtime ABI is all-I64, so an F64 value
        // passed against an I64 parameter is bitcast to its raw pattern.
        let mut av = ctx
            .val_map
            .get(a)
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        if ctx.builder.func.dfg.value_type(av) == clif_types::F64
            && callee_sig
                .params
                .get(i)
                .map(|p| p.value_type == clif_types::I64)
                .unwrap_or(false)
        {
            av = ctx.builder.ins().bitcast(
                clif_types::I64,
                cranelift_codegen::ir::MemFlagsData::new(),
                av,
            );
        }
        arg_vals.push(av);
    }
    let call_inst = ctx.builder.ins().call(callee_ref, &arg_vals);
    let results = ctx.builder.inst_results(call_inst);
    if results.len() != 2 {
        return Err(format!(
            "Code generation failed: call to '{}' did not produce the two SysV return registers the declaration promises",
            func
        ));
    }
    // Store each returned eightbyte into its Datara field slot (field `i` at
    // offset `i * 8`). Integer classes store the raw I64, SSE classes the
    // raw F64: both are the register contents, no reinterpretation.
    let flags = cranelift_codegen::ir::MachMemFlags::new();
    let (r0, r1) = (results[0], results[1]);
    ctx.builder.ins().store(flags, r0, ret_buf, 0);
    ctx.builder.ins().store(flags, r1, ret_buf, 8);
    ctx.val_map.insert(*dest, ret_buf);

    let class = if ty.is_empty() {
        ctx.dmir_module
            .extern_functions
            .get(func)
            .map(|(_, ret)| ret.clone())
            .unwrap_or_default()
    } else {
        ty.to_string()
    };
    if !class.is_empty() {
        ctx.val_to_class.insert(*dest, class);
    }
    Ok(())
}
