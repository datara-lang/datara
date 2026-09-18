//! Scalar Replacement of Aggregates (SRA) for Datara & Forgen (v1.2.3)
//!
//! Replaces non-escaping struct and aggregate allocations with discrete
//! scalar local variables. Bypasses heap and stack RAM entirely, enabling
//! Cranelift and LLVM to map struct fields directly to CPU registers
//! (RAX, RDX, XMM0-15).

use std::collections::HashMap;

use crate::dmir::{Function, Inst, ValueId};
use crate::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use crate::optimizer::escape::{EscapeAnalyzer, EscapeState};

pub struct SraOptimizer;

impl SraOptimizer {
    /// Runs SRA across the function's CFG using Escape Analysis results.
    pub fn scalarize(
        f: &mut Function,
        _cost_model: &CostModel,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let escape_res = EscapeAnalyzer::analyze(f, trace);
        if escape_res.non_escaping_count == 0 {
            return 0;
        }

        let mut eliminated_allocs = 0;
        let mut struct_field_aliases: HashMap<ValueId, HashMap<String, ValueId>> = HashMap::new();
        let mut non_escaping_vids = HashMap::new();

        for (vid, alloc) in &escape_res.allocations {
            if alloc.state == EscapeState::NonEscaping {
                non_escaping_vids.insert(*vid, alloc.clone());
                struct_field_aliases.insert(*vid, alloc.initial_fields.clone());
            }
        }

        // Map all alias VIDs to their root allocation
        let mut vid_to_root: HashMap<ValueId, ValueId> = HashMap::new();
        for (root, alloc) in &non_escaping_vids {
            for alias in &alloc.aliases {
                vid_to_root.insert(*alias, *root);
            }
        }

        // Transform instructions block-by-block
        for block in &mut f.blocks {
            let mut new_instructions = Vec::with_capacity(block.instructions.len());

            for inst in block.instructions.drain(..) {
                match inst {
                    Inst::StructInit {
                        dest,
                        ref class_name,
                        ref fields,
                    } => {
                        if let Some(alloc) = non_escaping_vids.get(&dest) {
                            eliminated_allocs += 1;
                            trace.record(
                                "SRA",
                                &format!("{}:v{}", f.name, dest.0),
                                "Applied",
                                "Eliminated heap/stack allocation of struct",
                                "0",
                                &format!(
                                    "Decomposed {} fields of {} into scalars",
                                    fields.len(),
                                    class_name
                                ),
                            );

                            // Decompose initial fields into scalar assignments
                            for (field_name, field_vid) in fields {
                                let scalar_var_name = format!("__sra_v{}_{}", dest.0, field_name);
                                new_instructions.push(Inst::AssignVar {
                                    name: scalar_var_name,
                                    value: *field_vid,
                                });
                            }

                            // If this struct was bound to variable names, assign fields for them too
                            for var_name in &alloc.bound_variables {
                                for (field_name, field_vid) in fields {
                                    let var_field_name = format!("{}${}", var_name, field_name);
                                    new_instructions.push(Inst::AssignVar {
                                        name: var_field_name,
                                        value: *field_vid,
                                    });
                                }
                            }
                        } else {
                            new_instructions.push(inst);
                        }
                    }

                    Inst::SetField {
                        object,
                        field,
                        value,
                    } => {
                        if let Some(&root) = vid_to_root.get(&object) {
                            let scalar_var_name = format!("__sra_v{}_{}", root.0, field);
                            new_instructions.push(Inst::AssignVar {
                                name: scalar_var_name,
                                value,
                            });

                            if let Some(alloc) = non_escaping_vids.get(&root) {
                                for var_name in &alloc.bound_variables {
                                    let var_field_name = format!("{}${}", var_name, field);
                                    new_instructions.push(Inst::AssignVar {
                                        name: var_field_name,
                                        value,
                                    });
                                }
                            }
                        } else {
                            new_instructions.push(Inst::SetField {
                                object,
                                field,
                                value,
                            });
                        }
                    }

                    Inst::GetField {
                        dest,
                        object,
                        field,
                        ty,
                    } => {
                        if let Some(&root) = vid_to_root.get(&object) {
                            let scalar_var_name = format!("__sra_v{}_{}", root.0, field);
                            new_instructions.push(Inst::LoadVar {
                                dest,
                                name: scalar_var_name,
                            });
                        } else {
                            new_instructions.push(Inst::GetField {
                                dest,
                                object,
                                field,
                                ty,
                            });
                        }
                    }

                    Inst::AssignVar { value, .. } => {
                        if non_escaping_vids.contains_key(&value)
                            || vid_to_root.contains_key(&value)
                        {
                            // Struct allocation was eliminated; whole-struct variable assignment is superseded
                            continue;
                        }
                        new_instructions.push(inst);
                    }

                    Inst::LoadVar { dest, .. } => {
                        if vid_to_root.contains_key(&dest) {
                            // Loading the whole-struct pointer that was eliminated into scalar fields
                            continue;
                        }
                        new_instructions.push(inst);
                    }

                    Inst::UnOp { dest, ref op, operand, .. } if op == "copy" => {
                        if let Some(&root) = vid_to_root.get(&operand) {
                            vid_to_root.insert(dest, root);
                            continue;
                        }
                        new_instructions.push(inst);
                    }

                    _ => {
                        new_instructions.push(inst);
                    }
                }
            }

            block.instructions = new_instructions;
        }

        eliminated_allocs
    }
}
