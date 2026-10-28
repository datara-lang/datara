//! M6 TMM Layer 1: dataflow lifetime inference — the last-use analysis.
//!
//! PLAN_V150 M6 sub-milestone 1 (AST→dataflow-граф→решатель). This module
//! walks the lowered DMIR and decides, per uniquely-owned object, where a
//! deterministic release may be inserted: immediately after the object's
//! LAST use inside the owning function, not at scope end.
//!
//! Soundness subset for v1.5.0 (per plan: "однонаправленное владение —
//! деревья, DAG без циклов — 100% покрыто"). An object qualifies for a
//! release decision only when every one of the following holds:
//!
//! 1. It was created by a `StructInit` of a class (or an identity chain
//!    through `datara_rt_arc_init`/`datara_rt_arc_acquire`), bound to
//!    exactly one local variable by `AssignVar`.
//! 2. The variable is never re-aliased: no other variable is ever bound to
//!    a copy of it (every identifier read lowers as `LoadVar`, so aliasing
//!    is always visible as a `LoadVar` result flowing into another name's
//!    `AssignVar`).
//! 3. It never escapes: not returned, not stored as a field value, never
//!    passed as a call/method argument, never re-released manually.
//! 4. Its whole lifetime — definition and every use, including through
//!    `LoadVar` copies — lives in ONE basic block and touches no loop
//!    body (`WhileLoop` nests are treated as opaque multi-use regions).
//! 5. Function parameters never qualify (the caller owns them).
//!
//! This is an ANALYSIS pass: the emission of the release instruction is the
//! next sub-milestone (M6.3), which must also coordinate with the codegen
//! escape predicate so a released object stays heap-allocated (or is
//! promoted to a stack slot with the release skipped).

use crate::dmir::ir::{
    ArenaHint, BasicBlockId, Function, Inst, Module, Terminator, ValueId,
};
pub use super::fresh::compute_fresh_set;
use super::fresh::{
    Fnv1a, LIFETIME_CACHE, LIFETIME_CACHE_HITS, LIFETIME_CACHE_MISSES, LIFETIME_CACHE_VERSION,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering;
use std::sync::Mutex;

/// M6.3: emit the release instructions into the lowered module. Called at
/// the end of `lower_program`, when every function body (including methods
/// and specializations) is final and the ValueId space is stable.
///
/// For each sound release decision the pass inserts a plain `Inst::Call`:
/// - `@arc` objects: `datara_rt_arc_release` (decrements the strong count;
///   the runtime frees at zero). Already declared in every backend.
/// - plain objects: `datara_rt_free` (frees the malloc block). Declared in
///   the Cranelift JIT/extension tables and the LLVM path links it as a
///   runtime symbol; the LLVM backend already names `datara_rt_free`.
///
/// The analyzer only decided releases for objects whose whole lifetime is
/// one basic block, so insertion is a plain splice at `after_index + 1` in
/// that block — no CFG edits, no cross-block moves. Releases are spliced
/// in DESCENDING (block, after_index) order so each splice lands at a
/// position no prior splice has shifted (reverse-splice technique). The
/// function is then RE-VERIFIED (use-before-def etc.) so a bad decision
/// cannot silently corrupt the IR.
///
/// Escape-coordination with codegen: a released object is still heap
/// allocated by the existing escape predicate (its `Call` argument use is
/// introduced AFTER the predicate's data walk only if the pass ran after
/// it — here the release Call contains the object as an argument, so the
/// predicate must see it. To keep the released object heap-allocated, the
/// escape predicate in compile_func.rs is taught to ignore the inserted
/// release calls (they are the LAST consumer, not an escape) via the
/// `RELEASE_CALLS` name set below — mirrored there as
/// `DATAFLOW_RELEASE_CALLS`.
pub fn insert_releases(module: &mut Module) {
    // M6 escape policy: the module context (fresh-set + global names) is
    // computed ONCE from the final module state — before any splice — so
    // the caller-side creations it enables are decided on the same IR the
    // releases are spliced into.
    let ctx = super::fresh::module_ctx(module);
    let ctx_hash = ctx.hash();
    let mut fns: Vec<String> = module.functions.keys().cloned().collect();
    fns.sort();
    for name in &fns {
        let f = module.functions.get_mut(name).expect("function exists");
        let plan = analyze_function_cached_fresh(
            f,
            &module.arc_classes,
            &module.cyclic_classes,
            &ctx,
            ctx_hash,
        );
        // Splice from the END of each block backwards: an insert at
        // position p only shifts elements at >= p, so every not-yet-
        // spliced (lower) release point stays valid. Forward-order
        // splices use stale indices — each prior insert shifts the
        // original layout — and can drop a release BEFORE the object's
        // last use (use-after-free in the emitted IR; found by the M6.3
        // traits-dispatch probe: 0xC0000374 heap corruption on the LLVM
        // path, silent UB on Cranelift).
        let mut rels = plan.releases;
        rels.sort_by_key(|r| std::cmp::Reverse((r.block.0, r.after_index)));
        for rel in &rels {
            // Fresh ValueId outside the used range of the function
            // (computed from an immutable borrow first).
            let max_vid = f
                .blocks
                .iter()
                .flat_map(|b| {
                    let mut vs = Vec::new();
                    for inst in &b.instructions {
                        inst.visit_vids(&mut |v| vs.push(*v));
                    }
                    match &b.terminator {
                        Terminator::Return { value: Some(v) } => vs.push(*v),
                        Terminator::Return { value: None } => {}
                        Terminator::Branch { args, .. } => vs.extend(args.iter().copied()),
                        Terminator::CondBranch { cond, then_args, else_args, .. } => {
                            vs.push(*cond);
                            vs.extend(then_args.iter().copied());
                            vs.extend(else_args.iter().copied());
                        }
                        Terminator::Unreachable => {}
                    }
                    vs
                })
                .map(|v| v.0)
                .max()
                .unwrap_or(0);
            let dest = ValueId(max_vid + 1);
            let Some(block) = f.get_block_mut(rel.block) else {
                continue;
            };
            let insert_at = rel
                .after_index
                .checked_add(1)
                .unwrap_or(block.instructions.len());
            let insert_at = insert_at.min(block.instructions.len());
            let func_name = if rel.is_arc {
                "datara_rt_arc_release"
            } else {
                "datara_rt_free"
            };
            // Idempotent splice (M6 diff-test guarantee: 0 double-free):
            // if an injected release for this exact value already sits in
            // the block — a second pass over post-insert IR (re-analysis,
            // manual re-invocation) re-decides the same release — skip it
            // instead of splicing a duplicate free.
            let already_released = block.instructions.iter().any(|i| {
                matches!(
                    i,
                    Inst::Call { func, args, .. }
                        if (func == "datara_rt_arc_release"
                            || func == "datara_rt_free")
                            && args.len() == 1
                            && args[0] == rel.value
                )
            });
            if already_released {
                continue;
            }
            block.instructions.insert(
                insert_at,
                Inst::Call {
                    dest,
                    func: func_name.into(),
                    args: vec![rel.value],
                    ty: if rel.is_arc {
                        rel.class.clone()
                    } else {
                        "RawPtr".into()
                    },
                },
            );
        }
        // Soundness gate: the spliced function must still verify.
        if let Err(e) = crate::dmir::verifier::verify_function(f) {
            // A bad splice would corrupt codegen; roll the function back
            // to its pre-pass state instead of emitting broken code.
            // Re-analyze from scratch would loop, so rebuild by removing
            // the inserted calls: identify by (func, single arg) shape.
            let injected: [&str; 2] = ["datara_rt_arc_release", "datara_rt_free"];
            for b in f.blocks.iter_mut() {
                let mut i = 0;
                while i < b.instructions.len() {
                    if let Inst::Call { func, args, .. } = &b.instructions[i] {
                        if injected.contains(&func.as_str()) && args.len() == 1 {
                            b.instructions.remove(i);
                            continue;
                        }
                    }
                    i += 1;
                }
            }
            let _ = e; // Diagnostic plumbing lands with the M6 diff tests.
        }
    }
}

/// Why an object does NOT get a Layer 1 release decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeReason {
    /// Another variable was ever bound to a copy of it.
    Aliased,
    /// Returned from the owning function.
    Returned,
    /// Stored as a field value into another object — the container owns it.
    StoredIntoField,
    /// Passed as a call/method argument — the callee may retain it.
    PassedToCall,
    /// Used (defined or used) in more than one basic block.
    CrossBlock,
    /// Defined or used inside a `WhileLoop` nest — iteration count makes a
    /// single release point unsound for v1.
    InLoop,
    /// The user released it manually (`arc_release()`/`destroy`/`drop`).
    ManuallyReleased,
    /// Function parameter — owned by the caller.
    CallerOwned,
    /// Bound from a `LoadVar` copy (not a fresh creation) — a shared view.
    CopyBound,
    /// M6.2 auto-region: the class participates in a strong ownership cycle
    /// (A↔B field edges or a container self-loop). No sound release order
    /// exists, so the object is region-resident: it reclaims with the frame
    /// (in-frame allocation) or leaks as a heap cluster when it escapes —
    /// exactly like before, but without ever handing a region/stack pointer
    /// to `datara_rt_free`.
    CyclicStructure,
    /// M6 escape policy: stored into a module global — the global outlives
    /// the frame and owns the object (other functions read it later, so an
    /// in-frame release would dangle every one of those reads).
    GlobalStore,
}

impl EscapeReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            EscapeReason::Aliased => "aliased to another variable",
            EscapeReason::Returned => "returned from the function",
            EscapeReason::StoredIntoField => "stored into another object's field",
            EscapeReason::PassedToCall => "passed to a call that may retain it",
            EscapeReason::CrossBlock => "used across basic blocks",
            EscapeReason::InLoop => "used inside a loop body",
            EscapeReason::ManuallyReleased => "released manually by the user",
            EscapeReason::CallerOwned => "function parameter (caller-owned)",
            EscapeReason::CopyBound => "bound from a copy, not a fresh object",
            EscapeReason::CyclicStructure => {
                "cyclic ownership structure — region-resident (auto-arena)"
            }
            EscapeReason::GlobalStore => "stored into a module global — the global owns it",
        }
    }
}

/// One sound release point: insert the release AFTER instruction index
/// `after_index` of `block` (or after the block's last instruction when
/// `after_index` equals the instruction count, i.e. before the terminator).
#[derive(Debug, Clone)]
pub struct ReleaseDecision {
    pub function: String,
    pub block: BasicBlockId,
    pub after_index: usize,
    pub value: ValueId,
    pub variable: String,
    pub class: String,
    pub is_arc: bool,
}

/// A variable whose object was kept alive instead (with the reason).
#[derive(Debug, Clone)]
pub struct Keep {
    pub function: String,
    pub variable: String,
    pub class: String,
    pub reason: EscapeReason,
}

/// Layer 1 plan for a whole module.
#[derive(Debug, Default)]
pub struct LifetimePlan {
    pub releases: Vec<ReleaseDecision>,
    pub keeps: Vec<Keep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Pos {
    pub(crate) block: usize,
    pub(crate) index: usize,
}

const TERMINATOR: usize = usize::MAX;

/// Runtime calls that pass an object's identity through unchanged; a value
/// produced by such a call resolves back to its source object.
const IDENTITY_CALLS: [&str; 2] = ["datara_rt_arc_init", "datara_rt_arc_acquire"];
/// Runtime calls that CONSUME an object reference — the user released it.
/// (The pass-inserted datara_rt_arc_release/datara_rt_free are handled
/// separately in walk_insts: they are NOT uses, so re-analysis of a
/// module that already carries Layer 1 releases stays idempotent.)
const RELEASE_CALLS: [&str; 4] = [
    "arc_release",
    "datara_rt_weak_drop",
    "destroy",
    "drop",
];
/// Method names that CONSUME the receiver (user-initiated release).
const RELEASE_METHODS: [&str; 4] = ["arc_release", "weak_drop", "destroy", "drop"];

/// True when the call is one of the runtime release functions inserted by
/// [`insert_releases`] (or spelled by the user). The codegen escape
/// predicate must treat these as non-escaping final consumers, not as
/// "passed to a call that may retain it".
pub fn is_release_call(func: &str) -> bool {
    matches!(
        func,
        "datara_rt_arc_release"
            | "datara_rt_free"
            | "arc_release"
            | "datara_rt_weak_drop"
            | "destroy"
            | "drop"
    )
}
/// Parameter type names that never denote an owning object. Pub: the LLVM
/// emitter reuses the set to skip the fresh-set walk for functions whose
/// return type can never be a fresh object.
pub(crate) const PRIMITIVE_TYPES: [&str; 10] = [
    "Int", "Int8", "Int16", "Int32", "Float", "Float32", "Bool", "Str", "String", "RawPtr",
];

/// M6 escape policy: functions proven to return ownership of a FRESHLY
/// created object of the mapped class. A caller binding such a call's
/// result (`val a = mk()`) owns the object and gets a last-use release —
/// without this set, ownership returned across a call boundary was never
/// reclaimed (the leak the escape-policy item closes). Empty set = strictly
/// the pre-policy behavior (conservative: no release, leak as before, never
/// a wrong release).
pub(crate) type FreshSet = HashMap<String, String>;

/// Module-wide context the per-function lifetime analysis reads beyond the
/// function's own body: the escape-policy fresh-set and the global variable
/// names (storing an object into a global is an escape — the global owns
/// it). Both are read surfaces OUTSIDE the per-function fingerprint, so the
/// context hash is folded into every lifetime-cache key.
#[derive(Default)]
pub(crate) struct ModuleCtx {
    pub(crate) fresh: FreshSet,
    pub(crate) globals: HashSet<String>,
}

impl ModuleCtx {
    /// Deterministic FNV-1a hash of the context — the cache key must remap
    /// when ANY function's freshness changes or when the global set differs.
    fn hash(&self) -> u64 {
        let mut h = Fnv1a(0x8422_2325_cbf2_9ce4);
        let mut entries: Vec<(&String, &String)> = self.fresh.iter().collect();
        entries.sort();
        for (name, class) in entries {
            1u8.hash(&mut h);
            name.hash(&mut h);
            class.hash(&mut h);
        }
        let mut globals: Vec<&String> = self.globals.iter().collect();
        globals.sort();
        for g in globals {
            2u8.hash(&mut h);
            g.hash(&mut h);
        }
        h.finish()
    }
}

/// Per-function working sets gathered in one linear walk.
pub(crate) struct FnFacts {
    /// StructInit: value → (class, position).
    pub(crate) creations: HashMap<ValueId, (String, Pos)>,
    /// Identity calls: dest resolves to args[0].
    pub(crate) identity: HashMap<ValueId, ValueId>,
    /// LoadVar copies: dest value → source variable name.
    pub(crate) copies: HashMap<ValueId, String>,
    /// AssignVar: (name, value, position), in order.
    pub(crate) bindings: Vec<(String, ValueId, Pos)>,
    /// Value uses by kind: (position, kind).
    pub(crate) value_uses: HashMap<ValueId, Vec<(Pos, UseKind)>>,
    /// Name reads: LoadVar { name } positions.
    pub(crate) name_reads: HashMap<String, Vec<(Pos, ValueId)>>,
    /// Positions inside WhileLoop nests (cond or body).
    pub(crate) loop_positions: HashSet<(usize, usize)>,
    /// Values referenced by block terminators (except Return, handled
    /// separately) — conservatively cross-block.
    pub(crate) terminator_uses: HashMap<ValueId, Vec<Pos>>,
    /// Blocks that participate in a CFG cycle (a block is in a cycle when it
    /// is reachable from one of its own successors). Positions inside these
    /// blocks are re-executed every iteration, so linear (block, index)
    /// ordering cannot see back-edge uses: a read placed BEFORE a def in the
    /// same block is the NEXT iteration's use of that def.
    pub(crate) cfg_cycle_blocks: HashSet<usize>,
    /// GetField results: (position, dest, object). A struct field read
    /// returns a pointer INTO the parent object (nested structs are stored
    /// flattened), so every use of the result is a use of the parent — and
    /// must outlive any release of it. Found by the game_loop family: the
    /// release of a returned struct was inserted in the MIDDLE of a
    /// GetField chain (`GetField inner; free(w); GetField xs` — use after
    /// free, segfault).
    pub(crate) field_views: Vec<(Pos, ValueId, ValueId)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UseKind {
    Read,
    Returned,
    StoredValue,
    CallArg,
    ManualRelease,
    Aliasing,
}

impl FnFacts {
    pub(crate) fn new() -> Self {
        Self {
            creations: HashMap::new(),
            identity: HashMap::new(),
            copies: HashMap::new(),
            bindings: Vec::new(),
            value_uses: HashMap::new(),
            name_reads: HashMap::new(),
            loop_positions: HashSet::new(),
            terminator_uses: HashMap::new(),
            cfg_cycle_blocks: HashSet::new(),
            field_views: Vec::new(),
        }
    }

    /// Blocks reachable from `start` following terminator edges.
    fn reachable_from(blocks: &[crate::dmir::ir::BasicBlock], start: usize) -> HashSet<usize> {
        let mut seen: HashSet<usize> = HashSet::new();
        let mut stack: Vec<usize> = Vec::new();
        match &blocks[start].terminator {
            Terminator::Branch { target, .. } => stack.push(target.0),
            Terminator::CondBranch { then_block, else_block, .. } => {
                stack.push(then_block.0);
                stack.push(else_block.0);
            }
            _ => {}
        }
        while let Some(b) = stack.pop() {
            if b >= blocks.len() || !seen.insert(b) {
                continue;
            }
            match &blocks[b].terminator {
                Terminator::Branch { target, .. } => stack.push(target.0),
                Terminator::CondBranch { then_block, else_block, .. } => {
                    stack.push(then_block.0);
                    stack.push(else_block.0);
                }
                _ => {}
            }
        }
        seen
    }

    /// Compute `cfg_cycle_blocks` for one function: a block is in a cycle
    /// when it can reach itself through at least one terminator edge.
    fn compute_cfg_cycles(&mut self, blocks: &[crate::dmir::ir::BasicBlock]) {
        for b in 0..blocks.len() {
            if Self::reachable_from(blocks, b).contains(&b) {
                self.cfg_cycle_blocks.insert(b);
            }
        }
    }

    fn record_use(&mut self, v: ValueId, pos: Pos, kind: UseKind) {
        self.value_uses.entry(v).or_default().push((pos, kind));
    }

    /// Walk one instruction list; `in_loop` marks WhileLoop nests. `fresh`
    /// (M6 escape policy) lists the functions that return a freshly created
    /// object — a call to one makes its dest a fresh creation in the caller.
    pub(crate) fn walk_insts(&mut self, block: usize, insts: &[Inst], in_loop: bool, fresh: &FreshSet) {
        for (i, inst) in insts.iter().enumerate() {
            let pos = Pos { block, index: i };
            if in_loop {
                self.loop_positions.insert((block, i));
            }
            match inst {
                Inst::StructInit { dest, class_name, fields } => {
                    self.creations.insert(*dest, (class_name.clone(), pos));
                    // Field values are consumed by the container: an object
                    // built as `Outer { inner: t }` hands t's ownership to
                    // the Outer object. Without recording this, an inline-
                    // field object would look unused and get released right
                    // after its creation while the container still holds it
                    // (use-after-free; found by the game_loop showcase —
                    // 0xC0000005 on `world.transforms.xs[i]`).
                    for (_, fv) in fields {
                        self.record_use(*fv, pos, UseKind::StoredValue);
                    }
                }
                Inst::Call { dest, func, args, ty } => {
                    if IDENTITY_CALLS.contains(&func.as_str())
                        && let Some(&src) = args.first()
                    {
                        self.identity.insert(*dest, src);
                    }
                    if func == "datara_rt_free" || func == "datara_rt_arc_release" {
                        // M6.3 inserted last-use release: NOT a use. The pass
                        // runs inside lower_program, so re-analysis (inspect
                        // --memory, diff tests) sees the inserted calls;
                        // counting them as consumptions would flip the sound
                        // decision into a keep and break idempotency.
                    } else if RELEASE_CALLS.contains(&func.as_str())
                        && let Some(&src) = args.first()
                    {
                        self.record_use(src, pos, UseKind::ManualRelease);
                    } else {
                        for a in args {
                            self.record_use(*a, pos, UseKind::CallArg);
                        }
                        // M6 escape policy: a call to a function proven to
                        // return a fresh object of class `ty` TRANSFERS
                        // ownership to the caller — the dest is a fresh
                        // creation here, so the def-window mechanism gives
                        // the caller's variable a last-use release. The
                        // class must match the callee's fresh class exactly;
                        // any other class-typed call result (a borrowed
                        // return, an unknown builtin, a container get) stays
                        // a non-creation — conservative keep/leak as before,
                        // never a wrong release.
                        if !ty.is_empty()
                            && !PRIMITIVE_TYPES.contains(&ty.as_str())
                            && fresh.get(func).is_some_and(|c| c == ty)
                        {
                            self.creations.insert(*dest, (ty.clone(), pos));
                        }
                    }
                }
                Inst::MethodCall { dest: _, object, method, args, .. } => {
                    // The receiver is a borrow (view self) EXCEPT for the
                    // consuming release methods; arguments may be retained
                    // by the callee.
                    if RELEASE_METHODS.contains(&method.as_str()) {
                        self.record_use(*object, pos, UseKind::ManualRelease);
                    } else {
                        self.record_use(*object, pos, UseKind::Read);
                    }
                    for a in args {
                        self.record_use(*a, pos, UseKind::CallArg);
                    }
                }
                Inst::AssignVar { name, value } => {
                    self.record_use(*value, pos, UseKind::Aliasing);
                    self.bindings.push((name.clone(), *value, pos));
                }
                Inst::LoadVar { dest, name } => {
                    self.copies.insert(*dest, name.clone());
                    self.name_reads
                        .entry(name.clone())
                        .or_default()
                        .push((pos, *dest));
                }
                Inst::SetField { object, value, .. } => {
                    self.record_use(*object, pos, UseKind::Read);
                    self.record_use(*value, pos, UseKind::StoredValue);
                }
                Inst::Return { value } => {
                    if let Some(v) = value {
                        self.record_use(*v, pos, UseKind::Returned);
                    }
                }
                Inst::GetField { dest, object, .. } => {
                    self.record_use(*object, pos, UseKind::Read);
                    self.field_views.push((pos, *dest, *object));
                }
                Inst::WhileLoop { condition_insts, cond_val, body_insts } => {
                    self.walk_insts(block, condition_insts, true, fresh);
                    self.record_use(*cond_val, pos, UseKind::Read);
                    self.walk_insts(block, body_insts, true, fresh);
                }
                other => {
                    // Everything else: visit use vids generically (skip the
                    // def vid, which is the FIRST vid of producing insts).
                    let (defs, uses) = split_def_uses(other);
                    let _ = defs;
                    for v in uses {
                        self.record_use(v, pos, UseKind::Read);
                    }
                }
            }
        }
    }

    pub(crate) fn walk_terminator(&mut self, block: usize, term: &Terminator) {
        match term {
            Terminator::Return { value } => {
                if let Some(v) = value {
                    self.record_use(
                        *v,
                        Pos { block, index: TERMINATOR },
                        UseKind::Returned,
                    );
                }
            }
            Terminator::Branch { args, .. } => {
                for v in args {
                    self.terminator_uses
                        .entry(*v)
                        .or_default()
                        .push(Pos { block, index: TERMINATOR });
                }
            }
            Terminator::CondBranch { cond, then_args, else_args, .. } => {
                self.terminator_uses
                    .entry(*cond)
                    .or_default()
                    .push(Pos { block, index: TERMINATOR });
                for v in then_args.iter().chain(else_args) {
                    self.terminator_uses
                        .entry(*v)
                        .or_default()
                        .push(Pos { block, index: TERMINATOR });
                }
            }
            Terminator::Unreachable => {}
        }
    }
}

/// Split an instruction's vids into (defs, uses). Producing instructions
/// define their FIRST vid (dest); everything else is a use. This mirrors
/// `Inst::visit_vids` ordering.
fn split_def_uses(inst: &Inst) -> (Option<ValueId>, Vec<ValueId>) {
    let mut all = Vec::new();
    inst.visit_vids(&mut |v| all.push(*v));
    let has_dest = matches!(
        inst,
        Inst::ConstInt { .. }
            | Inst::ConstFloat { .. }
            | Inst::ConstStr { .. }
            | Inst::ConstBool { .. }
            | Inst::LoadVar { .. }
            | Inst::BinOp { .. }
            | Inst::UnOp { .. }
            | Inst::Cast { .. }
            | Inst::Call { .. }
            | Inst::MethodCall { .. }
            | Inst::StructInit { .. }
            | Inst::FormatStr { .. }
            | Inst::Decide { .. }
            | Inst::GetFuncAddr { .. }
            | Inst::Select { .. }
            | Inst::VolatileLoad { .. }
    );
    if has_dest && !all.is_empty() {
        let def = all[0];
        (Some(def), all[1..].to_vec())
    } else {
        (None, all)
    }
}

/// Resolve a value back to a fresh object creation, chasing identity calls.
/// Returns the CANONICAL creation vid, its class and creation position.
/// A `LoadVar` copy or any other producer returns None (shared view).
pub(crate) fn resolve_creation(
    v: ValueId,
    creations: &HashMap<ValueId, (String, Pos)>,
    identity: &HashMap<ValueId, ValueId>,
) -> Option<(ValueId, String, Pos)> {
    let mut cur = v;
    for _ in 0..16 {
        if let Some((class, pos)) = creations.get(&cur) {
            return Some((cur, class.clone(), *pos));
        }
        match identity.get(&cur) {
            Some(&src) => cur = src,
            None => return None,
        }
    }
    None
}

fn hash_arena_hint<H: Hasher>(hint: &ArenaHint, h: &mut H) {
    match hint {
        ArenaHint::None => 0u8.hash(h),
        ArenaHint::Arena => 1u8.hash(h),
        ArenaHint::ArenaSized(n) => {
            2u8.hash(h);
            n.hash(h);
        }
        ArenaHint::Pool(n) => {
            3u8.hash(h);
            n.hash(h);
        }
    }
}

/// Structural fingerprint of the EXACT read surface of `analyze_function`:
/// function name, allocator tier, parameter list and the full block
/// structure (block params, instructions, terminators), plus the module
/// class sets (`@arc` routing, cyclic-region set) that change decisions
/// without touching the body. Refinements, contracts and overflow sets are
/// NOT read by the lifetime analysis and are deliberately excluded. Any
/// change — a global ValueId shift included — remaps the key.
fn fn_analysis_fingerprint(
    f: &Function,
    arc_classes: &HashSet<String>,
    cyclic_classes: &HashSet<String>,
    ctx_hash: u64,
) -> u64 {
    let mut h = Fnv1a(0xcbf2_9ce4_8422_2325);
    LIFETIME_CACHE_VERSION.hash(&mut h);
    ctx_hash.hash(&mut h);
    f.name.hash(&mut h);
    hash_arena_hint(&f.alloc_hint, &mut h);
    f.params.hash(&mut h);
    for b in &f.blocks {
        b.id.hash(&mut h);
        b.params.hash(&mut h);
        b.instructions.hash(&mut h);
        b.terminator.hash(&mut h);
    }
    let mut class_names: Vec<&String> =
        arc_classes.iter().chain(cyclic_classes.iter()).collect();
    class_names.sort();
    for c in class_names {
        c.hash(&mut h);
    }
    h.finish()
}

/// Cached wrapper around [`analyze_function`] — the per-function memo of the
/// incremental lifetime cache. An unchanged function reuses its plan; only
/// changed ones are rebuilt. `DATARA_NO_LIFETIME_CACHE=1` disables the memo
/// (A/B benchmarking).
fn analyze_function_cached_fresh(
    f: &Function,
    arc_classes: &HashSet<String>,
    cyclic_classes: &HashSet<String>,
    ctx: &ModuleCtx,
    ctx_hash: u64,
) -> FnLifetimePlan {
    if std::env::var_os("DATARA_NO_LIFETIME_CACHE").is_some() {
        return analyze_function_inner(f, arc_classes, cyclic_classes, ctx);
    }
    let key = fn_analysis_fingerprint(f, arc_classes, cyclic_classes, ctx_hash);
    let cache = LIFETIME_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(hit) = guard.get(&key) {
            LIFETIME_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return hit.clone();
        }
    }
    LIFETIME_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    let plan = analyze_function_inner(f, arc_classes, cyclic_classes, ctx);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, plan.clone());
    }
    plan
}

pub fn analyze_function(
    f: &Function,
    arc_classes: &HashSet<String>,
    cyclic_classes: &HashSet<String>,
) -> FnLifetimePlan {
    // Public single-function entry point: no module context, so the M6
    // escape-policy fresh-set is empty (call results are never fresh
    // creations) and no globals are known — strictly the pre-policy
    // conservative behavior. Module-aware callers go through
    // [`analyze_module`]/[`insert_releases`], which thread the context.
    analyze_function_inner(f, arc_classes, cyclic_classes, &ModuleCtx::default())
}

fn analyze_function_inner(
    f: &Function,
    arc_classes: &HashSet<String>,
    cyclic_classes: &HashSet<String>,
    ctx: &ModuleCtx,
) -> FnLifetimePlan {
    // M6.3 soundness guard: `@arena`/`@pool` functions reclaim EVERYTHING
    // through the return-time checkpoint restore / slab reset. A release
    // inserted mid-body would hand a slab-interior pointer to the general
    // allocator (heap corruption) or double-reclaim. The tier owns every
    // object in the frame — no Layer 1 decisions there.
    if f.alloc_hint != crate::dmir::ArenaHint::None {
        return FnLifetimePlan {
            function: f.name.clone(),
            releases: Vec::new(),
            keeps: Vec::new(),
        };
    }
    let mut facts = FnFacts::new();
    for (bi, block) in f.blocks.iter().enumerate() {
        facts.walk_insts(bi, &block.instructions, false, &ctx.fresh);
        facts.walk_terminator(bi, &block.terminator);
    }
    facts.compute_cfg_cycles(&f.blocks);

    // Index bindings by variable for def windows and alias detection.
    let mut by_var: BTreeMap<String, Vec<(ValueId, Pos)>> = BTreeMap::new();
    // Def windows grouped by OBJECT (canonical creation vid): the
    // shadow-save/restore of inline calls and closure snapshots re-binds the
    // SAME value under the same variable, producing several def windows for
    // one object. The release decision is made per object, not per window.
    let mut by_obj: BTreeMap<ValueId, Vec<WindowCandidate>> = BTreeMap::new();

    // Field-view propagation: a struct GetField result points INTO the
    // parent object, so every use of the result is a use of the parent at
    // the same position. Attach each view's uses (transitively through
    // view-of-view chains) to every def of the parent VARIABLE — the
    // def-window scan then sees the full chain and places the release
    // after its true last use. Attaching to all defs (not just the
    // positionally-active one) is conservative: over-keeping is sound,
    // a missed use is a use-after-free.
    {
        // views: view vid -> parent vid (from the GetField operand, itself
        // possibly a view).
        let mut view_edges: Vec<(ValueId, ValueId)> = Vec::new();
        for &(_, dest, object) in &facts.field_views {
            view_edges.push((dest, object));
        }
        // Transitive closure: view chains (w.inner.xs — three vids).
        let mut parent_of: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
        for (dest, object) in &view_edges {
            parent_of.entry(*dest).or_default().push(*object);
        }
        // Collect uses per vid, then propagate up the chains to fixpoint.
        for _round in 0..8 {
            let mut additions: Vec<(ValueId, (Pos, UseKind))> = Vec::new();
            for (dest, parents) in &parent_of {
                if let Some(uses) = facts.value_uses.get(dest) {
                    for parent in parents {
                        for &(p, k) in uses {
                            additions.push((*parent, (p, k)));
                        }
                    }
                }
            }
            if additions.is_empty() {
                break;
            }
            let mut any_new = false;
            for (parent, uk) in additions {
                let uses = facts.value_uses.entry(parent).or_default();
                if !uses.contains(&uk) {
                    uses.push(uk);
                    any_new = true;
                }
            }
            if !any_new {
                break;
            }
        }
        // Second step: map view vids back to the parent's def-vals via the
        // variable name. The GetField operand is a LoadVar copy whose name
        // is the variable; its defs carry the object. Append propagated
        // uses to each def of that name at every def position (the window
        // scan filters by (def_pos, window_end) anyway).
        let mut per_name_uses: HashMap<String, Vec<(Pos, UseKind)>> = HashMap::new();
        for &(_pos, dest, object) in &facts.field_views {
            if let Some(name) = facts.copies.get(&object).cloned() {
                let uses = facts.value_uses.get(&dest).cloned().unwrap_or_default();
                per_name_uses.entry(name).or_default().extend(uses);
            }
        }
        for (name, extra) in per_name_uses {
            if let Some(defs) = by_var.get(&name) {
                for (def_val, _dpos) in defs {
                    let uses = facts.value_uses.entry(*def_val).or_default();
                    for uk in &extra {
                        if !uses.contains(uk) {
                            uses.push(*uk);
                        }
                    }
                }
            }
        }
    }

    let mut plan = FnLifetimePlan {
        function: f.name.clone(),
        releases: Vec::new(),
        keeps: Vec::new(),
    };

    // Parameter vids are caller-owned. Params do NOT flow through
    // AssignVar bindings, so they never appear in by_var; record them
    // directly (object-typed params only — primitives carry no lifetime).
    let param_vids: HashSet<ValueId> = f.params.iter().map(|(_, _, v)| *v).collect();
    for (pname, pty, _) in &f.params {
        if !PRIMITIVE_TYPES.contains(&pty.as_str()) {
            plan.keeps.push(Keep {
                function: f.name.clone(),
                variable: pname.clone(),
                class: pty.clone(),
                reason: EscapeReason::CallerOwned,
            });
        }
    }

    for (name, value, pos) in &facts.bindings {
        by_var.entry(name.clone()).or_default().push((*value, *pos));
    }

    // M6 escape policy: module globals are long-lived containers — an
    // object stored into one outlives this frame (other functions read it
    // later), so the storing binding is an escape for its object. The
    // lowered IR gives globals no special instruction (plain AssignVar),
    // so this name-based check is the only visibility the analysis has.
    // (A global bound from a LoadVar COPY — `g = t` — is already covered
    // by the alias pre-pass below, which marks the source def Aliased for
    // every copy-binding regardless of the target's lifetime.)
    for (name, value, pos) in &facts.bindings {
        if ctx.globals.contains(name)
            && let Some((canon, _, _)) =
                resolve_creation(*value, &facts.creations, &facts.identity)
        {
            by_obj.entry(canon).or_default().push(WindowCandidate {
                variable: name.clone(),
                escaped: Some(EscapeReason::GlobalStore),
                def_pos: *pos,
                last_use: *pos,
            });
        }
    }

    // Pre-pass: alias detection through LoadVar copies. Every identifier
    // read lowers as `LoadVar`, so `val p = q` binds p to a COPY of q's
    // current object — a ValueId equality check against bindings can NEVER
    // see it. Chase copy sources: binding (m ← copy of s) aliases the def
    // of s active at that position. Transitive chains (p←q←r) are cut at
    // the first hop per binding, and every hop marks its own source def,
    // so all transitive members end up marked.
    let mut aliased_defs: HashSet<(String, usize)> = HashSet::new();
    for (_m, cval, mpos) in &facts.bindings {
        if let Some(src) = facts.copies.get(cval).cloned() {
            if let Some(sdefs) = by_var.get(&src) {
                for (i, &(_dv, dp)) in sdefs.iter().enumerate() {
                    let wend = sdefs.get(i + 1).map(|&(_, p)| p);
                    let in_window = *mpos >= dp && wend.is_none_or(|w| *mpos < w);
                    if in_window {
                        aliased_defs.insert((src.clone(), i));
                    }
                }
            }
        }
    }

    for (name, defs) in &by_var {
        for (di, &(def_val, def_pos)) in defs.iter().enumerate() {
            if aliased_defs.contains(&(name.clone(), di)) {
                // This def's object was copied into another variable.
                if let Some((canon, _, _)) =
                    resolve_creation(def_val, &facts.creations, &facts.identity)
                {
                    by_obj.entry(canon).or_default().push(WindowCandidate {
                        variable: name.clone(),
                        escaped: Some(EscapeReason::Aliased),
                        def_pos,
                        last_use: def_pos,
                    });
                }
                continue;
            }
            let Some((canon, class, _)) = resolve_creation(def_val, &facts.creations, &facts.identity)
            else {
                // Bound from a copy or a non-object producer. Report as
                // CopyBound only when the copy's SOURCE def is a fresh
                // object creation (an object shared through a copy view);
                // plain primitive copies stay silent.
                if let Some(src) = facts.copies.get(&def_val).cloned()
                    && let Some(sdefs) = by_var.get(&src)
                {
                    let active = sdefs.iter().rev().find(|&&(_, p)| p <= def_pos);
                    if let Some(&(sval, _)) = active
                        && resolve_creation(sval, &facts.creations, &facts.identity).is_some()
                    {
                        plan.keeps.push(Keep {
                            function: f.name.clone(),
                            variable: name.clone(),
                            class: String::new(),
                            reason: EscapeReason::CopyBound,
                        });
                    }
                }
                continue;
            };
            if param_vids.contains(&def_val) {
                by_obj.entry(canon).or_default().push(WindowCandidate {
                    variable: name.clone(),
                    escaped: Some(EscapeReason::CallerOwned),
                    def_pos,
                    last_use: def_pos,
                });
                continue;
            }
            let window_end = defs
                .get(di + 1)
                .map(|&(_, p)| p)
                .unwrap_or(Pos {
                    block: usize::MAX,
                    index: usize::MAX,
                });
            let _is_arc = arc_classes.contains(&class);

            // CFG-cycle guard: a def inside a loop body re-executes every
            // iteration. Linear (block, index) order cannot see back-edge
            // uses — a read placed BEFORE this def in the same block (or in
            // an earlier-numbered block the CFG runs later, e.g. a
            // post-loop exit block) is a use of THIS def on the next
            // iteration. Found by the game_loop showcase: the re-binding
            // `w = physics_step(w, ..)` inside a nested while got a release
            // right after the assignment while the next iteration still
            // read `w` (use-after-free; flat single loops escaped it only
            // because their post-loop reads happened to linearly follow the
            // def). Keep the object for the whole frame.
            if facts.cfg_cycle_blocks.contains(&def_pos.block) {
                by_obj.entry(canon).or_default().push(WindowCandidate {
                    variable: name.clone(),
                    escaped: Some(EscapeReason::InLoop),
                    def_pos,
                    last_use: def_pos,
                });
                continue;
            }

            // Collect the object's use surface: direct uses of def_val plus
            // uses of every LoadVar copy made from this variable inside the
            // def window.
            let mut use_positions: Vec<(Pos, UseKind)> = Vec::new();
            let mut escaped: Option<EscapeReason> = None;

            let mut direct_uses: Vec<(Pos, UseKind)> = facts
                .value_uses
                .get(&def_val)
                .cloned()
                .unwrap_or_default();
            direct_uses.retain(|(p, _)| *p < window_end && *p > def_pos);
            use_positions.append(&mut direct_uses);

            // Copies of this variable within the window.
            if let Some(reads) = facts.name_reads.get(name) {
                for &(rpos, cval) in reads {
                    if rpos <= def_pos || rpos >= window_end {
                        continue;
                    }
                    if let Some(uses) = facts.value_uses.get(&cval) {
                        for &(upos, kind) in uses {
                            if upos < window_end {
                                use_positions.push((upos, kind));
                            }
                        }
                    }
                }
            }

            for &(pos, kind) in &use_positions {
                if facts.loop_positions.contains(&(pos.block, pos.index)) {
                    escaped = Some(EscapeReason::InLoop);
                    break;
                }
                if pos.block != def_pos.block {
                    escaped = Some(EscapeReason::CrossBlock);
                    break;
                }
                match kind {
                    UseKind::Read => {}
                    UseKind::Aliasing => {
                        // Direct same-value rebinding under another name
                        // (rare — copy-free aliasing). Copy-based aliasing
                        // is handled by the pre-pass above.
                        let aliases_other = facts.bindings.iter().any(|(bn, bv, _)| {
                            *bv == def_val && bn != name
                        });
                        if aliases_other {
                            escaped = Some(EscapeReason::Aliased);
                            break;
                        }
                    }
                    UseKind::Returned => {
                        escaped = Some(EscapeReason::Returned);
                        break;
                    }
                    UseKind::StoredValue => {
                        escaped = Some(EscapeReason::StoredIntoField);
                        break;
                    }
                    UseKind::CallArg => {
                        escaped = Some(EscapeReason::PassedToCall);
                        break;
                    }
                    UseKind::ManualRelease => {
                        escaped = Some(EscapeReason::ManuallyReleased);
                        break;
                    }
                }
            }

            // Block-terminator references are conservatively cross-block.
            if escaped.is_none()
                && facts
                    .terminator_uses
                    .get(&def_val)
                    .is_some_and(|ps| !ps.is_empty())
            {
                escaped = Some(EscapeReason::CrossBlock);
            }

            if let Some(reason) = escaped {
                by_obj.entry(canon).or_default().push(WindowCandidate {
                    variable: name.clone(),
                    escaped: Some(reason),
                    def_pos,
                    last_use: def_pos,
                });
                continue;
            }

            // Sound release point: after the LAST use in the def's block
            // (or right after the creation when the object is never used).
            let last_use = use_positions
                .iter()
                .filter(|(p, _)| p.block == def_pos.block)
                .map(|(p, _)| *p)
                .max()
                .unwrap_or(def_pos);
            by_obj.entry(canon).or_default().push(WindowCandidate {
                variable: name.clone(),
                escaped: None,
                def_pos,
                last_use,
            });
        }
    }

    // Close the plan per OBJECT. A release is emitted only when EVERY window
    // of the object (all re-bindings, including shadow-save/restore copies)
    // is release-sound in the SAME block; the release point is the object's
    // true last use — the last window's last use. Any escape in ANY window
    // keeps the whole object alive for the function.
    for (canon, windows) in by_obj {
        let (_, class, _) = resolve_creation(canon, &facts.creations, &facts.identity)
            .expect("window keys are canonical creation vids");
        // M6.2 auto-region: cyclic-structure objects never get a release.
        // The region owns their lifetime (frame reclaim for in-frame
        // allocation); freeing a region/stack pointer would corrupt the
        // heap, and a release ORDER inside the cycle does not exist anyway.
        if cyclic_classes.contains(&class) {
            plan.keeps.push(Keep {
                function: f.name.clone(),
                variable: windows
                    .iter()
                    .map(|w| w.variable.as_str())
                    .min()
                    .unwrap_or("?")
                    .to_string(),
                class,
                reason: EscapeReason::CyclicStructure,
            });
            continue;
        }
        let is_arc = arc_classes.contains(&class);
        let variable = windows
            .iter()
            .map(|w| w.variable.as_str())
            .min()
            .unwrap_or("?")
            .to_string();
        if windows.iter().any(|w| w.escaped.is_some()) {
            let reason = windows
                .iter()
                .find_map(|w| w.escaped)
                .expect("any() found an escape");
            plan.keeps.push(Keep {
                function: f.name.clone(),
                variable,
                class,
                reason,
            });
            continue;
        }
        // All windows must share one block (each healthy window was already
        // proven same-block); the last use of the LAST window is the object's
        // true last use because shadow copies observe the same value.
        let last_window = windows.iter().max_by_key(|w| (w.def_pos.block, w.def_pos.index));
        let Some(w) = last_window else { continue };
        if !windows.iter().all(|x| x.def_pos.block == w.def_pos.block) {
            plan.keeps.push(Keep {
                function: f.name.clone(),
                variable,
                class,
                reason: EscapeReason::CrossBlock,
            });
            continue;
        }
        plan.releases.push(ReleaseDecision {
            function: f.name.clone(),
            block: BasicBlockId(w.def_pos.block),
            after_index: if w.last_use.index == TERMINATOR {
                usize::MAX
            } else {
                w.last_use.index
            },
            value: canon,
            variable,
            class,
            is_arc,
        });
    }

    plan
}

/// One def window of an object, normalized to its canonical creation vid.
#[derive(Debug)]
struct WindowCandidate {
    variable: String,
    escaped: Option<EscapeReason>,
    def_pos: Pos,
    last_use: Pos,
}

/// Per-function plan returned by [`analyze_function`].
#[derive(Debug, Default, Clone)]
pub struct FnLifetimePlan {
    pub function: String,
    pub releases: Vec<ReleaseDecision>,
    pub keeps: Vec<Keep>,
}

pub fn analyze_module(m: &Module) -> LifetimePlan {
    let ctx = super::fresh::module_ctx(m);
    let ctx_hash = ctx.hash();
    let mut plan = LifetimePlan::default();
    let mut fns: Vec<&String> = m.functions.keys().collect();
    fns.sort();
    for name in fns {
        let f = &m.functions[name];
        let fl =
            analyze_function_cached_fresh(f, &m.arc_classes, &m.cyclic_classes, &ctx, ctx_hash);
        plan.releases.extend(fl.releases);
        plan.keeps.extend(fl.keeps);
    }
    plan.releases.sort_by(|a, b| {
        (&a.function, a.block.0, a.after_index).cmp(&(&b.function, b.block.0, b.after_index))
    });
    plan
}

/// Human-readable Layer 1 section for `forgen inspect --memory`:
/// per function, the sound release decisions and the objects kept alive
/// with their escape reason.
pub fn layer1_report(m: &Module) -> String {
    let plan = analyze_module(m);
    let mut out = String::new();
    out.push_str("\nLayer 1 dataflow decisions (last-use releases):\n");
    if plan.releases.is_empty() && plan.keeps.is_empty() {
        out.push_str("  (no owning objects found)\n");
        return out;
    }
    let mut by_fn: BTreeMap<&str, (Vec<&ReleaseDecision>, Vec<&Keep>)> = BTreeMap::new();
    for r in &plan.releases {
        by_fn.entry(r.function.as_str()).or_default().0.push(r);
    }
    for k in &plan.keeps {
        by_fn.entry(k.function.as_str()).or_default().1.push(k);
    }
    for (fname, (rels, keeps)) in by_fn {
        out.push_str(&format!("  fn {fname}:\n"));
        for r in rels {
            out.push_str(&format!(
                "    release {} ({}{}) after last use at bb{}+{}{}\n",
                r.variable,
                r.class,
                if r.is_arc { ", @arc" } else { "" },
                r.block.0,
                if r.after_index == usize::MAX { "term" } else { "" },
                if r.after_index == usize::MAX {
                    String::new()
                } else {
                    format!(" after index {}", r.after_index)
                }
            ));
        }
        for k in keeps {
            if k.class.is_empty() {
                out.push_str(&format!(
                    "    keep {}: {}\n",
                    k.variable,
                    k.reason.as_str()
                ));
            } else {
                out.push_str(&format!(
                    "    keep {} ({}): {}\n",
                    k.variable,
                    k.class,
                    k.reason.as_str()
                ));
            }
        }
    }
    out
}
