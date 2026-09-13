//! Hardware Data Prefetch Injection for LLVM (v1.2.4)
//!
//! Emits `@llvm.prefetch` intrinsics for sequential array/tensor accesses
//! in hot loops, fetching cache lines into L1/L2 data cache 8–16 iterations
//! before the CPU executes the load instruction.

/// Locality hint for `@llvm.prefetch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefetchLocality {
    /// No temporal locality (stream once and discard, e.g. non-temporal store).
    None = 0,
    /// Low temporal locality (keep in L3/LLC).
    Low = 1,
    /// Moderate temporal locality (keep in L2).
    Moderate = 2,
    /// Extremely high temporal locality (keep in L1 cache).
    High = 3,
}

/// Emits the declaration of `@llvm.prefetch` in LLVM IR.
pub fn emit_llvm_prefetch_declaration() -> &'static str {
    "declare void @llvm.prefetch(ptr, i32, i32, i32)\n"
}

/// Generates an LLVM IR prefetch instruction for the given address pointer.
/// `is_write`: false for read, true for write.
/// `locality`: High (3) for L1 residency.
pub fn emit_prefetch_inst(ptr_var: &str, is_write: bool, locality: PrefetchLocality) -> String {
    let rw = if is_write { 1 } else { 0 };
    let loc = locality as i32;
    format!(
        "  call void @llvm.prefetch(ptr {}, i32 {}, i32 {}, i32 1)\n",
        ptr_var, rw, loc
    )
}
