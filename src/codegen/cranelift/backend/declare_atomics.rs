//! AtomicInt declarations (PLAN_V152 N1.2) — extracted from
//! declare_ext.rs for the 60 KB release size gate.

use super::*;
use cranelift_codegen::ir::{AbiParam, Signature, types as clif_types};
use cranelift_codegen::isa::CallConv;
use cranelift_module::{FuncId, Linkage, Module as ClifModule};

pub(crate) fn declare_atomics<M: ClifModule>(
    module: &mut M,
    func_ids: &mut HashMap<String, (FuncId, Signature)>,
    call_conv: CallConv,
    i64_1_sig: &Signature,
    i64_2_sig: &Signature,
) -> Result<(), String> {
    let atomic_new_id = module
        .declare_function("datara_rt_atomic_new", Linkage::Import, i64_1_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_atomic_new".into(),
        (atomic_new_id, i64_1_sig.clone()),
    );
    func_ids.insert("atomic_new".into(), (atomic_new_id, i64_1_sig.clone()));
    let atomic_load_id = module
        .declare_function("datara_rt_atomic_load", Linkage::Import, i64_1_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_atomic_load".into(),
        (atomic_load_id, i64_1_sig.clone()),
    );
    func_ids.insert("atomic_load".into(), (atomic_load_id, i64_1_sig.clone()));
    let atomic_store_id = module
        .declare_function("datara_rt_atomic_store", Linkage::Import, i64_2_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_atomic_store".into(),
        (atomic_store_id, i64_2_sig.clone()),
    );
    func_ids.insert("atomic_store".into(), (atomic_store_id, i64_2_sig.clone()));
    let atomic_faa_id = module
        .declare_function("datara_rt_atomic_fetch_add", Linkage::Import, i64_2_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_atomic_fetch_add".into(),
        (atomic_faa_id, i64_2_sig.clone()),
    );
    func_ids.insert(
        "atomic_fetch_add".into(),
        (atomic_faa_id, i64_2_sig.clone()),
    );
    let mut atomic_cas_sig = Signature::new(call_conv);
    for _ in 0..3 {
        atomic_cas_sig.params.push(AbiParam::new(clif_types::I64));
    }
    atomic_cas_sig.returns.push(AbiParam::new(clif_types::I64));
    let atomic_cas_id = module
        .declare_function("datara_rt_atomic_cas", Linkage::Import, &atomic_cas_sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(
        "datara_rt_atomic_cas".into(),
        (atomic_cas_id, atomic_cas_sig),
    );
    Ok(())
}
