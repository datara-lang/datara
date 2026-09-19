use super::*;
use crate::dmir::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

static FIELD_PTR_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl<'a> LlvmEmitter<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_instruction(
        &self,
        inst: &Inst,
        module: &Module,
        strings: &HashMap<String, usize>,
        value_types: &mut HashMap<ValueId, &'static str>,
        var_types: &mut HashMap<String, &'static str>,
        var_classes: &mut HashMap<String, String>,
        value_classes: &mut HashMap<ValueId, String>,
        bool_vids: &mut HashSet<ValueId>,
        bool_vars: &mut HashSet<String>,
        out: &mut String,
        fn_name: &str,
        types: &TypeChecker,
        range_metadata: &mut HashMap<(i64, i64), usize>,
        next_meta_id: &mut usize,
        address_taken: &HashSet<String>,
        stack_structs: &HashSet<ValueId>,
    ) -> Result<(), String> {
        match inst {
            Inst::ConstInt { dest, value } => {
                value_types.insert(*dest, "i64");
                out.push_str(&format!("  %v{} = add i64 0, {}\n", dest.0, value));
            }
            Inst::ConstFloat { dest, value } => {
                value_types.insert(*dest, "double");
                out.push_str(&format!(
                    "  %v{} = fadd double 0.0, {:.17}\n",
                    dest.0, value
                ));
            }
            Inst::ConstBool { dest, value } => {
                value_types.insert(*dest, "i64");
                bool_vids.insert(*dest);
                let b_val = if *value { 1 } else { 0 };
                out.push_str(&format!("  %v{} = add i64 0, {}\n", dest.0, b_val));
            }
            Inst::ConstStr { dest, value } => {
                value_types.insert(*dest, "ptr");
                let str_id = strings.get(value).copied().unwrap_or(0);
                out.push_str(&format!(
                    "  %v{} = getelementptr inbounds [0 x i8], ptr @.str.{}, i64 0, i64 0\n",
                    dest.0, str_id
                ));
            }
            Inst::LoadVar { dest, name } => {
                let vty = var_types.get(name).copied().unwrap_or("i64");
                value_types.insert(*dest, vty);
                if bool_vars.contains(name) {
                    bool_vids.insert(*dest);
                }
                if let Some(c) = var_classes.get(name) {
                    value_classes.insert(*dest, c.clone());
                }
                let align = if vty.starts_with('<') {
                    match vty {
                        "<16 x float>" => 64,
                        "<8 x float>" | "<4 x double>" | "<8 x i32>" => 32,
                        _ => 16,
                    }
                } else {
                    8
                };
                let ptr_str = if module.globals.contains_key(name) {
                    format!("@datara_global_{}", name)
                } else {
                    format!("%var_{}", name)
                };
                if let Some((min, max)) = get_range_for_var(fn_name, name, None, types) {
                    let high = max.saturating_add(1);
                    let meta_id = *range_metadata.entry((min, high)).or_insert_with(|| {
                        let id = *next_meta_id;
                        *next_meta_id += 1;
                        id
                    });
                    out.push_str(&format!(
                        "  %v{} = load {}, ptr {}, align {}, !range !{}\n",
                        dest.0, vty, ptr_str, align, meta_id
                    ));
                } else {
                    out.push_str(&format!(
                        "  %v{} = load {}, ptr {}, align {}\n",
                        dest.0, vty, ptr_str, align
                    ));
                }
            }
            Inst::AssignVar { name, value } => {
                let vty = if let Some((gty, _)) = module.globals.get(name) {
                    self.dmir_type_to_llvm(gty)
                } else {
                    value_types
                        .get(value)
                        .copied()
                        .or_else(|| var_types.get(name).copied())
                        .unwrap_or("i64")
                };
                var_types.insert(name.clone(), vty);
                if let Some(c) = value_classes.get(value) {
                    var_classes.insert(name.clone(), c.clone());
                } else {
                    var_classes.remove(name);
                }
                if bool_vids.contains(value) {
                    bool_vars.insert(name.clone());
                } else {
                    bool_vars.remove(name);
                }
                let align = if vty.starts_with('<') {
                    match vty {
                        "<16 x float>" => 64,
                        "<8 x float>" | "<4 x double>" | "<8 x i32>" => 32,
                        _ => 16,
                    }
                } else {
                    8
                };
                let ptr_str = if module.globals.contains_key(name) {
                    format!("@datara_global_{}", name)
                } else {
                    format!("%var_{}", name)
                };
                out.push_str(&format!(
                    "  store {} %v{}, ptr {}, align {}\n",
                    vty, value.0, ptr_str, align
                ));
                if let Some((min, max)) = get_range_for_var(fn_name, name, None, types) {
                    out.push_str(&format!(
                        "  %fvrp_amin_{}_{} = icmp sge i64 %v{}, {}\n",
                        name, value.0, value.0, min
                    ));
                    out.push_str(&format!(
                        "  call void @llvm.assume(i1 %fvrp_amin_{}_{})\n",
                        name, value.0
                    ));
                    out.push_str(&format!(
                        "  %fvrp_amax_{}_{} = icmp sle i64 %v{}, {}\n",
                        name, value.0, value.0, max
                    ));
                    out.push_str(&format!(
                        "  call void @llvm.assume(i1 %fvrp_amax_{}_{})\n",
                        name, value.0
                    ));
                }
            }
            Inst::BinOp {
                dest,
                op,
                left,
                right,
                ty,
            } => {
                let l_ty = value_types.get(left).copied().unwrap_or("i64");
                let r_ty = value_types.get(right).copied().unwrap_or("i64");
                let is_float = ty == "Float" || l_ty == "double" || r_ty == "double";
                let is_str = ty == "Str" || ty == "String";

                if (is_str || l_ty == "ptr" || r_ty == "ptr") && op == "+" {
                    value_types.insert(*dest, "ptr");
                    let mut conv = |vid: &ValueId, ty: &str, side: &str| -> String {
                        if ty == "ptr" {
                            return format!("%v{}", vid.0);
                        }
                        let tmp = format!("%str_conv_{}_{}", side, dest.0);
                        let fnc = if ty == "double" {
                            "datara_rt_float_to_str(double"
                        } else {
                            "datara_rt_int_to_str(i64"
                        };
                        out.push_str(&format!("  {} = call ptr @{} %v{})\n", tmp, fnc, vid.0));
                        tmp
                    };
                    let left_s = conv(left, l_ty, "l");
                    let right_s = conv(right, r_ty, "r");
                    out.push_str(&format!(
                        "  %v{} = call ptr @datara_rt_str_concat(ptr {}, ptr {})\n",
                        dest.0, left_s, right_s
                    ));
                } else if is_float {
                    let mut fconv = |vid: &ValueId, ty: &str, side: &str| -> String {
                        if ty == "i64" {
                            let tmp = format!("%fconv_{}_{}", side, dest.0);
                            out.push_str(&format!(
                                "  {} = sitofp i64 %v{} to double\n",
                                tmp, vid.0
                            ));
                            tmp
                        } else {
                            format!("%v{}", vid.0)
                        }
                    };
                    let left_v = fconv(left, l_ty, "l");
                    let right_v = fconv(right, r_ty, "r");

                    match op.as_str() {
                        "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                            value_types.insert(*dest, "i64");
                            bool_vids.insert(*dest);
                            let fcmp_op = match op.as_str() {
                                "==" => "oeq",
                                "!=" => "one",
                                "<" => "olt",
                                "<=" => "ole",
                                ">" => "ogt",
                                ">=" => "oge",
                                _ => "oeq",
                            };
                            let cmp_temp = format!("%fcmp_{}", dest.0);
                            out.push_str(&format!(
                                "  {} = fcmp {} double {}, {}\n",
                                cmp_temp, fcmp_op, left_v, right_v
                            ));
                            out.push_str(&format!(
                                "  %v{} = zext i1 {} to i64\n",
                                dest.0, cmp_temp
                            ));
                        }
                        _ => {
                            value_types.insert(*dest, "double");
                            let llvm_op = match op.as_str() {
                                "+" => "fadd",
                                "-" => "fsub",
                                "*" => "fmul",
                                "/" => "fdiv",
                                _ => "fadd",
                            };
                            out.push_str(&format!(
                                "  %v{} = {} double {}, {}\n",
                                dest.0, llvm_op, left_v, right_v
                            ));
                        }
                    }
                } else {
                    match op.as_str() {
                        "+" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_checked_add(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "-" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_checked_sub(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "*" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_checked_mul(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "/" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_checked_div(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "%" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_checked_rem(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "wrapping_+" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = add i64 %v{}, %v{}\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "wrapping_-" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = sub i64 %v{}, %v{}\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "wrapping_*" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = mul i64 %v{}, %v{}\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "saturating_+" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_saturating_add(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "saturating_-" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_saturating_sub(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "saturating_*" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = call i64 @datara_rt_saturating_mul(i64 %v{}, i64 %v{})\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                            value_types.insert(*dest, "i64");
                            bool_vids.insert(*dest);
                            if is_str {
                                let l_ptr = if l_ty == "ptr" {
                                    format!("%v{}", left.0)
                                } else {
                                    let tmp = format!("%l_ptr_{}", dest.0);
                                    out.push_str(&format!(
                                        "  {} = inttoptr i64 %v{} to ptr\n",
                                        tmp, left.0
                                    ));
                                    tmp
                                };
                                let r_ptr = if r_ty == "ptr" {
                                    format!("%v{}", right.0)
                                } else {
                                    let tmp = format!("%r_ptr_{}", dest.0);
                                    out.push_str(&format!(
                                        "  {} = inttoptr i64 %v{} to ptr\n",
                                        tmp, right.0
                                    ));
                                    tmp
                                };
                                let cmp_call = format!("%strcmp_{}", dest.0);
                                out.push_str(&format!(
                                    "  {} = call i64 @datara_rt_str_cmp(ptr {}, ptr {})\n",
                                    cmp_call, l_ptr, r_ptr
                                ));
                                let cmp_op = match op.as_str() {
                                    "==" => "eq",
                                    "!=" => "ne",
                                    "<" => "slt",
                                    "<=" => "sle",
                                    ">" => "sgt",
                                    ">=" => "sge",
                                    _ => "eq",
                                };
                                let cmp_temp = format!("%cmp_{}", dest.0);
                                out.push_str(&format!(
                                    "  {} = icmp {} i64 {}, 0\n",
                                    cmp_temp, cmp_op, cmp_call
                                ));
                                out.push_str(&format!(
                                    "  %v{} = zext i1 {} to i64\n",
                                    dest.0, cmp_temp
                                ));
                            } else if l_ty == "ptr" && r_ty == "ptr" {
                                let cmp_op = match op.as_str() {
                                    "==" => "eq",
                                    "!=" => "ne",
                                    "<" => "ult",
                                    "<=" => "ule",
                                    ">" => "ugt",
                                    ">=" => "uge",
                                    _ => "eq",
                                };
                                let cmp_temp = format!("%cmp_{}", dest.0);
                                out.push_str(&format!(
                                    "  {} = icmp {} ptr %v{}, %v{}\n",
                                    cmp_temp, cmp_op, left.0, right.0
                                ));
                                out.push_str(&format!(
                                    "  %v{} = zext i1 {} to i64\n",
                                    dest.0, cmp_temp
                                ));
                            } else if l_ty == "ptr" || r_ty == "ptr" {
                                let l_val = if l_ty == "ptr" {
                                    let tmp = format!("%l_i64_{}", dest.0);
                                    out.push_str(&format!(
                                        "  {} = ptrtoint ptr %v{} to i64\n",
                                        tmp, left.0
                                    ));
                                    tmp
                                } else {
                                    format!("%v{}", left.0)
                                };
                                let r_val = if r_ty == "ptr" {
                                    let tmp = format!("%r_i64_{}", dest.0);
                                    out.push_str(&format!(
                                        "  {} = ptrtoint ptr %v{} to i64\n",
                                        tmp, right.0
                                    ));
                                    tmp
                                } else {
                                    format!("%v{}", right.0)
                                };
                                let cmp_op = match op.as_str() {
                                    "==" => "eq",
                                    "!=" => "ne",
                                    "<" => "slt",
                                    "<=" => "sle",
                                    ">" => "sgt",
                                    ">=" => "sge",
                                    _ => "eq",
                                };
                                let cmp_temp = format!("%cmp_{}", dest.0);
                                out.push_str(&format!(
                                    "  {} = icmp {} i64 {}, {}\n",
                                    cmp_temp, cmp_op, l_val, r_val
                                ));
                                out.push_str(&format!(
                                    "  %v{} = zext i1 {} to i64\n",
                                    dest.0, cmp_temp
                                ));
                            } else {
                                let cmp_op = match op.as_str() {
                                    "==" => "eq",
                                    "!=" => "ne",
                                    "<" => "slt",
                                    "<=" => "sle",
                                    ">" => "sgt",
                                    ">=" => "sge",
                                    _ => "eq",
                                };
                                let cmp_temp = format!("%cmp_{}", dest.0);
                                out.push_str(&format!(
                                    "  {} = icmp {} i64 %v{}, %v{}\n",
                                    cmp_temp, cmp_op, left.0, right.0
                                ));
                                out.push_str(&format!(
                                    "  %v{} = zext i1 {} to i64\n",
                                    dest.0, cmp_temp
                                ));
                            }
                        }
                        "&" | "&&" => {
                            value_types.insert(*dest, "i64");
                            if bool_vids.contains(left) || bool_vids.contains(right) {
                                bool_vids.insert(*dest);
                            }
                            out.push_str(&format!(
                                "  %v{} = and i64 %v{}, %v{}\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "|" | "||" => {
                            value_types.insert(*dest, "i64");
                            if bool_vids.contains(left) || bool_vids.contains(right) {
                                bool_vids.insert(*dest);
                            }
                            out.push_str(&format!(
                                "  %v{} = or i64 %v{}, %v{}\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        "^" => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = xor i64 %v{}, %v{}\n",
                                dest.0, left.0, right.0
                            ));
                        }
                        // Shift counts are masked to 0..63 first: LLVM marks
                        // `shl`/`ashr` with an out-of-range count as poison,
                        // and the mask makes runtime behavior deterministic
                        // (same rule as WASM i64.shl / i64.shr_s and x86).
                        "<<" => {
                            value_types.insert(*dest, "i64");
                            let shift_temp = format!("%shift_{}", dest.0);
                            out.push_str(&format!(
                                "  {} = and i64 %v{}, 63\n",
                                shift_temp, right.0
                            ));
                            out.push_str(&format!(
                                "  %v{} = shl i64 %v{}, {}\n",
                                dest.0, left.0, shift_temp
                            ));
                        }
                        ">>" => {
                            value_types.insert(*dest, "i64");
                            let shift_temp = format!("%shift_{}", dest.0);
                            out.push_str(&format!(
                                "  {} = and i64 %v{}, 63\n",
                                shift_temp, right.0
                            ));
                            out.push_str(&format!(
                                "  %v{} = ashr i64 %v{}, {}\n",
                                dest.0, left.0, shift_temp
                            ));
                        }
                        _ => {
                            value_types.insert(*dest, "i64");
                            out.push_str(&format!(
                                "  %v{} = add i64 %v{}, %v{}\n",
                                dest.0, left.0, right.0
                            ));
                        }
                    }
                }
            }
            Inst::UnOp {
                dest,
                op,
                operand,
                ty,
            } => {
                if ty == "Float" {
                    value_types.insert(*dest, "double");
                    out.push_str(&format!("  %v{} = fneg double %v{}\n", dest.0, operand.0));
                } else if op == "!" {
                    value_types.insert(*dest, "i64");
                    bool_vids.insert(*dest);
                    // Logical NOT: any nonzero value is truthy, so compare
                    // against zero instead of xor 1 (wrong for non-canonical
                    // bools, e.g. 2 -> 3, still truthy).
                    let cmp_temp = format!("%not_c_{}", dest.0);
                    out.push_str(&format!(
                        "  {} = icmp eq i64 %v{}, 0\n",
                        cmp_temp, operand.0
                    ));
                    out.push_str(&format!("  %v{} = zext i1 {} to i64\n", dest.0, cmp_temp));
                } else if op == "copy" || op == "await" {
                    let oty = value_types.get(operand).copied().unwrap_or("i64");
                    value_types.insert(*dest, oty);
                    if bool_vids.contains(operand) {
                        bool_vids.insert(*dest);
                    }
                    if oty == "double" {
                        out.push_str(&format!(
                            "  %v{} = fadd double %v{}, 0.0\n",
                            dest.0, operand.0
                        ));
                    } else if oty == "ptr" {
                        out.push_str(&format!(
                            "  %v{} = getelementptr inbounds i8, ptr %v{}, i64 0\n",
                            dest.0, operand.0
                        ));
                    } else {
                        out.push_str(&format!("  %v{} = or i64 %v{}, 0\n", dest.0, operand.0));
                    }
                } else {
                    value_types.insert(*dest, "i64");
                    out.push_str(&format!("  %v{} = sub i64 0, %v{}\n", dest.0, operand.0));
                }
            }
            Inst::Call {
                dest,
                func,
                args,
                ty,
            } => {
                self.emit_call(
                    dest,
                    func,
                    args,
                    ty,
                    module,
                    value_types,
                    bool_vids,
                    value_classes,
                    address_taken,
                    out,
                )?;
            }
            Inst::MethodCall {
                dest,
                object,
                method,
                args,
                ty,
            } => {
                self.emit_method_call(
                    dest,
                    object,
                    method,
                    args,
                    ty,
                    module,
                    value_types,
                    bool_vids,
                    value_classes,
                    address_taken,
                    out,
                )?;
            }
            Inst::StructInit {
                dest,
                class_name,
                fields,
            } => {
                value_types.insert(*dest, "ptr");
                value_classes.insert(*dest, class_name.clone());
                if !stack_structs.contains(dest) {
                    let byte_size = fields.len().saturating_mul(8).max(8);
                    out.push_str(&format!(
                        "  %v{} = call ptr @datara_rt_pool_alloc(i64 {})\n",
                        dest.0, byte_size
                    ));
                }
                for (idx, (_, val_id)) in fields.iter().enumerate() {
                    let f_ty = value_types.get(val_id).copied().unwrap_or("i64");
                    let gep_reg = format!("%gep_{}_{}", dest.0, idx);
                    out.push_str(&format!(
                        "  {} = getelementptr inbounds i8, ptr %v{}, i64 {}\n",
                        gep_reg,
                        dest.0,
                        idx.saturating_mul(8)
                    ));
                    out.push_str(&format!(
                        "  store {} %v{}, ptr {}, align 8\n",
                        f_ty, val_id.0, gep_reg
                    ));
                }
            }
            Inst::GetField {
                dest,
                object,
                field,
                ty,
            } => {
                let f_ty = self.dmir_type_to_llvm(ty);
                value_types.insert(*dest, f_ty);

                let offset = self.find_field_offset(
                    module,
                    value_classes.get(object).map(|c| c.as_str()),
                    field,
                    fn_name,
                )?;
                let obj_is_ptr = value_types.get(object).copied() == Some("ptr");
                let obj_ptr = if obj_is_ptr {
                    format!("%v{}", object.0)
                } else {
                    let pid = FIELD_PTR_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let ptr_reg = format!("%iptr_g_{}", pid);
                    out.push_str(&format!(
                        "  {} = inttoptr i64 %v{} to ptr\n",
                        ptr_reg, object.0
                    ));
                    ptr_reg
                };
                // Track the class of struct-typed fields so chained access
                // (a.b.c) resolves each hop against the right layout.
                if let Some(rc) = self.field_declared_class(
                    module,
                    value_classes.get(object).map(|c| c.as_str()),
                    field,
                ) {
                    value_classes.insert(*dest, rc);
                }
                let gep_reg = format!("%fgep_{}", dest.0);
                out.push_str(&format!(
                    "  {} = getelementptr inbounds i8, ptr {}, i64 {}\n",
                    gep_reg, obj_ptr, offset
                ));
                let tbaa_tag = attributes::get_tbaa_tag(f_ty);
                out.push_str(&format!(
                    "  %v{} = load {}, ptr {}, align 8{}, !invariant.load !{{}}\n",
                    dest.0, f_ty, gep_reg, tbaa_tag
                ));
            }
            Inst::SetField {
                object,
                field,
                value,
            } => {
                let f_ty = value_types.get(value).copied().unwrap_or("i64");
                let offset = self.find_field_offset(
                    module,
                    value_classes.get(object).map(|c| c.as_str()),
                    field,
                    fn_name,
                )?;
                let obj_is_ptr = value_types.get(object).copied() == Some("ptr");
                let obj_ptr = if obj_is_ptr {
                    format!("%v{}", object.0)
                } else {
                    let pid = FIELD_PTR_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let ptr_reg = format!("%iptr_s_{}", pid);
                    out.push_str(&format!(
                        "  {} = inttoptr i64 %v{} to ptr\n",
                        ptr_reg, object.0
                    ));
                    ptr_reg
                };
                let pid = FIELD_PTR_COUNTER.fetch_add(1, Ordering::Relaxed);
                let gep_reg = format!("%fgep_s_{}", pid);
                out.push_str(&format!(
                    "  {} = getelementptr inbounds i8, ptr {}, i64 {}\n",
                    gep_reg, obj_ptr, offset
                ));
                out.push_str(&format!(
                    "  store {} %v{}, ptr {}, align 8\n",
                    f_ty, value.0, gep_reg
                ));
            }
            Inst::Out { value } => {
                let val_ty = value_types.get(value).copied().unwrap_or("i64");
                if bool_vids.contains(value) {
                    out.push_str(&format!(
                        "  call void @datara_rt_out_bool(i64 %v{})\n",
                        value.0
                    ));
                } else {
                    match val_ty {
                        "double" => {
                            out.push_str(&format!(
                                "  call void @datara_rt_out_float(double %v{})\n",
                                value.0
                            ));
                        }
                        "ptr" => {
                            out.push_str(&format!(
                                "  call void @datara_rt_out_str(ptr %v{})\n",
                                value.0
                            ));
                        }
                        _ => {
                            out.push_str(&format!(
                                "  call void @datara_rt_out_int(i64 %v{})\n",
                                value.0
                            ));
                        }
                    }
                }
            }
            Inst::Err { value } => {
                out.push_str(&format!("  call void @datara_rt_err(ptr %v{})\n", value.0));
            }
            Inst::FormatStr {
                dest,
                parts,
                values,
            } => {
                value_types.insert(*dest, "ptr");
                let empty_id = strings.get("").copied().unwrap_or(0);
                let mut pieces: Vec<String> = Vec::new();

                for (idx, p) in parts.iter().enumerate() {
                    if !p.is_empty() || (idx == 0 && values.is_empty()) {
                        let pid = strings.get(p.as_str()).copied().unwrap_or(empty_id);
                        let p_ptr = format!("%fmt_p_{}_{}", dest.0, idx);
                        out.push_str(&format!(
                            "  {} = getelementptr inbounds [0 x i8], ptr @.str.{}, i64 0, i64 0\n",
                            p_ptr, pid
                        ));
                        pieces.push(p_ptr);
                    }
                    if idx < values.len() {
                        let val_id = &values[idx];
                        let val_ty = value_types.get(val_id).copied().unwrap_or("i64");
                        let s_val = format!("%fmt_v_{}_{}", dest.0, idx);
                        if val_ty == "ptr" {
                            out.push_str(&format!(
                                "  {} = getelementptr inbounds i8, ptr %v{}, i64 0\n",
                                s_val, val_id.0
                            ));
                        } else if val_ty == "double" {
                            out.push_str(&format!(
                                "  {} = call ptr @datara_rt_float_to_str(double %v{})\n",
                                s_val, val_id.0
                            ));
                        } else if bool_vids.contains(val_id) {
                            out.push_str(&format!(
                                "  {} = call ptr @datara_rt_bool_to_str(i64 %v{})\n",
                                s_val, val_id.0
                            ));
                        } else {
                            out.push_str(&format!(
                                "  {} = call ptr @datara_rt_int_to_str(i64 %v{})\n",
                                s_val, val_id.0
                            ));
                        }
                        pieces.push(s_val);
                    }
                }

                match pieces.len() {
                    0 => {
                        out.push_str(&format!(
                            "  %v{} = getelementptr inbounds [0 x i8], ptr @.str.{}, i64 0, i64 0\n",
                            dest.0, empty_id
                        ));
                    }
                    1 => {
                        out.push_str(&format!(
                            "  %v{} = getelementptr inbounds i8, ptr {}, i64 0\n",
                            dest.0, pieces[0]
                        ));
                    }
                    2 => {
                        out.push_str(&format!(
                            "  %v{} = call ptr @datara_rt_str_concat(ptr {}, ptr {})\n",
                            dest.0, pieces[0], pieces[1]
                        ));
                    }
                    3 => {
                        out.push_str(&format!(
                            "  %v{} = call ptr @datara_rt_str_concat_3(ptr {}, ptr {}, ptr {})\n",
                            dest.0, pieces[0], pieces[1], pieces[2]
                        ));
                    }
                    4 => {
                        out.push_str(&format!(
                            "  %v{} = call ptr @datara_rt_str_concat_4(ptr {}, ptr {}, ptr {}, ptr {})\n",
                            dest.0, pieces[0], pieces[1], pieces[2], pieces[3]
                        ));
                    }
                    5 => {
                        out.push_str(&format!(
                            "  %v{} = call ptr @datara_rt_str_concat_5(ptr {}, ptr {}, ptr {}, ptr {}, ptr {})\n",
                            dest.0, pieces[0], pieces[1], pieces[2], pieces[3], pieces[4]
                        ));
                    }
                    _ => {
                        let mut curr = pieces[0].clone();
                        for (i, piece) in pieces[1..].iter().enumerate() {
                            let target = if i + 2 == pieces.len() {
                                format!("%v{}", dest.0)
                            } else {
                                format!("%fmt_cn_{}_{}", dest.0, i)
                            };
                            out.push_str(&format!(
                                "  {} = call ptr @datara_rt_str_concat(ptr {}, ptr {})\n",
                                target, curr, piece
                            ));
                            curr = target;
                        }
                    }
                }
            }
            Inst::GetFuncAddr { dest, func_name } => {
                value_types.insert(*dest, "i64");
                let ptr_temp = format!("%fptr_{}", dest.0);
                out.push_str(&format!(
                    "  {} = bitcast ptr @{} to ptr\n",
                    ptr_temp, func_name
                ));
                out.push_str(&format!(
                    "  %v{} = ptrtoint ptr {} to i64\n",
                    dest.0, ptr_temp
                ));
            }
            Inst::Select {
                dest,
                cond,
                then_val,
                else_val,
                ty,
            } => {
                let t_ty = value_types.get(then_val).copied();
                let e_ty = value_types.get(else_val).copied();
                let known_ty = match (t_ty, e_ty) {
                    (Some(a), Some(b)) if a == b => Some(a),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    _ => None,
                };
                let vty = match known_ty {
                    // Prefer the actual tracked operand types: the optimizer's
                    // if-conversion can mislabel Float selects as "Int".
                    Some(v) => v,
                    None => {
                        if ty == "Float" {
                            "double"
                        } else if ty == "String" || ty == "Str" {
                            "ptr"
                        } else {
                            "i64"
                        }
                    }
                };
                value_types.insert(*dest, vty);
                let cmp_temp = format!("%sel_c_{}", dest.0);
                out.push_str(&format!("  {} = icmp ne i64 %v{}, 0\n", cmp_temp, cond.0));
                out.push_str(&format!(
                    "  %v{} = select i1 {}, {} %v{}, {} %v{}\n",
                    dest.0, cmp_temp, vty, then_val.0, vty, else_val.0
                ));
            }
            Inst::Decide {
                dest,
                arms,
                else_val,
                ty,
            } => {
                let vty = if ty == "Float" {
                    "double"
                } else if ty == "String" || ty == "Str" {
                    "ptr"
                } else {
                    "i64"
                };
                value_types.insert(*dest, vty);
                // No else arm: fall back to a well-defined constant instead
                // of referencing an undefined %v0.
                let mut curr_val_str = match else_val {
                    Some(v) => format!("%v{}", v.0),
                    None => match vty {
                        "double" => String::from("0.0"),
                        "ptr" => String::from("null"),
                        _ => String::from("0"),
                    },
                };
                for (idx, (cond, val)) in arms.iter().enumerate().rev() {
                    let cmp_temp = format!("%dec_c_{}_{}", dest.0, idx);
                    let sel_temp = if idx == 0 {
                        format!("%v{}", dest.0)
                    } else {
                        format!("%dec_s_{}_{}", dest.0, idx)
                    };
                    out.push_str(&format!("  {} = icmp ne i64 %v{}, 0\n", cmp_temp, cond.0));
                    out.push_str(&format!(
                        "  {} = select i1 {}, {} %v{}, {} {}\n",
                        sel_temp, cmp_temp, vty, val.0, vty, curr_val_str
                    ));
                    curr_val_str = sel_temp;
                }
            }
            Inst::InlineAsm {
                template,
                outputs,
                inputs,
                clobbers,
                options,
            } => {
                let is_pure = options.iter().any(|o| o == "pure");
                let sideeffect_kw = if is_pure { "" } else { "sideeffect " };

                let mut constraint_parts = Vec::new();
                for (constraint, _) in outputs {
                    if constraint.is_empty() {
                        constraint_parts.push("=r".to_string());
                    } else if constraint.starts_with('=') {
                        constraint_parts.push(constraint.clone());
                    } else {
                        constraint_parts.push(format!("={}", constraint));
                    }
                }
                for (constraint, _) in inputs {
                    if constraint.is_empty() {
                        constraint_parts.push("r".to_string());
                    } else {
                        constraint_parts.push(constraint.clone());
                    }
                }
                for clobber in clobbers {
                    let clob = clobber.trim();
                    if !clob.is_empty() {
                        if clob.starts_with('~') {
                            constraint_parts.push(clob.to_string());
                        } else if clob.starts_with('{') {
                            constraint_parts.push(format!("~{}", clob));
                        } else {
                            constraint_parts.push(format!("~{{{}}}", clob));
                        }
                    }
                }
                if clobbers.is_empty() {
                    constraint_parts.push("~{dirflag}".to_string());
                    constraint_parts.push("~{fpsr}".to_string());
                    constraint_parts.push("~{flags}".to_string());
                }
                let constraints = constraint_parts.join(",");

                let arg_strs: Vec<String> = inputs
                    .iter()
                    .map(|(_, arg)| {
                        let ty = value_types.get(arg).copied().unwrap_or("i64");
                        format!("{} %v{}", ty, arg.0)
                    })
                    .collect();
                let args_joined = arg_strs.join(", ");
                let escaped_template = template.replace('\\', "\\\\").replace('"', "\\\"");

                if outputs.is_empty() {
                    out.push_str(&format!(
                        "  call void asm {}\"{}\", \"{}\"({})\n",
                        sideeffect_kw, escaped_template, constraints, args_joined
                    ));
                } else if outputs.len() == 1 {
                    let dest = outputs[0].1;
                    value_types.insert(dest, "i64");
                    out.push_str(&format!(
                        "  %v{} = call i64 asm {}\"{}\", \"{}\"({})\n",
                        dest.0, sideeffect_kw, escaped_template, constraints, args_joined
                    ));
                } else {
                    let ret_types = vec!["i64"; outputs.len()].join(", ");
                    let struct_ty = format!("{{ {} }}", ret_types);
                    let tmp = format!("%asm_out_{}", outputs[0].1.0);
                    out.push_str(&format!(
                        "  {} = call {} asm {}\"{}\", \"{}\"({})\n",
                        tmp, struct_ty, sideeffect_kw, escaped_template, constraints, args_joined
                    ));
                    for (idx, (_, dest)) in outputs.iter().enumerate() {
                        value_types.insert(*dest, "i64");
                        out.push_str(&format!(
                            "  %v{} = extractvalue {} {}, {}\n",
                            dest.0, struct_ty, tmp, idx
                        ));
                    }
                }
            }
            Inst::VolatileLoad { dest, addr, ty } => {
                let llvm_ty = match ty.as_str() {
                    "Int" | "i64" | "u64" => "i64",
                    "Int32" | "i32" | "u32" | "UInt32" => "i32",
                    "Int16" | "i16" | "u16" | "UInt16" => "i16",
                    "Int8" | "i8" | "u8" | "UInt8" | "Byte" => "i8",
                    "Float" | "f64" => "double",
                    "Float32" | "f32" => "float",
                    _ => "i64",
                };
                value_types.insert(*dest, llvm_ty);
                let ptr_vid = FIELD_PTR_COUNTER.fetch_add(1, Ordering::Relaxed);
                out.push_str(&format!(
                    "  %ptr_{} = inttoptr i64 %v{} to ptr\n",
                    ptr_vid, addr.0
                ));
                out.push_str(&format!(
                    "  %v{} = load volatile {}, ptr %ptr_{}\n",
                    dest.0, llvm_ty, ptr_vid
                ));
            }
            Inst::VolatileStore { addr, value, ty } => {
                let llvm_ty = match ty.as_str() {
                    "Int" | "i64" | "u64" => "i64",
                    "Int32" | "i32" | "u32" | "UInt32" => "i32",
                    "Int16" | "i16" | "u16" | "UInt16" => "i16",
                    "Int8" | "i8" | "u8" | "UInt8" | "Byte" => "i8",
                    "Float" | "f64" => "double",
                    "Float32" | "f32" => "float",
                    _ => "i64",
                };
                let ptr_vid = FIELD_PTR_COUNTER.fetch_add(1, Ordering::Relaxed);
                out.push_str(&format!(
                    "  %ptr_{} = inttoptr i64 %v{} to ptr\n",
                    ptr_vid, addr.0
                ));
                out.push_str(&format!(
                    "  store volatile {} %v{}, ptr %ptr_{}\n",
                    llvm_ty, value.0, ptr_vid
                ));
            }
            _ => {}
        }
        Ok(())
    }
}
