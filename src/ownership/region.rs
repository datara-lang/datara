//! M6.2 auto-region escape analysis for cyclic ownership structures
//! (PLAN_V150, TMM Layer 1 flagship).
//!
//! The Cranelift backend and the LLVM emitter both decide, per `StructInit`,
//! whether the object can live in a stack/region slot or must be promoted to
//! the heap. Their historical rules are conservative in the same direction —
//! any rebinding, field store, or call mention forces the heap — which makes
//! a cyclic structure (`A.peer: B` + `B.peer: A`, or `next: List<Node>`)
//! leak as an unreclaimable heap cluster even when the whole cycle lives and
//! dies inside one frame.
//!
//! [`compute_escape_set`] is the shared escape analysis for both backends.
//!
//! ## Why alias-flow (union-find)
//!
//! Lowering materializes every variable read as a `LoadVar { dest, name }`
//! COPY. The structural first cut missed that: `SetField` edges were read
//! off copies while the seeds (`AssignVar`) were dropped for cyclic classes,
//! so a cycle returned from a callee
//! (`fn mk() -> A { a.peer = b; b.peer = a; return a }`) had its `Return`
//! seed on the copy `v8 = LoadVar a`, which structurally never touched the
//! `StructInit` dest `v1`. The region kept the allocation in-frame, the
//! caller received a pointer into a dead frame → SEGFAULT.
//!
//! The fix: every `LoadVar { dest, name }` unions `dest` with ALL vids ever
//! bound to `name` (via `AssignVar { name, value }`), position-insensitive —
//! a conservative, path-insensitive over-approximation (no dominator
//! analysis, no SSA renaming). Loads of module globals are excluded (globals
//! are heap-resident by construction and need no alias union). Classes and
//! cyclic-ness resolve through the union-find ROOT, so `SetField` edges seen
//! on copies reach the real `StructInit` allocation sites. Seeds (Return /
//! Call / MethodCall / non-relaxed stores) then propagate through the alias
//! classes, and transitive propagation through field stores into cyclic
//! children completes the soundness argument: a heap object must never hold
//! a pointer into a dead frame.

use crate::dmir::{Function, Inst, Terminator, ValueId};
use std::collections::{HashMap, HashSet};

/// Union-find over value ids (alias classes of lowered copies).
struct Uf {
    parent: HashMap<ValueId, ValueId>,
}

impl Uf {
    fn find(&mut self, v: ValueId) -> ValueId {
        let mut root = v;
        loop {
            let p = *self.parent.entry(root).or_insert(root);
            if p == root {
                break;
            }
            root = p;
        }
        // Path compression (iterative — chains can be long in big fns).
        let mut cur = v;
        while cur != root {
            let next = *self.parent.get(&cur).unwrap_or(&root);
            self.parent.insert(cur, root);
            cur = next;
        }
        root
    }

    fn union(&mut self, a: ValueId, b: ValueId) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}

/// Computes the set of escaping value ids for `f`'s StructInit allocations.
///
/// * `cyclic_classes` — Layer-1 classes participating in an ownership cycle
///   (`tmm::cyclic_dataflow_classes`, carried on the DMIR module).
/// * `globals` — module-level variable names; a rebinding into one extends
///   the value's lifetime past the frame and always seeds the heap.
/// * `assign_var_seeds` — Cranelift historically counts every
///   `AssignVar { value }` as an escape; LLVM does not.
/// * `structinit_field_seeds` — LLVM historically counts every StructInit
///   field value as an escape; Cranelift does not.
///
/// Backend-specific historical extras (Cranelift's `return_type == class`
/// seed, LLVM's non-entry-block StructInit promotion) stay at the call
/// sites so each backend keeps its exact historical classification for
/// non-cyclic classes.
pub fn compute_escape_set(
    f: &Function,
    cyclic_classes: &HashSet<String>,
    globals: &HashSet<String>,
    assign_var_seeds: bool,
    structinit_field_seeds: bool,
) -> HashSet<ValueId> {
    let mut uf = Uf {
        parent: HashMap::new(),
    };

    // Pass 1: object classes, the field-store graph (parent → children),
    // and variable bindings (`AssignVar` value side per name).
    let mut class_of: HashMap<ValueId, String> = HashMap::new();
    let mut stored_into: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut name_vids: HashMap<&str, Vec<ValueId>> = HashMap::new();
    for b in &f.blocks {
        for inst in &b.instructions {
            match inst {
                Inst::StructInit {
                    dest,
                    class_name,
                    fields,
                } => {
                    class_of.insert(*dest, class_name.clone());
                    let entry = stored_into.entry(*dest).or_default();
                    for (_, fv) in fields {
                        entry.push(*fv);
                    }
                }
                Inst::SetField { object, value, .. } => {
                    stored_into.entry(*object).or_default().push(*value);
                }
                Inst::AssignVar { name, value } => {
                    name_vids.entry(name.as_str()).or_default().push(*value);
                }
                _ => {}
            }
        }
    }

    // Pass 2: alias unions. Every LoadVar copy joins the alias class of the
    // variable it reads — conservatively ALL vids ever bound to that name,
    // position-insensitive. Global reads are excluded (module globals are
    // heap-resident by construction).
    for b in &f.blocks {
        for inst in &b.instructions {
            if let Inst::LoadVar { dest, name } = inst {
                if globals.contains(name) {
                    continue;
                }
                if let Some(vids) = name_vids.get(name.as_str()) {
                    for v in vids {
                        uf.union(*dest, *v);
                    }
                }
            }
        }
    }

    // Class per alias-class root (built after all unions).
    let mut root_class: HashMap<ValueId, String> = HashMap::new();
    for (vid, class) in &class_of {
        root_class.insert(uf.find(*vid), class.clone());
    }
    let is_cyclic = |uf: &mut Uf, vid: ValueId| -> bool {
        root_class
            .get(&uf.find(vid))
            .is_some_and(|c| cyclic_classes.contains(c))
    };

    // Pass 3: escape seeds. Each seed is recorded on its alias-class ROOT,
    // so a seed on a LoadVar copy (e.g. the returned `LoadVar a`) reaches
    // the StructInit dest that produced the value.
    let mut escaped_roots: HashSet<ValueId> = HashSet::new();
    for b in &f.blocks {
        if let Terminator::Return { value: Some(v) } = &b.terminator {
            escaped_roots.insert(uf.find(*v));
        }
        for inst in &b.instructions {
            match inst {
                Inst::Call { args, .. } => {
                    for a in args {
                        escaped_roots.insert(uf.find(*a));
                    }
                }
                Inst::MethodCall { object, args, .. } => {
                    escaped_roots.insert(uf.find(*object));
                    for a in args {
                        escaped_roots.insert(uf.find(*a));
                    }
                }
                Inst::AssignVar { name, value } => {
                    if assign_var_seeds && !(is_cyclic(&mut uf, *value) && !globals.contains(name))
                    {
                        escaped_roots.insert(uf.find(*value));
                    }
                }
                Inst::SetField { object, value, .. } => {
                    // A store closing the cycle in-frame (both sides cyclic
                    // class objects) keeps both region-resident.
                    if !(is_cyclic(&mut uf, *value) && is_cyclic(&mut uf, *object)) {
                        escaped_roots.insert(uf.find(*value));
                    }
                }
                Inst::StructInit { dest, fields, .. } => {
                    if structinit_field_seeds {
                        let dest_cyclic = is_cyclic(&mut uf, *dest);
                        for (_, fv) in fields {
                            if !(is_cyclic(&mut uf, *fv) && dest_cyclic) {
                                escaped_roots.insert(uf.find(*fv));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Pass 4: transitive propagation — anything stored into an escaping
    // object must be heap-allocated as well, or the escaping object would
    // carry a pointer into a dead frame. Only cyclic children flip here;
    // non-cyclic children keep their historical classification.
    loop {
        let mut grew = false;
        for (parent, children) in &stored_into {
            if !escaped_roots.contains(&uf.find(*parent)) {
                continue;
            }
            for child in children {
                if is_cyclic(&mut uf, *child) && escaped_roots.insert(uf.find(*child)) {
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }

    // Pass 5: expand roots back to allocation sites (StructInit dests).
    class_of
        .keys()
        .copied()
        .filter(|v| escaped_roots.contains(&uf.find(*v)))
        .collect()
}
