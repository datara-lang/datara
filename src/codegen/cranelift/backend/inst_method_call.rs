use crate::dmir::ValueId;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue, types as clif_types};
use cranelift_module::Module as ClifModule;

use super::types::FunctionCompileCtx;

pub fn compile_method_call<M: ClifModule>(
    ctx: &mut FunctionCompileCtx<'_, '_, M>,
    dest: &ValueId,
    object: &ValueId,
    method: &str,
    args: &[ValueId],
    ty: &str,
) -> Result<(), String> {
    let mut all_args = vec![
        ctx.val_map
            .get(object)
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0)),
    ];
    for a in args {
        // Preserve arity: a missing value must still
        // occupy its argument slot in the signature.
        let av = ctx
            .val_map
            .get(a)
            .copied()
            .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
        all_args.push(av);
    }
    if std::env::var("DATARA_CODEGEN_TRACE").is_ok() {
        eprintln!(
            "[method_call] fn={} method={} obj={:?} class={:?} is_list={}",
            ctx.current_func.name,
            method,
            object,
            ctx.val_to_class.get(object),
            ctx.list_vids.contains(object)
        );
    }
    // Numeric conversion intrinsics (Gate 7 explicit casts): `.to_float()`
    // and `.to_int()` lower to a single conversion instruction
    // (sitofp / fptosi / bitcast), never a runtime call or method
    // dispatch. Collection and string receivers are excluded: their
    // protocol dispatch below must stay authoritative. A user-defined
    // class method with the same name also shadows the intrinsic: the
    // class's specialized function is dispatched below instead of
    // bit-converting the receiver's pointer.
    if matches!(method, "to_float" | "to_int")
        && !ctx.list_vids.contains(object)
        && !ctx.map_vids.contains(object)
        && !ctx.string_vids.contains(object)
    {
        let class_shadows = ctx
            .val_to_class
            .get(object)
            .map(|c| {
                let base = c.split('<').next().unwrap_or(c);
                ctx.func_ids.contains_key(&format!("{}_{}", base, method))
            })
            .unwrap_or(false);
        if !class_shadows {
            return compile_numeric_convert(ctx, dest, object, method);
        }
    }
    // v1.4.0: StrBuf builder methods. Dispatched on the declared class of
    // the receiver (`val_to_class` is populated by StructInit), BEFORE the
    // list protocol, so `StrBuf.push` can never reach rt_list_append.
    if ctx
        .val_to_class
        .get(object)
        .map(|c| c == "StrBuf")
        .unwrap_or(false)
    {
        let special_id = match method {
            "push" => Some(ctx.runtime.rt_strbuf_push_id),
            "push_int" => Some(ctx.runtime.rt_strbuf_push_int_id),
            "join" => Some(ctx.runtime.rt_strbuf_join_id),
            "len" | "length" => Some(ctx.runtime.rt_strbuf_len_id),
            _ => None,
        };
        if let Some(special_id) = special_id {
            let mut call_args: Vec<ClifValue> = Vec::new();
            if let Some(&obj_v) = ctx.val_map.get(object) {
                call_args.push(obj_v);
            }
            for a in args {
                let mut av = ctx
                    .val_map
                    .get(a)
                    .copied()
                    .unwrap_or_else(|| ctx.builder.ins().iconst(clif_types::I64, 0));
                if ctx.builder.func.dfg.value_type(av) == clif_types::F64 {
                    av = ctx.builder.ins().bitcast(
                        clif_types::I64,
                        cranelift_codegen::ir::MemFlagsData::new(),
                        av,
                    );
                }
                call_args.push(av);
            }
            let callee_ref = ctx
                .module
                .declare_func_in_func(special_id, ctx.builder.func);
            let call_inst = ctx.builder.ins().call(callee_ref, &call_args);
            let results = ctx.builder.inst_results(call_inst);
            if let Some(&r) = results.first() {
                ctx.val_map.insert(*dest, r);
                if method == "join" {
                    ctx.string_vids.insert(*dest);
                }
            } else {
                let zero = ctx.builder.ins().iconst(clif_types::I64, 0);
                ctx.val_map.insert(*dest, zero);
            }
            return Ok(());
        }
    }
    // List and String protocol methods: dispatch on the object's
    // runtime shape, not the class method table.
    let list_special = if ctx.map_vids.contains(object) {
        // Map protocol: without this branch a `map.get(k)`
        // falls into the name-suffix fallback, which can
        // dispatch to an unrelated runtime function whose
        // signature mismatch then fails Cranelift
        // verification.
        match method {
            "get" | "at" => Some(ctx.runtime.rt_map_get_id),
            "insert" | "set" | "add" | "put" => Some(ctx.runtime.rt_map_insert_id),
            _ => None,
        }
    } else if ctx.list_vids.contains(object)
        || ctx
            .val_to_class
            .get(object)
            .map(|c| c.starts_with("List"))
            .unwrap_or(false)
        || (ctx.val_to_class.get(object).is_none()
            && !ctx.string_vids.contains(object)
            && matches!(
                method,
                "push"
                    | "append"
                    | "pop"
                    | "set"
                    | "get"
                    | "at"
                    | "len"
                    | "length"
                    | "count"
                    | "first"
                    | "last"
                    | "is_empty"
                    | "sort"
                    | "reverse"
                    | "clear"
                    | "slice"
            ))
    {
        match method {
            "length" | "count" | "len" => Some(ctx.runtime.rt_list_len_id),
            "get" | "at" => Some(ctx.runtime.rt_list_get_id),
            "set" => Some(ctx.runtime.rt_list_set_id),
            "push" | "append" | "add" => Some(ctx.runtime.rt_list_append_id),
            // v1.4.1: pop/first/last return Outcome<T> objects.
            "pop" => Some(ctx.runtime.rt_list_pop_outcome_id),
            "first" => Some(ctx.runtime.rt_list_first_id),
            "last" => Some(ctx.runtime.rt_list_last_id),
            "sort" => Some(ctx.runtime.rt_list_sort_id),
            "remove_at" => Some(ctx.runtime.rt_list_remove_at_id),
            "remove_value" => Some(ctx.runtime.rt_list_remove_value_id),
            "insert_at" => Some(ctx.runtime.rt_list_insert_at_id),
            "contains" => Some(ctx.runtime.rt_list_contains_id),
            "index_of" => Some(ctx.runtime.rt_list_index_of_id),
            "reverse" => Some(ctx.runtime.rt_list_reverse_id),
            "clear" => Some(ctx.runtime.rt_list_clear_id),
            "slice" => Some(ctx.runtime.rt_list_slice_id),
            "is_empty" => Some(ctx.runtime.rt_list_is_empty_id),
            _ => None,
        }
    } else if ctx.string_vids.contains(object) {
        match method {
            "length" | "count" | "len" | "byte_len" => Some(ctx.runtime.str_len_id),
            "char_len" | "chars" => Some(ctx.runtime.str_chars_id),
            "byte_at" => Some(ctx.runtime.str_byte_at_id),
            "char_at" => Some(ctx.runtime.rt_str_char_at_id),
            _ => None,
        }
    } else {
        None
    };
    if let Some(special_id) = list_special {
        for a in &mut all_args {
            if ctx.builder.func.dfg.value_type(*a) == clif_types::F64 {
                *a = ctx.builder.ins().bitcast(
                    clif_types::I64,
                    cranelift_codegen::ir::MemFlagsData::new(),
                    *a,
                );
            }
        }
        // v1.4.1: element-kind injection. sort takes (mode, elem_kind);
        // remove_value/contains/index_of take elem_kind. The kind is keyed
        // off the MethodCall's element repr ("List<Float>" -> 1,
        // "List<Str>" -> 2, everything else 0), mirroring the C runtime's
        // encoding.
        let elem_kind = list_elem_kind_from_ty(ty);
        match method {
            "sort" => {
                let mode = ctx.builder.ins().iconst(clif_types::I64, 0);
                let kind = ctx.builder.ins().iconst(clif_types::I64, elem_kind as i64);
                all_args.push(mode);
                all_args.push(kind);
            }
            "remove_value" | "contains" | "index_of" => {
                let kind = ctx.builder.ins().iconst(clif_types::I64, elem_kind as i64);
                all_args.push(kind);
            }
            _ => {}
        }
        let callee_ref = ctx
            .module
            .declare_func_in_func(special_id, ctx.builder.func);
        let call_inst = ctx.builder.ins().call(callee_ref, &all_args);
        let results = ctx.builder.inst_results(call_inst);
        if let Some(&r) = results.first() {
            let mut r_val = r;
            if ty == "Float" && ctx.builder.func.dfg.value_type(r) == clif_types::I64 {
                r_val = ctx.builder.ins().bitcast(
                    clif_types::F64,
                    cranelift_codegen::ir::MemFlagsData::new(),
                    r,
                );
            }
            ctx.val_map.insert(*dest, r_val);
            // Only set/push/append return the (possibly
            // reallocated) list itself; length/get
            // return plain ints.
            if matches!(method, "set" | "push" | "append" | "add") {
                ctx.list_vids.insert(*dest);
            }
            // v1.4.1: first/last/pop always yield an Outcome<T> object and
            // insert_at/sort/reverse/clear/slice always yield the list
            // handle, whatever repr the lowering's default ty guessed.
            if matches!(method, "first" | "last" | "pop") {
                let class = if ty.starts_with("Outcome<") {
                    ty.to_string()
                } else {
                    "Outcome<Val>".to_string()
                };
                ctx.val_to_class.insert(*dest, class);
            }
            if matches!(method, "insert_at" | "sort" | "reverse" | "clear" | "slice") {
                ctx.list_vids.insert(*dest);
            }
            // contains/is_empty are Bool results: without the bool tag they
            // would print as 1/0 instead of true/false.
            if ty == "Bool" {
                ctx.bool_vids.insert(*dest);
            }
            if ty.contains("List") || ty.starts_with('[') {
                ctx.list_vids.insert(*dest);
            }
            if ty.contains("Map") {
                ctx.map_vids.insert(*dest);
            }
        }
    } else {
        let (callee_id, callee_name) = {
            let class_matched = if let Some(c) = ctx.val_to_class.get(object) {
                let specialized = format!("{}_{}", c, method);
                if let Some(target) = ctx.func_ids.get(&specialized) {
                    Some((target.0, specialized))
                } else {
                    let base_c = c
                        .split('<')
                        .next()
                        .unwrap_or(c)
                        .split('_')
                        .next()
                        .unwrap_or(c);
                    let base_spec = format!("{}_{}", base_c, method);
                    ctx.func_ids
                        .get(&base_spec)
                        .map(|target| (target.0, base_spec))
                }
            } else {
                None
            };

            let dbg_obj_class = ctx.val_to_class.get(object).cloned();
            class_matched
                .or_else(|| {
                    let cands: Vec<String> = ctx
                        .func_ids
                        .keys()
                        .filter(|k| k.split_once('_').map(|(_, m)| m == method).unwrap_or(false))
                        .cloned()
                        .collect();
                    if std::env::var("DATARA_CODEGEN_TRACE").is_ok() {
                        eprintln!(
                            "[dispatch] fn={} method={:?} obj_class={:?} cands={:?}",
                            ctx.current_func.name, method, dbg_obj_class, cands
                        );
                    }
                    let mut cands_sorted = cands.clone();
                    cands_sorted.sort();
                    cands_sorted
                        .first()
                        .and_then(|k| ctx.func_ids.get(k).map(|v| (v.0, k.clone())))
                })
                .or_else(|| ctx.func_ids.get(method).map(|v| (v.0, method.to_string())))
                .unwrap_or_else(|| {
                    if std::env::var("DATARA_CODEGEN_TRACE").is_ok() {
                        eprintln!(
                            "[datara-codegen] UNRESOLVED METHOD: {} in {}",
                            method, ctx.current_func.name
                        );
                    }
                    (cranelift_module::FuncId::from_u32(0), String::new())
                })
        };
        if callee_name.is_empty() {
            return Err(format!(
                "Code generation failed: unresolved method call '{}' on object with class '{:?}' in function '{}'",
                method,
                ctx.val_to_class.get(object),
                ctx.current_func.name
            ));
        }
        let callee_ref = ctx.module.declare_func_in_func(callee_id, ctx.builder.func);
        let call_inst = ctx.builder.ins().call(callee_ref, &all_args);
        let results = ctx.builder.inst_results(call_inst);
        if let Some(&r) = results.first() {
            let ret_ty = ctx
                .dmir_module
                .functions
                .get(&callee_name)
                .map(|f| f.return_type.as_str())
                .unwrap_or("");
            let obj_class = ctx.val_to_class.get(object).cloned();
            let is_float = ty == "Float"
                || ret_ty == "Float"
                || (ret_ty == "T"
                    && obj_class
                        .as_ref()
                        .map(|c| {
                            c.contains("<Float>")
                                || c.contains("<f64>")
                                || c.ends_with("_Float")
                                || c.ends_with("_Float64")
                        })
                        .unwrap_or(false));
            let mut r_val = r;
            if is_float && ctx.builder.func.dfg.value_type(r) == clif_types::I64 {
                r_val = ctx.builder.ins().bitcast(
                    clif_types::F64,
                    cranelift_codegen::ir::MemFlagsData::new(),
                    r,
                );
            }
            ctx.val_map.insert(*dest, r_val);

            let obj_is_str_class = obj_class
                .as_ref()
                .map(|c| {
                    c.ends_with("String")
                        || c.ends_with("Str")
                        || c.contains("<Str>")
                        || c.contains("<String>")
                        || c.ends_with("_Str")
                        || c.ends_with("_String")
                })
                .unwrap_or(false);
            if ty == "String"
                || ty.contains("Str")
                || ret_ty == "String"
                || ret_ty == "Str"
                || (ret_ty == "T" && obj_is_str_class)
            {
                ctx.string_vids.insert(*dest);
            }
            if ty == "Bool"
                || ret_ty == "Bool"
                || (ret_ty == "T"
                    && obj_class
                        .as_ref()
                        .map(|c| c.contains("<Bool>") || c.ends_with("_Bool"))
                        .unwrap_or(false))
            {
                ctx.bool_vids.insert(*dest);
            }

            if ty.contains("List")
                || ret_ty.contains("List")
                || ty.starts_with('[')
                || ret_ty.starts_with('[')
            {
                ctx.list_vids.insert(*dest);
            }
            if ty.contains("Map") || ret_ty.contains("Map") {
                ctx.map_vids.insert(*dest);
            }

            let effective_class = if ret_ty == "T" {
                obj_class
                    .as_ref()
                    .and_then(|c| {
                        if let (Some(start), Some(end)) = (c.find('<'), c.rfind('>')) {
                            if start < end {
                                Some(c[start + 1..end].trim().to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default()
            } else if !ret_ty.is_empty() {
                ret_ty.to_string()
            } else {
                ty.to_string()
            };
            if effective_class.starts_with("List") || effective_class.starts_with('[') {
                ctx.list_vids.insert(*dest);
            }
            if effective_class.starts_with("Map") {
                ctx.map_vids.insert(*dest);
            }
            let stripped_class = effective_class
                .split('<')
                .next()
                .unwrap_or(&effective_class);
            if !stripped_class.is_empty()
                && stripped_class != "Int"
                && stripped_class != "Float"
                && stripped_class != "Bool"
                && stripped_class != "Str"
                && stripped_class != "String"
                && stripped_class != "List"
                && stripped_class != "Map"
                && stripped_class != "Unit"
                && !stripped_class.starts_with('[')
            {
                ctx.val_to_class.insert(*dest, effective_class.to_string());
            }
        }
    }

    Ok(())
}

/// Lower a call to an imported C function that returns a by-value struct
/// through the hidden sret return-slot ABI.
///

fn list_elem_kind_from_ty(ty: &str) -> i64 {
    if ty.contains("Float") {
        1
    } else if ty.contains("Str") {
        2
    } else {
        0
    }
}

/// Lower `.to_float()` / `.to_int()` on a numeric receiver to the single
/// machine conversion the target provides.
///
/// The receiver's Cranelift type is authoritative: an F64 SSA value is a
/// float, an integer-typed value is an integer. One exception exists for
/// the all-I64 runtime ABI, where a float can travel as its IEEE-754 bit
/// pattern inside an I64 slot; `val_to_class` identifies that case so the
/// conversion bitcasts instead of reinterpreting the bits as an integer.
fn compile_numeric_convert<M: ClifModule>(
    ctx: &mut FunctionCompileCtx<'_, '_, M>,
    dest: &ValueId,
    object: &ValueId,
    method: &str,
) -> Result<(), String> {
    let Some(&obj) = ctx.val_map.get(object) else {
        return Err(format!(
            "Code generation failed: conversion method '{}' on a receiver that was never materialized in function '{}'",
            method, ctx.current_func.name
        ));
    };
    let obj_ty = ctx.builder.func.dfg.value_type(obj);
    let class_says_float = ctx
        .val_to_class
        .get(object)
        .map(|c| c.contains("Float") || c.contains("float"))
        .unwrap_or(false);

    // All-I64 ABI: float bit pattern stored in an integer slot.
    let obj = if obj_ty == clif_types::I64 && class_says_float {
        ctx.builder.ins().bitcast(
            clif_types::F64,
            cranelift_codegen::ir::MemFlagsData::new(),
            obj,
        )
    } else {
        obj
    };
    let obj_ty = ctx.builder.func.dfg.value_type(obj);

    let result = if method == "to_float" {
        match obj_ty {
            clif_types::F64 => obj,
            clif_types::F32 => ctx.builder.ins().fpromote(clif_types::F64, obj),
            clif_types::I64 => ctx.builder.ins().fcvt_from_sint(clif_types::F64, obj),
            clif_types::I32 | clif_types::I16 | clif_types::I8 => {
                let widened = ctx.builder.ins().sextend(clif_types::I64, obj);
                ctx.builder.ins().fcvt_from_sint(clif_types::F64, widened)
            }
            other => {
                return Err(format!(
                    "Code generation failed: '.to_float()' cannot convert a value of Cranelift type {} in function '{}'",
                    other, ctx.current_func.name
                ));
            }
        }
    } else {
        match obj_ty {
            clif_types::F64 => ctx.builder.ins().fcvt_to_sint(clif_types::I64, obj),
            clif_types::F32 => {
                let widened = ctx.builder.ins().fpromote(clif_types::F64, obj);
                ctx.builder.ins().fcvt_to_sint(clif_types::I64, widened)
            }
            clif_types::I64 => obj,
            clif_types::I32 | clif_types::I16 | clif_types::I8 => {
                ctx.builder.ins().sextend(clif_types::I64, obj)
            }
            other => {
                return Err(format!(
                    "Code generation failed: '.to_int()' cannot convert a value of Cranelift type {} in function '{}'",
                    other, ctx.current_func.name
                ));
            }
        }
    };
    ctx.val_map.insert(*dest, result);
    Ok(())
}
