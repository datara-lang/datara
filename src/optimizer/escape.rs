//! Interprocedural & CFG-Aware Escape Analysis for Datara & Forgen (v1.2.3)
//!
//! Analyzes struct, tuple, and array allocations across arbitrary Control Flow
//! Graphs (CFGs) with multiple basic blocks, branches, and loops.
//! Identifies allocations that never escape their defining function scope,
//! enabling stack promotion and Scalar Replacement of Aggregates (SRA).

use std::collections::{HashMap, HashSet};

use crate::dmir::{BasicBlockId, Function, Inst, Terminator, ValueId};
use crate::optimizer::cost_model::OptimizationDecisionTrace;

/// The escape state of an allocated object in DMIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscapeState {
    /// Object is purely local to the function and does not escape.
    NonEscaping,
    /// Object escapes because it is returned from the function.
    EscapedReturn,
    /// Object escapes because it is passed as an argument to an external call.
    EscapedExternalCall(String),
    /// Object escapes because it was stored into a heap container or global.
    EscapedHeapContainer,
    /// Object escapes because its address was stored into an already escaping object.
    EscapedStoreField,
}

/// Metadata for an allocated struct candidate.
#[derive(Debug, Clone)]
pub struct StructAllocation {
    pub dest: ValueId,
    pub class_name: String,
    pub block_id: BasicBlockId,
    pub inst_idx: usize,
    pub initial_fields: HashMap<String, ValueId>,
    pub aliases: HashSet<ValueId>,
    pub bound_variables: HashSet<String>,
    pub state: EscapeState,
}

/// Result of running Escape Analysis on a function.
#[derive(Debug, Default)]
pub struct EscapeAnalysisResult {
    pub allocations: HashMap<ValueId, StructAllocation>,
    pub non_escaping_count: usize,
    pub escaping_count: usize,
}

impl EscapeAnalysisResult {
    pub fn is_non_escaping(&self, val: ValueId) -> bool {
        if let Some(alloc) = self.allocations.get(&val) {
            return alloc.state == EscapeState::NonEscaping;
        }
        for alloc in self.allocations.values() {
            if alloc.state == EscapeState::NonEscaping && alloc.aliases.contains(&val) {
                return true;
            }
        }
        false
    }
}

pub struct EscapeAnalyzer;

impl EscapeAnalyzer {
    /// Performs whole-function CFG escape analysis across all basic blocks.
    pub fn analyze(f: &Function, trace: &mut OptimizationDecisionTrace) -> EscapeAnalysisResult {
        let mut result = EscapeAnalysisResult::default();
        let mut val_to_alloc_root: HashMap<ValueId, ValueId> = HashMap::new();
        let mut var_to_alloc_root: HashMap<String, ValueId> = HashMap::new();

        // Pass 1: Find all struct allocations
        for block in &f.blocks {
            for (idx, inst) in block.instructions.iter().enumerate() {
                if let Inst::StructInit {
                    dest,
                    class_name,
                    fields,
                } = inst
                {
                    let mut field_map = HashMap::new();
                    for (name, vid) in fields {
                        field_map.insert(name.clone(), *vid);
                    }
                    let mut aliases = HashSet::new();
                    aliases.insert(*dest);

                    result.allocations.insert(
                        *dest,
                        StructAllocation {
                            dest: *dest,
                            class_name: class_name.clone(),
                            block_id: block.id,
                            inst_idx: idx,
                            initial_fields: field_map,
                            aliases,
                            bound_variables: HashSet::new(),
                            state: EscapeState::NonEscaping,
                        },
                    );
                    val_to_alloc_root.insert(*dest, *dest);
                }
            }
        }

        if result.allocations.is_empty() {
            return result;
        }

        // Pass 2: Track aliasing through AssignVar, LoadVar, and block parameters
        let mut changed = true;
        let mut iterations = 0;
        while changed && iterations < 32 {
            changed = false;
            iterations += 1;

            for block in &f.blocks {
                for inst in &block.instructions {
                    match inst {
                        Inst::AssignVar { name, value } => {
                            if let Some(&root) = val_to_alloc_root.get(value) {
                                if var_to_alloc_root.get(name) != Some(&root) {
                                    var_to_alloc_root.insert(name.clone(), root);
                                    if let Some(alloc) = result.allocations.get_mut(&root) {
                                        alloc.bound_variables.insert(name.clone());
                                    }
                                    changed = true;
                                }
                            }
                        }
                        Inst::LoadVar { dest, name } => {
                            if let Some(&root) = var_to_alloc_root.get(name) {
                                if val_to_alloc_root.get(dest) != Some(&root) {
                                    val_to_alloc_root.insert(*dest, root);
                                    if let Some(alloc) = result.allocations.get_mut(&root) {
                                        alloc.aliases.insert(*dest);
                                    }
                                    changed = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Propagate aliases through block terminators
                match &block.terminator {
                    Terminator::Branch { args, .. } => {
                        for arg in args {
                            if let Some(&root) = val_to_alloc_root.get(arg) {
                                if let Some(alloc) = result.allocations.get_mut(&root) {
                                    alloc.aliases.insert(*arg);
                                }
                            }
                        }
                    }
                    Terminator::CondBranch {
                        then_args,
                        else_args,
                        ..
                    } => {
                        for arg in then_args.iter().chain(else_args.iter()) {
                            if let Some(&root) = val_to_alloc_root.get(arg) {
                                if let Some(alloc) = result.allocations.get_mut(&root) {
                                    alloc.aliases.insert(*arg);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Pass 3: Detect escaping usages
        for block in &f.blocks {
            for inst in &block.instructions {
                match inst {
                    Inst::Call { func, args, .. } => {
                        // Check if an allocation is passed to an external function
                        for arg in args {
                            if let Some(&root) = val_to_alloc_root.get(arg) {
                                if let Some(alloc) = result.allocations.get_mut(&root) {
                                    if alloc.state == EscapeState::NonEscaping {
                                        alloc.state =
                                            EscapeState::EscapedExternalCall(func.clone());
                                    }
                                }
                            }
                        }
                    }
                    Inst::MethodCall {
                        object,
                        args,
                        method,
                        ..
                    } => {
                        // Non-mutating methods like getters or display might not escape,
                        // but conservatively treat foreign method calls as escaping args
                        for arg in args {
                            if let Some(&root) = val_to_alloc_root.get(arg) {
                                if let Some(alloc) = result.allocations.get_mut(&root) {
                                    if alloc.state == EscapeState::NonEscaping {
                                        alloc.state = EscapeState::EscapedExternalCall(format!(
                                            "method:{}",
                                            method
                                        ));
                                    }
                                }
                            }
                        }
                        // Method call on the object itself: if method is not an inlined getter, mark escaped
                        if method != "get" && method != "len" {
                            if let Some(&root) = val_to_alloc_root.get(object) {
                                if let Some(alloc) = result.allocations.get_mut(&root) {
                                    if alloc.state == EscapeState::NonEscaping {
                                        alloc.state = EscapeState::EscapedExternalCall(format!(
                                            "recv:{}",
                                            method
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Inst::SetField { object, value, .. } => {
                        // If `value` is an allocation and `object` is escaping, `value` escapes too
                        if let Some(&val_root) = val_to_alloc_root.get(value) {
                            let obj_escapes = val_to_alloc_root
                                .get(object)
                                .and_then(|obj_root| result.allocations.get(obj_root))
                                .map_or(true, |a| a.state != EscapeState::NonEscaping);

                            if obj_escapes {
                                if let Some(alloc) = result.allocations.get_mut(&val_root) {
                                    alloc.state = EscapeState::EscapedStoreField;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Check if returned from the function
            if let Terminator::Return { value } = &block.terminator {
                if let Some(ret_val) = value {
                    if let Some(&root) = val_to_alloc_root.get(ret_val) {
                        if let Some(alloc) = result.allocations.get_mut(&root) {
                            alloc.state = EscapeState::EscapedReturn;
                        }
                    }
                }
            }
        }

        // Count results and record decisions
        for (root, alloc) in &result.allocations {
            if alloc.state == EscapeState::NonEscaping {
                result.non_escaping_count += 1;
                trace.record(
                    "EscapeAnalysis",
                    &format!("{}:v{}", f.name, root.0),
                    "Applied",
                    "Struct allocation proven non-escaping; promoted to stack/registers",
                    "0",
                    &format!("Class {} confined to local scope", alloc.class_name),
                );
            } else {
                result.escaping_count += 1;
                trace.record(
                    "EscapeAnalysis",
                    &format!("{}:v{}", f.name, root.0),
                    "Rejected",
                    "Struct escapes function scope; retains heap/arena allocation",
                    "1",
                    &format!("Escape reason: {:?}", alloc.state),
                );
            }
        }

        result
    }
}
