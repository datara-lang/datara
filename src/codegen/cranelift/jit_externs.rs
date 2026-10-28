//! JIT-side `extern "C"` declarations of the Datara C runtime.
//!
//! Split out of jit.rs to keep both files under the 60 KB monolith gate
//! (PLAN_SPEED_FIXES P7 / test_phase18 file-size limit).

use std::os::raw::c_char;

unsafe extern "C" {
    pub fn datara_rt_out_int(v: i64);
    pub fn datara_rt_out_bool(v: i64);
    pub fn datara_rt_bool_to_str(v: i64) -> *const c_char;
    pub fn datara_rt_out_float(v: f64);
    pub fn datara_rt_out_f32(v: f32);
    pub fn datara_rt_float_to_str(v: f64) -> *const c_char;
    pub fn datara_rt_out_str(s: *const c_char);
    pub fn datara_rt_out_dec64(v: i64);
    pub fn datara_rt_err(s: *const c_char);
    pub fn datara_rt_exit(code: i32);
    pub fn datara_rt_input(prompt: *const c_char) -> *const c_char;
    pub fn datara_rt_input_int(prompt: *const c_char) -> i64;
    pub fn datara_rt_input_float(prompt: *const c_char) -> f64;

    pub fn datara_rt_print_str(s: *const c_char);
    pub fn datara_rt_print_int(v: i64);
    pub fn datara_rt_print_float(v: f64);
    pub fn datara_rt_print_f32(v: f32);
    pub fn datara_rt_print_dec64(v: i64);
    pub fn datara_rt_print_bool(v: i64);
    pub fn datara_rt_print_space();
    pub fn datara_rt_print_newline();
    pub fn datara_rt_flush();
    pub fn datara_rt_print_list(list: *mut ());
    pub fn datara_rt_err_print_str(s: *const c_char);
    pub fn datara_rt_err_print_int(v: i64);
    pub fn datara_rt_err_print_dec64_str(v: i64);
    pub fn datara_rt_err_print_float(v: f64);
    pub fn datara_rt_err_print_bool(v: i64);
    pub fn datara_rt_err_print_newline();
    pub fn datara_rt_print(s: *const c_char);
    pub fn datara_rt_eprintln(s: *const c_char);
    pub fn datara_rt_panic(s: *const c_char);
    pub fn datara_rt_print_backtrace();
    pub fn datara_rt_assert(cond: i64, msg: *const c_char);
    pub fn datara_rt_int_fits(v: i64, to_ty: *const c_char) -> i64;
    pub fn datara_rt_cast_overflow(v: i64, to_ty: *const c_char);
    pub fn datara_rt_len(s: *const c_char) -> i64;
    pub fn datara_rt_pgo_hit_func(name: *const c_char);
    pub fn datara_rt_pgo_hit_branch(branch_id: *const c_char, taken: i64);
    pub fn datara_rt_pgo_hit_loop(loop_id: *const c_char, trip_count: i64);
    pub fn datara_rt_pgo_set_output_file(path: *const c_char);
    pub fn datara_rt_pgo_flush(path: *const c_char);
    pub fn datara_rt_pgo_reset();

    pub fn datara_rt_cap_set_mask(mask: u64);
    pub fn datara_rt_cap_get_mask() -> u64;
    pub fn datara_rt_cap_revoke(mask: u64);
    pub fn datara_rt_cap_grant(mask: u64);
    pub fn datara_rt_cap_require(required_bit: u64, op_name: *const c_char);

    pub fn datara_rt_chase_lev_create(capacity: i64) -> *mut ();
    pub fn datara_rt_chase_lev_destroy(q: *mut ());
    pub fn datara_rt_chase_lev_push(q: *mut (), task_id: i64);
    pub fn datara_rt_chase_lev_pop(q: *mut ()) -> i64;
    pub fn datara_rt_chase_lev_steal(q: *mut ()) -> i64;
    pub fn datara_rt_chase_lev_size(q: *mut ()) -> i64;

    pub fn datara_rt_fast_memcpy(dest: *mut (), src: *const (), n: usize) -> *mut ();
    pub fn datara_rt_fast_memset(dest: *mut (), c: i32, n: usize) -> *mut ();
    pub fn datara_rt_fast_strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> i32;
    pub fn datara_rt_fast_memcmp(s1: *const (), s2: *const (), n: usize) -> i32;

    pub fn datara_rt_pin_thread(core_id: i64) -> i32;
    pub fn datara_rt_get_current_core() -> i64;
    pub fn datara_rt_pin_worker_threads();

    pub fn datara_rt_set_capture(enable: i32);
    pub fn datara_rt_get_capture() -> *const c_char;
    pub fn datara_rt_get_err_capture() -> *const c_char;
    pub fn datara_rt_clear_capture();

    pub fn datara_rt_int_to_str(v: i64) -> *const c_char;
    pub fn datara_rt_dec_to_str(v: i64) -> *const c_char;
    // v1.4.5 W4 fmt specifiers: precision/radix converters.
    pub fn datara_rt_float_to_str_prec(v: f64, prec: i64) -> *const c_char;
    pub fn datara_rt_int_to_str_radix(v: i64, radix: i64, upper: i64) -> *const c_char;
    pub fn datara_rt_dec64_to_str_prec(v: i64, prec: i64) -> *const c_char;
    pub fn datara_rt_str_concat(a: *const c_char, b: *const c_char) -> *const c_char;
    pub fn datara_rt_str_concat_mut(a: *const c_char, b: *const c_char) -> *const c_char;
    pub fn datara_rt_str_concat_3(
        a: *const c_char,
        b: *const c_char,
        c: *const c_char,
    ) -> *const c_char;
    pub fn datara_rt_str_concat_4(
        a: *const c_char,
        b: *const c_char,
        c: *const c_char,
        d: *const c_char,
    ) -> *const c_char;
    pub fn datara_rt_str_concat_5(
        a: *const c_char,
        b: *const c_char,
        c: *const c_char,
        d: *const c_char,
        e: *const c_char,
    ) -> *const c_char;
    pub fn datara_rt_format_str_i64_str_i64(
        s1: *const c_char,
        n1: i64,
        s2: *const c_char,
        n2: i64,
    ) -> *const c_char;
    pub fn datara_rt_str_eq(a: *const c_char, b: *const c_char) -> i64;
    pub fn datara_rt_str_cmp(a: *const c_char, b: *const c_char) -> i64;
    pub fn datara_rt_env_set(key: *const c_char, val: *const c_char) -> i64;
    pub fn datara_rt_str_from_byte(b: i64) -> *const c_char;
    pub fn datara_rt_str_from_bytes(list: *mut i64) -> *const c_char;
    pub fn datara_rt_str_bytes(s: *const c_char) -> *mut i64;
    pub fn datara_rt_str_len(s: *const c_char) -> i64;
    pub fn datara_rt_byte_len(s: *const c_char) -> i64;
    pub fn datara_rt_str_chars(s: *const c_char) -> i64;
    pub fn datara_rt_char_len(s: *const c_char) -> i64;
    pub fn datara_rt_validate_utf8(s: *const c_char) -> i32;
    pub fn datara_rt_str_sanitize_utf8(s: *const c_char) -> *const c_char;
    pub fn datara_rt_str_next_scalar(s: *const c_char, inout_offset: *mut i64) -> *const c_char;
    pub fn datara_rt_str_scalar_at(s: *const c_char, offset: i64) -> *const c_char;
    pub fn datara_rt_str_scalar_at_cp(s: *const c_char, cp_index: i64) -> *const c_char;
    pub fn datara_rt_str_byte_at(s: *const c_char, idx: i64) -> i64;
    pub fn datara_rt_str_next_offset(s: *const c_char, current_offset: i64) -> i64;
    pub fn datara_rt_str_contains(s: *const c_char, sub: *const c_char) -> i64;
    pub fn datara_rt_str_starts_with(s: *const c_char, pre: *const c_char) -> i64;
    pub fn datara_rt_str_ends_with(s: *const c_char, suf: *const c_char) -> i64;
    pub fn datara_rt_str_index_of(s: *const c_char, sub: *const c_char) -> i64;
    pub fn datara_rt_str_trim(s: *const c_char) -> *const c_char;
    pub fn datara_rt_str_to_int(s: *const c_char) -> i64;
    pub fn datara_rt_str_to_float(s: *const c_char) -> f64;
    pub fn datara_rt_str_substring(s: *const c_char, start: i64, len: i64) -> *const c_char;
    pub fn datara_rt_str_char_at(s: *const c_char, idx: i64) -> *const c_char;
    pub fn datara_rt_str_repeat(s: *const c_char, count: i64) -> *const c_char;
    pub fn datara_rt_str_pad_left(
        s: *const c_char,
        total_len: i64,
        pad: *const c_char,
    ) -> *const c_char;
    pub fn datara_rt_str_pad_right(
        s: *const c_char,
        total_len: i64,
        pad: *const c_char,
    ) -> *const c_char;
    pub fn datara_rt_str_replace(
        s: *const c_char,
        target: *const c_char,
        replacement: *const c_char,
    ) -> *const c_char;
    pub fn datara_rt_str_to_upper(s: *const c_char) -> *const c_char;
    pub fn datara_rt_str_to_lower(s: *const c_char) -> *const c_char;
    pub fn datara_rt_str_split(s: *const c_char, delim: *const c_char) -> *mut i64;
    pub fn datara_rt_str_join(list: *mut i64, delim: *const c_char) -> *const c_char;
    pub fn datara_rt_format_percent(val: f64, decimals: i64) -> *const c_char;
    pub fn datara_rt_format_int_with_commas(n: i64) -> *const c_char;

    pub fn datara_js_eval(code: *const c_char) -> *const c_char;
    pub fn datara_js_eval_int(code: *const c_char) -> i64;
    pub fn datara_js_eval_float(code: *const c_char) -> f64;
    pub fn datara_js_require(module_name: *const c_char) -> i64;
    pub fn datara_js_call(fn_name: *const c_char, args_json: *const c_char) -> *const c_char;
    pub fn datara_js_call_0(fn_name: *const c_char) -> *const c_char;
    pub fn datara_js_call_1(fn_name: *const c_char, arg0: *const c_char) -> *const c_char;
    pub fn datara_js_call_2(
        fn_name: *const c_char,
        arg0: *const c_char,
        arg1: *const c_char,
    ) -> *const c_char;
    pub fn datara_js_set_global(name: *const c_char, json_val: *const c_char) -> i64;
    pub fn datara_js_get_global(name: *const c_char) -> *const c_char;

    pub fn datara_py_eval(code: *const c_char) -> *const c_char;
    pub fn datara_py_eval_safe(code: *const c_char) -> *const c_char;
    pub fn datara_py_eval_int(code: *const c_char) -> i64;
    pub fn datara_py_eval_float(code: *const c_char) -> f64;
    pub fn datara_py_exec(code: *const c_char) -> i32;
    pub fn datara_py_import(module_name: *const c_char) -> u32;
    pub fn datara_py_call(fn_name: *const c_char, args_json: *const c_char) -> *const c_char;
    pub fn datara_py_call_1_str(fn_name: *const c_char, arg0: *const c_char) -> *const c_char;
    pub fn datara_py_call_1_float(fn_name: *const c_char, arg0: f64) -> f64;
    pub fn datara_py_call_2_float(fn_name: *const c_char, a: f64, b: f64) -> f64;
    pub fn datara_py_call_1_int(fn_name: *const c_char, a: i64) -> i64;
    pub fn datara_py_call_2_int(fn_name: *const c_char, a: i64, b: i64) -> i64;
    pub fn datara_py_call_list_f64(fn_name: *const c_char, in_list: *mut i64) -> *mut i64;
    pub fn datara_py_eval_batch(json_exprs: *const c_char) -> *const c_char;
    pub fn datara_py_last_error() -> *const c_char;
    pub fn datara_py_clear_error();
    pub fn datara_py_is_available() -> i32;
    pub fn datara_py_export_list_f64(var_name: *const c_char, list: *mut i64) -> i32;
    pub fn datara_py_assert_same_ptr(var_name: *const c_char, list: *mut i64) -> i32;

    pub fn datara_zig_eval_int(code: *const c_char) -> i64;
    pub fn datara_zig_call(symbol: *const c_char, arg: i64) -> i64;
    pub fn datara_csharp_invoke_i64(
        lib_name: *const c_char,
        method_name: *const c_char,
        arg: i64,
    ) -> i64;
    pub fn datara_csharp_invoke_f64(
        lib_name: *const c_char,
        method_name: *const c_char,
        arg: f64,
    ) -> f64;
    pub fn datara_lua_eval_int(code: *const c_char) -> i64;
    pub fn datara_lua_eval_float(code: *const c_char) -> f64;
    pub fn datara_lua_exec(code: *const c_char) -> i64;
    pub fn datara_polyglot_parallel_exec(engine_type: *const c_char, code: *const c_char) -> i64;
    pub fn datara_rt_arena_used() -> i64;
    pub fn datara_rt_arena_clear();
    pub fn datara_rt_mem_alloc(bytes: i64) -> *mut u8;
    pub fn datara_rt_mem_free(ptr: *mut u8);
    pub fn datara_rt_mem_copy(dst: *mut u8, src: *const u8, bytes: i64);
    pub fn datara_rt_ptr_read_i64(ptr: *const u8, offset: i64) -> i64;
    pub fn datara_rt_ptr_write_i64(ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_rt_ptr_read_f64(ptr: *const u8, offset: i64) -> f64;
    pub fn datara_rt_ptr_write_f64(ptr: *mut u8, offset: i64, val: f64);
    pub fn datara_rt_ptr_read_u8(ptr: *const u8, offset: i64) -> i64;
    pub fn datara_rt_ptr_write_u8(ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_rt_cpu_fence();
    pub fn datara_rt_cpu_prefetch(ptr: *const u8);

    pub fn datara_rt_file_read(path: *const c_char) -> *const c_char;
    pub fn datara_rt_file_read_bytes(path: *const c_char) -> *mut i64;
    pub fn datara_rt_file_write_bytes(path: *const c_char, bytes: *const i64) -> i64;
    pub fn datara_rt_file_write(path: *const c_char, content: *const c_char) -> i64;
    pub fn datara_rt_file_append(path: *const c_char, content: *const c_char) -> i64;
    pub fn datara_rt_file_exists(path: *const c_char) -> i64;
    pub fn datara_rt_sys_caps_create() -> *mut i64;
    pub fn datara_rt_file_read_checked(path: *const c_char) -> *mut i64;
    pub fn datara_rt_str_to_int_checked(s: *const c_char) -> *mut i64;
    pub fn datara_rt_str_cp_to_byte(s: *const c_char, cp: i64) -> i64;
    pub fn datara_rt_str_byte_to_cp(s: *const c_char, byte: i64) -> i64;
    pub fn datara_rt_str_substring_cp(s: *const c_char, start: i64, len: i64) -> *const c_char;
    pub fn datara_rt_str_to_float_checked(s: *const c_char) -> *mut i64;
    pub fn datara_rt_env_get_checked(name: *const c_char) -> *mut i64;
    pub fn datara_rt_dir_list(path: *const c_char) -> *mut i64;
    pub fn datara_rt_path_exists(path: *const c_char) -> i64;

    pub fn datara_rt_sleep(ms: i64);
    pub fn datara_rt_now_ms() -> i64;
    pub fn datara_rt_now_unix_ms() -> i64;
    pub fn datara_rt_now_precise_ms() -> i64;
    pub fn datara_rt_now_ns() -> i64;
    pub fn now_ms() -> i64;
    pub fn datara_rt_env_get(key: *const c_char) -> *const c_char;
    pub fn datara_rt_path_join(a: *const c_char, b: *const c_char) -> *const c_char;
    pub fn datara_rt_set_args(argc: i32, argv: *const *const c_char);
    pub fn datara_rt_args_count() -> i64;
    pub fn datara_rt_args_get(index: i64) -> *const c_char;

    pub fn datara_rt_list_create(cap: i64) -> *mut i64;
    pub fn datara_rt_list_create_capacity(cap: i64) -> *mut i64;
    pub fn datara_rt_list_create_repeat(len: i64, val: i64) -> *mut i64;
    pub fn datara_rt_list_append(list: *mut i64, val: i64) -> *mut i64;
    pub fn datara_rt_list_len(list: *mut i64) -> i64;
    pub fn datara_rt_list_get(list: *mut i64, idx: i64) -> i64;
    pub fn datara_rt_list_set(list: *mut i64, idx: i64, val: i64) -> i64;
    // v1.4.1: checked pop (Outcome<T>) and the full List<T> protocol.
    pub fn datara_rt_list_pop_outcome(list: *mut i64) -> *mut ();
    // v1.4.5 W1: checked arithmetic returning Outcome<IntN> objects.
    pub fn datara_rt_checked_add_outcome(bits: i64, a: i64, b: i64) -> *mut ();
    pub fn datara_rt_checked_sub_outcome(bits: i64, a: i64, b: i64) -> *mut ();
    pub fn datara_rt_checked_mul_outcome(bits: i64, a: i64, b: i64) -> *mut ();
    pub fn datara_rt_list_first(list: *mut i64) -> *mut ();
    pub fn datara_rt_list_last(list: *mut i64) -> *mut ();
    pub fn datara_rt_list_sort(list: *mut i64, mode: i64, elem_kind: i64) -> i64;
    pub fn datara_rt_list_remove_at(list: *mut i64, idx: i64) -> i64;
    pub fn datara_rt_list_remove_value(list: *mut i64, val: i64, elem_kind: i64) -> i64;
    pub fn datara_rt_list_insert_at(list: *mut i64, idx: i64, val: i64) -> i64;
    pub fn datara_rt_list_contains(list: *mut i64, val: i64, elem_kind: i64) -> i64;
    pub fn datara_rt_list_index_of(list: *mut i64, val: i64, elem_kind: i64) -> i64;
    pub fn datara_rt_list_reverse(list: *mut i64) -> i64;
    pub fn datara_rt_list_clear(list: *mut i64) -> i64;
    pub fn datara_rt_list_is_empty(list: *mut i64) -> i64;
    pub fn datara_rt_list_slice(list: *mut i64, start: i64, end: i64) -> *mut i64;
    pub fn datara_rt_exec_utf8(cmd: *const c_char) -> *mut ();
    // v1.5.0 (M1): process spawn / pipes / wait.
    pub fn datara_rt_proc_spawn(cmd: *const c_char) -> *mut ();
    pub fn datara_rt_proc_run_input(cmd: *const c_char, input: *const c_char) -> *mut ();
    pub fn datara_rt_proc_wait(pid: i64) -> *mut ();
    pub fn datara_rt_range_str(start: i64, end: i64) -> *const c_char;
    pub fn datara_rt_map_create() -> *mut ();
    pub fn datara_rt_map_create_2(k1: i64, v1: i64, k2: i64, v2: i64) -> *mut ();
    pub fn datara_rt_map_create_1(k1: i64, v1: i64) -> *mut ();
    pub fn datara_rt_map_create_3(k1: i64, v1: i64, k2: i64, v2: i64, k3: i64, v3: i64) -> *mut ();
    pub fn datara_rt_map_create_4(
        k1: i64,
        v1: i64,
        k2: i64,
        v2: i64,
        k3: i64,
        v3: i64,
        k4: i64,
        v4: i64,
    ) -> *mut ();
    pub fn datara_rt_map_create_5(
        k1: i64,
        v1: i64,
        k2: i64,
        v2: i64,
        k3: i64,
        v3: i64,
        k4: i64,
        v4: i64,
        k5: i64,
        v5: i64,
    ) -> *mut ();
    pub fn datara_rt_map_insert(map: *mut i64, key: *const c_char, val: i64) -> *mut i64;
    pub fn datara_rt_map_insert_i64(map: *mut i64, key: i64, val: i64) -> *mut i64;
    pub fn datara_rt_map_get(map: *mut i64, key: *const c_char) -> i64;
    pub fn datara_rt_map_get_i64(map: *mut i64, key: i64) -> i64;
    pub fn datara_rt_map_get_outcome(map: *mut i64, key: *const c_char) -> *mut ();
    pub fn datara_rt_map_get_outcome_i64(map: *mut i64, key: i64) -> *mut ();
    pub fn datara_rt_map_contains(map: *mut i64, key: *const c_char) -> i64;
    pub fn datara_rt_map_contains_i64(map: *mut i64, key: i64) -> i64;
    pub fn datara_rt_map_create_i64_1(k0: i64, v0: i64) -> *mut i64;
    pub fn datara_rt_map_create_i64_2(k0: i64, v0: i64, k1: i64, v1: i64) -> *mut i64;
    pub fn datara_rt_map_create_i64_3(
        k0: i64,
        v0: i64,
        k1: i64,
        v1: i64,
        k2: i64,
        v2: i64,
    ) -> *mut i64;
    pub fn datara_rt_map_create_i64_4(
        k0: i64,
        v0: i64,
        k1: i64,
        v1: i64,
        k2: i64,
        v2: i64,
        k3: i64,
        v3: i64,
    ) -> *mut i64;
    pub fn datara_rt_map_create_i64_5(
        k0: i64,
        v0: i64,
        k1: i64,
        v1: i64,
        k2: i64,
        v2: i64,
        k3: i64,
        v3: i64,
        k4: i64,
        v4: i64,
    ) -> *mut i64;
    pub fn datara_rt_map_len(map: *mut i64) -> i64;
    pub fn datara_rt_map_free(map: *mut ());

    pub fn datara_rt_socket_create(is_tcp: i64) -> i64;
    pub fn datara_rt_socket_bind(sock: i64, host: *const c_char, port: i64) -> i64;
    pub fn datara_rt_socket_listen(sock: i64, backlog: i64) -> i64;
    pub fn datara_rt_socket_accept(sock: i64) -> i64;
    pub fn datara_rt_socket_connect(sock: i64, host: *const c_char, port: i64) -> i64;
    pub fn datara_rt_socket_send(sock: i64, data: *const c_char) -> i64;
    pub fn datara_rt_socket_recv(sock: i64, max_bytes: i64) -> *const c_char;
    pub fn datara_rt_socket_recv_outcome(sock: i64, max_bytes: i64) -> *mut ();
    pub fn datara_rt_socket_set_timeout(sock: i64, ms: i64) -> *mut ();
    pub fn datara_rt_socket_nonblocking(sock: i64, on: i64) -> *mut ();
    pub fn datara_rt_socket_close(sock: i64);
    pub fn datara_rt_http_get(url: *const c_char) -> *const c_char;
    pub fn datara_rt_fast_read_int() -> i64;
    pub fn datara_rt_fast_read_float() -> f64;

    pub fn datara_rt_sha256(input: *const c_char) -> *const c_char;
    pub fn datara_rt_base64_encode(input: *const c_char) -> *const c_char;
    pub fn datara_rt_base64_decode(input: *const c_char) -> *const c_char;
    pub fn datara_rt_random_bytes(buf: *mut u8, len: i64) -> i64;
    pub fn datara_rt_uuid_v4() -> *const c_char;

    pub fn datara_rt_dialog_info(title: *const c_char, msg: *const c_char) -> i64;
    pub fn datara_rt_dialog_alert(title: *const c_char, msg: *const c_char) -> i64;
    pub fn datara_rt_dialog_confirm(title: *const c_char, msg: *const c_char) -> i64;

    pub fn datara_rt_system(cmd: *const c_char) -> i64;
    pub fn datara_rt_exec(cmd: *const c_char) -> *const c_char;

    pub fn datara_rt_math_sqrt(x: f64) -> f64;
    pub fn datara_rt_math_pow(base: f64, exp: f64) -> f64;
    pub fn datara_rt_math_abs(x: f64) -> f64;
    pub fn datara_rt_math_sin(x: f64) -> f64;
    pub fn datara_rt_math_cos(x: f64) -> f64;
    pub fn datara_rt_math_tan(x: f64) -> f64;
    pub fn datara_rt_math_floor(x: f64) -> f64;
    pub fn datara_rt_math_ceil(x: f64) -> f64;
    pub fn datara_rt_math_round(x: f64) -> f64;
    pub fn datara_rt_math_min(a: f64, b: f64) -> f64;
    pub fn datara_rt_math_max(a: f64, b: f64) -> f64;
    pub fn datara_rt_math_clamp(val: f64, min_val: f64, max_val: f64) -> f64;
    pub fn datara_rt_math_hypot(a: f64, b: f64) -> f64;
    pub fn datara_rt_math_log(x: f64) -> f64;
    pub fn datara_rt_math_exp(x: f64) -> f64;
    pub fn datara_rt_math_min_int(a: i64, b: i64) -> i64;
    pub fn datara_rt_math_max_int(a: i64, b: i64) -> i64;
    pub fn datara_rt_math_clamp_int(val: i64, min_val: i64, max_val: i64) -> i64;
    pub fn datara_rt_math_abs_int(x: i64) -> i64;
    pub fn datara_rt_math_ctz(x: i64) -> i64;
    pub fn datara_rt_math_shr(v: i64, s: i64) -> i64;
    pub fn datara_rt_math_shl(v: i64, s: i64) -> i64;
    pub fn datara_rt_math_xor(a: i64, b: i64) -> i64;
    pub fn datara_rt_math_and(a: i64, b: i64) -> i64;
    pub fn datara_rt_math_or(a: i64, b: i64) -> i64;

    pub fn datara_rt_slice(list: *mut i64, start: i64, end: i64) -> *mut i64;
    pub fn datara_rt_out_val(v: i64);
    pub fn datara_val_from_int(v: i64) -> u64;
    pub fn datara_rt_val_from_f64_bits(v: i64) -> u64;
    pub fn datara_val_from_bool(v: i32) -> u64;
    pub fn datara_val_from_str(v: *const u8) -> u64;
    pub fn datara_val_is_int(v: u64) -> i32;
    pub fn datara_val_is_float(v: u64) -> i32;
    pub fn datara_val_is_bool(v: u64) -> i32;
    pub fn datara_val_is_str(v: u64) -> i32;
    pub fn datara_val_to_int(v: u64) -> i64;
    pub fn datara_val_to_float(v: u64) -> f64;
    pub fn datara_val_to_bool(v: u64) -> i32;
    pub fn datara_val_to_str(v: u64) -> *const u8;
    pub fn datara_rt_val_as_int_outcome(v: u64) -> *const u8;
    pub fn datara_rt_val_as_float_outcome(v: u64) -> *const u8;
    pub fn datara_rt_val_to_int_checked(v: u64) -> i64;
    pub fn datara_rt_val_to_f64_bits_checked(v: u64) -> i64;
    pub fn datara_rt_val_to_str_tagged(v: u64) -> *const u8;
    pub fn datara_rt_parallel_for(start: i64, end: i64, worker_fn: usize, ctx: usize);
    pub fn datara_rt_parallel_invoke(fn1: usize, ctx1: usize, fn2: usize, ctx2: usize);
    pub fn datara_rt_fiber_spawn(fn_addr: usize, arg: i64, stack_kb: i64) -> i64;
    pub fn datara_rt_fiber_yield();
    pub fn datara_rt_fiber_sleep_ms(ms: i64);
    pub fn datara_rt_fiber_join(handle: i64) -> i64;
    pub fn datara_rt_fiber_run_until_done() -> i64;
    pub fn datara_rt_fiber_count() -> i64;
    pub fn datara_rt_fiber_done_count() -> i64;
    pub fn datara_rt_fiber_reset();
    pub fn datara_rt_fiber_chan_create() -> i64;
    pub fn datara_rt_fiber_chan_send(chan_id: i64, value: i64);
    pub fn datara_rt_fiber_chan_recv(chan_id: i64) -> i64;
    pub fn datara_rt_fiber_now_ms() -> i64;
    pub fn datara_rt_fiber_mt_start(n: i64) -> i64;
    pub fn datara_rt_fiber_mt_spawn(fn_addr: usize, arg: i64) -> i64;
    pub fn datara_rt_fiber_mt_join(handle: i64) -> i64;
    pub fn datara_rt_fiber_mt_done_count() -> i64;
    pub fn datara_rt_fiber_mt_spawned() -> i64;
    pub fn datara_rt_fiber_mt_wait_idle();
    pub fn datara_rt_fiber_mt_shutdown();
    pub fn datara_rt_num_workers() -> i64;

    pub fn datara_rt_arena_alloc(bytes: i64) -> *mut ();
    pub fn datara_rt_arena_checkpoint() -> i64;
    pub fn datara_rt_arena_reset(saved_top: i64);
    pub fn datara_rt_global_set(name: *const c_char, val: i64);
    pub fn datara_rt_global_get(name: *const c_char) -> i64;
    pub fn datara_rt_strbuf_new() -> *mut ();
    pub fn datara_rt_strbuf_push(sb: *mut (), s: *const c_char) -> *mut ();
    pub fn datara_rt_strbuf_push_int(sb: *mut (), v: i64) -> *mut ();
    pub fn datara_rt_strbuf_join(sb: *mut ()) -> *const c_char;
    pub fn datara_rt_strbuf_len(sb: *mut ()) -> i64;
    pub fn datara_rt_heap_live() -> i64;
    pub fn datara_rt_free(ptr: *mut ());
    pub fn datara_rt_str_free(s: *const c_char);
    pub fn datara_rt_list_free(list: *mut ());
    pub fn datara_rt_own_acquire(val: i64) -> i64;
    pub fn datara_rt_own_release(val: i64);
    pub fn datara_rt_arc_init(obj: *mut ()) -> i64;
    pub fn datara_rt_arc_acquire(obj: *mut ()) -> i64;
    pub fn datara_rt_arc_release(obj: *mut ());
    pub fn datara_rt_arc_count(obj: *mut ()) -> i64;
    pub fn datara_rt_weak_create(obj: *mut ()) -> *mut ();
    pub fn datara_rt_weak_upgrade(cell: *mut ()) -> *mut ();
    pub fn datara_rt_weak_drop(cell: *mut ());

    pub fn datara_rt_pool_alloc(sz: usize) -> *mut ();
    pub fn datara_rt_pool_free(ptr: *mut (), sz: usize);
    pub fn datara_rt_box_alloc(val: i64) -> *mut i64;
    pub fn datara_rt_box_get(b: *mut i64) -> i64;
    pub fn datara_rt_box_free(b: *mut i64);
    pub fn datara_rt_str_sso(s: *const c_char) -> *const c_char;
    pub fn datara_rt_str_is_sso(s: *const c_char) -> i64;
    pub fn datara_rt_heap_alloc_count() -> i64;
    pub fn datara_rt_reset_heap_alloc_count();
    pub fn datara_rt_list_init_stack(stack_buf: *mut (), cap: i64) -> *mut i64;
    pub fn datara_rt_list_is_small_vec(list: *mut i64) -> i64;

    pub fn datara_rt_overflow_panic();
    pub fn datara_rt_div_zero_panic();
    pub fn datara_rt_checked_add(a: i64, b: i64) -> i64;
    pub fn datara_rt_checked_sub(a: i64, b: i64) -> i64;
    pub fn datara_rt_checked_mul(a: i64, b: i64) -> i64;
    pub fn datara_rt_checked_div(a: i64, b: i64) -> i64;
    pub fn datara_rt_checked_rem(a: i64, b: i64) -> i64;
    pub fn datara_rt_saturating_add(a: i64, b: i64) -> i64;
    pub fn datara_rt_saturating_sub(a: i64, b: i64) -> i64;
    pub fn datara_rt_saturating_mul(a: i64, b: i64) -> i64;
    pub fn datara_rt_wrapping_add(a: i64, b: i64) -> i64;
    pub fn datara_rt_wrapping_sub(a: i64, b: i64) -> i64;
    pub fn datara_rt_wrapping_mul(a: i64, b: i64) -> i64;

    pub fn malloc(size: usize) -> *mut u8;
    pub fn free(ptr: *mut u8);
}
