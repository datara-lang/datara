//! LLVM IR Standard Runtime Declarations & Inlined Arithmetic Builtins.

pub fn emit_runtime_declarations(ir: &mut String) {
    ir.push_str(RUNTIME_DECLARATIONS);
}

const RUNTIME_DECLARATIONS: &str = r#"; --- Datara Standard Runtime Declarations ---
declare void @llvm.assume(i1)declare void @datara_rt_overflow_panic() cold noreturn nounwind

declare void @datara_rt_div_zero_panic() cold noreturn nounwind

declare void @datara_rt_panic(ptr) cold noreturn nounwind
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

define internal double @datara_rt_list_get_f64_unchecked(ptr readonly %list, i64 %idx) alwaysinline {
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

define internal i64 @datara_rt_list_get(ptr readonly %list, i64 %idx) alwaysinline {
entry:
  %null_check = icmp eq ptr %list, null
  br i1 %null_check, label %oob, label %chk_cnt, !prof !9

chk_cnt:
  %count = load i64, ptr %list, align 8
  %neg_check = icmp slt i64 %idx, 0
  %hi_check = icmp sge i64 %idx, %count
  %oob_check = or i1 %neg_check, %hi_check
  br i1 %oob_check, label %oob, label %in_bounds, !prof !9

in_bounds:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds i64, ptr %list, i64 %off
  %val = load i64, ptr %ptr, align 8
  ret i64 %val

oob:
  ret i64 0
}

define internal double @datara_rt_list_get_f64(ptr readonly %list, i64 %idx) alwaysinline {
entry:
  %null_check = icmp eq ptr %list, null
  br i1 %null_check, label %oob, label %chk_cnt, !prof !9

chk_cnt:
  %count = load i64, ptr %list, align 8
  %neg_check = icmp slt i64 %idx, 0
  %hi_check = icmp sge i64 %idx, %count
  %oob_check = or i1 %neg_check, %hi_check
  br i1 %oob_check, label %oob, label %in_bounds, !prof !9

in_bounds:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds double, ptr %list, i64 %off
  %val = load double, ptr %ptr, align 8
  ret double %val

oob:
  ret double 0.0
}

define internal ptr @datara_rt_list_set(ptr %list, i64 %idx, i64 %val) alwaysinline {
entry:
  %null_check = icmp eq ptr %list, null
  br i1 %null_check, label %done, label %chk_cnt, !prof !9

chk_cnt:
  %count = load i64, ptr %list, align 8
  %neg_check = icmp slt i64 %idx, 0
  %hi_check = icmp sge i64 %idx, %count
  %oob_check = or i1 %neg_check, %hi_check
  br i1 %oob_check, label %done, label %in_bounds, !prof !9

in_bounds:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds i64, ptr %list, i64 %off
  store i64 %val, ptr %ptr, align 8
  br label %done

done:
  ret ptr %list
}

define internal ptr @datara_rt_list_set_f64(ptr %list, i64 %idx, double %val) alwaysinline {
entry:
  %null_check = icmp eq ptr %list, null
  br i1 %null_check, label %done, label %chk_cnt, !prof !9

chk_cnt:
  %count = load i64, ptr %list, align 8
  %neg_check = icmp slt i64 %idx, 0
  %hi_check = icmp sge i64 %idx, %count
  %oob_check = or i1 %neg_check, %hi_check
  br i1 %oob_check, label %done, label %in_bounds, !prof !9

in_bounds:
  %off = add i64 %idx, 1
  %ptr = getelementptr inbounds double, ptr %list, i64 %off
  store double %val, ptr %ptr, align 8
  br label %done

done:
  ret ptr %list
}

declare i64 @datara_rt_own_acquire(i64)
declare void @datara_rt_own_release(i64)
declare void @datara_rt_out_int(i64)
declare void @datara_rt_out_float(double)
declare void @datara_rt_print_f32(float)
declare void @datara_rt_print_dec64(i64)
declare void @datara_rt_out_dec64(i64)
declare void @datara_rt_out_bool(i64)
declare void @datara_rt_out_str(ptr)
declare void @datara_rt_err(ptr)
declare void @datara_rt_err_print_str(ptr)
declare void @datara_rt_err_print_int(i64)
declare void @datara_rt_err_print_dec64_str(i64)
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
declare ptr @datara_rt_dec_to_str(i64)
declare ptr @datara_rt_bool_to_str(i64)
declare ptr @datara_rt_float_to_str(double)
; v1.4.5 W4 fmt specifiers: precision / radix converters.
declare ptr @datara_rt_float_to_str_prec(double, i64)
declare ptr @datara_rt_dec64_to_str_prec(i64, i64)
declare ptr @datara_rt_int_to_str_radix(i64, i64)
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
declare noalias ptr @datara_rt_list_create(i64)
declare noalias ptr @datara_rt_list_create_1(i64)
declare noalias ptr @datara_rt_list_create_2(i64, i64)
declare noalias ptr @datara_rt_list_create_3(i64, i64, i64)
declare noalias ptr @datara_rt_list_create_4(i64, i64, i64, i64)
declare noalias ptr @datara_rt_list_create_5(i64, i64, i64, i64, i64)
declare noalias ptr @datara_rt_stack_alloc(i64)
declare ptr @datara_rt_list_append_slow(ptr, i64)

define internal ptr @datara_rt_list_append(ptr %list, i64 %v) alwaysinline {
entry:
  %null_chk = icmp eq ptr %list, null
  br i1 %null_chk, label %slow, label %chk_magic, !prof !9

chk_magic:
  %hdr_magic_ptr = getelementptr inbounds i64, ptr %list, i64 -1
  %magic = load i64, ptr %hdr_magic_ptr, align 8
  %magic_masked = and i64 %magic, -16
  %not_magic = icmp ne i64 %magic_masked, 4918304954689737776
  br i1 %not_magic, label %slow, label %chk_cap, !prof !9

chk_cap:
  %hdr_cap_ptr = getelementptr inbounds i64, ptr %list, i64 -2
  %cap = load i64, ptr %hdr_cap_ptr, align 8
  %count = load i64, ptr %list, align 8
  %new_count = add i64 %count, 1
  %no_cap = icmp sgt i64 %new_count, %cap
  br i1 %no_cap, label %slow, label %fast, !prof !9

fast:
  store i64 %new_count, ptr %list, align 8
  %elem_ptr = getelementptr inbounds i64, ptr %list, i64 %new_count
  store i64 %v, ptr %elem_ptr, align 8
  ret ptr %list

slow:
  %res = call ptr @datara_rt_list_append_slow(ptr %list, i64 %v)
  ret ptr %res
}

define internal ptr @datara_rt_list_append_unchecked(ptr %list, i64 %v) alwaysinline {
entry:
  %count = load i64, ptr %list, align 8
  %new_count = add i64 %count, 1
  store i64 %new_count, ptr %list, align 8
  %elem_ptr = getelementptr inbounds i64, ptr %list, i64 %new_count
  store i64 %v, ptr %elem_ptr, align 8
  ret ptr %list
}

define internal ptr @datara_rt_list_append_f64_unchecked(ptr %list, double %v) alwaysinline {
entry:
  %count = load i64, ptr %list, align 8
  %new_count = add i64 %count, 1
  store i64 %new_count, ptr %list, align 8
  %elem_ptr = getelementptr inbounds double, ptr %list, i64 %new_count
  store double %v, ptr %elem_ptr, align 8
  ret ptr %list
}
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

; v1.4.5 W1: checked arithmetic returning Outcome<IntN> objects.
declare ptr @datara_rt_checked_add_outcome(i64, i64, i64)
declare ptr @datara_rt_checked_sub_outcome(i64, i64, i64)
declare ptr @datara_rt_checked_mul_outcome(i64, i64, i64)
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
declare ptr @datara_rt_arena_alloc(i64)
declare i64 @datara_rt_arena_checkpoint()
declare void @datara_rt_arena_reset(i64)
declare i64 @datara_rt_arena_remaining()
declare i64 @datara_rt_arena_used()
declare void @datara_rt_arena_clear()
declare ptr @arena_alloc(i64)
declare void @arena_reset(i64)
declare i64 @arena_used()
declare void @arena_clear()
declare ptr @mem_alloc(i64)
declare ptr @stack_alloc(i64)
declare void @mem_free(ptr)
declare void @mem_copy(ptr, ptr, i64)
declare i64 @ptr_read_i64(ptr, i64)
declare void @ptr_write_i64(ptr, i64, i64)
declare double @ptr_read_f64(ptr, i64)
declare void @ptr_write_f64(ptr, i64, double)
declare i64 @ptr_read_u8(ptr, i64)
declare void @ptr_write_u8(ptr, i64, i64)
declare void @cpu_fence()
declare void @cpu_prefetch(ptr)
declare void @datara_rt_global_set(ptr, i64)
declare i64 @datara_rt_global_get(ptr)
declare ptr @spawn(ptr)
declare i64 @join(ptr)
declare i64 @join_timeout(ptr, i64)
declare void @ThreadHandle_free(ptr)
declare ptr @channel_create(i64)
declare ptr @channel_new()
declare i32 @channel_send(ptr, i64)
declare i64 @channel_recv(ptr)
declare i64 @channel_try_recv(ptr)
declare void @channel_close(ptr)
declare i64 @channel_len(ptr)
declare void @Channel_free(ptr)
declare i64 @scratch_enter()
declare ptr @scratch_alloc(i64, i64)
declare void @scratch_exit(i64)
declare ptr @scratch_promote(ptr, i64)
declare void @parallel_for(i64, i64, ptr, ptr)
declare i64 @bswap16(i64)
declare i64 @bswap32(i64)
declare i64 @bswap64(i64)
declare i64 @hton16(i64)
declare i64 @ntoh16(i64)
declare i64 @hton32(i64)
declare i64 @ntoh32(i64)
declare i64 @hton64(i64)
declare i64 @ntoh64(i64)
declare ptr @slice_alloc(i64)
declare ptr @slice_from_ptr(ptr, i64)
declare void @slice_free(ptr)
declare i64 @SliceView_len(ptr)
declare i64 @SliceView_get_byte(ptr, i64)
declare void @SliceView_set_byte(ptr, i64, i64)
declare i64 @SliceView_read_u16_be(ptr, i64)
declare i64 @SliceView_read_u16_le(ptr, i64)
declare i64 @SliceView_read_u32_be(ptr, i64)
declare i64 @SliceView_read_u32_le(ptr, i64)
declare i64 @SliceView_read_u64_be(ptr, i64)
declare i64 @SliceView_read_u64_le(ptr, i64)
declare void @SliceView_write_u16_be(ptr, i64, i64)
declare void @SliceView_write_u16_le(ptr, i64, i64)
declare void @SliceView_write_u32_be(ptr, i64, i64)
declare void @SliceView_write_u32_le(ptr, i64, i64)
declare void @SliceView_write_u64_be(ptr, i64, i64)
declare void @SliceView_write_u64_le(ptr, i64, i64)
declare ptr @SliceView_subslice(ptr, i64, i64)
declare void @SliceView_free(ptr)
declare ptr @volatile_ptr(ptr)
declare i64 @volatile_read8(ptr)
declare i64 @volatile_read16(ptr)
declare i64 @volatile_read32(ptr)
declare i64 @volatile_read64(ptr)
declare void @volatile_write8(ptr, i64)
declare void @volatile_write16(ptr, i64)
declare void @volatile_write32(ptr, i64)
declare void @volatile_write64(ptr, i64)
declare i64 @VolatilePtr_read8(ptr)
declare i64 @VolatilePtr_read16(ptr)
declare i64 @VolatilePtr_read32(ptr)
declare i64 @VolatilePtr_read64(ptr)
declare void @VolatilePtr_write8(ptr, i64)
declare void @VolatilePtr_write16(ptr, i64)
declare void @VolatilePtr_write32(ptr, i64)
declare void @VolatilePtr_write64(ptr, i64)
declare void @atomic_fence_acquire()
declare void @atomic_fence_release()
declare void @atomic_fence_acq_rel()
declare void @atomic_fence_seq_cst()
declare void @atomic_fence(ptr)
declare ptr @typed_zero_init(i64)
"#;
