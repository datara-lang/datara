use super::*;
use crate::dmir::*;
use std::collections::{HashMap, HashSet};

impl<'a> LlvmEmitter<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_call(
        &self,
        dest: &ValueId,
        func: &str,
        args: &[ValueId],
        ty: &str,
        module: &Module,
        value_types: &mut HashMap<ValueId, &'static str>,
        bool_vids: &mut HashSet<ValueId>,
        value_classes: &mut HashMap<ValueId, String>,
        address_taken: &HashSet<String>,
        out: &mut String,
    ) -> Result<(), String> {
        let ret_ty = self.dmir_type_to_llvm(ty);
        value_types.insert(*dest, ret_ty);
        if ty == "Bool"
            || module
                .functions
                .get(func)
                .map_or(false, |f| f.return_type == "Bool")
        {
            bool_vids.insert(*dest);
        }
        // Track struct-returning call results so later GetField /
        // SetField instructions resolve against the right layout
        // instead of guessing (E0944).
        if let Some(rc) = class_type_name(ty)
            .or_else(|| module.functions.get(func).map(|f| f.return_type.clone()))
            .or_else(|| module.extern_functions.get(func).map(|(_, r)| r.clone()))
        {
            value_classes.insert(*dest, rc);
        }
        if func == "slice_alloc" || func == "slice_from_ptr" || func == "slice_subslice" {
            value_classes.insert(*dest, "SliceView".to_string());
        }

        if (func == "math_ctz" || func == "ctz") && args.len() == 1 {
            value_types.insert(*dest, "i64");
            out.push_str(&format!(
                "  %v{} = call i64 @llvm.cttz.i64(i64 %v{}, i1 false)\n",
                dest.0, args[0].0
            ));
            return Ok(());
        }
        if (func == "math_shr" || func == "shr") && args.len() == 2 {
            value_types.insert(*dest, "i64");
            out.push_str(&format!(
                "  %v{} = lshr i64 %v{}, %v{}\n",
                dest.0, args[0].0, args[1].0
            ));
            return Ok(());
        }
        if (func == "math_shl" || func == "shl") && args.len() == 2 {
            value_types.insert(*dest, "i64");
            out.push_str(&format!(
                "  %v{} = shl i64 %v{}, %v{}\n",
                dest.0, args[0].0, args[1].0
            ));
            return Ok(());
        }
        if (func == "math_xor" || func == "xor") && args.len() == 2 {
            value_types.insert(*dest, "i64");
            out.push_str(&format!(
                "  %v{} = xor i64 %v{}, %v{}\n",
                dest.0, args[0].0, args[1].0
            ));
            return Ok(());
        }
        if (func == "math_and" || func == "and") && args.len() == 2 {
            value_types.insert(*dest, "i64");
            out.push_str(&format!(
                "  %v{} = and i64 %v{}, %v{}\n",
                dest.0, args[0].0, args[1].0
            ));
            return Ok(());
        }
        if (func == "math_or" || func == "or") && args.len() == 2 {
            value_types.insert(*dest, "i64");
            out.push_str(&format!(
                "  %v{} = or i64 %v{}, %v{}\n",
                dest.0, args[0].0, args[1].0
            ));
            return Ok(());
        }

        if crate::codegen::llvm::simd::try_emit_simd_call(func, dest, args, value_types, out) {
            return Ok(());
        }

        // Map standard runtime names to datara_rt equivalents if needed
        let actual_func = match func {
            "math_sqrt" | "sqrt" => "datara_rt_math_sqrt",
            "math_pow" | "pow" => "datara_rt_math_pow",
            "math_abs" | "abs" => "datara_rt_math_abs",
            "math_sin" | "sin" => "datara_rt_math_sin",
            "math_cos" | "cos" => "datara_rt_math_cos",
            "math_tan" | "tan" => "datara_rt_math_tan",
            "math_floor" | "floor" => "datara_rt_math_floor",
            "math_ceil" | "ceil" => "datara_rt_math_ceil",
            "math_round" | "round" => "datara_rt_math_round",
            "math_min" => "datara_rt_math_min",
            "math_max" => "datara_rt_math_max",
            "math_clamp" | "clamp" => "datara_rt_math_clamp",
            "math_hypot" | "hypot" => "datara_rt_math_hypot",
            "math_log" | "log" => "datara_rt_math_log",
            "math_exp" | "exp" => "datara_rt_math_exp",
            "math_min_int" => "datara_rt_math_min_int",
            "math_max_int" => "datara_rt_math_max_int",
            "math_clamp_int" | "clamp_int" => "datara_rt_math_clamp_int",
            "math_abs_int" => "datara_rt_math_abs_int",
            "sleep" => "datara_rt_sleep",
            "now" | "now_ms" => "datara_rt_now_ms",
            "now_ns" => "datara_rt_now_ns",
            "now_precise_ms" => "datara_rt_now_precise_ms",
            "path_join" => "datara_rt_path_join",
            "file_write" => "datara_rt_file_write",
            "file_read" => "datara_rt_file_read",
            "file_read_bytes" => "datara_rt_file_read_bytes",
            "file_write_bytes" => "datara_rt_file_write_bytes",
            "file_append" => "datara_rt_file_append",
            "file_exists" => "datara_rt_file_exists",
            "file_read_checked" => "datara_rt_file_read_checked",
            "exec" => "datara_rt_exec",
            "exec_utf8" => "datara_rt_exec_utf8",
            "env_get" => "datara_rt_env_get",
            "env_set" => "datara_rt_env_set",
            "env_get_checked" => "datara_rt_env_get_checked",
            "args_count" => "datara_rt_args_count",
            "args_get" => "datara_rt_args_get",
            "dir_list" => "datara_rt_dir_list",
            "path_exists" => "datara_rt_path_exists",
            "exit" => "datara_rt_exit",
            "str_len" | "byte_len" => "datara_rt_str_len",
            "str_chars" | "char_len" => "datara_rt_str_chars",
            "validate_utf8" => "datara_rt_validate_utf8",
            "str_sanitize_utf8" => "datara_rt_str_sanitize_utf8",
            "str_scalar_at" => "datara_rt_str_scalar_at",
            "str_next_offset" => "datara_rt_str_next_offset",
            "str_char_at" | "char_at" => "datara_rt_str_char_at",
            "str_byte_at" | "byte_at" => "datara_rt_str_byte_at",
            "str_trim" => "datara_rt_str_trim",
            "str_to_int" => "datara_rt_str_to_int",
            "int_to_str" => "datara_rt_int_to_str",
            "float_to_str" => "datara_rt_float_to_str",
            "str_contains" => "datara_rt_str_contains",
            "str_starts_with" => "datara_rt_str_starts_with",
            "str_ends_with" => "datara_rt_str_ends_with",
            "str_index_of" => "datara_rt_str_index_of",
            "own_acquire" => "datara_rt_own_acquire",
            "own_release" => "datara_rt_own_release",
            "cap_set_mask" => "datara_rt_cap_set_mask",
            "cap_get_mask" => "datara_rt_cap_get_mask",
            "cap_revoke" => "datara_rt_cap_revoke",
            "cap_grant" => "datara_rt_cap_grant",
            "cap_require" => "datara_rt_cap_require",
            "socket_create" => "datara_rt_socket_create",
            "socket_bind" => "datara_rt_socket_bind",
            "socket_listen" => "datara_rt_socket_listen",
            "socket_accept" => "datara_rt_socket_accept",
            "socket_connect" => "datara_rt_socket_connect",
            "socket_send" => "datara_rt_socket_send",
            "socket_recv" => "datara_rt_socket_recv",
            "socket_close" => "datara_rt_socket_close",
            "http_get" => "datara_rt_http_get",
            "sha256" => "datara_rt_sha256",
            "base64_encode" => "datara_rt_base64_encode",
            "base64_decode" => "datara_rt_base64_decode",
            "bool_to_str" => "datara_rt_bool_to_str",
            "str_split" => "datara_rt_str_split",
            "str_join" => "datara_rt_str_join",
            "str_substring" | "substring" => "datara_rt_str_substring",
            "str_eq" => "datara_rt_str_eq",
            "str_cmp" => "datara_rt_str_cmp",
            "str_from_byte" => "datara_rt_str_from_byte",
            "str_from_bytes" => "datara_rt_str_from_bytes",
            "str_bytes" => "datara_rt_str_bytes",
            "uuid_v4" => "datara_rt_uuid_v4",
            "random_bytes" => "datara_rt_random_bytes",
            "dialog_info" => "datara_rt_dialog_info",
            "dialog_alert" => "datara_rt_dialog_alert",
            "dialog_confirm" => "datara_rt_dialog_confirm",
            "str_repeat" | "repeat" => "datara_rt_str_repeat",
            "str_pad_left" | "pad_left" => "datara_rt_str_pad_left",
            "str_pad_right" | "pad_right" => "datara_rt_str_pad_right",
            "str_replace" | "replace" => "datara_rt_str_replace",
            "str_to_upper" | "to_upper" => "datara_rt_str_to_upper",
            "str_to_lower" | "to_lower" => "datara_rt_str_to_lower",
            "str_substr" => "datara_rt_str_substring",
            "format_percent" => "datara_rt_format_percent",
            "format_int_with_commas" => "datara_rt_format_int_with_commas",
            "range_str" => "datara_rt_range_str",
            "system" | "process_run" => "datara_rt_system",
            "process_output" => "datara_rt_exec",
            "num_workers" => "datara_rt_num_workers",
            "schedule_run" => "datara_rt_schedule_run",
            "schedule_cancel" => "datara_rt_schedule_cancel",
            "spawn" => "spawn",
            "join" => "join",
            "join_timeout" => "join_timeout",
            "channel_create" => "channel_create",
            "channel_new" => "channel_new",
            "channel_send" => "channel_send",
            "channel_recv" => "channel_recv",
            "channel_try_recv" => "channel_try_recv",
            "channel_close" => "channel_close",
            "channel_len" => "channel_len",
            "scratch_enter" => "scratch_enter",
            "scratch_alloc" => "scratch_alloc",
            "scratch_exit" => "scratch_exit",
            "scratch_promote" => "scratch_promote",
            "parallel_for" => "parallel_for",
            other => other,
        };

        let is_str_concat = actual_func.starts_with("datara_rt_str_concat");
        let mut converted_args = Vec::new();
        // http_get historically had a zero-arg builtin signature;
        // keep `http_get()` calls valid by padding a null URL.
        if actual_func.ends_with("http_get") && args.is_empty() {
            converted_args.push("ptr null".to_string());
        }
        let is_set_f64 = (actual_func == "datara_rt_list_set"
            || actual_func == "datara_rt_list_set_unchecked")
            && args.len() >= 3
            && value_types.get(&args[2]).copied() == Some("double");
        let is_append_f64 = actual_func == "datara_rt_list_append_unchecked"
            && args.len() >= 2
            && value_types.get(&args[1]).copied() == Some("double");
        let actual_func = if is_set_f64 {
            if actual_func == "datara_rt_list_set_unchecked" {
                "datara_rt_list_set_f64_unchecked"
            } else {
                "datara_rt_list_set_f64"
            }
        } else if is_append_f64 {
            "datara_rt_list_append_f64_unchecked"
        } else {
            actual_func
        };

        for (idx, a) in args.iter().enumerate() {
            let aty = value_types.get(a).copied().unwrap_or("i64");
            // datara_rt_exit takes i32: truncate the i64 code.
            if actual_func == "datara_rt_exit" && idx == 0 {
                let tmp = format!("%exit_code_{}_{}", dest.0, a.0);
                out.push_str(&format!("  {} = trunc i64 %v{} to i32\n", tmp, a.0));
                converted_args.push(format!("i32 {}", tmp));
                continue;
            }
            let is_list_slot_f64 = aty == "double"
                && !is_set_f64
                && (((actual_func == "datara_rt_list_append"
                    || actual_func == "datara_rt_list_append_unchecked")
                    && idx == 1)
                    || (actual_func == "datara_rt_list_set" && idx == 2)
                    || (actual_func == "datara_rt_list_create_repeat" && idx == 1)
                    || (actual_func.starts_with("datara_rt_list_create_")
                        && actual_func != "datara_rt_list_create_repeat"
                        && actual_func != "datara_rt_list_create_capacity"));
            if is_list_slot_f64 {
                let tmp = format!("%bitcast_f64_i64_{}_{}", dest.0, a.0);
                out.push_str(&format!("  {} = bitcast double %v{} to i64\n", tmp, a.0));
                converted_args.push(format!("i64 {}", tmp));
            } else if is_str_concat && aty != "ptr" {
                let tmp = format!("%sc_arg_{}_{}", dest.0, idx);
                out.push_str(&format!(
                    "  {} = call ptr @datara_rt_int_to_str(i64 %v{})\n",
                    tmp, a.0
                ));
                converted_args.push(format!("ptr {}", tmp));
            } else {
                converted_args.push(format!("{} %v{}", aty, a.0));
            }
        }
        let args_str = converted_args.join(", ");

        let is_internal = actual_func != "main"
            && module.functions.contains_key(actual_func)
            && !module.extern_functions.contains_key(actual_func)
            && !address_taken.contains(actual_func);
        let call_prefix = if is_internal { "call fastcc" } else { "call" };

        if ret_ty == "void" {
            out.push_str(&format!(
                "  {} void @{}({})\n",
                call_prefix, actual_func, args_str
            ));
        } else if actual_func == "datara_rt_list_get_f64_unchecked"
            || (actual_func == "datara_rt_list_get_unchecked" && ret_ty == "double")
            || (actual_func == "datara_rt_list_get" && ret_ty == "double")
        {
            let callee = if actual_func.contains("unchecked") {
                "datara_rt_list_get_f64_unchecked"
            } else {
                "datara_rt_list_get_f64"
            };
            out.push_str(&format!(
                "  %v{} = {} double @{}({})\n",
                dest.0, call_prefix, callee, args_str
            ));
        } else {
            out.push_str(&format!(
                "  %v{} = {} {} @{}({})\n",
                dest.0, call_prefix, ret_ty, actual_func, args_str
            ));
        }
        if !actual_func.contains("unchecked")
            && actual_func.starts_with("datara_rt_list_get")
            && args.len() >= 2
        {
            out.push_str(&format!(
                "  %fvrp_bce_min_{} = icmp sge i64 %v{}, 0\n",
                dest.0, args[1].0
            ));
            out.push_str(&format!(
                "  call void @llvm.assume(i1 %fvrp_bce_min_{})\n",
                dest.0
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_method_call(
        &self,
        dest: &ValueId,
        object: &ValueId,
        method: &str,
        args: &[ValueId],
        ty: &str,
        module: &Module,
        value_types: &mut HashMap<ValueId, &'static str>,
        bool_vids: &mut HashSet<ValueId>,
        value_classes: &mut HashMap<ValueId, String>,
        address_taken: &HashSet<String>,
        out: &mut String,
    ) -> Result<(), String> {
        // Numeric conversion intrinsics (Gate 7 explicit casts):
        // a single LLVM conversion instruction, no runtime call.
        if (method == "to_float" || method == "to_int") && args.is_empty() {
            let obj_ty = value_types.get(object).copied().unwrap_or("i64");
            if method == "to_float" {
                value_types.insert(*dest, "double");
                if obj_ty == "i64" {
                    out.push_str(&format!(
                        "  %v{} = sitofp i64 %v{} to double\n",
                        dest.0, object.0
                    ));
                } else {
                    // Identity for an already-double receiver; freeze
                    // preserves the bit pattern including -0.0.
                    out.push_str(&format!("  %v{} = freeze double %v{}\n", dest.0, object.0));
                }
            } else {
                value_types.insert(*dest, "i64");
                if obj_ty == "double" {
                    out.push_str(&format!(
                        "  %v{} = fptosi double %v{} to i64\n",
                        dest.0, object.0
                    ));
                } else {
                    out.push_str(&format!("  %v{} = add i64 0, %v{}\n", dest.0, object.0));
                }
            }
            return Ok(());
        }
        if value_classes
            .get(object)
            .map(|c| c == "SliceView")
            .unwrap_or(false)
        {
            let fn_name = format!("SliceView_{}", method);
            let mut args_val = vec![format!("ptr %v{}", object.0)];
            for a in args {
                let aty = value_types.get(a).copied().unwrap_or("i64");
                args_val.push(format!("{} %v{}", aty, a.0));
            }
            let call_ret_ty = if method == "subslice" {
                value_classes.insert(*dest, "SliceView".to_string());
                value_types.insert(*dest, "ptr");
                "ptr"
            } else if method.starts_with("write_") || method == "set_byte" || method == "free" {
                value_types.insert(*dest, "void");
                "void"
            } else {
                value_types.insert(*dest, "i64");
                "i64"
            };
            if call_ret_ty == "void" {
                out.push_str(&format!(
                    "  call void @{}({})\n",
                    fn_name,
                    args_val.join(", ")
                ));
            } else {
                out.push_str(&format!(
                    "  %v{} = call {} @{}({})\n",
                    dest.0,
                    call_ret_ty,
                    fn_name,
                    args_val.join(", ")
                ));
            }
            return Ok(());
        }

        if value_classes
            .get(object)
            .map(|c| c == "VolatilePtr")
            .unwrap_or(false)
        {
            let fn_name = format!("VolatilePtr_{}", method);
            let mut args_val = vec![format!("ptr %v{}", object.0)];
            for a in args {
                let aty = value_types.get(a).copied().unwrap_or("i64");
                args_val.push(format!("{} %v{}", aty, a.0));
            }
            let call_ret_ty = if method.starts_with("write") {
                value_types.insert(*dest, "void");
                "void"
            } else {
                value_types.insert(*dest, "i64");
                "i64"
            };
            if call_ret_ty == "void" {
                out.push_str(&format!(
                    "  call void @{}({})\n",
                    fn_name,
                    args_val.join(", ")
                ));
            } else {
                out.push_str(&format!(
                    "  %v{} = call {} @{}({})\n",
                    dest.0,
                    call_ret_ty,
                    fn_name,
                    args_val.join(", ")
                ));
            }
            return Ok(());
        }

        let mut ret_ty = self.dmir_type_to_llvm(ty);

        let actual_func = match method {
            "len" | "count" | "length" => {
                ret_ty = "i64";
                "datara_rt_list_len".to_string()
            }
            "byte_len" => {
                ret_ty = "i64";
                "datara_rt_str_len".to_string()
            }
            "char_len" => {
                ret_ty = "i64";
                "datara_rt_str_chars".to_string()
            }
            "str_byte_at" | "byte_at" => {
                ret_ty = "i64";
                "datara_rt_str_byte_at".to_string()
            }
            "char_at" => {
                ret_ty = "ptr";
                "datara_rt_str_char_at".to_string()
            }
            "append" | "push" => {
                ret_ty = "ptr";
                "datara_rt_list_append".to_string()
            }
            "append_unchecked" | "push_unchecked" => {
                ret_ty = "ptr";
                "datara_rt_list_append_unchecked".to_string()
            }
            "get" | "at" => "datara_rt_list_get".to_string(),
            "set" => "datara_rt_list_set".to_string(),
            "insert" => "datara_rt_map_insert".to_string(),
            // v1.4.1: full List<T> protocol. pop/first/last return
            // Outcome<T> objects; the mutators and slice return the
            // list handle; the result repr flows from the lowering's
            // ty (List<...>/Outcome<...> map to ptr in
            // dmir_type_to_llvm).
            "pop" => "datara_rt_list_pop_outcome".to_string(),
            "first" => "datara_rt_list_first".to_string(),
            "last" => "datara_rt_list_last".to_string(),
            "sort" => "datara_rt_list_sort".to_string(),
            "remove_at" => "datara_rt_list_remove_at".to_string(),
            "remove_value" => "datara_rt_list_remove_value".to_string(),
            "insert_at" => "datara_rt_list_insert_at".to_string(),
            "contains" => "datara_rt_list_contains".to_string(),
            "index_of" => "datara_rt_list_index_of".to_string(),
            "reverse" => "datara_rt_list_reverse".to_string(),
            "clear" => "datara_rt_list_clear".to_string(),
            "slice" => "datara_rt_list_slice".to_string(),
            "is_empty" => "datara_rt_list_is_empty".to_string(),
            _ if module.functions.contains_key(method) => method.to_string(),
            _ => {
                // Collect suffix matches and pick the smallest name
                // deterministically; HashMap iteration order previously
                // made the dispatch target vary run to run.
                let mut candidates: Vec<&String> = module
                    .functions
                    .keys()
                    .filter(|k| {
                        k.len() > method.len() + 1
                            && k.ends_with(method)
                            && k.as_bytes()[k.len() - method.len() - 1] == b'_'
                    })
                    .collect();
                candidates.sort();
                candidates
                    .first()
                    .map(|k| (*k).clone())
                    .unwrap_or_else(|| method.to_string())
            }
        };

        value_types.insert(*dest, ret_ty);

        // Track struct-returning method results so later GetField /
        // SetField instructions resolve against the right layout
        // instead of guessing (E0944).
        if let Some(rf) = module.functions.get(&actual_func)
            && let Some(rc) = class_type_name(&rf.return_type)
        {
            value_classes.insert(*dest, rc);
        }

        let obj_ty = value_types.get(object).copied().unwrap_or("ptr");
        let obj_arg = if obj_ty == "i64" {
            let tmp = format!("%mcast_{}_{}", object.0, dest.0);
            out.push_str(&format!("  {} = inttoptr i64 %v{} to ptr\n", tmp, object.0));
            format!("ptr {}", tmp)
        } else {
            format!("ptr %v{}", object.0)
        };

        let is_set_f64 = (actual_func == "datara_rt_list_set"
            || actual_func == "datara_rt_list_set_unchecked")
            && args.len() >= 2
            && value_types.get(&args[1]).copied() == Some("double");
        let is_append_f64 = actual_func == "datara_rt_list_append_unchecked"
            && !args.is_empty()
            && value_types.get(&args[0]).copied() == Some("double");
        let actual_func = if is_set_f64 {
            if actual_func == "datara_rt_list_set_unchecked" {
                "datara_rt_list_set_f64_unchecked".to_string()
            } else {
                "datara_rt_list_set_f64".to_string()
            }
        } else if is_append_f64 {
            "datara_rt_list_append_f64_unchecked".to_string()
        } else {
            actual_func
        };

        let mut all_args = vec![obj_arg];
        for (idx, a) in args.iter().enumerate() {
            let aty = value_types.get(a).copied().unwrap_or("i64");
            if (actual_func == "datara_rt_list_append" && aty == "double")
                || (actual_func == "datara_rt_list_set" && idx == 1 && aty == "double")
            {
                let tmp = format!("%mcast_f64_i64_{}_{}", dest.0, a.0);
                out.push_str(&format!("  {} = bitcast double %v{} to i64\n", tmp, a.0));
                all_args.push(format!("i64 {}", tmp));
            } else {
                all_args.push(format!("{} %v{}", aty, a.0));
            }
        }
        // v1.4.1: element-kind injection, keyed off the lowering's
        // element repr exactly like the Cranelift backend (sort takes
        // mode + elem_kind; remove_value/contains/index_of take
        // elem_kind).
        let llvm_elem_kind = if ty.contains("Float") {
            1i64
        } else if ty.contains("Str") {
            2
        } else {
            0
        };
        match method {
            "sort" => {
                all_args.push("i64 0".to_string());
                all_args.push(format!("i64 {}", llvm_elem_kind));
            }
            "remove_value" | "contains" | "index_of" => {
                all_args.push(format!("i64 {}", llvm_elem_kind));
            }
            _ => {}
        }
        let args_str = all_args.join(", ");

        let is_internal = actual_func != "main"
            && module.functions.contains_key(&actual_func)
            && !module.extern_functions.contains_key(&actual_func)
            && !address_taken.contains(&actual_func);
        let call_prefix = if is_internal { "call fastcc" } else { "call" };

        if ret_ty == "void" {
            out.push_str(&format!(
                "  {} void @{}({})\n",
                call_prefix, actual_func, args_str
            ));
        } else if actual_func == "datara_rt_list_get_f64_unchecked"
            || (actual_func == "datara_rt_list_get_unchecked" && ret_ty == "double")
            || (actual_func == "datara_rt_list_get" && ret_ty == "double")
        {
            let callee = if actual_func.contains("unchecked") {
                "datara_rt_list_get_f64_unchecked"
            } else {
                "datara_rt_list_get_f64"
            };
            out.push_str(&format!(
                "  %v{} = {} double @{}({})\n",
                dest.0, call_prefix, callee, args_str
            ));
        } else {
            out.push_str(&format!(
                "  %v{} = {} {} @{}({})\n",
                dest.0, call_prefix, ret_ty, actual_func, args_str
            ));
        }
        if !actual_func.contains("unchecked")
            && actual_func.starts_with("datara_rt_list_get")
            && !args.is_empty()
        {
            out.push_str(&format!(
                "  %fvrp_mbce_min_{} = icmp sge i64 %v{}, 0\n",
                dest.0, args[0].0
            ));
            out.push_str(&format!(
                "  call void @llvm.assume(i1 %fvrp_mbce_min_{})\n",
                dest.0
            ));
        }
        // v1.4.1: record the Outcome class on checked-accessor
        // results so later GetField (unwrap/`?`/err) resolves the
        // field offset against the Outcome layout (E0944-safe).
        if ty.starts_with("Outcome<") || matches!(method, "first" | "last" | "pop") {
            let outcome_class = if ty.starts_with("Outcome<") {
                ty.to_string()
            } else {
                "Outcome".to_string()
            };
            value_classes.insert(*dest, outcome_class);
        }
        // contains/is_empty are Bool results: without the bool tag
        // they would print as 1/0 instead of true/false.
        if ty == "Bool" {
            bool_vids.insert(*dest);
        }
        Ok(())
    }
}
