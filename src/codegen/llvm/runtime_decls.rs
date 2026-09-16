//! LLVM IR Standard Runtime Declarations & Inlined Arithmetic Builtins.

pub fn emit_runtime_declarations(ir: &mut String) {
    ir.push_str(RUNTIME_DECLARATIONS);
}

const RUNTIME_DECLARATIONS: &str = r#"; --- Datara Standard Runtime Declarations ---
declare void @llvm.assume(i1)
declare void @datara_rt_overflow_panic()
declare void @datara_rt_div_zero_panic()
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare i64 @llvm.sadd.sat.i64(i64, i64)
declare i64 @llvm.ssub.sat.i64(i64, i64)

; --- Inlined Checked & Fast Arithmetic Builtins ---
define internal i64 @datara_rt_checked_add(i64 %a, i64 %b) alwaysinline {
entry:
  %res = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue { i64, i1 } %res, 0
  %ovf = extractvalue { i64, i1 } %res, 1
  br i1 %ovf, label %trap, label %ok, !prof !9
trap:
  call void @datara_rt_overflow_panic()
  unreachable
ok:
  ret i64 %val
}

define internal i64 @datara_rt_checked_sub(i64 %a, i64 %b) alwaysinline {
entry:
  %res = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue { i64, i1 } %res, 0
  %ovf = extractvalue { i64, i1 } %res, 1
  br i1 %ovf, label %trap, label %ok, !prof !9
trap:
  call void @datara_rt_overflow_panic()
  unreachable
ok:
  ret i64 %val
}

define internal i64 @datara_rt_checked_mul(i64 %a, i64 %b) alwaysinline {
entry:
  %res = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue { i64, i1 } %res, 0
  %ovf = extractvalue { i64, i1 } %res, 1
  br i1 %ovf, label %trap, label %ok, !prof !9
trap:
  call void @datara_rt_overflow_panic()
  unreachable
ok:
  ret i64 %val
}

define internal i64 @datara_rt_checked_div(i64 %a, i64 %b) alwaysinline {
entry:
  %zero = icmp eq i64 %b, 0
  br i1 %zero, label %trap_zero, label %chk_min, !prof !9
trap_zero:
  call void @datara_rt_div_zero_panic()
  unreachable
chk_min:
  %is_min = icmp eq i64 %a, -9223372036854775808
  %is_neg1 = icmp eq i64 %b, -1
  %ovf = and i1 %is_min, %is_neg1
  br i1 %ovf, label %trap_ovf, label %ok, !prof !9
trap_ovf:
  call void @datara_rt_overflow_panic()
  unreachable
ok:
  %val = sdiv i64 %a, %b
  ret i64 %val
}

define internal i64 @datara_rt_checked_rem(i64 %a, i64 %b) alwaysinline {
entry:
  %zero = icmp eq i64 %b, 0
  br i1 %zero, label %trap_zero, label %chk_min, !prof !9
trap_zero:
  call void @datara_rt_div_zero_panic()
  unreachable
chk_min:
  %is_min = icmp eq i64 %a, -9223372036854775808
  %is_neg1 = icmp eq i64 %b, -1
  %ovf = and i1 %is_min, %is_neg1
  br i1 %ovf, label %ret_zero, label %ok, !prof !9
ret_zero:
  ret i64 0
ok:
  %val = srem i64 %a, %b
  ret i64 %val
}

define internal i64 @datara_rt_saturating_add(i64 %a, i64 %b) alwaysinline {
entry:
  %res = call i64 @llvm.sadd.sat.i64(i64 %a, i64 %b)
  ret i64 %res
}

define internal i64 @datara_rt_saturating_sub(i64 %a, i64 %b) alwaysinline {
entry:
  %res = call i64 @llvm.ssub.sat.i64(i64 %a, i64 %b)
  ret i64 %res
}

declare i64 @datara_rt_saturating_mul(i64, i64)
define internal i64 @datara_rt_wrapping_add(i64 %a, i64 %b) alwaysinline {
entry:
  %res = add i64 %a, %b
  ret i64 %res
}

define internal i64 @datara_rt_wrapping_sub(i64 %a, i64 %b) alwaysinline {
entry:
  %res = sub i64 %a, %b
  ret i64 %res
}

define internal i64 @datara_rt_wrapping_mul(i64 %a, i64 %b) alwaysinline {
entry:
  %res = mul i64 %a, %b
  ret i64 %res
}

define internal i64 @datara_rt_list_get_unchecked(ptr %list, i64 %idx) alwaysinline {
entry:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds i64, ptr %list, i64 %off
  %val = load i64, ptr %ptr, align 8
  ret i64 %val
}

define internal ptr @datara_rt_list_set_unchecked(ptr %list, i64 %idx, i64 %val) alwaysinline {
entry:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds i64, ptr %list, i64 %off
  store i64 %val, ptr %ptr, align 8
  ret ptr %list
}

define internal double @datara_rt_list_get_f64_unchecked(ptr %list, i64 %idx) alwaysinline {
entry:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds double, ptr %list, i64 %off
  %val = load double, ptr %ptr, align 8
  ret double %val
}

define internal ptr @datara_rt_list_set_f64_unchecked(ptr %list, i64 %idx, double %val) alwaysinline {
entry:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds double, ptr %list, i64 %off
  store double %val, ptr %ptr, align 8
  ret ptr %list
}

declare i64 @datara_rt_own_acquire(i64)
declare void @datara_rt_own_release(i64)
declare void @datara_rt_out_int(i64)
declare void @datara_rt_out_float(double)
declare void @datara_rt_out_bool(i64)
declare void @datara_rt_out_str(ptr)
declare void @datara_rt_err(ptr)
declare void @datara_rt_print_str(ptr)
declare void @datara_rt_print_int(i64)
declare void @datara_rt_print_float(double)
declare void @datara_rt_print_bool(i64)
declare void @datara_rt_print_space()
declare void @datara_rt_print_newline()
declare void @datara_rt_flush()
declare void @datara_rt_print_list(ptr)
declare ptr @datara_rt_str_concat(ptr, ptr)
declare ptr @datara_rt_str_concat_3(ptr, ptr, ptr)
declare ptr @datara_rt_str_concat_4(ptr, ptr, ptr, ptr)
declare ptr @datara_rt_str_concat_5(ptr, ptr, ptr, ptr, ptr)
declare ptr @datara_rt_format_str_i64_str_i64(ptr, i64, ptr, i64)
declare ptr @datara_rt_int_to_str(i64)
declare ptr @datara_rt_bool_to_str(i64)
declare ptr @datara_rt_float_to_str(double)
declare i64 @datara_rt_str_len(ptr)
declare i64 @datara_rt_byte_len(ptr)
declare i64 @datara_rt_str_chars(ptr)
declare i64 @datara_rt_char_len(ptr)
declare i32 @datara_rt_validate_utf8(ptr)
declare ptr @datara_rt_str_sanitize_utf8(ptr)
declare ptr @datara_rt_str_scalar_at(ptr, i64)
declare i64 @datara_rt_str_next_offset(ptr, i64)
declare ptr @datara_rt_str_char_at(ptr, i64)
declare i64 @datara_rt_str_byte_at(ptr, i64)
declare i64 @datara_rt_str_eq(ptr, ptr)
declare i64 @datara_rt_str_cmp(ptr, ptr)
declare i64 @datara_rt_env_set(ptr, ptr)
declare ptr @datara_rt_str_from_byte(i64)
declare ptr @datara_rt_str_from_bytes(ptr)
declare ptr @datara_rt_str_bytes(ptr)
declare ptr @datara_rt_str_trim(ptr)
declare i64 @datara_rt_str_to_int(ptr)
declare double @datara_rt_str_to_float(ptr)
declare i64 @datara_rt_str_contains(ptr, ptr)
declare i64 @datara_rt_str_starts_with(ptr, ptr)
declare i64 @datara_rt_str_ends_with(ptr, ptr)
declare i64 @datara_rt_str_index_of(ptr, ptr)
declare ptr @datara_rt_str_substring(ptr, i64, i64)
declare ptr @datara_rt_str_repeat(ptr, i64)
declare ptr @datara_rt_str_pad_left(ptr, i64, ptr)
declare ptr @datara_rt_str_pad_right(ptr, i64, ptr)
declare ptr @datara_rt_str_replace(ptr, ptr, ptr)
declare ptr @datara_rt_str_to_upper(ptr)
declare ptr @datara_rt_str_to_lower(ptr)
declare ptr @datara_rt_format_percent(double, i64)
declare ptr @datara_rt_format_int_with_commas(i64)
declare ptr @datara_rt_range_str(i64, i64)
declare ptr @datara_rt_str_split(ptr, ptr)
declare ptr @datara_rt_str_join(ptr, ptr)
declare void @datara_rt_assert(i64, ptr)
declare i64 @destroy(i64)
declare i64 @datara_rt_file_write(ptr, ptr)
declare ptr @datara_rt_file_read(ptr)
declare ptr @datara_rt_file_read_bytes(ptr)
declare i64 @datara_rt_file_write_bytes(ptr, ptr)
declare i64 @datara_rt_file_append(ptr, ptr)
declare i64 @datara_rt_file_exists(ptr)
declare ptr @datara_rt_file_read_checked(ptr)
declare ptr @datara_rt_env_get(ptr)
declare ptr @datara_rt_env_get_checked(ptr)
declare ptr @datara_rt_exec(ptr)
declare i64 @datara_rt_args_count()
declare ptr @datara_rt_args_get(i64)
declare ptr @datara_rt_dir_list(ptr)
declare i64 @datara_rt_path_exists(ptr)
declare void @datara_rt_exit(i32)
declare ptr @datara_rt_list_create(i64)
declare ptr @datara_rt_list_create_1(i64)
declare ptr @datara_rt_list_create_2(i64, i64)
declare ptr @datara_rt_list_create_3(i64, i64, i64)
declare ptr @datara_rt_list_create_4(i64, i64, i64, i64)
declare ptr @datara_rt_list_create_5(i64, i64, i64, i64, i64)
declare ptr @datara_rt_list_append(ptr, i64)
declare i64 @datara_rt_list_get(ptr, i64)
declare ptr @datara_rt_list_set(ptr, i64, i64)
declare i64 @datara_rt_list_len(ptr)
declare i64 @datara_rt_list_sort(ptr, i64, i64)
declare i64 @datara_rt_list_remove_at(ptr, i64)
declare i64 @datara_rt_list_remove_value(ptr, i64, i64)
declare ptr @datara_rt_list_insert_at(ptr, i64, i64)
declare i64 @datara_rt_list_contains(ptr, i64, i64)
declare i64 @datara_rt_list_index_of(ptr, i64, i64)
declare ptr @datara_rt_list_reverse(ptr)
declare ptr @datara_rt_list_clear(ptr)
declare i64 @datara_rt_list_is_empty(ptr)
declare ptr @datara_rt_list_slice(ptr, i64, i64)
declare ptr @datara_rt_list_first(ptr)
declare ptr @datara_rt_list_last(ptr)
declare ptr @datara_rt_list_pop_outcome(ptr)
declare ptr @datara_rt_exec_utf8(ptr)
declare ptr @datara_rt_map_create()
declare ptr @datara_rt_map_create_1(ptr, i64)
declare ptr @datara_rt_map_create_2(ptr, i64, ptr, i64)
declare ptr @datara_rt_map_create_3(ptr, i64, ptr, i64, ptr, i64)
declare ptr @datara_rt_map_create_4(ptr, i64, ptr, i64, ptr, i64, ptr, i64)
declare ptr @datara_rt_map_create_5(ptr, i64, ptr, i64, ptr, i64, ptr, i64, ptr, i64)
declare ptr @datara_rt_map_insert(ptr, ptr, i64)
declare i64 @datara_rt_map_get(ptr, ptr)
declare i64 @datara_rt_map_contains(ptr, ptr)
declare i64 @datara_rt_map_len(ptr)
declare void @datara_rt_map_free(ptr)
declare i64 @datara_rt_now_ms()
declare i64 @datara_rt_now_ns()
declare i64 @datara_rt_now_precise_ms()
declare void @datara_rt_sleep(i64)
declare ptr @datara_rt_path_join(ptr, ptr)
declare ptr @datara_rt_http_get(ptr)
declare i64 @datara_rt_socket_create(i64)
declare i64 @datara_rt_socket_bind(i64, ptr, i64)
declare i64 @datara_rt_socket_listen(i64, i64)
declare i64 @datara_rt_socket_accept(i64)
declare i64 @datara_rt_socket_connect(i64, ptr, i64)
declare i64 @datara_rt_socket_send(i64, ptr)
declare ptr @datara_rt_socket_recv(i64, i64)
declare void @datara_rt_socket_close(i64)
declare double @datara_rt_math_sqrt(double)
declare double @datara_rt_math_log(double)
declare double @datara_rt_math_exp(double)
declare double @datara_rt_math_clamp(double, double, double)
declare double @datara_rt_math_pow(double, double)
declare double @datara_rt_math_abs(double)
declare double @datara_rt_math_sin(double)
declare double @datara_rt_math_cos(double)
declare double @datara_rt_math_tan(double)
declare double @datara_rt_math_floor(double)
declare double @datara_rt_math_ceil(double)
declare double @datara_rt_math_round(double)
declare double @datara_rt_math_min(double, double)
declare double @datara_rt_math_max(double, double)
declare double @datara_rt_math_hypot(double, double)
declare i64 @datara_rt_math_min_int(i64, i64)
declare i64 @datara_rt_math_max_int(i64, i64)
declare i64 @datara_rt_math_clamp_int(i64, i64, i64)
declare i64 @datara_rt_math_abs_int(i64)
declare i64 @datara_rt_math_ctz(i64)
declare i64 @datara_rt_math_shr(i64, i64)
declare i64 @datara_rt_math_shl(i64, i64)
declare i64 @llvm.cttz.i64(i64, i1)
declare ptr @malloc(i64)
declare void @free(ptr)
declare <4 x float> @llvm.minnum.v4f32(<4 x float>, <4 x float>)
declare <4 x float> @llvm.maxnum.v4f32(<4 x float>, <4 x float>)
declare float @llvm.vector.reduce.fadd.v4f32(float, <4 x float>)
declare ptr @datara_rt_sha256(ptr)
declare ptr @datara_rt_base64_encode(ptr)
declare ptr @datara_rt_base64_decode(ptr)
declare ptr @datara_rt_uuid_v4()
declare i64 @datara_rt_random_bytes(ptr, i64)
declare i64 @datara_rt_dialog_info(ptr, ptr)
declare i64 @datara_rt_dialog_alert(ptr, ptr)
declare i64 @datara_rt_dialog_confirm(ptr, ptr)
declare void @datara_rt_parallel_for(i64, i64, i64, i64)
declare void @datara_rt_parallel_invoke(i64, i64, i64, i64)
declare i64 @datara_rt_system(ptr)
declare i64 @datara_rt_num_workers()
declare i64 @datara_rt_schedule_run(i64, i64, i64)
declare i64 @datara_rt_schedule_cancel(i64)
declare ptr @datara_rt_pool_alloc(i64)
declare void @datara_rt_pool_free(ptr, i64)
declare ptr @datara_rt_box_alloc(i64)
declare i64 @datara_rt_box_get(ptr)
declare void @datara_rt_box_free(ptr)
declare ptr @datara_rt_str_sso(ptr)
declare i64 @datara_rt_str_is_sso(ptr)
declare i64 @datara_rt_heap_alloc_count()
declare void @datara_rt_reset_heap_alloc_count()
declare ptr @datara_rt_list_init_stack(ptr, i64)
declare i64 @datara_rt_list_is_small_vec(ptr)
declare void @datara_rt_pgo_hit_func(ptr)
declare void @datara_rt_pgo_hit_branch(ptr, i64)
declare void @datara_rt_pgo_hit_loop(ptr, i64)
declare void @datara_rt_pgo_set_output_file(ptr)
declare void @datara_rt_pgo_flush(ptr)
declare void @datara_rt_pgo_reset()
declare ptr @datara_rt_chase_lev_create(i64)
declare void @datara_rt_chase_lev_destroy(ptr)
declare void @datara_rt_chase_lev_push(ptr, i64)
declare i64 @datara_rt_chase_lev_pop(ptr)
declare i64 @datara_rt_chase_lev_steal(ptr)
declare i64 @datara_rt_chase_lev_size(ptr)
declare ptr @datara_rt_fast_memcpy(ptr, ptr, i64)
declare ptr @datara_rt_fast_memset(ptr, i32, i64)
declare i32 @datara_rt_fast_strncmp(ptr, ptr, i64)
declare i32 @datara_rt_fast_memcmp(ptr, ptr, i64)
declare i32 @datara_rt_pin_thread(i64)
declare i64 @datara_rt_get_current_core()
declare void @datara_rt_pin_worker_threads()
declare void @datara_rt_cap_set_mask(i64)
declare i64 @datara_rt_cap_get_mask()
declare void @datara_rt_cap_revoke(i64)
declare void @datara_rt_cap_grant(i64)
declare void @datara_rt_cap_require(i64, ptr)
"#;
