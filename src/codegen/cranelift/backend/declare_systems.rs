use cranelift_codegen::ir::{AbiParam, Signature, types as clif_types};
use cranelift_codegen::isa::CallConv;
use cranelift_module::{FuncId, Linkage, Module as ClifModule};
use std::collections::HashMap;

fn reg_fn<M: ClifModule>(
    module: &mut M,
    func_ids: &mut HashMap<String, (FuncId, Signature)>,
    name: &str,
    alt_names: &[&str],
    sig: Signature,
) -> Result<FuncId, String> {
    let id = module
        .declare_function(name, Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert(name.into(), (id, sig.clone()));
    for alt in alt_names {
        func_ids.insert((*alt).into(), (id, sig.clone()));
    }
    Ok(id)
}

/// Declares native runtime systems, endianness, and SliceView functions for Cranelift backend.
pub fn declare_runtime_systems<M: ClifModule>(
    module: &mut M,
    call_conv: CallConv,
    func_ids: &mut HashMap<String, (FuncId, Signature)>,
) -> Result<(), String> {
    // 1. Endianness & byte swap intrinsics: (i64) -> i64
    let unary_i64_ops = [
        ("hton16", "datara_sys_hton16"),
        ("ntoh16", "datara_sys_ntoh16"),
        ("hton32", "datara_sys_hton32"),
        ("ntoh32", "datara_sys_ntoh32"),
        ("hton64", "datara_sys_hton64"),
        ("ntoh64", "datara_sys_ntoh64"),
        ("bswap16", "datara_sys_bswap16"),
        ("bswap32", "datara_sys_bswap32"),
        ("bswap64", "datara_sys_bswap64"),
    ];
    for (datara_name, c_name) in unary_i64_ops {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, c_name, &[datara_name], sig)?;
    }

    // 2. Slice constructors:
    // slice_alloc: (i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_sys_slice_alloc",
            &["slice_alloc"],
            sig,
        )?;
    }

    // slice_from_buffer: (i64, i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_sys_slice_from_buffer",
            &["slice_from_buffer"],
            sig,
        )?;
    }

    // slice_free: (i64) -> void
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_sys_slice_free",
            &["slice_free", "SliceView_free"],
            sig,
        )?;
    }

    // SliceView_len: (i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_sys_slice_len",
            &["SliceView_len"],
            sig,
        )?;
    }

    // SliceView_get_byte: (i64, i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_sys_slice_get_byte",
            &["SliceView_get_byte"],
            sig,
        )?;
    }

    // SliceView_set_byte: (i64, i64, i64) -> void
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_sys_slice_set_byte",
            &["SliceView_set_byte"],
            sig,
        )?;
    }

    // SliceView read operations: (i64, i64) -> i64
    let read_ops = [
        ("SliceView_read_u16_be", "datara_sys_slice_read_u16_be"),
        ("SliceView_read_u16_le", "datara_sys_slice_read_u16_le"),
        ("SliceView_read_u32_be", "datara_sys_slice_read_u32_be"),
        ("SliceView_read_u32_le", "datara_sys_slice_read_u32_le"),
        ("SliceView_read_u64_be", "datara_sys_slice_read_u64_be"),
        ("SliceView_read_u64_le", "datara_sys_slice_read_u64_le"),
    ];
    for (datara_name, c_name) in read_ops {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, c_name, &[datara_name], sig)?;
    }

    // SliceView write operations: (i64, i64, i64) -> void
    let write_ops = [
        ("SliceView_write_u16_be", "datara_sys_slice_write_u16_be"),
        ("SliceView_write_u16_le", "datara_sys_slice_write_u16_le"),
        ("SliceView_write_u32_be", "datara_sys_slice_write_u32_be"),
        ("SliceView_write_u32_le", "datara_sys_slice_write_u32_le"),
        ("SliceView_write_u64_be", "datara_sys_slice_write_u64_be"),
        ("SliceView_write_u64_le", "datara_sys_slice_write_u64_le"),
    ];
    for (datara_name, c_name) in write_ops {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, c_name, &[datara_name], sig)?;
    }

    // SliceView_subslice: (i64, i64, i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_sys_slice_subslice",
            &["SliceView_subslice"],
            sig,
        )?;
    }

    // 3. Level 4: Hardware MMIO & Volatile Access
    let volatile_read_ops = [
        (
            "volatile_read8",
            "datara_hw_volatile_read8",
            "VolatilePtr_read8",
        ),
        (
            "volatile_read16",
            "datara_hw_volatile_read16",
            "VolatilePtr_read16",
        ),
        (
            "volatile_read32",
            "datara_hw_volatile_read32",
            "VolatilePtr_read32",
        ),
        (
            "volatile_read64",
            "datara_hw_volatile_read64",
            "VolatilePtr_read64",
        ),
    ];
    for (dtr_name, c_name, vptr_name) in volatile_read_ops {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, c_name, &[dtr_name, vptr_name], sig)?;
    }

    let volatile_write_ops = [
        (
            "volatile_write8",
            "datara_hw_volatile_write8",
            "VolatilePtr_write8",
        ),
        (
            "volatile_write16",
            "datara_hw_volatile_write16",
            "VolatilePtr_write16",
        ),
        (
            "volatile_write32",
            "datara_hw_volatile_write32",
            "VolatilePtr_write32",
        ),
        (
            "volatile_write64",
            "datara_hw_volatile_write64",
            "VolatilePtr_write64",
        ),
    ];
    for (dtr_name, c_name, vptr_name) in volatile_write_ops {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, c_name, &[dtr_name, vptr_name], sig)?;
    }

    // volatile_ptr: (i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_hw_volatile_ptr",
            &["volatile_ptr"],
            sig,
        )?;
    }

    // typed_zero_init: (i64) -> i64
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_hw_typed_zero_init",
            &["typed_zero_init"],
            sig,
        )?;
    }

    // 4. Level 4: Hardware Memory Fences
    let fence_ops = [
        ("atomic_fence_acquire", "datara_hw_atomic_fence_acquire"),
        ("atomic_fence_release", "datara_hw_atomic_fence_release"),
        ("atomic_fence_acq_rel", "datara_hw_atomic_fence_acq_rel"),
        ("atomic_fence_seq_cst", "datara_hw_atomic_fence_seq_cst"),
    ];
    for (dtr_name, c_name) in fence_ops {
        let sig = Signature::new(call_conv);
        reg_fn(module, func_ids, c_name, &[dtr_name], sig)?;
    }

    // atomic_fence(order: Str): (i64) -> void
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "datara_hw_atomic_fence",
            &["atomic_fence"],
            sig,
        )?;
    }

    Ok(())
}
