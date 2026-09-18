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

/// Declares native runtime concurrency and scratchpad arena functions for Cranelift backend.
pub fn declare_runtime_concurrency<M: ClifModule>(
    module: &mut M,
    call_conv: CallConv,
    func_ids: &mut HashMap<String, (FuncId, Signature)>,
) -> Result<(), String> {
    // 1. Thread spawn
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "spawn",
            &["datara_thread_spawn_wrap"],
            sig,
        )?;
    }

    // 2. Thread join
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "join", &["ThreadHandle_join"], sig)?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "join_timeout",
            &["ThreadHandle_join_timeout"],
            sig,
        )?;
    }

    // 3. Thread handle free
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "ThreadHandle_free",
            &["datara_thread_free"],
            sig,
        )?;
    }

    // 4. Channel create & new
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "channel_create",
            &["datara_channel_create"],
            sig,
        )?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(module, func_ids, "channel_new", &["Channel_new"], sig)?;
    }

    // 5. Channel send
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "channel_send",
            &["Channel_send", "datara_channel_send_wrap"],
            sig,
        )?;
    }

    // 6. Channel recv & try_recv
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "channel_recv",
            &["Channel_recv"],
            sig.clone(),
        )?;
        reg_fn(
            module,
            func_ids,
            "channel_try_recv",
            &["Channel_try_recv"],
            sig,
        )?;
    }

    // 7. Channel close
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "channel_close",
            &["Channel_close", "datara_channel_close"],
            sig,
        )?;
    }

    // 8. Channel len
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "channel_len",
            &["Channel_len", "datara_channel_len"],
            sig,
        )?;
    }

    // 9. Channel free
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "Channel_free",
            &["datara_channel_free"],
            sig,
        )?;
    }

    // 10. Parallel For
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "parallel_for",
            &["datara_parallel_for"],
            sig,
        )?;
    }

    // 11. Scratchpad Memory
    {
        let mut sig = Signature::new(call_conv);
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "scratch_enter",
            &["datara_scratch_enter"],
            sig,
        )?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "scratch_alloc",
            &["datara_scratch_alloc"],
            sig,
        )?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "scratch_exit",
            &["datara_scratch_exit"],
            sig,
        )?;
    }
    {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.params.push(AbiParam::new(clif_types::I64));
        sig.returns.push(AbiParam::new(clif_types::I64));
        reg_fn(
            module,
            func_ids,
            "scratch_promote",
            &["datara_scratch_promote"],
            sig,
        )?;
    }

    Ok(())
}
