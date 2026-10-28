//! M6 escape policy: the module-wide fresh-set fixpoint (functions that
//! return a freshly created object of their class). Split out of
//! dataflow.rs for the 60 KB release size gate; the lifetime solver in
//! dataflow.rs consumes `ModuleCtx::fresh`.

use crate::dmir::ir::*;
use super::dataflow::{
    FreshSet, FnLifetimePlan, ModuleCtx, PRIMITIVE_TYPES, FnFacts, Pos,
    resolve_creation,
};
use crate::dmir::ir::ValueId;
use std::hash::Hasher;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// M6 escape policy (PLAN_V150 M6 item "Escape-политика: объект, ушедший за
// границу (возвращает функция/кладётся в долго живущий контейнер) → план
// освобождения у владельца").
// ---------------------------------------------------------------------------

/// Build the module context once per analysis run: the fresh-set fixpoint
/// over every function plus the global-name set.
pub(crate) fn module_ctx(m: &Module) -> ModuleCtx {
    ModuleCtx {
        fresh: compute_fresh_set(m),
        globals: m.globals.keys().cloned().collect(),
    }
}

/// Compute the module-wide fresh-set (M6 escape policy). A function is
/// fresh when its return type is a plain class (not @arc — Layer 2 owns
/// that refcount lifetime, and the LLVM path emits no arc header on
/// StructInit, so a caller-side datara_rt_arc_release has no counter to
/// decrement; not cyclic — M6.2 regions never get caller releases either)
/// and EVERY `return <value>` in it transfers a freshly created object of
/// that class: a StructInit, an identity chain from one, or — through the
/// monotone fixpoint — a call to another fresh function. Uncertain
/// functions stay out of the set: their callers keep the pre-policy
/// conservative behavior (no release, leak as before — never a wrong
/// release).
pub fn compute_fresh_set(m: &Module) -> FreshSet {
    let mut names: Vec<&String> = m.functions.keys().collect();
    names.sort();
    let candidates: Vec<&Function> = names
        .iter()
        .filter_map(|n| m.functions.get(*n))
        .filter(|f| {
            !f.return_type.is_empty()
                && !PRIMITIVE_TYPES.contains(&f.return_type.as_str())
                && !m.arc_classes.contains(&f.return_type)
                && !m.cyclic_classes.contains(&f.return_type)
        })
        .collect();
    let mut fresh: FreshSet = HashMap::new();
    loop {
        let mut changed = false;
        for f in &candidates {
            if fresh.contains_key(&f.name) {
                continue;
            }
            let mut facts = FnFacts::new();
            for (bi, block) in f.blocks.iter().enumerate() {
                facts.walk_insts(bi, &block.instructions, false, &fresh);
                facts.walk_terminator(bi, &block.terminator);
            }
            let mut by_var: BTreeMap<String, Vec<(ValueId, Pos)>> = BTreeMap::new();
            for (name, value, pos) in &facts.bindings {
                by_var.entry(name.clone()).or_default().push((*value, *pos));
            }
            // Every return-with-value must transfer a fresh object of the
            // function's declared return class. No return at all (should be
            // unreachable for a class return type) keeps the function out.
            let mut ok = true;
            let mut seen = false;
            for block in &f.blocks {
                for inst in &block.instructions {
                    if let Inst::Return { value: Some(v) } = inst {
                        seen = true;
                        ok &= matches!(
                            resolve_return_class(*v, &facts, &by_var),
                            Some(c) if c == f.return_type
                        );
                    }
                }
                if let Terminator::Return { value: Some(v) } = &block.terminator {
                    seen = true;
                    ok &= matches!(
                        resolve_return_class(*v, &facts, &by_var),
                        Some(c) if c == f.return_type
                    );
                }
            }
            if ok && seen {
                fresh.insert(f.name.clone(), f.return_type.clone());
                changed = true;
            }
        }
        if !changed {
            return fresh;
        }
    }
}

/// Resolve a RETURNED value to the class of the fresh object it transfers,
/// for the fresh-set computation. A `return a` lowers as `LoadVar a` — the
/// returned vid is a COPY, not the creation itself, so the direct
/// [`resolve_creation`] lookup misses. Chase identity calls first, then
/// transitive LoadVar copies (`copies[v]` names the source VARIABLE, whose
/// def windows are looked up in `by_var`). Returns the class only when the
/// chain bottoms out in fresh creations of ONE class — the position-free
/// over-approximation ("all defs of the source variable are fresh") is
/// sound: if every def is a fresh creation, the def active at the return
/// position is one too.
fn resolve_return_class(
    v: ValueId,
    facts: &FnFacts,
    by_var: &BTreeMap<String, Vec<(ValueId, Pos)>>,
) -> Option<String> {
    resolve_return_class_depth(v, facts, by_var, 0)
}

fn resolve_return_class_depth(
    v: ValueId,
    facts: &FnFacts,
    by_var: &BTreeMap<String, Vec<(ValueId, Pos)>>,
    depth: usize,
) -> Option<String> {
    if depth > 16 {
        return None;
    }
    if let Some((_, class, _)) = resolve_creation(v, &facts.creations, &facts.identity) {
        return Some(class);
    }
    let src = facts.copies.get(&v)?.clone();
    let defs = by_var.get(&src)?;
    let mut class: Option<String> = None;
    for &(def_val, _) in defs {
        let c = resolve_return_class_depth(def_val, facts, by_var, depth + 1)?;
        match &class {
            Some(prev) if prev != &c => return None,
            _ => class = Some(c),
        }
    }
    class
}

// ---------------------------------------------------------------------------
// M6 incremental lifetime cache (PLAN_V150 M6 item "Инкрементальность:
// lifetime-граф кэшируется по модулям, перестройка только изменённых").
// ---------------------------------------------------------------------------
/// Version of the lifetime-cache key space. Bump when `analyze_function`
/// starts reading a `Function` field OUTSIDE the fingerprinted read surface
/// (name, alloc_hint, params, blocks) or when decision semantics change.
/// v2: the M6 escape policy added the module context (fresh-set + global
/// names) to the read surface — a caller's decisions now depend on the
/// bodies of the functions it calls, so the context hash is folded into
/// every cache key.
pub const LIFETIME_CACHE_VERSION: u64 = 2;

pub(crate) static LIFETIME_CACHE: OnceLock<Mutex<HashMap<u64, FnLifetimePlan>>> = OnceLock::new();
pub(crate) static LIFETIME_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
pub(crate) static LIFETIME_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

/// (hits, misses) of the per-function lifetime memo — used by the
/// incrementality regression tests and `forgen inspect --memory`.
pub fn lifetime_cache_stats() -> (u64, u64) {
    (
        LIFETIME_CACHE_HITS.load(Ordering::Relaxed),
        LIFETIME_CACHE_MISSES.load(Ordering::Relaxed),
    )
}

/// Drops every memoized plan (and zeroes the counters). Test/bench hook.
pub fn lifetime_cache_reset() {
    if let Some(cache) = LIFETIME_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            guard.clear();
        }
    }
    LIFETIME_CACHE_HITS.store(0, Ordering::Relaxed);
    LIFETIME_CACHE_MISSES.store(0, Ordering::Relaxed);
}

/// Deterministic FNV-1a hasher — the cache key must be reproducible, a
/// per-process `RandomState` would silently disable cross-run reuse.
pub(crate) struct Fnv1a(pub(crate) u64);

impl Hasher for Fnv1a {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        self.0 = h;
    }
}

