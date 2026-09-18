use crate::ast::*;
use crate::resolver::Resolver;
use crate::types::{DataraType, TypeChecker};
use std::collections::HashMap;

use crate::dmir::ir::*;

pub mod asm;
pub mod decl;
pub mod expr;
pub mod expr_call;
pub mod expr_call_bridge;
pub mod expr_composite;
pub mod higher_order;
pub mod infer;
pub mod match_arm;
pub mod stmt;
pub mod out;

/// An inlineable function body for lambda-argument static dispatch: the
/// declared parameters, the statements before the single trailing return,
/// and the returned expression (None for a Unit body).
pub type InlineableFnBody = (Vec<Param>, Vec<Stmt>, Option<Expr>);

pub struct Lowering<'a> {
    pub resolver: &'a Resolver,
    pub types: &'a TypeChecker<'a>,
    pub val_counter: usize,
    pub block_counter: usize,
    pub symbol_values: HashMap<String, ValueId>,
    pub current_blocks: Vec<BasicBlock>,
    pub class_field_types: HashMap<String, String>,
    pub function_return_types: HashMap<String, String>,
    pub current_fn_name: String,
    pub local_var_types: HashMap<String, DataraType>,
    pub enum_variant_tags: HashMap<String, i64>,
    /// v1.3.3: module namespace aliases for qualified calls `alias.func()`.
    pub module_alias_functions: HashMap<String, Vec<String>>,
    /// v1.4.2: declarative foreign bridge registry.
    pub bridge_registry: crate::bridge_decl::BridgeRegistry,
    pub enum_variant_names: HashMap<i64, String>,
    pub enum_slots: HashMap<String, Vec<String>>,
    pub current_line_spans: Vec<crate::diagnostics::SourceSpan>,
    pub in_wrapping_mode: bool,
    pub in_saturating_mode: bool,
    pub local_lambdas: HashMap<String, (Vec<Param>, Expr)>,
    /// Lambda-argument static dispatch: functions whose bodies are a
    /// (possibly empty) statement sequence ending in a single
    /// `return <expr>`, or an `=> expr` shorthand, indexed as
    /// `(params, inline_stmts, tail_expr)` so a call site that passes a
    /// lambda argument can inline the body and bind the lambda to the
    /// parameter name (see lower_expr_call).
    pub inlineable_fns: HashMap<String, InlineableFnBody>,
    /// By-value capture snapshots for named lambdas: at `let f = <lambda>`
    /// the current SSA value of every free variable is recorded, and at each
    /// inline call the body sees (and mutates) that snapshot, never the
    /// caller's variable. Mirrors the capture model of arrow lambdas for
    /// Int/Float/Bool/Str.
    pub lambda_captures: HashMap<String, Vec<(String, ValueId)>>,
    /// Stack of open loops: `(continue_target, break_target)`. `continue`
    /// branches to the first element, `break` to the second. For counted
    /// loops the continue target is the increment block so the induction
    /// variable still advances.
    pub loop_stack: Vec<(BasicBlockId, BasicBlockId)>,
    pub global_vars: HashMap<String, (String, bool)>,
    pub program_globals: Vec<GlobalDecl>,
}

impl<'a> Lowering<'a> {
    pub fn new(resolver: &'a Resolver, types: &'a TypeChecker<'a>) -> Self {
        let mut function_return_types = HashMap::new();
        function_return_types.insert("str_to_int".into(), "Int".into());
        function_return_types.insert("datara_rt_str_to_int".into(), "Int".into());
        function_return_types.insert("str_to_float".into(), "Float".into());
        function_return_types.insert("datara_rt_str_to_float".into(), "Float".into());
        function_return_types.insert("str_index_of".into(), "Int".into());
        function_return_types.insert("datara_rt_str_index_of".into(), "Int".into());
        function_return_types.insert("str_contains".into(), "Bool".into());
        function_return_types.insert("datara_rt_str_contains".into(), "Bool".into());
        function_return_types.insert("str_starts_with".into(), "Bool".into());
        function_return_types.insert("datara_rt_str_starts_with".into(), "Bool".into());
        function_return_types.insert("str_ends_with".into(), "Bool".into());
        function_return_types.insert("datara_rt_str_ends_with".into(), "Bool".into());
        function_return_types.insert("str_trim".into(), "String".into());
        function_return_types.insert("datara_rt_str_trim".into(), "String".into());
        function_return_types.insert("input".into(), "String".into());
        function_return_types.insert("read_line".into(), "String".into());
        function_return_types.insert("datara_rt_input".into(), "String".into());
        function_return_types.insert("http_get".into(), "String".into());
        function_return_types.insert("datara_rt_http_get".into(), "String".into());
        function_return_types.insert("file_read".into(), "String".into());
        function_return_types.insert("read_file".into(), "String".into());
        function_return_types.insert("datara_rt_file_read".into(), "String".into());
        function_return_types.insert("file_write".into(), "Int".into());
        function_return_types.insert("write_file".into(), "Int".into());
        function_return_types.insert("datara_rt_file_write".into(), "Int".into());
        function_return_types.insert("file_read_bytes".into(), "List<Int>".into());
        function_return_types.insert("datara_rt_file_read_bytes".into(), "List<Int>".into());
        function_return_types.insert("file_write_bytes".into(), "Int".into());
        function_return_types.insert("datara_rt_file_write_bytes".into(), "Int".into());
        function_return_types.insert("file_append".into(), "Int".into());
        function_return_types.insert("datara_rt_file_append".into(), "Int".into());
        function_return_types.insert("file_exists".into(), "Bool".into());
        function_return_types.insert("datara_rt_file_exists".into(), "Bool".into());
        // Checked I/O builtins return Outcome<Str> objects (stdlib Outcome<T>
        // layout). The codegen call classifier special-cases the
        // "Outcome<...>" prefix so the object pointer is tagged as a class
        // value, never as a raw string/list/map.
        function_return_types.insert("file_read_checked".into(), "Outcome<Str>".into());
        function_return_types.insert("datara_rt_file_read_checked".into(), "Outcome<Str>".into());
        function_return_types.insert("env_get_checked".into(), "Outcome<Str>".into());
        function_return_types.insert("datara_rt_env_get_checked".into(), "Outcome<Str>".into());
        function_return_types.insert("dir_list".into(), "List<Str>".into());
        function_return_types.insert("datara_rt_dir_list".into(), "List<Str>".into());
        function_return_types.insert("path_exists".into(), "Bool".into());
        function_return_types.insert("datara_rt_path_exists".into(), "Bool".into());
        function_return_types.insert("args_count".into(), "Int".into());
        function_return_types.insert("datara_rt_args_count".into(), "Int".into());
        function_return_types.insert("args_get".into(), "String".into());
        function_return_types.insert("datara_rt_args_get".into(), "String".into());
        function_return_types.insert("env_get".into(), "String".into());
        function_return_types.insert("datara_rt_env_get".into(), "String".into());
        function_return_types.insert("env_set".into(), "Int".into());
        function_return_types.insert("datara_rt_env_set".into(), "Int".into());
        function_return_types.insert("str_cmp".into(), "Int".into());
        function_return_types.insert("datara_rt_str_cmp".into(), "Int".into());
        function_return_types.insert("str_from_byte".into(), "String".into());
        function_return_types.insert("datara_rt_str_from_byte".into(), "String".into());
        function_return_types.insert("str_from_bytes".into(), "String".into());
        function_return_types.insert("datara_rt_str_from_bytes".into(), "String".into());
        function_return_types.insert("str_bytes".into(), "List<Int>".into());
        function_return_types.insert("datara_rt_str_bytes".into(), "List<Int>".into());
        function_return_types.insert("path_join".into(), "String".into());
        function_return_types.insert("datara_rt_path_join".into(), "String".into());
        function_return_types.insert("now".into(), "Int".into());
        function_return_types.insert("now_ms".into(), "Int".into());
        function_return_types.insert("now_ns".into(), "Int".into());
        function_return_types.insert("datara_rt_now_ns".into(), "Int".into());
        function_return_types.insert("now_precise_ms".into(), "Int".into());
        function_return_types.insert("time_precise_ms".into(), "Float".into());
        function_return_types.insert("datara_rt_time_precise_ms".into(), "Float".into());
        function_return_types.insert("time_delta_ms".into(), "Float".into());
        function_return_types.insert("datara_rt_time_delta_ms".into(), "Float".into());
        function_return_types.insert("length".into(), "Int".into());
        function_return_types.insert("count".into(), "Int".into());
        function_return_types.insert("map".into(), "List".into());
        function_return_types.insert("filter".into(), "List".into());
        function_return_types.insert("reduce".into(), "Int".into());
        function_return_types.insert("find".into(), "Int".into());
        function_return_types.insert("any".into(), "Bool".into());
        function_return_types.insert("all".into(), "Bool".into());
        function_return_types.insert("str_len".into(), "Int".into());
        function_return_types.insert("datara_rt_str_len".into(), "Int".into());
        function_return_types.insert("int_to_str".into(), "String".into());
        function_return_types.insert("datara_rt_int_to_str".into(), "String".into());
        function_return_types.insert("float_to_str".into(), "String".into());
        function_return_types.insert("datara_rt_float_to_str".into(), "String".into());
        for f in &["split", "str_split", "datara_rt_str_split"] {
            function_return_types.insert((*f).into(), "List<String>".into());
        }
        for f in &["str_join", "datara_rt_str_join"] {
            function_return_types.insert((*f).into(), "String".into());
        }
        for f in &[
            "substring",
            "substr",
            "str_substring",
            "str_substr",
            "datara_rt_str_substring",
        ] {
            function_return_types.insert((*f).into(), "String".into());
        }
        for f in &["repeat", "str_repeat", "datara_rt_str_repeat"] {
            function_return_types.insert((*f).into(), "String".into());
        }
        for f in &[
            "pad_left",
            "str_pad_left",
            "datara_rt_str_pad_left",
            "pad_right",
            "str_pad_right",
            "datara_rt_str_pad_right",
        ] {
            function_return_types.insert((*f).into(), "String".into());
        }
        for f in &["replace", "str_replace", "datara_rt_str_replace"] {
            function_return_types.insert((*f).into(), "String".into());
        }
        function_return_types.insert("socket_create".into(), "Int".into());
        function_return_types.insert("socket_bind".into(), "Int".into());
        function_return_types.insert("socket_listen".into(), "Int".into());
        function_return_types.insert("socket_accept".into(), "Int".into());
        function_return_types.insert("socket_connect".into(), "Int".into());
        function_return_types.insert("socket_send".into(), "Int".into());
        function_return_types.insert("socket_recv".into(), "String".into());
        function_return_types.insert("socket_close".into(), "Unit".into());
        function_return_types.insert("sha256".into(), "String".into());
        function_return_types.insert("base64_encode".into(), "String".into());
        function_return_types.insert("base64_decode".into(), "String".into());
        function_return_types.insert("uuid_v4".into(), "String".into());
        function_return_types.insert("datara_rt_uuid_v4".into(), "String".into());
        function_return_types.insert("datara_rt_dialog_info".into(), "Int".into());
        function_return_types.insert("datara_rt_dialog_alert".into(), "Int".into());
        function_return_types.insert("datara_rt_dialog_confirm".into(), "Int".into());
        function_return_types.insert("process_run".into(), "Int".into());
        function_return_types.insert("system".into(), "Int".into());
        function_return_types.insert("process_output".into(), "String".into());
        function_return_types.insert("exec".into(), "String".into());
        // v1.4.1: UTF-8 checked exec returns an Outcome<Str> object (stdlib
        // Outcome<T> layout); the codegen call classifier special-cases the
        // "Outcome<...>" prefix so the object pointer is tagged as a class
        // value, never as a raw string.
        function_return_types.insert("exec_utf8".into(), "Outcome<Str>".into());
        function_return_types.insert("datara_rt_exec_utf8".into(), "Outcome<Str>".into());
        for f in &[
            "math_sqrt",
            "datara_rt_math_sqrt",
            "sqrt",
            "math_pow",
            "datara_rt_math_pow",
            "pow",
            "math_abs",
            "datara_rt_math_abs",
            "abs",
            "math_sin",
            "datara_rt_math_sin",
            "sin",
            "math_cos",
            "datara_rt_math_cos",
            "cos",
            "math_tan",
            "datara_rt_math_tan",
            "tan",
            "math_floor",
            "datara_rt_math_floor",
            "floor",
            "math_ceil",
            "datara_rt_math_ceil",
            "ceil",
            "math_round",
            "datara_rt_math_round",
            "round",
            "math_min",
            "datara_rt_math_min",
            "math_max",
            "datara_rt_math_max",
            "math_hypot",
            "datara_rt_math_hypot",
            "hypot",
            "math_log",
            "datara_rt_math_log",
            "log",
            "math_exp",
            "datara_rt_math_exp",
            "exp",
            "math_clamp",
            "datara_rt_math_clamp",
            "clamp",
        ] {
            function_return_types.insert((*f).into(), "Float".into());
        }
        for f in &[
            "math_min_int",
            "datara_rt_math_min_int",
            "math_max_int",
            "datara_rt_math_max_int",
            "math_clamp_int",
            "datara_rt_math_clamp_int",
            "clamp_int",
            "math_abs_int",
            "datara_rt_math_abs_int",
            "math_ctz",
            "datara_rt_math_ctz",
            "ctz",
            "math_shr",
            "datara_rt_math_shr",
            "shr",
            "math_shl",
            "datara_rt_math_shl",
            "shl",
            "math_xor",
            "datara_rt_math_xor",
            "xor",
            "math_and",
            "datara_rt_math_and",
            "and",
            "math_or",
            "datara_rt_math_or",
            "or",
        ] {
            function_return_types.insert((*f).into(), "Int".into());
        }
        for f in &[
            "int4",
            "datara_rt_int4",
            "i32x4",
            "datara_rt_i32x4",
            "i32x4_add",
            "i32x4_sub",
            "i32x4_mul",
            "i32x4_div",
            "i32x4_min",
            "i32x4_max",
        ] {
            function_return_types.insert((*f).into(), "Int4".into());
        }
        for f in &[
            "i32x8",
            "datara_rt_i32x8",
            "i32x8_add",
            "i32x8_sub",
            "i32x8_mul",
            "i32x8_div",
            "i32x8_min",
            "i32x8_max",
        ] {
            function_return_types.insert((*f).into(), "i32x8".into());
        }
        for f in &[
            "float4",
            "datara_rt_float4",
            "f32x4",
            "datara_rt_f32x4",
            "min4",
            "max4",
            "f32x4_add",
            "f32x4_sub",
            "f32x4_mul",
            "f32x4_div",
            "f32x4_cross",
            "f32x4_min",
            "f32x4_max",
            "f32x4_lerp",
            "f32x4_normalize",
        ] {
            function_return_types.insert((*f).into(), "Float4".into());
        }
        for f in &[
            "f32x8",
            "datara_rt_f32x8",
            "f32x8_add",
            "f32x8_sub",
            "f32x8_mul",
            "f32x8_div",
            "f32x8_min",
            "f32x8_max",
            "f32x8_lerp",
            "f32x8_normalize",
        ] {
            function_return_types.insert((*f).into(), "f32x8".into());
        }
        for f in &[
            "f32x16",
            "datara_rt_f32x16",
            "f32x16_add",
            "f32x16_sub",
            "f32x16_mul",
            "f32x16_div",
            "f32x16_min",
            "f32x16_max",
            "f32x16_lerp",
            "f32x16_normalize",
        ] {
            function_return_types.insert((*f).into(), "f32x16".into());
        }
        for f in &[
            "f64x2",
            "datara_rt_f64x2",
            "f64x2_add",
            "f64x2_sub",
            "f64x2_mul",
            "f64x2_div",
            "f64x2_min",
            "f64x2_max",
            "f64x2_lerp",
            "f64x2_normalize",
        ] {
            function_return_types.insert((*f).into(), "f64x2".into());
        }
        for f in &[
            "f64x4",
            "datara_rt_f64x4",
            "f64x4_add",
            "f64x4_sub",
            "f64x4_mul",
            "f64x4_div",
            "f64x4_min",
            "f64x4_max",
            "f64x4_lerp",
            "f64x4_normalize",
        ] {
            function_return_types.insert((*f).into(), "f64x4".into());
        }
        for f in &[
            "dot",
            "datara_rt_float4_dot",
            "float4_x",
            "float4_y",
            "float4_z",
            "float4_w",
            "f32x4_dot",
            "f32x4_horizontal_add",
            "f32x4_distance",
            "aabb_intersects",
            "datara_rt_aabb_intersects",
            "f32x8_dot",
            "f32x8_horizontal_add",
            "f32x8_distance",
            "f32x16_dot",
            "f32x16_horizontal_add",
            "f32x16_distance",
            "f64x2_dot",
            "f64x2_horizontal_add",
            "f64x2_distance",
            "f64x4_dot",
            "f64x4_horizontal_add",
            "f64x4_distance",
            "dot_f32_array",
            "datara_rt_dot_f32_array",
            "ray_sphere_intersect_simd",
            "ray_sphere_intersect_scalar",
            "datara_rt_ray_sphere_intersect_simd",
            "datara_rt_ray_sphere_intersect_scalar",
            "fma",
            "fmaf",
            "datara_rt_fma",
            "datara_rt_fmaf",
            "lane0",
            "lane1",
            "lane2",
            "lane3",
        ] {
            function_return_types.insert((*f).into(), "Float".into());
        }
        for f in &[
            "int4_x",
            "int4_y",
            "int4_z",
            "int4_w",
            "i32x4_dot",
            "i32x4_horizontal_add",
            "i32x8_dot",
            "i32x8_horizontal_add",
            "arena_alloc",
            "datara_rt_arena_alloc",
            "mem_alloc",
            "datara_rt_mem_alloc",
            "arena_used",
            "datara_rt_arena_used",
            "ptr_read_i64",
            "datara_rt_ptr_read_i64",
            "ptr_read_u8",
            "datara_rt_ptr_read_u8",
            "datara_rt_global_get",
        ] {
            function_return_types.insert((*f).into(), "Int".into());
        }
        for f in &["ptr_read_f64", "datara_rt_ptr_read_f64"] {
            function_return_types.insert((*f).into(), "Float".into());
        }
        for f in &[
            "arena_reset",
            "datara_rt_arena_reset",
            "arena_clear",
            "datara_rt_arena_clear",
            "mem_free",
            "datara_rt_mem_free",
            "mem_copy",
            "datara_rt_mem_copy",
            "ptr_write_i64",
            "datara_rt_ptr_write_i64",
            "ptr_write_f64",
            "datara_rt_ptr_write_f64",
            "ptr_write_u8",
            "datara_rt_ptr_write_u8",
            "cpu_fence",
            "datara_rt_cpu_fence",
            "cpu_prefetch",
            "datara_rt_cpu_prefetch",
            "datara_rt_global_set",
        ] {
            function_return_types.insert((*f).into(), "Unit".into());
        }

        Self {
            resolver,
            types,
            val_counter: 0,
            block_counter: 0,
            symbol_values: HashMap::new(),
            current_blocks: Vec::new(),
            class_field_types: HashMap::new(),
            function_return_types,
            current_fn_name: String::new(),
            local_var_types: HashMap::new(),
            enum_variant_tags: HashMap::new(),
            module_alias_functions: HashMap::new(),
            bridge_registry: crate::bridge_decl::BridgeRegistry::new(),
            enum_variant_names: HashMap::new(),
            enum_slots: HashMap::new(),
            current_line_spans: Vec::new(),
            in_wrapping_mode: false,
            in_saturating_mode: false,
            local_lambdas: HashMap::new(),
            inlineable_fns: HashMap::new(),
            lambda_captures: HashMap::new(),
            loop_stack: Vec::new(),
            global_vars: HashMap::new(),
            program_globals: Vec::new(),
        }
    }

    pub fn lookup_var_type(&self, var_name: &str) -> Option<DataraType> {
        if let Some(ty) = self.local_var_types.get(var_name) {
            return Some(ty.clone());
        }
        if !self.current_fn_name.is_empty()
            && let Some(ty) = self
                .types
                .fn_symbol_types
                .get(&(self.current_fn_name.clone(), var_name.to_string()))
        {
            return Some(ty.clone());
        }
        if let Some(ty) = self.types.symbol_types.get(var_name) {
            return Some(ty.clone());
        }
        if let Some((ty_str, _)) = self.global_vars.get(var_name) {
            return Some(match ty_str.as_str() {
                "String" | "Str" => DataraType::String,
                "Float" | "Float64" | "f64" => DataraType::Float,
                "Bool" => DataraType::Bool,
                _ => DataraType::Int,
            });
        }
        None
    }

    pub fn next_val(&mut self) -> ValueId {
        let v = ValueId(self.val_counter);
        self.val_counter += 1;
        v
    }

    pub fn create_block(&mut self, label: &str) -> BasicBlockId {
        let id = BasicBlockId(self.block_counter);
        self.block_counter += 1;
        self.current_blocks.push(BasicBlock {
            id,
            label: format!("{}_{}", label, id.0),
            params: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    pub fn get_block_mut(&mut self, id: BasicBlockId) -> &mut BasicBlock {
        if id.0 < self.current_blocks.len() && self.current_blocks[id.0].id == id {
            return &mut self.current_blocks[id.0];
        }
        self.current_blocks
            .iter_mut()
            .find(|b| b.id == id)
            .expect("Block must exist")
    }

    /// True when `id` still has the placeholder terminator, i.e. control really
    /// reaches the end of the block.
    pub fn block_falls_through(&self, id: BasicBlockId) -> bool {
        if id.0 < self.current_blocks.len() && self.current_blocks[id.0].id == id {
            return matches!(
                self.current_blocks[id.0].terminator,
                Terminator::Unreachable
            );
        }
        self.current_blocks
            .iter()
            .find(|b| b.id == id)
            .map(|b| matches!(b.terminator, Terminator::Unreachable))
            .unwrap_or(false)
    }

    /// Wires a loop back-edge from `from` to `target`.
    ///
    /// The edge is only installed when the block still falls through. A body
    /// that ends in `return` already has `Terminator::Return`; overwriting it
    /// would silently discard the early return and produce an infinite loop.
    pub fn set_back_edge(&mut self, from: BasicBlockId, target: BasicBlockId) {
        if self.block_falls_through(from) {
            self.get_block_mut(from).terminator = Terminator::Branch {
                target,
                args: Vec::new(),
            };
        }
    }

    /// Runtime representation string of a declared type.
    ///
    /// The abstract Result/Option spellings map to their concrete stdlib
    /// representations so the backend sees a class type (a returned pointer)
    /// instead of a bare scalar: `T!E`/`Result<T, E>` -> `Outcome<T>`,
    /// `T?`/`Option<T>` -> `Maybe<T>`. `full_type_name()` alone would reduce
    /// `Int!String` to "Int" and make the backend treat a returned Outcome
    /// object as an integer.
    fn repr_type_string(tn: &TypeNode) -> String {
        let is_result = tn.error_type.is_some() || tn.name == "Result";
        let is_option = tn.is_option || tn.name == "Option";
        if is_result {
            let ok = if tn.name == "Result" && !tn.generic_args.is_empty() {
                tn.generic_args[0].full_type_name()
            } else {
                tn.full_type_name()
            };
            format!("Outcome<{}>", ok)
        } else if is_option {
            let inner = if tn.name == "Option" && !tn.generic_args.is_empty() {
                tn.generic_args[0].full_type_name()
            } else {
                tn.full_type_name()
            };
            format!("Maybe<{}>", inner)
        } else if matches!(
            tn.name.as_str(),
            "Float"
                | "Float64"
                | "Float32"
                | "Int"
                | "Int64"
                | "Int32"
                | "Int16"
                | "Int8"
                | "UInt"
                | "UInt64"
                | "UInt32"
                | "UInt16"
                | "UInt8"
                | "Byte"
        ) {
            tn.name.clone()
        } else {
            tn.full_type_name()
        }
    }

    pub fn lower_program(&mut self, program: &Program, name: &str) -> Module {
        let mut module = Module::new(name);
        module.link_libraries = program.link_libraries.clone();
        self.module_alias_functions = program
            .module_aliases
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.bridge_registry = crate::bridge_decl::BridgeRegistry::from_program(program);
        self.global_vars.clear();
        self.program_globals.clear();

        for decl in &program.declarations {
            if let Decl::Global(g) = decl {
                let ty_str = if let Some(t) = &g.type_node {
                    Self::repr_type_string(t)
                } else if let Some(ty) = self.types.symbol_types.get(&g.name) {
                    ty.to_string()
                } else {
                    "Int".to_string()
                };
                self.global_vars
                    .insert(g.name.clone(), (ty_str.clone(), g.is_mut));
                self.program_globals.push(g.clone());
                module.globals.insert(g.name.clone(), (ty_str, g.is_mut));
            }
        }

        for decl in &program.declarations {
            if let Decl::Class(c) = decl {
                for item in &c.body_items {
                    if let ClassItem::Field(f) = item {
                        if let Some(t) = &f.type_node {
                            self.class_field_types
                                .insert(format!("{}.{}", c.name, f.name), t.full_type_name());
                            self.class_field_types
                                .insert(f.name.clone(), t.full_type_name());
                        }
                    } else if let ClassItem::Method(m) = item {
                        let ret = m
                            .return_type
                            .as_ref()
                            .map(|t| t.full_type_name())
                            .unwrap_or_else(|| "Unit".into());
                        self.function_return_types
                            .insert(format!("{}_{}", c.name, m.name), ret.clone());
                        self.function_return_types.insert(m.name.clone(), ret);
                    }
                }
            } else if let Decl::Component(c) = decl {
                module.component_classes.insert(c.name.clone());
                for item in &c.body_items {
                    if let ClassItem::Field(f) = item
                        && let Some(t) = &f.type_node
                    {
                        self.class_field_types
                            .insert(format!("{}.{}", c.name, f.name), t.full_type_name());
                        self.class_field_types
                            .insert(f.name.clone(), t.full_type_name());
                    }
                }
            } else if let Decl::Behavior(b) = decl {
                for item in &b.body_items {
                    if let ClassItem::Field(f) = item {
                        if let Some(t) = &f.type_node {
                            self.class_field_types.insert(
                                format!("{}.{}", b.target_type, f.name),
                                t.full_type_name(),
                            );
                            self.class_field_types
                                .insert(f.name.clone(), t.full_type_name());
                        }
                    } else if let ClassItem::Method(m) = item {
                        let ret = m
                            .return_type
                            .as_ref()
                            .map(Self::repr_type_string)
                            .unwrap_or_else(|| "Unit".into());
                        self.function_return_types
                            .insert(format!("{}_{}", b.target_type, m.name), ret.clone());
                        self.function_return_types.insert(m.name.clone(), ret);
                    }
                }
            } else if let Decl::Impl(i) = decl {
                for m in &i.methods {
                    let ret = m
                        .return_type
                        .as_ref()
                        .map(Self::repr_type_string)
                        .unwrap_or_else(|| "Unit".into());
                    self.function_return_types
                        .insert(format!("{}_{}", i.target_type, m.name), ret.clone());
                    self.function_return_types.insert(m.name.clone(), ret);
                }
            } else if let Decl::Enum(e) = decl {
                let max_fields = e.variants.iter().map(|v| v.fields.len()).max().unwrap_or(0);
                let mut slot_types: Vec<String> = vec!["Int".to_string(); max_fields];
                for v in &e.variants {
                    for (idx, fty) in v.fields.iter().enumerate() {
                        slot_types[idx] = fty.full_type_name();
                    }
                }
                for (v_idx, v) in e.variants.iter().enumerate() {
                    let full_vname = format!("{}_{}", e.name, v.name);
                    self.enum_variant_tags
                        .insert(format!("{}.{}", e.name, v.name), v_idx as i64);
                    self.enum_variant_tags.insert(v.name.clone(), v_idx as i64);
                    self.enum_variant_names
                        .insert(v_idx as i64, full_vname.clone());
                    self.enum_slots
                        .insert(format!("{}.{}", e.name, v.name), slot_types.clone());
                    self.enum_slots.insert(v.name.clone(), slot_types.clone());
                    self.class_field_types
                        .insert(format!("{}.__tag", full_vname), "Int".into());
                    self.class_field_types.insert("__tag".into(), "Int".into());
                    for (s_idx, s_ty) in slot_types.iter().enumerate() {
                        self.class_field_types
                            .insert(format!("{}.f{}", full_vname, s_idx), s_ty.clone());
                        self.class_field_types
                            .insert(format!("f{}", s_idx), s_ty.clone());
                    }
                }
            } else if let Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) = decl {
                let ret = f
                    .return_type
                    .as_ref()
                    .map(Self::repr_type_string)
                    .unwrap_or_else(|| "Unit".into());
                self.function_return_types.insert(f.name.clone(), ret);
                // Lambda-argument static dispatch: index function bodies that
                // are a simple statement sequence ending in one `return
                // <expr>` (or the `=> expr` shorthand) so a call site that
                // passes a lambda argument can inline the body and bind the
                // lambda to the parameter name. Only these trivially
                // inlinable bodies are eligible; anything with early
                // control flow stays a real call.
                let body_parts: Option<(Vec<Stmt>, Option<Expr>)> = match &*f.body {
                    Stmt::Return(Some(e), _) => Some((Vec::new(), Some(e.clone()))),
                    Stmt::Expr(e, _) if f.is_expression_body => Some((Vec::new(), Some(e.clone()))),
                    Stmt::Block(stmts, _) => match stmts.split_last() {
                        Some((Stmt::Return(Some(e), _), head)) => {
                            let has_early_exit = head.iter().any(|s| {
                                matches!(s, Stmt::Return(_, _) | Stmt::Break(_) | Stmt::Continue(_))
                            });
                            if has_early_exit {
                                None
                            } else {
                                Some((head.to_vec(), Some(e.clone())))
                            }
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some((inline_stmts, tail_expr)) = body_parts {
                    self.inlineable_fns
                        .insert(f.name.clone(), (f.params.clone(), inline_stmts, tail_expr));
                }
            } else if let Decl::ExternFn(ef) = decl {
                let ret = ef
                    .return_type
                    .as_ref()
                    .map(Self::repr_type_string)
                    .unwrap_or_else(|| "Unit".into());
                let params: Vec<String> = ef
                    .params
                    .iter()
                    .map(|p| {
                        p.type_node
                            .as_ref()
                            .map(Self::repr_type_string)
                            .unwrap_or_else(|| "Int".into())
                    })
                    .collect();
                module
                    .extern_functions
                    .insert(ef.name.clone(), (params, ret.clone()));
                if let Some(size) = ef.sret_size {
                    module.extern_sret.insert(ef.name.clone(), size);
                }
                if let Some(classes) = ef.sysv_classes {
                    module.extern_sysv.insert(ef.name.clone(), classes);
                }
                self.function_return_types.insert(ef.name.clone(), ret);
            } else if let Decl::Impl(i) = decl {
                for m in &i.methods {
                    let ret = m
                        .return_type
                        .as_ref()
                        .map(Self::repr_type_string)
                        .unwrap_or_else(|| "Unit".into());
                    self.function_return_types
                        .insert(format!("{}_{}", i.target_type, m.name), ret.clone());
                    self.function_return_types.insert(m.name.clone(), ret);
                }
            }
        }

        // Field DECLARATION order is ABI-visible: SysV AMD64 register-pair
        // struct returns (v1.3.3) hand each eightbyte to the caller in C
        // header order, and GetField/StructInit address fields by their
        // index in this vector. Sorting names here silently renumbered
        // field offsets whenever the declaration order was not
        // alphabetical (e.g. cimport structs), corrupting every
        // mixed-class FFI boundary. HashMap iteration order is
        // nondeterministic, so the order source is the Program AST:
        // ClassDecl body_items in declaration order, with the resolver's
        // sorted view kept only as a fallback for programmatically
        // injected prelude classes.
        let mut type_synonyms: HashMap<String, String> = HashMap::new();
        let mut decl_field_order: HashMap<String, Vec<String>> = HashMap::new();
        // Raw per-class pieces in source order, plus inheritance shape.
        let mut own_fields: HashMap<String, Vec<String>> = HashMap::new();
        let mut class_shape: HashMap<String, (Option<String>, Vec<String>)> = HashMap::new();
        for decl in &program.declarations {
            match decl {
                Decl::Class(c) => {
                    let names: Vec<String> = c
                        .body_items
                        .iter()
                        .filter_map(|item| match item {
                            ClassItem::Field(f) => Some(f.name.clone()),
                            _ => None,
                        })
                        .collect();
                    let uses: Vec<String> = c
                        .body_items
                        .iter()
                        .filter_map(|item| match item {
                            ClassItem::Using(u, _) => Some(u.clone()),
                            _ => None,
                        })
                        .collect();
                    own_fields.insert(c.name.clone(), names);
                    class_shape.insert(c.name.clone(), (c.base_type.clone(), uses));
                }
                Decl::Component(comp) => {
                    let names: Vec<String> = comp
                        .body_items
                        .iter()
                        .filter_map(|item| match item {
                            ClassItem::Field(f) => Some(f.name.clone()),
                            _ => None,
                        })
                        .collect();
                    own_fields.entry(comp.name.clone()).or_insert(names);
                }
                Decl::Type(t) => {
                    // A type synonym inherits its target's field order.
                    type_synonyms.insert(t.name.clone(), t.base_type.name.clone());
                }
                _ => {}
            }
        }
        // Resolve the full field order per class: own fields first (they
        // win, matching the resolver's skip-existing merge), then the base
        // class order, then each composition's order -- the same sequence
        // merge_class_hierarchy uses when inserting inherited fields.
        fn compose_order(
            name: &str,
            own: &HashMap<String, Vec<String>>,
            shape: &HashMap<String, (Option<String>, Vec<String>)>,
            synonyms: &HashMap<String, String>,
            out: &mut HashMap<String, Vec<String>>,
            seen: &mut Vec<String>,
        ) {
            if out.contains_key(name) || seen.iter().any(|s| s == name) {
                return;
            }
            seen.push(name.to_string());
            // Follow type synonyms to the underlying class, if any.
            let effective = synonyms
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_string());
            if effective != *name {
                compose_order(&effective, own, shape, synonyms, out, seen);
                if let Some(order) = out.get(&effective).cloned() {
                    out.insert(name.to_string(), order);
                }
                seen.pop();
                return;
            }
            let mut merged: Vec<String> = Vec::new();
            let push_unique = |src: &Vec<String>, merged: &mut Vec<String>| {
                for f in src {
                    if !merged.iter().any(|m| m == f) {
                        merged.push(f.clone());
                    }
                }
            };
            if let Some(fields) = own.get(name) {
                push_unique(fields, &mut merged);
            }
            if let Some((base, comps)) = shape.get(name) {
                if let Some(base) = base {
                    compose_order(base, own, shape, synonyms, out, seen);
                    if let Some(order) = out.get(base) {
                        push_unique(order, &mut merged);
                    }
                }
                for comp in comps {
                    compose_order(comp, own, shape, synonyms, out, seen);
                    if let Some(order) = out.get(comp) {
                        push_unique(order, &mut merged);
                    }
                }
            }
            out.insert(name.to_string(), merged);
            seen.pop();
        }
        let mut class_keys: Vec<String> = own_fields.keys().cloned().collect();
        class_keys.sort();
        for name in &class_keys {
            let mut seen: Vec<String> = Vec::new();
            compose_order(
                name,
                &own_fields,
                &class_shape,
                &type_synonyms,
                &mut decl_field_order,
                &mut seen,
            );
        }
        let mut sorted_classes: Vec<_> = self.resolver.classes.iter().collect();
        sorted_classes.sort_by_key(|(name, _)| (*name).clone());
        for (cls_name, cls_sym) in sorted_classes {
            let f_names = decl_field_order.get(cls_name).cloned().unwrap_or_else(|| {
                let mut names: Vec<String> = cls_sym.fields.keys().cloned().collect();
                names.sort();
                names
            });
            module.class_fields.insert(cls_name.clone(), f_names);
        }

        // Pre-register generic specialization return types so earlier functions (like main)
        // can resolve calls to specialized functions (e.g. format_entity_Admin).
        for decl in &program.declarations {
            if let Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) = decl
                && !f.generic_params.is_empty()
            {
                if let Some(specs) = self.types.generic_specializations.get(&f.name) {
                    for spec_args in specs {
                        let mut type_substs: HashMap<String, String> = HashMap::new();
                        let mut mangled_suffixes = Vec::new();
                        for (gp, concrete_ty) in f.generic_params.iter().zip(spec_args.iter()) {
                            let c_name = match concrete_ty {
                                DataraType::Class(c) => c.clone(),
                                DataraType::Int => "Int".to_string(),
                                DataraType::Float => "Float".to_string(),
                                DataraType::String => "String".to_string(),
                                DataraType::Bool => "Bool".to_string(),
                                other => other.to_string(),
                            };
                            type_substs.insert(gp.clone(), c_name.clone());
                            mangled_suffixes.push(c_name);
                        }
                        let mangled_name = format!("{}_{}", f.name, mangled_suffixes.join("_"));
                        let specialized_f =
                            self.specialize_function_decl(f, &mangled_name, &type_substs);
                        let ret = specialized_f
                            .return_type
                            .as_ref()
                            .map(Self::repr_type_string)
                            .unwrap_or_else(|| "Unit".into());
                        self.function_return_types.insert(mangled_name, ret);
                    }
                }
            }
        }

        for decl in &program.declarations {
            match decl {
                Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                    let lowered_fn = self.lower_function(f);
                    // Lambda-argument static dispatch: a function whose body
                    // invokes one of its own parameters (`return g(v)` where
                    // `g: T`) can only ever run inline at a call site that
                    // binds a lambda (see lower_expr_call). Its standalone
                    // body would hold an unresolved
                    // `Inst::Call { func: <param> }`, so it is never emitted
                    // as a real function. A normal (non-lambda) call to it
                    // then fails codegen with an unresolved-function error
                    // instead of crashing the backend.
                    let is_lambda_dispatch_only = self.inlineable_fns.contains_key(&f.name)
                        && lowered_fn.blocks.iter().any(|b| {
                            b.instructions.iter().any(|inst| {
                                matches!(inst, Inst::Call { func, .. } if
                                    f.params.iter().any(|p| p.name == *func))
                            })
                        });
                    if !is_lambda_dispatch_only {
                        module.functions.insert(f.name.clone(), lowered_fn);
                        module.function_spans.insert(f.name.clone(), f.span.clone());
                        module
                            .function_line_spans
                            .insert(f.name.clone(), self.current_line_spans.clone());
                    }
                }
                Decl::Class(c) => {
                    if c.attributes.iter().any(|a| a.name == "packed") {
                        module.packed_classes.insert(c.name.clone());
                    }
                    if let Some(a) = c.attributes.iter().find(|a| a.name == "endian") {
                        let order = a.args.first().map(|(arg, _)| arg.as_str()).unwrap_or("big");
                        module
                            .endian_classes
                            .insert(c.name.clone(), order.to_string());
                    }
                    self.lower_class(c, program, &mut module);
                }
                Decl::Behavior(b) => {
                    self.lower_behavior(b, &mut module);
                }
                Decl::Impl(i) => {
                    self.lower_impl(i, &mut module);
                }
                _ => {}
            }
        }

        // Static Monomorphization: instantiate generic functions per concrete type argument
        for decl in &program.declarations {
            if let Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) = decl
                && !f.generic_params.is_empty()
            {
                if let Some(specs) = self.types.generic_specializations.get(&f.name) {
                    for spec_args in specs {
                        // Lambda arguments bind the generic parameter to a
                        // Function type; such call sites are inline-lowered
                        // (lambda-argument static dispatch) and must not be
                        // instantiated, because the instantiated body would
                        // still call the parameter by name.
                        if spec_args
                            .iter()
                            .any(|t| matches!(t, DataraType::Function { .. }))
                        {
                            continue;
                        }
                        let mut type_substs: HashMap<String, String> = HashMap::new();
                        let mut mangled_suffixes = Vec::new();
                        for (gp, concrete_ty) in f.generic_params.iter().zip(spec_args.iter()) {
                            let c_name = match concrete_ty {
                                DataraType::Class(c) => c.clone(),
                                DataraType::Int => "Int".to_string(),
                                DataraType::Float => "Float".to_string(),
                                DataraType::String => "String".to_string(),
                                DataraType::Bool => "Bool".to_string(),
                                other => other.to_string(),
                            };
                            type_substs.insert(gp.clone(), c_name.clone());
                            mangled_suffixes.push(c_name);
                        }
                        let mangled_name = format!("{}_{}", f.name, mangled_suffixes.join("_"));
                        let specialized_f =
                            self.specialize_function_decl(f, &mangled_name, &type_substs);
                        let ret = specialized_f
                            .return_type
                            .as_ref()
                            .map(Self::repr_type_string)
                            .unwrap_or_else(|| "Unit".into());
                        self.function_return_types.insert(mangled_name.clone(), ret);
                        let lowered_spec = self.lower_function(&specialized_f);
                        module.functions.insert(mangled_name.clone(), lowered_spec);
                        module
                            .function_spans
                            .insert(mangled_name.clone(), f.span.clone());
                        module
                            .function_line_spans
                            .insert(mangled_name.clone(), self.current_line_spans.clone());
                    }
                }
            }
        }

        module.class_field_types = self.class_field_types.clone();
        module
    }
}
