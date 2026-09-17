use cranelift_codegen::ir::{types as clif_types, AbiParam, Signature};
use cranelift_codegen::isa::CallConv;
use cranelift_module::{FuncId, Linkage, Module as ClifModule};
use std::collections::HashMap;

fn reg_fn<M: ClifModule>(
    module: &mut M,
    func_ids: &mut HashMap<String, (FuncId, Signature)>,
    name: &str,
    alt_name: Option<&str>,
    sig: Signature,
) -> Result<FuncId, String> {
    let id = module
        .declare_function(name, Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(name.into(), (id, sig.clone()));
    if let Some(alt) = alt_name {
        func_ids.insert(alt.into(), (id, sig));
    }
    Ok(id)
}

/// Declares runtime functions for the 4-tier progressive memory spectrum:
/// Tier 2 (Arena allocation & stats),
/// Tier 3 (Raw pointer allocation, reading, writing, and freeing),
/// Tier 4 (Hardware CPU fences and cache line prefetch hints),
/// as well as module-level global storage functions.
pub fn declare_runtime_memory<M: ClifModule>(
    module: &mut M,
    call_conv: CallConv,
    func_ids: &mut HashMap<String, (FuncId, Signature)>,
) -> Result<(), String> {
    // Tier 2: Arena stats and reset
    {
        let mut sig = Signature::new(call_conv);
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_arena_used", Some("arena_used"), sig)?;
    }
    {
        let sig = Signature::new(call_conv);
        reg_fn(module, func_ids, "datara_rt_arena_clear", Some("arena_clear"), sig)?;
    }

    // Tier 3: Raw Heap allocation and Pointer operations
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_mem_alloc", Some("mem_alloc"), sig)?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_mem_free", Some("mem_free"), sig)?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_mem_copy", Some("mem_copy"), sig)?;
    }

    // ptr_read_i64: (i64, i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_ptr_read_i64", Some("ptr_read_i64"), sig)?;
    }
    // ptr_write_i64: (i64, i64, i64) -> ()
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_ptr_write_i64", Some("ptr_write_i64"), sig)?;
    }

    // ptr_read_u8: (i64, i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_ptr_read_u8", Some("ptr_read_u8"), sig)?;
    }
    // ptr_write_u8: (i64, i64, i64) -> ()
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_ptr_write_u8", Some("ptr_write_u8"), sig)?;
    }

    // ptr_read_f64: (i64, i64) -> f64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::F64));
        reg_fn(module, func_ids, "datara_rt_ptr_read_f64", Some("ptr_read_f64"), sig)?;
    }
    // ptr_write_f64: (i64, i64, f64) -> ()
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::F64));
        reg_fn(module, func_ids, "datara_rt_ptr_write_f64", Some("ptr_write_f64"), sig)?;
    }

    // Tier 4: Hardware CPU fences and cache prefetch
    {
        let sig = Signature::new(call_conv);
        reg_fn(module, func_ids, "datara_rt_cpu_fence", Some("cpu_fence"), sig)?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "datara_rt_cpu_prefetch", Some("cpu_prefetch"), sig)?;
    }

    // Module-level global state
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        let id = module
            .declare_function("datara_rt_global_set", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?;
        func_ids.insert("datara_rt_global_set".into(), (id, sig));
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        let id = module
            .declare_function("datara_rt_global_get", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?;
        func_ids.insert("datara_rt_global_get".into(), (id, sig));
    }

    Ok(())
}
