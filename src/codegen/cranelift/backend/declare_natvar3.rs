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

    Ok(())
}
