//! Core builtin function-signature table (PLAN_V151: extracted from
//! prelude.rs for the 60 KB release size gate — same pattern as
//! prelude_concurrency/prelude_systems).

use super::{DataraType, TypeChecker};
use std::collections::HashMap;

impl<'a> TypeChecker<'a> {
    /// Registers the core builtin function signatures into the prelude table.
    pub(crate) fn register_core_signatures(
        function_signatures: &mut HashMap<String, (Vec<DataraType>, DataraType, Vec<String>)>,
    ) {
        function_signatures.insert(
            "fs_open".to_string(),
            (
                vec![DataraType::String],
                DataraType::Class("FileHandle".into()),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "fs_read".to_string(),
            (
                vec![DataraType::Class("FileHandle".into())],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "fs_write".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "net_connect".to_string(),
            (
                vec![DataraType::String, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "net_listen".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "proc_spawn".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );

        function_signatures.insert(
            "println".to_string(),
            (vec![DataraType::String], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "print".to_string(),
            (vec![DataraType::String], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "eprintln".to_string(),
            (vec![DataraType::String], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "panic".to_string(),
            (vec![DataraType::String], DataraType::Never, Vec::new()),
        );
        function_signatures.insert(
            "assert".to_string(),
            (
                vec![DataraType::Bool, DataraType::String],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "len".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "length".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert("now".to_string(), (vec![], DataraType::Int, Vec::new()));
        function_signatures.insert("now_ms".to_string(), (vec![], DataraType::Int, Vec::new()));
        function_signatures.insert("now_ns".to_string(), (vec![], DataraType::Int, Vec::new()));
        function_signatures.insert(
            "datara_rt_now_ns".to_string(),
            (vec![], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "now_precise_ms".to_string(),
            (vec![], DataraType::Int, Vec::new()),
        );
        for name in &[
            "time_precise_ms",
            "datara_rt_time_precise_ms",
            "time_delta_ms",
            "datara_rt_time_delta_ms",
        ] {
            function_signatures.insert(name.to_string(), (vec![], DataraType::Float, Vec::new()));
        }
        for name in &["path_join", "datara_rt_path_join"] {
            function_signatures.insert(
                name.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for name in &["math_ctz", "ctz"] {
            function_signatures.insert(
                name.to_string(),
                (vec![DataraType::Int], DataraType::Int, Vec::new()),
            );
        }
        for name in &[
            "math_shr", "shr", "math_shl", "shl", "math_xor", "xor", "math_and", "and", "math_or",
            "or",
        ] {
            function_signatures.insert(
                name.to_string(),
                (
                    vec![DataraType::Int, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for name in &["wrapping", "saturating"] {
            function_signatures.insert(
                name.to_string(),
                (vec![DataraType::Int], DataraType::Int, Vec::new()),
            );
        }
        for name in &[
            "wrapping_add",
            "wrapping_sub",
            "wrapping_mul",
            "saturating_add",
            "saturating_sub",
            "saturating_mul",
        ] {
            function_signatures.insert(
                name.to_string(),
                (
                    vec![DataraType::Int, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        function_signatures.insert(
            "exit".to_string(),
            (vec![DataraType::Int], DataraType::Never, Vec::new()),
        );
        function_signatures.insert(
            "input".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "sleep".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "file_read".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "read_file".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "file_read_bytes".to_string(),
            (
                vec![DataraType::String],
                DataraType::List(Box::new(DataraType::Int)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "file_write_bytes".to_string(),
            (
                vec![
                    DataraType::String,
                    DataraType::List(Box::new(DataraType::Int)),
                ],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "file_write".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "write_file".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "file_append".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "file_exists".to_string(),
            (vec![DataraType::String], DataraType::Bool, Vec::new()),
        );
        // Checked I/O builtins return the language Outcome type: the
        // checker-level representation is Result(T, Str), matching how
        // `Outcome<T>` (single generic arg) resolves (src/types/resolve.rs)
        // and how `?` propagation reads the flag/payload fields.
        // v1.4.5 W1: checked arithmetic returns the language Outcome type
        // (checker-level Result(T, Str)) so `?`/is_err/unwrap resolve like
        // the checked-I/O builtins. Operand widths stay matching (Gate 7).
        for &(op, ret_bits) in &[
            ("checked_add", "Int"),
            ("checked_sub", "Int"),
            ("checked_mul", "Int"),
        ] {
            let ret = DataraType::Result(
                Box::new(match ret_bits {
                    "Int" => DataraType::Int,
                    _ => DataraType::Int,
                }),
                Box::new(DataraType::String),
            );
            function_signatures.insert(
                op.to_string(),
                (vec![DataraType::Int, DataraType::Int], ret, Vec::new()),
            );
        }
        function_signatures.insert(
            "file_read_checked".to_string(),
            (
                vec![DataraType::String],
                DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "parse_int".to_string(),
            (
                vec![DataraType::String],
                DataraType::Result(Box::new(DataraType::Int), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "parse_float".to_string(),
            (
                vec![DataraType::String],
                DataraType::Result(Box::new(DataraType::Float), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "env_get_checked".to_string(),
            (
                vec![DataraType::String],
                DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "dir_list".to_string(),
            (
                vec![DataraType::String],
                DataraType::List(Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "path_exists".to_string(),
            (vec![DataraType::String], DataraType::Bool, Vec::new()),
        );
        function_signatures.insert(
            "cap_set_mask".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_cap_set_mask".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "cap_get_mask".to_string(),
            (vec![], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_cap_get_mask".to_string(),
            (vec![], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "cap_revoke".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_cap_revoke".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "cap_grant".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_cap_grant".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "cap_require".to_string(),
            (
                vec![DataraType::Int, DataraType::String],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_cap_require".to_string(),
            (
                vec![DataraType::Int, DataraType::String],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "env_get".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "env_set".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_env_set".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_cmp".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_str_cmp".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_from_byte".to_string(),
            (vec![DataraType::Int], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_str_from_byte".to_string(),
            (vec![DataraType::Int], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "str_from_bytes".to_string(),
            (
                vec![DataraType::List(Box::new(DataraType::Int))],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_str_from_bytes".to_string(),
            (
                vec![DataraType::List(Box::new(DataraType::Int))],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_bytes".to_string(),
            (
                vec![DataraType::String],
                DataraType::List(Box::new(DataraType::Int)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_str_bytes".to_string(),
            (
                vec![DataraType::String],
                DataraType::List(Box::new(DataraType::Int)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "args_count".to_string(),
            (vec![], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "args_get".to_string(),
            (vec![DataraType::Int], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "str_contains".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Bool,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_starts_with".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Bool,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_ends_with".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Bool,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_index_of".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        // M1 UTF-8 codepoint indexing helpers.
        for f in &[
            "str_cp_to_byte",
            "datara_rt_str_cp_to_byte",
            "str_byte_to_cp",
            "datara_rt_str_byte_to_cp",
        ] {
            function_signatures.insert(
                (*f).to_string(),
                (
                    vec![DataraType::String, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &["str_substring_cp", "datara_rt_str_substring_cp"] {
            function_signatures.insert(
                (*f).to_string(),
                (
                    vec![DataraType::String, DataraType::Int, DataraType::Int],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "str_len",
            "datara_rt_str_len",
            "byte_len",
            "datara_rt_byte_len",
            "str_chars",
            "datara_rt_str_chars",
            "char_len",
            "datara_rt_char_len",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["str_byte_at", "datara_rt_str_byte_at", "byte_at"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &["str_sanitize_utf8", "datara_rt_str_sanitize_utf8"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::String, Vec::new()),
            );
        }
        for f in &["validate_utf8", "datara_rt_validate_utf8"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Bool, Vec::new()),
            );
        }
        for f in &[
            "str_scalar_at",
            "datara_rt_str_scalar_at",
            "str_scalar_at_cp",
            "datara_rt_str_scalar_at_cp",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Int],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["str_next_offset", "datara_rt_str_next_offset"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "str_substring",
            "substring",
            "substr",
            "str_substr",
            "datara_rt_str_substring",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Int, DataraType::Int],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        function_signatures.insert(
            "str_char_at".to_string(),
            (
                vec![DataraType::String, DataraType::Int],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_trim".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "str_to_int".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "str_to_float".to_string(),
            (vec![DataraType::String], DataraType::Float, Vec::new()),
        );
        for f in &["str_repeat", "repeat", "datara_rt_str_repeat"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Int],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "str_pad_left",
            "pad_left",
            "datara_rt_str_pad_left",
            "str_pad_right",
            "pad_right",
            "datara_rt_str_pad_right",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Int, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["str_replace", "replace", "datara_rt_str_replace"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "str_to_upper",
            "to_upper",
            "datara_rt_str_to_upper",
            "str_to_lower",
            "to_lower",
            "datara_rt_str_to_lower",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::String, Vec::new()),
            );
        }
        for f in &["str_split", "split", "datara_rt_str_split"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::List(Box::new(DataraType::String)),
                    Vec::new(),
                ),
            );
        }
        for f in &["str_join", "join", "datara_rt_str_join"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![
                        DataraType::List(Box::new(DataraType::String)),
                        DataraType::String,
                    ],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        // PLAN_V152 N1.3: non-blocking fiber completion poll.
        function_signatures.insert(
            "fiber_is_done".to_string(),
            (vec![DataraType::Int], DataraType::Bool, Vec::new()),
        );

        // PLAN_V152 N1.2 fiber mutexes: lock returns Outcome<Str>
        // (ok "locked" / err "timeout").
        function_signatures.insert(
            "mutex_new".to_string(),
            (vec![], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "mutex_lock".to_string(),
            // DataraType::Result (the fiber_net_recv convention): a bare
            // Outcome generic made `is_success` resolve to Unit.
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "mutex_unlock".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );

        // PLAN_V152 N3: hash-map iteration surface. Keys of a Str-keyed
        // map come back as List<Str>; Int-keyed maps use map_keys_int.
        // map_values assumes int-valued maps (map_values_str for Str).
        for f in &["map_keys", "datara_rt_map_keys_str"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::Class("Map".into())],
                    DataraType::List(Box::new(DataraType::String)),
                    Vec::new(),
                ),
            );
        }
        for f in &["map_keys_int", "datara_rt_map_keys_int"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::Class("Map".into())],
                    DataraType::List(Box::new(DataraType::Int)),
                    Vec::new(),
                ),
            );
        }
        function_signatures.insert(
            "map_values".to_string(),
            (
                vec![DataraType::Class("Map".into())],
                DataraType::List(Box::new(DataraType::Int)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "map_values_str".to_string(),
            (
                vec![DataraType::Class("Map".into())],
                DataraType::List(Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        for f in &["format_percent", "datara_rt_format_percent"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::Float, DataraType::Int],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["format_int_with_commas", "datara_rt_format_int_with_commas"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::Int], DataraType::String, Vec::new()),
            );
        }
        for f in &["js_eval", "datara_js_eval"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::String, Vec::new()),
            );
        }
        for f in &["js_eval_int", "datara_js_eval_int"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["js_eval_float", "datara_js_eval_float"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Float, Vec::new()),
            );
        }
        for f in &["js_require", "datara_js_require"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["js_call", "datara_js_call"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["js_call_0", "datara_js_call_0"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::String, Vec::new()),
            );
        }
        for f in &["js_call_1", "datara_js_call_1"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["js_call_2", "datara_js_call_2"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["js_set_global", "datara_js_set_global"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &["js_get_global", "datara_js_get_global"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::String, Vec::new()),
            );
        }
        for f in &[
            "py_eval",
            "datara_py_eval",
            "py_eval_safe",
            "datara_py_eval_safe",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::String, Vec::new()),
            );
        }
        for f in &["py_eval_int", "datara_py_eval_int"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["py_eval_float", "datara_py_eval_float"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Float, Vec::new()),
            );
        }
        for f in &["zig_eval_int", "datara_zig_eval_int"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["zig_call", "datara_zig_call"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &["csharp_invoke_i64", "datara_csharp_invoke_i64"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &["csharp_invoke_f64", "datara_csharp_invoke_f64"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String, DataraType::Float],
                    DataraType::Float,
                    Vec::new(),
                ),
            );
        }
        for f in &["lua_eval_int", "datara_lua_eval_int"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["lua_eval_float", "datara_lua_eval_float"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Float, Vec::new()),
            );
        }
        for f in &["lua_exec", "datara_lua_exec"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["polyglot_parallel_exec", "datara_polyglot_parallel_exec"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &["py_call", "datara_py_call"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["py_call_1_str", "datara_py_call_1_str"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::String,
                    Vec::new(),
                ),
            );
        }
        for f in &["py_call_1_float", "datara_py_call_1_float"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::String, DataraType::Float],
                    DataraType::Float,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "arena_alloc",
            "datara_rt_arena_alloc",
            "mem_alloc",
            "datara_rt_mem_alloc",
            "stack_alloc",
            "datara_rt_stack_alloc",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::Int], DataraType::RawPtr, Vec::new()),
            );
        }
        for f in &["arena_reset", "datara_rt_arena_reset"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::Int], DataraType::Unit, Vec::new()),
            );
        }
        for f in &["arena_used", "datara_rt_arena_used"] {
            function_signatures.insert(f.to_string(), (Vec::new(), DataraType::Int, Vec::new()));
        }
        for f in &["arena_clear", "datara_rt_arena_clear"] {
            function_signatures.insert(f.to_string(), (Vec::new(), DataraType::Unit, Vec::new()));
        }
        for f in &["mem_free", "datara_rt_mem_free"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::RawPtr], DataraType::Unit, Vec::new()),
            );
        }
        for f in &["mem_copy", "datara_rt_mem_copy"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::RawPtr, DataraType::RawPtr, DataraType::Int],
                    DataraType::Unit,
                    Vec::new(),
                ),
            );
        }
        // M5 TMM Layer 2: ARC primitives. Pointers flow as RawPtr; the
        // @arc-annotated class objects are pointer-sized handles.
        for f in &[
            "arc_init",
            "datara_rt_arc_init",
            "arc_acquire",
            "datara_rt_arc_acquire",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::RawPtr], DataraType::RawPtr, Vec::new()),
            );
        }
        for f in &[
            "arc_release",
            "datara_rt_arc_release",
            "weak_drop",
            "datara_rt_weak_drop",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::RawPtr], DataraType::Unit, Vec::new()),
            );
        }
        for f in &["arc_count", "datara_rt_arc_count"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::RawPtr], DataraType::Int, Vec::new()),
            );
        }
        for f in &["weak_create", "datara_rt_weak_create"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::RawPtr], DataraType::RawPtr, Vec::new()),
            );
        }
        // weak_upgrade returns Outcome<T>: the checker models it as a
        // generic Outcome payload; callers use `?`/`or` as with map.pop().
        for f in &["weak_upgrade", "datara_rt_weak_upgrade"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::RawPtr],
                    DataraType::GenericInstance {
                        name: "Outcome".into(),
                        args: vec![DataraType::RawPtr],
                    },
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "ptr_read_i64",
            "datara_rt_ptr_read_i64",
            "ptr_read_u8",
            "datara_rt_ptr_read_u8",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::RawPtr, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "ptr_write_i64",
            "datara_rt_ptr_write_i64",
            "ptr_write_u8",
            "datara_rt_ptr_write_u8",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::RawPtr, DataraType::Int, DataraType::Int],
                    DataraType::Unit,
                    Vec::new(),
                ),
            );
        }
        for f in &["ptr_read_f64", "datara_rt_ptr_read_f64"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::RawPtr, DataraType::Int],
                    DataraType::Float,
                    Vec::new(),
                ),
            );
        }
        for f in &["ptr_write_f64", "datara_rt_ptr_write_f64"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::RawPtr, DataraType::Int, DataraType::Float],
                    DataraType::Unit,
                    Vec::new(),
                ),
            );
        }
        for f in &["cpu_fence", "datara_rt_cpu_fence"] {
            function_signatures.insert(f.to_string(), (Vec::new(), DataraType::Unit, Vec::new()));
        }
        for f in &["cpu_prefetch", "datara_rt_cpu_prefetch"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::RawPtr], DataraType::Unit, Vec::new()),
            );
        }
        for f in &["py_import", "datara_py_import"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["py_exec", "datara_py_exec"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::Int, Vec::new()),
            );
        }
        for f in &["py_last_error", "datara_py_last_error"] {
            function_signatures.insert(f.to_string(), (vec![], DataraType::String, Vec::new()));
        }
        for f in &["py_clear_error", "datara_py_clear_error"] {
            function_signatures.insert(f.to_string(), (vec![], DataraType::Unit, Vec::new()));
        }
        for f in &[
            "datara_py_export_list_f64",
            "datara_py_assert_same_ptr",
            "datara_js_export_list_f64",
            "datara_js_assert_same_ptr",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![
                        DataraType::String,
                        DataraType::List(Box::new(DataraType::Float)),
                    ],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }

        for f in &[
            "math_sqrt",
            "datara_rt_math_sqrt",
            "sqrt",
            "math_abs",
            "datara_rt_math_abs",
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
            "math_log",
            "datara_rt_math_log",
            "log",
            "math_exp",
            "datara_rt_math_exp",
            "exp",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::Float], DataraType::Float, Vec::new()),
            );
        }
        for f in &["math_clamp", "datara_rt_math_clamp", "clamp"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::Float, DataraType::Float, DataraType::Float],
                    DataraType::Float,
                    Vec::new(),
                ),
            );
        }
        for f in &["math_clamp_int", "datara_rt_math_clamp_int", "clamp_int"] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::Int, DataraType::Int, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "math_pow",
            "datara_rt_math_pow",
            "pow",
            "math_min",
            "datara_rt_math_min",
            "math_max",
            "datara_rt_math_max",
            "math_hypot",
            "datara_rt_math_hypot",
            "hypot",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::Float, DataraType::Float],
                    DataraType::Float,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "math_min_int",
            "datara_rt_math_min_int",
            "math_max_int",
            "datara_rt_math_max_int",
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
            "math_bitwise_and",
            "math_bitwise_or",
            "math_bitwise_xor",
        ] {
            function_signatures.insert(
                f.to_string(),
                (
                    vec![DataraType::Int, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for f in &[
            "math_abs_int",
            "datara_rt_math_abs_int",
            "math_ctz",
            "datara_rt_math_ctz",
            "ctz",
            "math_clz",
            "clz",
            "math_popcnt",
            "popcnt",
            "math_bitwise_not",
        ] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::Int], DataraType::Int, Vec::new()),
            );
        }
        function_signatures.insert(
            "read_line".to_string(),
            (vec![], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "read_int".to_string(),
            (vec![], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "read_float".to_string(),
            (vec![], DataraType::Float, Vec::new()),
        );
        for f in &["py_eval_batch", "datara_py_eval_batch"] {
            function_signatures.insert(
                f.to_string(),
                (vec![DataraType::String], DataraType::String, Vec::new()),
            );
        }
        function_signatures.insert(
            "socket_create".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "socket_bind".to_string(),
            (
                vec![DataraType::Int, DataraType::String, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_listen".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_accept".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "socket_connect".to_string(),
            (
                vec![DataraType::Int, DataraType::String, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_send".to_string(),
            (
                vec![DataraType::Int, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_recv".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_set_timeout".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::GenericInstance {
                    name: "Outcome".into(),
                    args: vec![DataraType::Unit],
                },
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_nonblocking".to_string(),
            (
                vec![DataraType::Int, DataraType::Bool],
                DataraType::GenericInstance {
                    name: "Outcome".into(),
                    args: vec![DataraType::Unit],
                },
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_recv_outcome".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::GenericInstance {
                    name: "Outcome".into(),
                    args: vec![DataraType::String],
                },
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "socket_close".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "sha256".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "base64_encode".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "base64_decode".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "str_len".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_str_len".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "byte_len".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_byte_len".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "str_chars".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_str_chars".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "char_len".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_char_len".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "str_byte_at".to_string(),
            (
                vec![DataraType::String, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "byte_at".to_string(),
            (
                vec![DataraType::String, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_str_byte_at".to_string(),
            (
                vec![DataraType::String, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "str_sanitize_utf8".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_str_sanitize_utf8".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "validate_utf8".to_string(),
            (vec![DataraType::String], DataraType::Bool, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_validate_utf8".to_string(),
            (vec![DataraType::String], DataraType::Bool, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_str_scalar_at".to_string(),
            (
                vec![DataraType::String, DataraType::Int],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_str_next_offset".to_string(),
            (
                vec![DataraType::String, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "int_to_str".to_string(),
            (vec![DataraType::Int], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_int_to_str".to_string(),
            (vec![DataraType::Int], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "float_to_str".to_string(),
            (vec![DataraType::Float], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_float_to_str".to_string(),
            (vec![DataraType::Float], DataraType::String, Vec::new()),
        );
        // AtomicInt (PLAN_V152 N1.2)
        function_signatures.insert(
            "atomic_new".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_atomic_new".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "atomic_load".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_atomic_load".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "atomic_store".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_atomic_store".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "atomic_fetch_add".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_atomic_fetch_add".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "atomic_cas".to_string(),
            (
                vec![DataraType::Int, DataraType::Int, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_atomic_cas".to_string(),
            (
                vec![DataraType::Int, DataraType::Int, DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        // fiber_net_recv -> Outcome<Str> (ptr carrier, Int-typed at the DMIR level)
        function_signatures.insert(
            "fiber_net_recv".to_string(),
            (
                vec![DataraType::Int, DataraType::Int, DataraType::Int],
                DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_fiber_net_recv".to_string(),
            (
                vec![DataraType::Int, DataraType::Int, DataraType::Int],
                DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        // v1.4.5 W4 fmt precision converters — the runtime + all three
        // backends have them; only the prelude signature was missing
        // (found by the N7 pipeline_demo flagship: float_to_str_prec(x, n)).
        function_signatures.insert(
            "float_to_str_prec".to_string(),
            (
                vec![DataraType::Float, DataraType::Int],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_float_to_str_prec".to_string(),
            (
                vec![DataraType::Float, DataraType::Int],
                DataraType::String,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "bool_to_str".to_string(),
            (vec![DataraType::Bool], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_bool_to_str".to_string(),
            (vec![DataraType::Bool], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "uuid_v4".to_string(),
            (Vec::new(), DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_uuid_v4".to_string(),
            (Vec::new(), DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "datara_rt_dialog_info".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_dialog_alert".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_dialog_confirm".to_string(),
            (
                vec![DataraType::String, DataraType::String],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "process_run".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "system".to_string(),
            (vec![DataraType::String], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "process_output".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        function_signatures.insert(
            "exec".to_string(),
            (vec![DataraType::String], DataraType::String, Vec::new()),
        );
        // v1.4.1: UTF-8 checked exec. Checker-level representation is
        // Result(T, Str), matching `?` propagation and the checked-I/O
        // builtins.
        function_signatures.insert(
            "exec_utf8".to_string(),
            (
                vec![DataraType::String],
                DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "datara_rt_exec_utf8".to_string(),
            (
                vec![DataraType::String],
                DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                Vec::new(),
            ),
        );
        // v1.5.0 (M1): process spawn / pipes / wait. spawn yields the PID
        // (Outcome<Int>), run_input feeds stdin and captures merged stdout
        // (Outcome<Str>), wait reaps a spawned PID (Outcome<Int>). All are
        // escalated ops gated by the manifest `exec` capability.
        for spawn_name in &["proc_spawn", "datara_rt_proc_spawn"] {
            function_signatures.insert(
                spawn_name.to_string(),
                (
                    vec![DataraType::String],
                    DataraType::Result(Box::new(DataraType::Int), Box::new(DataraType::String)),
                    Vec::new(),
                ),
            );
        }
        for run_name in &["proc_run_input", "datara_rt_proc_run_input"] {
            function_signatures.insert(
                run_name.to_string(),
                (
                    vec![DataraType::String, DataraType::String],
                    DataraType::Result(Box::new(DataraType::String), Box::new(DataraType::String)),
                    Vec::new(),
                ),
            );
        }
        for wait_name in &["proc_wait", "datara_rt_proc_wait"] {
            function_signatures.insert(
                wait_name.to_string(),
                (
                    vec![DataraType::Int],
                    DataraType::Result(Box::new(DataraType::Int), Box::new(DataraType::String)),
                    Vec::new(),
                ),
            );
        }
        crate::types::prelude_natvar::insert_fiber_signatures(function_signatures);
    }
}
