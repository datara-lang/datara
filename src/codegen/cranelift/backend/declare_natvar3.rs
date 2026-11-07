//! PLAN_V152 N1.2/N3 runtime declarations extracted from declare_ext.rs
//! for the 60 KB release size gate: fiber mutexes + hash-map iteration.

use cranelift_codegen::ir::AbiParam;
use cranelift_codegen::ir::Signature;
use cranelift_codegen::ir::types as clif_types;
use cranelift_codegen::isa::CallConv;
use cranelift_module::Linkage;
use cranelift_module::Module as ClifModule;
use std::collections::HashMap;

type FuncIds = HashMap<String, (cranelift_module::FuncId, cranelift_codegen::ir::Signature)>;

pub fn declare<M: ClifModule>(
    module: &mut M,
    func_ids: &mut FuncIds,
    call_conv: CallConv,
) -> Result<(), String> {
    // PLAN_V152 N1.2 fiber mutexes: new() -> i64, lock(id, ms) -> ptr
    // (Outcome object), unlock(id) -> void.
    let mut rt_mutex_new_sig = Signature::new(call_conv);
    rt_mutex_new_sig
        .returns
        .push(AbiParam::new(clif_types::I64));
    let rt_mutex_new_id = module
        .declare_function("datara_rt_mutex_new", Linkage::Import, &rt_mutex_new_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_mutex_new".into(),
        (rt_mutex_new_id, rt_mutex_new_sig.clone()),
    );
    func_ids.insert("mutex_new".into(), (rt_mutex_new_id, rt_mutex_new_sig));

    let mut rt_mutex_lock_sig = Signature::new(call_conv);
    rt_mutex_lock_sig
        .params
        .push(AbiParam::new(clif_types::I64));
    rt_mutex_lock_sig
        .params
        .push(AbiParam::new(clif_types::I64));
    rt_mutex_lock_sig
        .returns
        .push(AbiParam::new(clif_types::I64));
    let rt_mutex_lock_id = module
        .declare_function("datara_rt_mutex_lock", Linkage::Import, &rt_mutex_lock_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_mutex_lock".into(),
        (rt_mutex_lock_id, rt_mutex_lock_sig.clone()),
    );
    func_ids.insert("mutex_lock".into(), (rt_mutex_lock_id, rt_mutex_lock_sig));

    let mut rt_mutex_unlock_sig = Signature::new(call_conv);
    rt_mutex_unlock_sig
        .params
        .push(AbiParam::new(clif_types::I64));
    let rt_mutex_unlock_id = module
        .declare_function(
            "datara_rt_mutex_unlock",
            Linkage::Import,
            &rt_mutex_unlock_sig,
        )
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_mutex_unlock".into(),
        (rt_mutex_unlock_id, rt_mutex_unlock_sig.clone()),
    );
    func_ids.insert(
        "mutex_unlock".into(),
        (rt_mutex_unlock_id, rt_mutex_unlock_sig),
    );

    // PLAN_V152 N1.3: fiber_is_done(handle) -> i64 (non-blocking poll).
    let mut rt_fib_done_sig = Signature::new(call_conv);
    rt_fib_done_sig.params.push(AbiParam::new(clif_types::I64));
    rt_fib_done_sig.returns.push(AbiParam::new(clif_types::I64));
    let rt_fib_done_id = module
        .declare_function("datara_rt_fiber_is_done", Linkage::Import, &rt_fib_done_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_fiber_is_done".into(),
        (rt_fib_done_id, rt_fib_done_sig.clone()),
    );
    func_ids.insert("fiber_is_done".into(), (rt_fib_done_id, rt_fib_done_sig));

    // PLAN_V152 N3: hash-map iteration surface (map.dtr / hashmap.dtr).
    // keys_str/keys_int/values: (map: ptr) -> ptr (list of slots).
    let mut rt_map_iter_sig = Signature::new(call_conv);
    rt_map_iter_sig.params.push(AbiParam::new(clif_types::I64));
    rt_map_iter_sig.returns.push(AbiParam::new(clif_types::I64));
    for name in &[
        "datara_rt_map_keys_str",
        "datara_rt_map_keys_int",
        "datara_rt_map_values",
    ] {
        let id = module
            .declare_function(name, Linkage::Import, &rt_map_iter_sig)
            .map_err(|e| e.to_string())?;
        func_ids.insert((*name).into(), (id, rt_map_iter_sig.clone()));
    }
    for alias in &["map_keys", "map_keys_int", "map_values", "map_values_str"] {
        let (cname, _) = match *alias {
            "map_keys" => ("datara_rt_map_keys_str", 0),
            "map_keys_int" => ("datara_rt_map_keys_int", 0),
            _ => ("datara_rt_map_values", 0),
        };
        let (id, sig) = func_ids.get(cname).cloned().unwrap();
        func_ids.insert((*alias).into(), (id, sig));
    }

    // PLAN_V153 A2: fiber_net_accept(sock, timeout) -> Outcome<Str>
    // (handle encoded in the value slot; parking on would-block).
    let mut rt_acc_sig = Signature::new(call_conv);
    rt_acc_sig.params.push(AbiParam::new(clif_types::I64));
    rt_acc_sig.params.push(AbiParam::new(clif_types::I64));
    rt_acc_sig.returns.push(AbiParam::new(clif_types::I64));
    let rt_acc_id = module
        .declare_function("datara_rt_fiber_net_accept", Linkage::Import, &rt_acc_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_fiber_net_accept".into(),
        (rt_acc_id, rt_acc_sig.clone()),
    );
    func_ids.insert("fiber_net_accept".into(), (rt_acc_id, rt_acc_sig));

    // PLAN_V153 A3/A4: date parts + fs tree operations.
    // date_parts(unix_ms) -> ptr(list of 6); fs ops -> i64 status codes;
    // fs size/mtime -> i64; fs rename takes two paths.
    let mut rt_dp_sig = Signature::new(call_conv);
    rt_dp_sig.params.push(AbiParam::new(clif_types::I64));
    rt_dp_sig.returns.push(AbiParam::new(clif_types::I64));
    let rt_dp_id = module
        .declare_function("datara_rt_date_parts", Linkage::Import, &rt_dp_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert("datara_rt_date_parts".into(), (rt_dp_id, rt_dp_sig.clone()));
    func_ids.insert("date_parts".into(), (rt_dp_id, rt_dp_sig));

    let mut rt_unix_sig = Signature::new(call_conv);
    rt_unix_sig.returns.push(AbiParam::new(clif_types::I64));
    let rt_unix_id = module
        .declare_function("datara_rt_now_unix_ms", Linkage::Import, &rt_unix_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_now_unix_ms".into(),
        (rt_unix_id, rt_unix_sig.clone()),
    );
    func_ids.insert("now_unix_ms".into(), (rt_unix_id, rt_unix_sig));

    let mut rt_fs_path_sig = Signature::new(call_conv);
    rt_fs_path_sig.params.push(AbiParam::new(clif_types::I64));
    rt_fs_path_sig.returns.push(AbiParam::new(clif_types::I64));
    for (name, cname) in &[
        ("fs_mkdir", "datara_rt_fs_mkdir"),
        ("fs_remove", "datara_rt_fs_remove"),
        ("fs_file_size", "datara_rt_fs_file_size"),
        ("fs_mtime", "datara_rt_fs_mtime"),
    ] {
        let id = module
            .declare_function(cname, Linkage::Import, &rt_fs_path_sig)
            .map_err(|e| e.to_string())?;
        func_ids.insert((*cname).into(), (id, rt_fs_path_sig.clone()));
        func_ids.insert((*name).into(), (id, rt_fs_path_sig.clone()));
    }

    let mut rt_fs_rename_sig = Signature::new(call_conv);
    rt_fs_rename_sig.params.push(AbiParam::new(clif_types::I64));
    rt_fs_rename_sig.params.push(AbiParam::new(clif_types::I64));
    rt_fs_rename_sig
        .returns
        .push(AbiParam::new(clif_types::I64));
    let rt_fs_rename_id = module
        .declare_function("datara_rt_fs_rename", Linkage::Import, &rt_fs_rename_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_fs_rename".into(),
        (rt_fs_rename_id, rt_fs_rename_sig.clone()),
    );
    func_ids.insert("fs_rename".into(), (rt_fs_rename_id, rt_fs_rename_sig));

    Ok(())
}
