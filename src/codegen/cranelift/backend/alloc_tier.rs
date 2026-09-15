//! v1.4.0 allocator tiers in the Cranelift backend: `@arena` / `@pool(n)`
//! frame setup, region reclaim on return, and the `StrBuf` constructor.
//!
//! The tier state is emitted ENTIRELY in the backend, not as DMIR
//! instructions: the arena checkpoint lives in a Cranelift variable of the
//! current frame (recursion-safe stack discipline of checkpoints) and the
//! pool slab/counter are per-frame locals. Keeping it out of DMIR means no
//! optimizer pass can reorder or eliminate the region bookkeeping.
//!
//! ```text
//! @arena fn f() {           ->  checkpoint = rt_arena_checkpoint()
//!     let p = P { .. }      ->  p = rt_arena_alloc(sizeof P)     (escaping)
//! }                         ->  rt_arena_reset(checkpoint)       (every return)
//!
//! @pool(4) fn g() {         ->  checkpoint = rt_arena_checkpoint()
//!     let p = P { .. }      ->  slab = rt_arena_alloc(4 * 64)
//! }                         ->  cnt = 0
//!     let q = P { .. }      ->  if cnt >= 4 { rt_panic(msg) }
//!                               addr = slab + cnt * 64; cnt += 1
//! ```

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types as clif_types;
use cranelift_codegen::ir::{InstBuilder, StackSlotData, StackSlotKind, Value as ClifValue};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataId, Module as ClifModule};
use std::collections::HashMap;

use super::types::RuntimeIds;
use crate::dmir::{ArenaHint, Function};

/// Byte size of one `@pool` object slot. Must cover the largest class the
/// driver-level validation allows into a pool (8 fields x 8 bytes); bigger
/// classes are rejected with E1406 before codegen.
pub(crate) const POOL_SLOT_SIZE: i64 = 64;

/// String-table key of the pool-capacity trap message. Declared in the
/// string literal table whenever a `@pool` function is present.
pub(crate) const POOL_OVERFLOW_MSG: &str = "runtime trap: @pool capacity exceeded";

/// Per-frame tier bookkeeping emitted at function entry.
pub(crate) struct TierFrame {
    /// Cranelift variable holding the arena checkpoint taken at entry.
    /// `None` for plain functions (no `@arena`/`@pool`).
    pub arena_cp_var: Option<Variable>,
    /// `(slab_base, slot_count, capacity)` for `@pool` frames.
    pub pool_state: Option<(Variable, Variable, u64)>,
}

impl TierFrame {
    pub fn none() -> Self {
        Self {
            arena_cp_var: None,
            pool_state: None,
        }
    }
}

/// Emits the frame prologue into the entry block (which the caller has just
/// switched to): checkpoint capture, and for `@pool` the slab bump plus the
/// slot counter reset.
pub(crate) fn emit_prologue<M: ClifModule>(
    builder: &mut FunctionBuilder,
    module: &mut M,
    runtime: &RuntimeIds,
    f: &Function,
) -> TierFrame {
    if f.alloc_hint == ArenaHint::None {
        return TierFrame::none();
    }
    let cp_ref = module.declare_func_in_func(runtime.rt_arena_checkpoint_id, builder.func);
    let cp_call = builder.ins().call(cp_ref, &[]);
    let cp_val = builder.inst_results(cp_call)[0];
    let cp_var = builder.declare_var(clif_types::I64);
    builder.def_var(cp_var, cp_val);

    let mut frame = TierFrame {
        arena_cp_var: Some(cp_var),
        pool_state: None,
    };

    if let ArenaHint::Pool(capacity) = f.alloc_hint {
        let alloc_ref = module.declare_func_in_func(runtime.rt_arena_alloc_id, builder.func);
        let slab_bytes = capacity
            .saturating_mul(POOL_SLOT_SIZE as u64)
            .min(i64::MAX as u64) as i64;
        let slab_sz = builder.ins().iconst(clif_types::I64, slab_bytes);
        let base_call = builder.ins().call(alloc_ref, &[slab_sz]);
        let base_val = builder.inst_results(base_call)[0];
        let base_var = builder.declare_var(clif_types::I64);
        builder.def_var(base_var, base_val);
        let cnt_var = builder.declare_var(clif_types::I64);
        let zero = builder.ins().iconst(clif_types::I64, 0);
        builder.def_var(cnt_var, zero);
        frame.pool_state = Some((base_var, cnt_var, capacity));
    }
    frame
}

/// Restores the entry checkpoint before a return: one instruction reclaims
/// the whole region (arena bumps, pool slab, and every escaping object).
pub(crate) fn emit_return_reset<M: ClifModule>(
    builder: &mut FunctionBuilder,
    module: &mut M,
    runtime: &RuntimeIds,
    frame: &TierFrame,
) {
    let Some(cp_var) = frame.arena_cp_var else {
        return;
    };
    let cp_val = builder.use_var(cp_var);
    let reset_ref = module.declare_func_in_func(runtime.rt_arena_reset_id, builder.func);
    builder.ins().call(reset_ref, &[cp_val]);
}

/// Selects the storage for an escaping object instantiation, honoring the
/// frame's allocator tier:
///
/// * `StrBuf` — always the runtime-managed builder (`rt_strbuf_new`), never
///   a stack scalar or a raw malloc block.
/// * `@pool` — a slot from the per-call slab under the runtime counter, with
///   the capacity trap (E1406 covers provable overflow at compile time).
/// * `@arena` — a bump allocation from the frame arena, reclaimed by the
///   return-time checkpoint restore.
/// * otherwise — plain `malloc`.
pub(crate) fn emit_escaping_alloc<M: ClifModule>(
    builder: &mut FunctionBuilder,
    module: &mut M,
    runtime: &RuntimeIds,
    string_literal_map: &HashMap<String, DataId>,
    frame: &TierFrame,
    malloc_id: cranelift_module::FuncId,
    f: &Function,
    class_name: &str,
    byte_size: u32,
) -> ClifValue {
    if class_name == "StrBuf" {
        let new_ref = module.declare_func_in_func(runtime.rt_strbuf_new_id, builder.func);
        let call_inst = builder.ins().call(new_ref, &[]);
        return builder.inst_results(call_inst)[0];
    }

    if let Some((base_var, cnt_var, capacity)) = frame.pool_state
        && (byte_size as i64) <= POOL_SLOT_SIZE
    {
        // @pool: serve the object from the per-call slab under a runtime
        // slot counter; exceeding the capacity traps with a dedicated
        // message. The counter check is the runtime half of the E1406
        // contract — dynamic counts are only provable here.
        let base = builder.use_var(base_var);
        let cnt_pre = builder.use_var(cnt_var);
        let over =
            builder
                .ins()
                .icmp_imm_u(IntCC::UnsignedGreaterThanOrEqual, cnt_pre, capacity as i64);
        let trap_blk = builder.create_block();
        builder.set_cold_block(trap_blk);
        let cont_blk = builder.create_block();
        builder.ins().brif(over, trap_blk, &[], cont_blk, &[]);
        builder.switch_to_block(trap_blk);
        if let Some(data_id) = string_literal_map.get(POOL_OVERFLOW_MSG) {
            let global_val = module.declare_data_in_func(*data_id, builder.func);
            let msg_ptr = builder.ins().symbol_value(clif_types::I64, global_val);
            let panic_ref = module.declare_func_in_func(runtime.rt_panic_id, builder.func);
            builder.ins().call(panic_ref, &[msg_ptr]);
        }
        builder
            .ins()
            .trap(cranelift_codegen::ir::TrapCode::unwrap_user(1));
        builder.switch_to_block(cont_blk);
        let off = builder.ins().imul_imm_s(cnt_pre, POOL_SLOT_SIZE);
        let slot_ptr = builder.ins().iadd(base, off);
        let cnt_next = builder.ins().iadd_imm_s(cnt_pre, 1);
        builder.def_var(cnt_var, cnt_next);
        return slot_ptr;
    }

    let alloc_id = if f.alloc_hint == ArenaHint::Arena || matches!(f.alloc_hint, ArenaHint::Pool(_))
    {
        runtime.rt_arena_alloc_id
    } else {
        malloc_id
    };
    let alloc_ref = module.declare_func_in_func(alloc_id, builder.func);
    let size_val = builder.ins().iconst(clif_types::I64, byte_size as i64);
    let call_inst = builder.ins().call(alloc_ref, &[size_val]);
    builder.inst_results(call_inst)[0]
}

/// Stack-slot storage for a non-escaping object (the pre-tier path), kept
/// here so the StructInit arm reads as one allocation decision tree.
pub(crate) fn emit_stack_slot_alloc(builder: &mut FunctionBuilder, byte_size: u32) -> ClifValue {
    let slot_data = StackSlotData::new(StackSlotKind::ExplicitSlot, byte_size, 3);
    let slot = builder.create_sized_stack_slot(slot_data);
    builder.ins().stack_addr(clif_types::I64, slot, 0)
}
