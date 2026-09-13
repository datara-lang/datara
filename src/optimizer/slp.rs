//! Superword-Level Parallelism (SLP) Vectorization Optimizer
//!
//! Scans basic blocks for isomorphic scalar BinOp operations on independent values,
//! combining them into 128-bit vector instructions (f32x4_add, f32x4_mul, i32x4_add, etc.).
//! This maximizes instruction throughput in Cranelift JIT, LLVM, and WebAssembly pipelines.

use crate::dmir::{Function, Inst, ValueId};
use crate::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use std::collections::HashSet;

pub struct SLPOptimizer;

impl SLPOptimizer {
    /// Computes the maximum ValueId across all blocks, instructions, and parameters.
    pub fn compute_max_vid(f: &Function) -> usize {
        let mut max_vid = 0;
        for param in &f.params {
            max_vid = max_vid.max(param.2.0);
        }
        for block in &f.blocks {
            for param in &block.params {
                max_vid = max_vid.max(param.val.0);
            }
            for inst in &block.instructions {
                Self::for_each_inst_vid(inst, |v| {
                    max_vid = max_vid.max(v.0);
                });
            }
        }
        max_vid
    }

    fn for_each_inst_vid<F: FnMut(ValueId)>(inst: &Inst, mut f: F) {
        match inst {
            Inst::BinOp {
                dest, left, right, ..
            } => {
                f(*dest);
                f(*left);
                f(*right);
            }
            Inst::UnOp { dest, operand, .. } => {
                f(*dest);
                f(*operand);
            }
            Inst::ConstInt { dest, .. }
            | Inst::ConstFloat { dest, .. }
            | Inst::ConstBool { dest, .. }
            | Inst::ConstStr { dest, .. }
            | Inst::LoadVar { dest, .. } => {
                f(*dest);
            }
            Inst::AssignVar { value, .. } => {
                f(*value);
            }
            Inst::Call { dest, args, .. } => {
                f(*dest);
                for a in args {
                    f(*a);
                }
            }
            Inst::MethodCall {
                dest, object, args, ..
            } => {
                f(*dest);
                f(*object);
                for a in args {
                    f(*a);
                }
            }
            Inst::GetField { dest, object, .. } => {
                f(*dest);
                f(*object);
            }
            Inst::SetField { object, value, .. } => {
                f(*object);
                f(*value);
            }
            Inst::StructInit { dest, fields, .. } => {
                f(*dest);
                for (_, val) in fields {
                    f(*val);
                }
            }
            Inst::FormatStr { dest, values, .. } => {
                f(*dest);
                for val in values {
                    f(*val);
                }
            }
            Inst::Decide {
                dest,
                arms,
                else_val,
                ..
            } => {
                f(*dest);
                for (cond, val) in arms {
                    f(*cond);
                    f(*val);
                }
                if let Some(ev) = else_val {
                    f(*ev);
                }
            }
            _ => {}
        }
    }

    /// Performs SLP vectorization on basic blocks with isomorphic scalar BinOps.
    pub fn vectorize(
        f: &mut Function,
        _cost_model: &CostModel,
        trace: &mut OptimizationDecisionTrace,
    ) -> usize {
        let mut max_vid = Self::compute_max_vid(f);
        let mut total_vectorized = 0;

        for block in &mut f.blocks {
            let mut new_instructions = Vec::with_capacity(block.instructions.len());
            let mut i = 0;

            while i < block.instructions.len() {
                // Check if we have 4 consecutive BinOps with the same op and matching type
                if i + 3 < block.instructions.len() {
                    let quad = (
                        &block.instructions[i],
                        &block.instructions[i + 1],
                        &block.instructions[i + 2],
                        &block.instructions[i + 3],
                    );

                    if let (
                        Inst::BinOp {
                            dest: d0,
                            op: op0,
                            left: l0,
                            right: r0,
                            ty: ty0,
                        },
                        Inst::BinOp {
                            dest: d1,
                            op: op1,
                            left: l1,
                            right: r1,
                            ty: ty1,
                        },
                        Inst::BinOp {
                            dest: d2,
                            op: op2,
                            left: l2,
                            right: r2,
                            ty: ty2,
                        },
                        Inst::BinOp {
                            dest: d3,
                            op: op3,
                            left: l3,
                            right: r3,
                            ty: ty3,
                        },
                    ) = quad
                    {
                        let is_float = ty0 == "Float" || ty0 == "Float32" || ty0 == "f32";
                        let is_int = ty0 == "Int32" || ty0 == "i32";

                        let same_op = op0 == op1
                            && op0 == op2
                            && op0 == op3
                            && (op0 == "+" || op0 == "-" || op0 == "*");
                        let same_ty =
                            ty0 == ty1 && ty0 == ty2 && ty0 == ty3 && (is_float || is_int);

                        // Ensure all destinations are distinct
                        let distinct_dests =
                            d0 != d1 && d0 != d2 && d0 != d3 && d1 != d2 && d1 != d3 && d2 != d3;

                        // Ensure no intra-quad data dependencies
                        let dest_set: HashSet<ValueId> = [*d0, *d1, *d2, *d3].into_iter().collect();
                        let no_intra_deps = ![*l1, *r1].contains(d0)
                            && ![*l2, *r2].contains(d0)
                            && ![*l2, *r2].contains(d1)
                            && ![*l3, *r3].contains(d0)
                            && ![*l3, *r3].contains(d1)
                            && ![*l3, *r3].contains(d2);

                        if same_op
                            && same_ty
                            && distinct_dests
                            && no_intra_deps
                            && dest_set.len() == 4
                        {
                            // Pack 4 scalar operations into 128-bit SIMD vector instructions
                            max_vid += 1;
                            let vec_l = ValueId(max_vid);
                            max_vid += 1;
                            let vec_r = ValueId(max_vid);
                            max_vid += 1;
                            let vec_res = ValueId(max_vid);

                            let (ctor, vec_func, ext_prefix, vec_ty) = if is_float {
                                let func = match op0.as_str() {
                                    "-" => "f32x4_sub",
                                    "*" => "f32x4_mul",
                                    _ => "f32x4_add",
                                };
                                ("float4", func, "float4", "Float4")
                            } else {
                                let func = match op0.as_str() {
                                    "-" => "i32x4_sub",
                                    "*" => "i32x4_mul",
                                    _ => "i32x4_add",
                                };
                                ("int4", func, "int4", "Int4")
                            };

                            // Construct left vector: [l0, l1, l2, l3]
                            new_instructions.push(Inst::Call {
                                dest: vec_l,
                                func: ctor.to_string(),
                                args: vec![*l0, *l1, *l2, *l3],
                                ty: vec_ty.to_string(),
                            });

                            // Construct right vector: [r0, r1, r2, r3]
                            new_instructions.push(Inst::Call {
                                dest: vec_r,
                                func: ctor.to_string(),
                                args: vec![*r0, *r1, *r2, *r3],
                                ty: vec_ty.to_string(),
                            });

                            // Vector SIMD operation: vec_l OP vec_r
                            new_instructions.push(Inst::Call {
                                dest: vec_res,
                                func: vec_func.to_string(),
                                args: vec![vec_l, vec_r],
                                ty: vec_ty.to_string(),
                            });

                            // Extract lanes back to scalar destinations
                            let suffixes = ["_x", "_y", "_z", "_w"];
                            let dests = [*d0, *d1, *d2, *d3];
                            for (lane, d) in dests.iter().enumerate() {
                                new_instructions.push(Inst::Call {
                                    dest: *d,
                                    func: format!("{}{}", ext_prefix, suffixes[lane]),
                                    args: vec![vec_res],
                                    ty: ty0.clone(),
                                });
                            }

                            trace.record(
                                "SLPVectorization",
                                &format!("{}:bb{}_slp4", f.name, block.id.0),
                                "Applied",
                                &format!(
                                    "Packed 4x isomorphic {} scalar operations into 128-bit SIMD {}",
                                    op0, vec_func
                                ),
                                "1 SIMD vector instruction replaces 4 scalar ALUs",
                                "Maximized XMM execution port throughput and instruction-level parallelism",
                            );

                            total_vectorized += 1;
                            i += 4;
                            continue;
                        }
                    }
                }

                // If not vectorized, keep original instruction
                new_instructions.push(block.instructions[i].clone());
                i += 1;
            }

            block.instructions = new_instructions;
        }

        total_vectorized
    }
}
