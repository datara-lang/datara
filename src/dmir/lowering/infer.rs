use crate::ast::*;
use crate::dmir::ir::*;
use crate::types::DataraType;

use super::Lowering;

impl<'a> Lowering<'a> {
    pub(crate) fn infer_expr_datara_type(&self, expr: &Expr) -> Option<DataraType> {
        match expr {
            Expr::Literal(lit, _) => match lit {
                LiteralValue::Int(_) => Some(DataraType::Int),
                LiteralValue::Float(_) => Some(DataraType::Float),
                LiteralValue::Dec64(_) => Some(DataraType::Dec64),
                LiteralValue::Bool(_) => Some(DataraType::Bool),
                LiteralValue::String(_) => Some(DataraType::String),
                LiteralValue::Char(_) => Some(DataraType::Char),
                LiteralValue::None => Some(DataraType::Unit),
            },
            Expr::Identifier(name, _) => {
                if let Some(ty) = self.lookup_var_type(name) {
                    return Some(ty);
                }
                if let Some(t) = self.class_field_types.get(name) {
                    if t.starts_with("List<") && t.ends_with('>') {
                        let inner = &t[5..t.len() - 1];
                        let elem_ty = match inner {
                            "Float" | "Float64" => DataraType::Float,
                            "Float32" => DataraType::Float32,
                            "Int" | "Int64" | "isize" => DataraType::Int,
                            "Int32" | "i32" => DataraType::Int32,
                            "Int16" | "i16" => DataraType::Int16,
                            "Int8" | "i8" => DataraType::Int8,
                            "UInt" | "UInt64" | "u64" | "usize" => DataraType::UInt64,
                            "UInt32" | "u32" => DataraType::UInt32,
                            "UInt16" | "u16" => DataraType::UInt16,
                            "UInt8" | "Byte" | "u8" => DataraType::UInt8,
                            "String" | "Str" => DataraType::String,
                            "Bool" => DataraType::Bool,
                            _ => DataraType::Class(inner.to_string()),
                        };
                        return Some(DataraType::List(Box::new(elem_ty)));
                    }
                    match t.as_str() {
                        "Float" | "Float64" => return Some(DataraType::Float),
                        "Float32" => return Some(DataraType::Float32),
                        "Int" | "Int64" | "isize" => return Some(DataraType::Int),
                        "Int32" | "i32" => return Some(DataraType::Int32),
                        "Int16" | "i16" => return Some(DataraType::Int16),
                        "Int8" | "i8" => return Some(DataraType::Int8),
                        "UInt" | "UInt64" | "u64" | "usize" => return Some(DataraType::UInt64),
                        "UInt32" | "u32" => return Some(DataraType::UInt32),
                        "UInt16" | "u16" => return Some(DataraType::UInt16),
                        "UInt8" | "Byte" | "u8" => return Some(DataraType::UInt8),
                        "String" | "Str" => return Some(DataraType::String),
                        "Bool" => return Some(DataraType::Bool),
                        _ => return Some(DataraType::Class(t.clone())),
                    }
                }
                None
            }
            Expr::MemberAccess { object, member, .. } => {
                let obj_ty = self.infer_expr_datara_type(object)?;
                match obj_ty {
                    DataraType::GenericInstance { name, args } => {
                        let (params, t_fields) = self.types.generic_templates.get(&name)?;
                        let field_type = t_fields.get(member)?;
                        if let DataraType::TypeParam(p) = field_type
                            && let Some(idx) = params.iter().position(|param| param == p)
                            && idx < args.len()
                        {
                            return Some(args[idx].clone());
                        }
                        Some(field_type.clone())
                    }
                    DataraType::Class(cls_name) => {
                        if let Some(fields) = self.types.class_fields.get(&cls_name) {
                            if let Some(ft) = fields.get(member) {
                                return Some(ft.clone());
                            }
                        }
                        let key = format!("{}.{}", cls_name, member);
                        if let Some(t) = self.class_field_types.get(&key) {
                            if t.starts_with("List<") && t.ends_with('>') {
                                let inner = &t[5..t.len() - 1];
                                let elem_ty = match inner {
                                    "Float" | "Float64" => DataraType::Float,
                                    "Float32" => DataraType::Float32,
                                    "Int" | "Int64" | "Int32" => DataraType::Int,
                                    "String" | "Str" => DataraType::String,
                                    "Bool" => DataraType::Bool,
                                    _ => DataraType::Class(inner.to_string()),
                                };
                                return Some(DataraType::List(Box::new(elem_ty)));
                            }
                            match t.as_str() {
                                "Float" | "Float64" => return Some(DataraType::Float),
                                "Float32" => return Some(DataraType::Float32),
                                "Int" | "Int64" | "isize" => return Some(DataraType::Int),
                                "Int32" | "i32" => return Some(DataraType::Int32),
                                "Int16" | "i16" => return Some(DataraType::Int16),
                                "Int8" | "i8" => return Some(DataraType::Int8),
                                "UInt" | "UInt64" | "u64" | "usize" => {
                                    return Some(DataraType::UInt64);
                                }
                                "UInt32" | "u32" => return Some(DataraType::UInt32),
                                "UInt16" | "u16" => return Some(DataraType::UInt16),
                                "UInt8" | "Byte" | "u8" => return Some(DataraType::UInt8),
                                "String" | "Str" => return Some(DataraType::String),
                                "Bool" => return Some(DataraType::Bool),
                                _ => return Some(DataraType::Class(t.clone())),
                            }
                        }
                        None
                    }
                    DataraType::Result(ok, err) => match member.as_str() {
                        "is_success" | "is_ok" | "is_err" => Some(DataraType::Bool),
                        "value" | "ok" => Some(*ok),
                        "error_msg" | "error" | "err" => Some(*err),
                        _ => None,
                    },
                    DataraType::Option(val) => match member.as_str() {
                        "is_some" | "is_none" => Some(DataraType::Bool),
                        "value" | "val" => Some(*val),
                        _ => None,
                    },
                    _ => None,
                }
            }
            Expr::IndexAccess { object, .. } => {
                let obj_ty = self.infer_expr_datara_type(object)?;
                match obj_ty {
                    DataraType::List(elem) => Some(*elem),
                    DataraType::Map(_, val) => Some(*val),
                    _ => None,
                }
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                if matches!(
                    op.as_str(),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
                ) {
                    return Some(DataraType::Bool);
                }
                let l_ty = self.infer_expr_datara_type(left);
                let r_ty = self.infer_expr_datara_type(right);
                // v1.4.5 W2: Float32 must be checked BEFORE Float — a bare
                // f64 literal (`0.1`) infers as Float, which would silently
                // widen an f32 arithmetic tree back to f64 here.
                if l_ty == Some(DataraType::Float32) || r_ty == Some(DataraType::Float32) {
                    return Some(DataraType::Float32);
                }
                if l_ty == Some(DataraType::Float) || r_ty == Some(DataraType::Float) {
                    return Some(DataraType::Float);
                }
                if l_ty == Some(DataraType::String) || r_ty == Some(DataraType::String) {
                    return Some(DataraType::String);
                }
                l_ty.or(r_ty)
            }
            Expr::Unary { op, expr, .. } => {
                if op == "!" {
                    Some(DataraType::Bool)
                } else {
                    self.infer_expr_datara_type(expr)
                }
            }
            Expr::Call { callee, args, .. } => match &**callee {
                Expr::Identifier(fn_name, _) => {
                    if fn_name == "join"
                        || fn_name == "thread_join"
                        || fn_name == "ThreadHandle_join"
                    {
                        if args.len() == 1 {
                            return Some(DataraType::Int);
                        } else if args.len() >= 2 {
                            return Some(DataraType::String);
                        }
                    }
                    let ret = self.infer_fn_ret_ty(fn_name);
                    if let Some(rest) = ret.strip_prefix("List<")
                        && let Some(inner) = rest.strip_suffix('>')
                    {
                        // Explicit element type (e.g. a declared
                        // List<Str> return such as dir_list): record it
                        // so the let below and any inline `parts[i]`
                        // indexing know the element kind. Unknown
                        // elements stay unresolved rather than being
                        // guessed.
                        let elem_ty = match inner {
                            "Float" | "Float64" => DataraType::Float,
                            "Float32" => DataraType::Float32,
                            "Bool" => DataraType::Bool,
                            "Int" | "Int64" | "Int32" => DataraType::Int,
                            "String" | "Str" => DataraType::String,
                            _ => return None,
                        };
                        return Some(DataraType::List(Box::new(elem_ty)));
                    }
                    if let Some(rest) = ret.strip_prefix("Outcome<")
                        && let Some(inner) = rest.strip_suffix('>')
                    {
                        // Checked-I/O builtins (file_read_checked,
                        // env_get_checked) return Outcome<Str> objects.
                        // Infer the GenericInstance so receiver method
                        // calls (unwrap/is_ok/is_err/err) resolve the
                        // payload type instead of falling back to Int.
                        let arg_ty = match inner {
                            "Float" => DataraType::Float,
                            "Bool" => DataraType::Bool,
                            "String" | "Str" => DataraType::String,
                            "Int" | "Int64" | "Int32" => DataraType::Int,
                            other => DataraType::Class(other.to_string()),
                        };
                        return Some(DataraType::GenericInstance {
                            name: "Outcome".to_string(),
                            args: vec![arg_ty],
                        });
                    }
                    match ret.as_str() {
                        "Float" => Some(DataraType::Float),
                        "String" => Some(DataraType::String),
                        "Bool" => Some(DataraType::Bool),
                        "Int" => Some(DataraType::Int),
                        // String splits are the canonical List<Str>
                        // source: infer_fn_ret_ty reports the bare
                        // "List" for them, but without an element type
                        // the let never records one and inline
                        // `parts[i]` indexes read as raw pointers
                        // instead of string values.
                        "List"
                            if matches!(
                                fn_name.as_str(),
                                "str_split" | "split" | "datara_rt_str_split"
                            ) =>
                        {
                            Some(DataraType::List(Box::new(DataraType::String)))
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
            Expr::ListLiteral(elements, _) => {
                let elem_ty = elements
                    .first()
                    .and_then(|e| self.infer_expr_datara_type(e))
                    .unwrap_or(DataraType::Int);
                Some(DataraType::List(Box::new(elem_ty)))
            }
            Expr::ArrayRepeatLiteral { elem, .. } => {
                let elem_ty = self.infer_expr_datara_type(elem).unwrap_or_else(|| {
                    if self.is_expr_float(elem) {
                        DataraType::Float
                    } else {
                        DataraType::Int
                    }
                });
                Some(DataraType::List(Box::new(elem_ty)))
            }
            _ => None,
        }
    }

    pub(crate) fn member_field_repr(&self, object: &Expr, member: &str) -> Option<String> {
        let obj_ty = self.infer_expr_datara_type(object)?;
        match obj_ty {
            DataraType::GenericInstance { name, args } => {
                let (params, t_fields) = self.types.generic_templates.get(&name)?;
                let field_type = t_fields.get(member)?;
                if let DataraType::TypeParam(p) = field_type
                    && let Some(idx) = params.iter().position(|param| param == p)
                    && idx < args.len()
                {
                    return Some(args[idx].to_string());
                }
                Some(field_type.to_string())
            }
            DataraType::Class(cls_name) => {
                if let Some(fields) = self.types.class_fields.get(&cls_name) {
                    if let Some(f) = fields.get(member) {
                        return Some(f.to_string());
                    }
                }
                let key = format!("{}.{}", cls_name, member);
                self.class_field_types.get(&key).cloned()
            }
            DataraType::Result(ok, err) => match member {
                "is_success" | "is_ok" | "is_err" => Some("Bool".to_string()),
                "value" | "ok" => Some(ok.to_string()),
                "error_msg" | "error" | "err" => Some(err.to_string()),
                _ => None,
            },
            DataraType::Option(val) => match member {
                "is_some" | "is_none" => Some("Bool".to_string()),
                "value" | "val" => Some(val.to_string()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn infer_fn_ret_ty(&self, func_name: &str) -> String {
        if let Some(ty) = self.function_return_types.get(func_name) {
            return ty.clone();
        }
        if func_name == "str_to_int"
            || func_name == "datara_rt_str_to_int"
            || func_name == "js_eval_int"
            || func_name == "datara_js_eval_int"
            || func_name == "py_eval_int"
            || func_name == "datara_py_eval_int"
            || func_name == "py_import"
            || func_name == "datara_py_import"
            || func_name == "py_exec"
            || func_name == "datara_py_exec"
            || func_name == "datara_py_export_list_f64"
            || func_name == "datara_py_assert_same_ptr"
            || func_name == "zig_eval_int"
            || func_name == "datara_zig_eval_int"
            || func_name == "zig_call"
            || func_name == "datara_zig_call"
            || func_name == "csharp_invoke_i64"
            || func_name == "datara_csharp_invoke_i64"
            || func_name == "lua_eval_int"
            || func_name == "datara_lua_eval_int"
            || func_name == "lua_exec"
            || func_name == "datara_lua_exec"
            || func_name == "polyglot_parallel_exec"
            || func_name == "datara_polyglot_parallel_exec"
            || func_name == "str_len"
            || func_name == "datara_rt_str_len"
            || func_name == "ThreadHandle_join"
            || func_name == "thread_join"
            || func_name == "datara_thread_join"
            || func_name == "Channel_len"
            || func_name == "datara_channel_len"
            || func_name == "Channel_recv"
            || func_name == "datara_channel_recv"
            || func_name == "channel_recv"
            || func_name == "scratch_enter"
            || func_name == "datara_scratch_enter"
            || func_name.starts_with("hton")
            || func_name.starts_with("ntoh")
            || func_name.starts_with("bswap")
            || func_name.starts_with("datara_sys_hton")
            || func_name.starts_with("datara_sys_ntoh")
            || func_name.starts_with("datara_sys_bswap")
            || func_name.starts_with("SliceView_read_")
            || func_name.starts_with("SliceView_get_")
            || func_name == "SliceView_len"
            || func_name.ends_with("_to_int")
            || func_name.contains("count")
            || func_name.contains("index")
        {
            "Int".into()
        } else if func_name == "slice_alloc"
            || func_name == "slice_from_buffer"
            || func_name == "SliceView_subslice"
            || func_name.starts_with("datara_sys_slice_alloc")
            || func_name.starts_with("datara_sys_slice_from")
            || func_name.starts_with("datara_sys_slice_sub")
        {
            "SliceView".into()
        } else if func_name == "volatile_ptr" || func_name == "datara_hw_volatile_ptr" {
            "VolatilePtr".into()
        } else if func_name == "typed_zero_init" || func_name == "datara_hw_typed_zero_init" {
            "RawPtr".into()
        } else if func_name == "str_to_float"
            || func_name == "datara_rt_str_to_float"
            || func_name == "js_eval_float"
            || func_name == "datara_js_eval_float"
            || func_name == "py_eval_float"
            || func_name == "datara_py_eval_float"
            || func_name == "py_call_1_float"
            || func_name == "datara_py_call_1_float"
            || func_name == "csharp_invoke_f64"
            || func_name == "datara_csharp_invoke_f64"
            || func_name == "lua_eval_float"
            || func_name == "datara_lua_eval_float"
            || func_name.ends_with("_to_float")
            || func_name.contains("float")
            || func_name.contains("flt")
        {
            "Float".into()
        } else if func_name == "str_split"
            || func_name == "split"
            || func_name == "datara_rt_str_split"
        {
            "List".into()
        } else if func_name.contains("is_")
            || func_name.contains("has_")
            || func_name.contains("contains")
            || func_name.contains("starts_with")
            || func_name.ends_with("_with")
        {
            "Bool".into()
        } else if func_name.contains("string")
            || func_name.contains("to_str")
            || func_name.ends_with("_str")
            || func_name.starts_with("js_")
            || func_name.starts_with("datara_js_")
            || func_name.starts_with("py_")
            || func_name.starts_with("datara_py_")
            || func_name.contains("classify")
            || func_name.contains("handle")
            || func_name.contains("format")
            || func_name.contains("render")
            || func_name.contains("quote")
            || func_name.contains("summary")
            || func_name.contains("repeat")
            || func_name.contains("pad")
            || func_name.contains("replace")
            || func_name.contains("upper")
            || func_name.contains("lower")
            || func_name == "str_join"
            || func_name == "datara_rt_str_join"
            || func_name == "read"
            || func_name == "file_read"
            || func_name == "datara_rt_file_read"
            || func_name == "env_get"
            || func_name == "datara_rt_env_get"
            || func_name == "args_get"
            || func_name == "datara_rt_args_get"
            || func_name == "str_trim"
            || func_name == "datara_rt_str_trim"
            || func_name == "socket_recv"
            || func_name == "datara_rt_socket_recv"
            || func_name == "sha256"
            || func_name == "datara_rt_sha256"
            || func_name == "base64_encode"
            || func_name == "datara_rt_base64_encode"
            || func_name == "base64_decode"
            || func_name == "datara_rt_base64_decode"
            || func_name == "uuid_v4"
            || func_name == "datara_rt_uuid_v4"
            || func_name == "str_substring"
            || func_name == "datara_rt_str_substring"
            || func_name == "str_substr"
            || func_name == "int_to_str"
            || func_name == "datara_rt_int_to_str"
            || func_name == "float_to_str"
            || func_name == "datara_rt_float_to_str"
            || func_name == "str_char_at"
            || func_name == "datara_rt_str_char_at"
            || func_name == "char_at"
            || func_name.ends_with("_char_at")
        {
            "String".into()
        } else {
            "Int".into()
        }
    }

    pub(crate) fn is_expr_str(&self, expr: &Expr) -> bool {
        if let Some(crate::types::DataraType::String) = self.infer_expr_datara_type(expr) {
            return true;
        }
        match expr {
            Expr::Literal(LiteralValue::String(_), _) | Expr::InterpolatedString { .. } => true,
            Expr::MemberAccess { member, .. } => {
                if let Some(t) = self.class_field_types.get(member) {
                    return t == "String" || t == "Str";
                }
                member == "name"
                    || member == "version"
                    || member == "title"
                    || member == "path"
                    || member == "str"
            }
            Expr::Identifier(name, ..) => {
                if let Some(ty) = self.lookup_var_type(name) {
                    return ty == crate::types::DataraType::String;
                }
                if let Some(t) = self.class_field_types.get(name) {
                    return t == "String" || t == "Str";
                }
                name == "name"
                    || name == "version"
                    || name == "title"
                    || name == "path"
                    || name == "str"
                    || name == "msg"
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                if op == "+" {
                    self.is_expr_str(left) || self.is_expr_str(right)
                } else {
                    false
                }
            }
            Expr::Call { callee, .. } => match &**callee {
                Expr::Identifier(fn_name, _) => {
                    let ret = self.infer_fn_ret_ty(fn_name);
                    ret == "String" || ret == "Str"
                }
                Expr::MemberAccess { object, member, .. } => {
                    if (member == "unwrap" || member == "unwrap_or")
                        && let Expr::Identifier(var_name, _) = &**object
                        && let Some(DataraType::GenericInstance { name, args }) =
                            self.lookup_var_type(var_name)
                        && name == "Outcome"
                        && args.first() == Some(&DataraType::String)
                    {
                        return true;
                    }
                    let ret = self.infer_fn_ret_ty(member);
                    ret == "String" || ret == "Str"
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// v1.4.5 W1: the integer width (bits) of this expression's type, for
    /// the checked-arithmetic builtins. Only integer types count; anything
    /// else reports the default 64.
    pub(crate) fn expr_int_bits(&self, expr: &Expr) -> i64 {
        let ty = self.infer_expr_datara_type(expr);
        match ty.as_ref().unwrap_or(&DataraType::Int) {
            DataraType::Int8 | DataraType::UInt8 => 8,
            DataraType::Int16 | DataraType::UInt16 => 16,
            DataraType::Int32 | DataraType::UInt32 => 32,
            _ => 64,
        }
    }

    /// v1.4.5 W3: does this expression carry Dec64 (fixed-point ×10⁴) values?
    pub(crate) fn is_expr_dec64(&self, expr: &Expr) -> bool {
        if let Some(DataraType::Dec64) = self.infer_expr_datara_type(expr) {
            return true;
        }
        match expr {
            Expr::Literal(LiteralValue::Dec64(_), _) => true,
            Expr::Identifier(name, ..) => self
                .lookup_var_type(name)
                .map(|ty| ty == crate::types::DataraType::Dec64)
                .unwrap_or(false),
            Expr::Binary { left, right, .. } => {
                self.is_expr_dec64(left) || self.is_expr_dec64(right)
            }
            _ => false,
        }
    }

    pub(crate) fn is_expr_float(&self, expr: &Expr) -> bool {
        if let Some(DataraType::Float) = self.infer_expr_datara_type(expr) {
            return true;
        }
        match expr {
            Expr::Literal(LiteralValue::Float(_), _) => true,
            Expr::MemberAccess { member, .. } => {
                if let Some(t) = self.class_field_types.get(member) {
                    return t == "Float" || t == "Float64";
                }
                member.contains("flt") || member.contains("float")
            }
            Expr::Identifier(name, ..) => {
                if let Some(ty) = self.lookup_var_type(name) {
                    // v1.4.5 W2: a Float32-typed variable is NOT a f64
                    // float for arithmetic routing — the is_f32 path in the
                    // backends owns it. Falling through to name heuristics
                    // here would misroute f32 vars back into f64 ops.
                    return ty == crate::types::DataraType::Float;
                }
                if let Some(t) = self.class_field_types.get(name) {
                    return t == "Float" || t == "Float64";
                }
                name.contains("flt") || name.contains("float")
            }
            Expr::Binary { left, right, .. } => {
                self.is_expr_float(left) || self.is_expr_float(right)
            }
            Expr::Unary { expr, .. } => self.is_expr_float(expr),
            Expr::Call { callee, .. } => match &**callee {
                Expr::Identifier(name, _) => name.contains("float") || name.contains("flt"),
                Expr::MemberAccess { object, member, .. } => {
                    if (member == "unwrap" || member == "unwrap_or")
                        && let Expr::Identifier(var_name, _) = &**object
                        && let Some(DataraType::GenericInstance { name, args }) =
                            self.lookup_var_type(var_name)
                        && name == "Outcome"
                        && args.first() == Some(&DataraType::Float)
                    {
                        return true;
                    }
                    member.contains("float") || member.contains("flt")
                }
                _ => false,
            },
            Expr::IndexAccess { object, .. } => {
                if let Some(DataraType::List(elem)) = self.infer_expr_datara_type(object) {
                    return *elem == DataraType::Float;
                }
                if let Expr::Identifier(name, ..) = &**object {
                    if let Some(ty) = self.lookup_var_type(name) {
                        if let crate::types::DataraType::List(elem) = ty {
                            return *elem == crate::types::DataraType::Float;
                        }
                    }
                    if let Some(t) = self.class_field_types.get(name) {
                        return t == "Float" || t == "Float64" || t == "Float32";
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub(crate) fn is_expr_bool(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Literal(LiteralValue::Bool(_), _) => true,
            Expr::Identifier(name, ..) => {
                if let Some(ty) = self.lookup_var_type(name)
                    && ty == crate::types::DataraType::Bool
                {
                    return true;
                }
                false
            }
            Expr::Binary { op, .. } => {
                matches!(
                    op.as_str(),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
                )
            }
            Expr::Unary { op, .. } => op == "!",
            _ => false,
        }
    }

    pub(crate) fn is_expr_list(&self, expr: &Expr) -> bool {
        if matches!(
            expr,
            Expr::ListLiteral(..) | Expr::ArrayRepeatLiteral { .. }
        ) {
            return true;
        }
        if let Expr::Identifier(name, _) = expr
            && let Some(ty) = self.lookup_var_type(name)
        {
            if matches!(ty, crate::types::DataraType::List(_)) {
                return true;
            }
            if let crate::types::DataraType::Class(cls) = ty
                && cls == "List"
            {
                return true;
            }
        }
        if let Expr::Call { callee, args, .. } = expr {
            // v1.4.1: the list-returning protocol methods keep the value a
            // list (in-place mutators return the receiver handle, slice and
            // collect produce a fresh one), so downstream printing/chaining
            // must keep treating them as list expressions.
            if let Expr::MemberAccess { object, member, .. } = &**callee
                && ((member == "map" || member == "filter")
                    || matches!(
                        member.as_str(),
                        "sort" | "reverse" | "clear" | "insert_at" | "slice" | "collect"
                    ))
                && self.is_expr_list(object)
            {
                return true;
            }
            if let Expr::Identifier(fn_name, _) = &**callee {
                if fn_name == "str_split" || fn_name == "split" || fn_name == "datara_rt_str_split"
                {
                    return true;
                }
                if (fn_name == "map" || fn_name == "filter" || fn_name == "collect")
                    && !args.is_empty()
                    && self.is_expr_list(&args[0])
                {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn is_expr_map(&self, expr: &Expr) -> bool {
        if matches!(expr, Expr::MapLiteral(..)) {
            return true;
        }
        if let Some(ty) = self.infer_expr_datara_type(expr) {
            match ty {
                crate::types::DataraType::Map(..) => return true,
                crate::types::DataraType::Class(ref cls) if cls == "Map" => return true,
                crate::types::DataraType::GenericInstance { ref name, .. } if name == "Map" => {
                    return true;
                }
                _ => return false,
            }
        }
        if let Expr::Identifier(name, _) = expr
            && let Some(ty) = self.lookup_var_type(name)
        {
            match ty {
                crate::types::DataraType::Map(..) => return true,
                crate::types::DataraType::Class(ref cls) if cls == "Map" => return true,
                crate::types::DataraType::GenericInstance { ref name, .. } if name == "Map" => {
                    return true;
                }
                _ => return false,
            }
        }
        false
    }

    pub(crate) fn is_callable_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Lambda { .. } => true,
            Expr::Identifier(name, _) => {
                if self.local_lambdas.contains_key(name) {
                    return true;
                }
                if let Some(ty) = self.lookup_var_type(name)
                    && matches!(ty, crate::types::DataraType::Function { .. })
                {
                    return true;
                }
                if self.resolver.functions.contains_key(name)
                    || self.function_return_types.contains_key(name)
                {
                    return true;
                }
                if let Some(global) = self.resolver.scopes.first()
                    && global.get(name).is_some()
                {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    pub(crate) fn lower_inline_closure_call(
        &mut self,
        callable: &Expr,
        arg_vals: &[ValueId],
        cur_block: &mut BasicBlockId,
    ) -> Option<ValueId> {
        let (params, body) = match callable {
            Expr::Lambda { params, body, .. } => (params.clone(), (**body).clone()),
            Expr::Identifier(fn_name, _) => {
                if let Some(pair) = self.local_lambdas.get(fn_name).cloned() {
                    pair
                } else {
                    let dest = self.next_val();
                    let ret_ty = self
                        .function_return_types
                        .get(fn_name)
                        .cloned()
                        .unwrap_or_else(|| "Int".into());
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: fn_name.clone(),
                            args: arg_vals.to_vec(),
                            ty: ret_ty,
                        });
                    return Some(dest);
                }
            }
            _ => {
                return None;
            }
        };

        let mut shadowed_symbols: Vec<(String, Option<ValueId>)> = Vec::new();
        // By-value captures of a named lambda: bind the registration-time
        // snapshot before the params so the body reads the snapshotted
        // values (shadow-restored with the params below).
        if let Expr::Identifier(fn_name, _) = callable
            && let Some(caps) = self.lambda_captures.get(fn_name).cloned()
        {
            for (cname, cval) in caps {
                let old_val = self.symbol_values.get(&cname).copied();
                shadowed_symbols.push((cname.clone(), old_val));
                self.symbol_values.insert(cname.clone(), cval);
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::AssignVar {
                        name: cname,
                        value: cval,
                    });
            }
        }
        for (param, &aval) in params.iter().zip(arg_vals) {
            let old_val = self.symbol_values.get(&param.name).copied();
            shadowed_symbols.push((param.name.clone(), old_val));
            self.symbol_values.insert(param.name.clone(), aval);
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::AssignVar {
                    name: param.name.clone(),
                    value: aval,
                });
        }

        let res = self.lower_expr(&body, cur_block);

        // Restore in reverse push order so a name shadowed twice (a lambda
        // parameter named like a captured variable) unwinds correctly.
        for (name, old_val) in shadowed_symbols.into_iter().rev() {
            if let Some(ov) = old_val {
                self.symbol_values.insert(name.clone(), ov);
                self.get_block_mut(*cur_block)
                    .instructions
                    .push(Inst::AssignVar { name, value: ov });
            } else {
                self.symbol_values.remove(&name);
            }
        }

        res
    }

    /// v1.4.5 W1: the integer width an expression carries. Returns the
    /// repr-string name for the widest known integer type of the expression
    /// (`Int8`, `UInt32`, ...), or `None` when the expression is not a
    /// known-width integer (plain Int, Float, Str, unknown). Used to stamp
    /// `Inst::BinOp.ty` so the backends can emit width-correct arithmetic.
    pub(crate) fn int_expr_repr(&self, expr: &Expr) -> Option<&'static str> {
        match self.infer_expr_datara_type(expr) {
            Some(DataraType::Int8) => Some("Int8"),
            Some(DataraType::Int16) => Some("Int16"),
            Some(DataraType::Int32) => Some("Int32"),
            Some(DataraType::UInt8) => Some("UInt8"),
            Some(DataraType::UInt16) => Some("UInt16"),
            Some(DataraType::UInt32) => Some("UInt32"),
            Some(DataraType::UInt64) => Some("UInt64"),
            Some(DataraType::UInt) => Some("UInt"),
            _ => None,
        }
    }

    /// True when the expression is a Float32 (`Float32` repr string). The
    /// f32 arithmetic path in the backends keys off this marker.
    pub(crate) fn is_expr_f32(&self, expr: &Expr) -> bool {
        matches!(self.infer_expr_datara_type(expr), Some(DataraType::Float32))
    }

    /// v1.4.5 W4: map a fmt"{expr:spec}" specifier to a synthetic converter
    /// call: (runtime fn, constant extra args, DMIR result type). Routing:
    /// - ".N" on a Dec64 value -> dec64_to_str_prec(N) (fixed-point scale-10^4,
    ///   round-half-away-from-zero, zero padding beyond 4 digits)
    /// - ".N" otherwise        -> float_to_str_prec(N) (f64 %.*f, NaN/Inf kept)
    /// - "x"/"X"/"o"/"b"       -> int_to_str_radix(v, radix, upper): the
    ///   two's-complement bit pattern; applied regardless of the declared int
    ///   width because every int repr rides the I64 slot.
    pub(crate) fn fmt_spec_converter(
        &self,
        expr: &Expr,
        spec: &str,
    ) -> Option<(&'static str, Vec<i64>, &'static str)> {
        if let Some(digits) = spec.strip_prefix('.') {
            let prec: i64 = digits.parse().unwrap_or(0);
            if self.is_expr_dec64(expr) {
                return Some(("datara_rt_dec64_to_str_prec", vec![prec], "Str"));
            }
            return Some(("datara_rt_float_to_str_prec", vec![prec], "Str"));
        }
        let (radix, upper): (i64, i64) = match spec {
            "x" => (16, 0),
            "X" => (16, 1),
            "o" => (8, 0),
            "b" => (2, 0),
            _ => return None,
        };
        Some(("datara_rt_int_to_str_radix", vec![radix, upper], "Str"))
    }
}
