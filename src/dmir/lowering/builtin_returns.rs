//! Builtin function return-type table (extracted from lowering/mod.rs for
//! the 60 KB release size gate). Maps Datara builtin/extern function names
//! to their DMIR return-type string — the LLVM backend uses it to type the
//! call result (`ret i64` vs `ret ptr` parity class of bugs).

use std::collections::HashMap;

pub(crate) fn register(map: &mut HashMap<String, String>) {
    map.insert("str_to_int".into(), "Int".into());
    map.insert("datara_rt_str_to_int".into(), "Int".into());
    map.insert("str_to_float".into(), "Float".into());
    map.insert("datara_rt_str_to_float".into(), "Float".into());
    map.insert("str_index_of".into(), "Int".into());
    map.insert("datara_rt_str_index_of".into(), "Int".into());
    map.insert("str_contains".into(), "Bool".into());
    map.insert("datara_rt_str_contains".into(), "Bool".into());
    map.insert("str_starts_with".into(), "Bool".into());
    map.insert("datara_rt_str_starts_with".into(), "Bool".into());
    map.insert("str_ends_with".into(), "Bool".into());
    map.insert("datara_rt_str_ends_with".into(), "Bool".into());
    map.insert("str_trim".into(), "String".into());
    map.insert("datara_rt_str_trim".into(), "String".into());
    map.insert("input".into(), "String".into());
    map.insert("read_line".into(), "String".into());
    map.insert("datara_rt_input".into(), "String".into());
    map.insert("http_get".into(), "String".into());
    map.insert("datara_rt_http_get".into(), "String".into());
    map.insert("file_read".into(), "String".into());
    map.insert("read_file".into(), "String".into());
    map.insert("datara_rt_file_read".into(), "String".into());
    map.insert("file_write".into(), "Int".into());
    map.insert("write_file".into(), "Int".into());
    map.insert("datara_rt_file_write".into(), "Int".into());
    map.insert("file_read_bytes".into(), "List<Int>".into());
    map.insert("datara_rt_file_read_bytes".into(), "List<Int>".into());
    map.insert("file_write_bytes".into(), "Int".into());
    map.insert("datara_rt_file_write_bytes".into(), "Int".into());
    map.insert("file_append".into(), "Int".into());
    map.insert("datara_rt_file_append".into(), "Int".into());
    map.insert("file_exists".into(), "Bool".into());
    map.insert("datara_rt_file_exists".into(), "Bool".into());
    // Capability token factory: `SystemCapabilities {}` lowers to this
    // zero-arg runtime call; the result is the opaque token pointer.
    map.insert(
        "datara_rt_sys_caps_create".into(),
        "SystemCapabilities".into(),
    );
    // Checked I/O builtins return Outcome<Str> objects (stdlib Outcome<T>
    // layout). The codegen call classifier special-cases the
    // "Outcome<...>" prefix so the object pointer is tagged as a class
    // value, never as a raw string/list/map.
    map.insert("file_read_checked".into(), "Outcome<Str>".into());
    map.insert("datara_rt_file_read_checked".into(), "Outcome<Str>".into());
    for f in [
        "str_cp_to_byte",
        "datara_rt_str_cp_to_byte",
        "str_byte_to_cp",
        "datara_rt_str_byte_to_cp",
    ] {
        map.insert((*f).into(), "Int".into());
    }
    for f in ["str_substring_cp", "datara_rt_str_substring_cp"] {
        map.insert((*f).into(), "String".into());
    }
    // Codepoint/byte scalar access returns a Str pointer. Without this
    // entry the DMIR Call carried ty="Int" (generic fallback), which
    // Cranelift tolerated (untyped i64 carrier) but LLVM emitted as
    // `ret i64` inside a ptr-returning function — llc type error,
    // silent Cranelift fallback, false-OK parity row.
    for f in [
        "str_scalar_at",
        "datara_rt_str_scalar_at",
        "str_scalar_at_cp",
        "datara_rt_str_scalar_at_cp",
    ] {
        map.insert((*f).into(), "String".into());
    }
    map.insert("parse_int".into(), "Outcome<Int>".into());
    map.insert("datara_rt_str_to_int_checked".into(), "Outcome<Int>".into());
    map.insert("parse_float".into(), "Outcome<Float>".into());
    map.insert(
        "datara_rt_str_to_float_checked".into(),
        "Outcome<Float>".into(),
    );
    map.insert("env_get_checked".into(), "Outcome<Str>".into());
    map.insert("datara_rt_env_get_checked".into(), "Outcome<Str>".into());
    // v1.5.0 (M1): process spawn / pipes / wait return Outcome objects.
    // Without these entries the call classifier tags the object pointer
    // as a raw Int and every `or`/is_ok site fails with E0944.
    map.insert("proc_spawn".into(), "Outcome<Int>".into());
    map.insert("datara_rt_proc_spawn".into(), "Outcome<Int>".into());
    map.insert("proc_run_input".into(), "Outcome<Str>".into());
    map.insert("datara_rt_proc_run_input".into(), "Outcome<Str>".into());
    map.insert("proc_wait".into(), "Outcome<Int>".into());
    map.insert("datara_rt_proc_wait".into(), "Outcome<Int>".into());
    map.insert("dir_list".into(), "List<Str>".into());
    map.insert("datara_rt_dir_list".into(), "List<Str>".into());
    map.insert("path_exists".into(), "Bool".into());
    map.insert("datara_rt_path_exists".into(), "Bool".into());
    map.insert("args_count".into(), "Int".into());
    map.insert("datara_rt_args_count".into(), "Int".into());
    map.insert("args_get".into(), "String".into());
    map.insert("datara_rt_args_get".into(), "String".into());
    map.insert("env_get".into(), "String".into());
    map.insert("datara_rt_env_get".into(), "String".into());
    map.insert("env_set".into(), "Int".into());
    map.insert("datara_rt_env_set".into(), "Int".into());
    map.insert("str_cmp".into(), "Int".into());
    map.insert("datara_rt_str_cmp".into(), "Int".into());
    map.insert("str_from_byte".into(), "String".into());
    map.insert("datara_rt_str_from_byte".into(), "String".into());
    map.insert("str_from_bytes".into(), "String".into());
    map.insert("datara_rt_str_from_bytes".into(), "String".into());
    map.insert("str_bytes".into(), "List<Int>".into());
    map.insert("datara_rt_str_bytes".into(), "List<Int>".into());
    map.insert("path_join".into(), "String".into());
    map.insert("datara_rt_path_join".into(), "String".into());
    map.insert("now".into(), "Int".into());
    map.insert("now_ms".into(), "Int".into());
    map.insert("now_ns".into(), "Int".into());
    map.insert("datara_rt_now_ns".into(), "Int".into());
    map.insert("now_precise_ms".into(), "Int".into());
    map.insert("time_precise_ms".into(), "Float".into());
    map.insert("datara_rt_time_precise_ms".into(), "Float".into());
    map.insert("time_delta_ms".into(), "Float".into());
    map.insert("datara_rt_time_delta_ms".into(), "Float".into());
    map.insert("length".into(), "Int".into());
    map.insert("count".into(), "Int".into());
    map.insert("map".into(), "List".into());
    map.insert("filter".into(), "List".into());
    map.insert("reduce".into(), "Int".into());
    map.insert("find".into(), "Int".into());
    map.insert("any".into(), "Bool".into());
    map.insert("all".into(), "Bool".into());
    map.insert("str_len".into(), "Int".into());
    map.insert("datara_rt_str_len".into(), "Int".into());
    map.insert("int_to_str".into(), "String".into());
    map.insert("datara_rt_int_to_str".into(), "String".into());
    map.insert("float_to_str".into(), "String".into());
    map.insert("datara_rt_float_to_str".into(), "String".into());
    for f in &["split", "str_split", "datara_rt_str_split"] {
        map.insert((*f).into(), "List<String>".into());
    }
    for f in &["str_join", "datara_rt_str_join"] {
        map.insert((*f).into(), "String".into());
    }
    for f in &[
        "substring",
        "substr",
        "str_substring",
        "str_substr",
        "datara_rt_str_substring",
    ] {
        map.insert((*f).into(), "String".into());
    }
    for f in &["repeat", "str_repeat", "datara_rt_str_repeat"] {
        map.insert((*f).into(), "String".into());
    }
    for f in &[
        "pad_left",
        "str_pad_left",
        "datara_rt_str_pad_left",
        "pad_right",
        "str_pad_right",
        "datara_rt_str_pad_right",
    ] {
        map.insert((*f).into(), "String".into());
    }
    for f in &["replace", "str_replace", "datara_rt_str_replace"] {
        map.insert((*f).into(), "String".into());
    }
    map.insert("socket_create".into(), "Int".into());
    map.insert("socket_bind".into(), "Int".into());
    map.insert("socket_listen".into(), "Int".into());
    map.insert("socket_accept".into(), "Int".into());
    map.insert("socket_connect".into(), "Int".into());
    map.insert("socket_send".into(), "Int".into());
    map.insert("socket_recv".into(), "String".into());
    map.insert("socket_close".into(), "Unit".into());
    map.insert("sha256".into(), "String".into());
    map.insert("base64_encode".into(), "String".into());
    map.insert("base64_decode".into(), "String".into());
    map.insert("uuid_v4".into(), "String".into());
    map.insert("datara_rt_uuid_v4".into(), "String".into());
    map.insert("datara_rt_dialog_info".into(), "Int".into());
    map.insert("datara_rt_dialog_alert".into(), "Int".into());
    map.insert("datara_rt_dialog_confirm".into(), "Int".into());
    map.insert("process_run".into(), "Int".into());
    map.insert("system".into(), "Int".into());
    map.insert("process_output".into(), "String".into());
    map.insert("exec".into(), "String".into());
    // v1.4.1: UTF-8 checked exec returns an Outcome<Str> object (stdlib
    // Outcome<T> layout); the codegen call classifier special-cases the
    // "Outcome<...>" prefix so the object pointer is tagged as a class
    // value, never as a raw string.
    map.insert("exec_utf8".into(), "Outcome<Str>".into());
    map.insert("datara_rt_exec_utf8".into(), "Outcome<Str>".into());
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
        map.insert((*f).into(), "Float".into());
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
        map.insert((*f).into(), "Int".into());
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
        map.insert((*f).into(), "Int4".into());
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
        map.insert((*f).into(), "i32x8".into());
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
        map.insert((*f).into(), "Float4".into());
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
        map.insert((*f).into(), "f32x8".into());
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
        map.insert((*f).into(), "f32x16".into());
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
        map.insert((*f).into(), "f64x2".into());
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
        map.insert((*f).into(), "f64x4".into());
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
        map.insert((*f).into(), "Float".into());
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
        "arc_count",
        "datara_rt_arc_count",
    ] {
        map.insert((*f).into(), "Int".into());
    }
    for f in &[
        "arc_init",
        "datara_rt_arc_init",
        "arc_acquire",
        "datara_rt_arc_acquire",
        "weak_create",
        "datara_rt_weak_create",
    ] {
        map.insert((*f).into(), "RawPtr".into());
    }
    for f in &[
        "arc_release",
        "datara_rt_arc_release",
        "weak_drop",
        "datara_rt_weak_drop",
    ] {
        map.insert((*f).into(), "Unit".into());
    }
    for f in &["ptr_read_f64", "datara_rt_ptr_read_f64"] {
        map.insert((*f).into(), "Float".into());
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
        map.insert((*f).into(), "Unit".into());
    }
}
