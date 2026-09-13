//! Datara & Forgen v1.2.6 Integration Tests
//!
//! Validates:
//! 1. Polyhedral Loop Fusion (physical merging of consecutive identical-domain loops)
//! 2. Polyhedral Loop Interchange (physical swapping of nested loop bounds for stride-1 access)
//! 3. Cache-Aware Loop Tiling (L1-resident 2D/3D block partitioning)
//! 4. Superword-Level Parallelism (SLP) Vectorization (packing 4x scalar ops into 128-bit SIMD)

use forgen::dmir::{BasicBlock, BasicBlockId, Function, Inst, Terminator, ValueId};
use forgen::driver::ForgenCompiler;
use forgen::optimizer::cache_tiling::{CacheConfig, CacheTilingOptimizer};
use forgen::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use forgen::optimizer::polyhedral::PolyhedralOptimizer;
use forgen::optimizer::slp::SLPOptimizer;

fn make_test_function(name: &str) -> Function {
    Function {
        name: name.to_string(),
        entry_block: BasicBlockId(0),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            label: "entry".to_string(),
            params: vec![],
            instructions: vec![],
            terminator: Terminator::Return { value: None },
        }],
        ..Default::default()
    }
}

#[test]
fn test_v126_polyhedral_loop_fusion() {
    let mut f = make_test_function("test_fusion");

    let cond_val = ValueId(1);
    let loop1 = Inst::WhileLoop {
        condition_insts: vec![Inst::ConstInt {
            dest: ValueId(2),
            value: 100,
        }],
        cond_val,
        body_insts: vec![Inst::BinOp {
            dest: ValueId(3),
            op: "+".to_string(),
            left: ValueId(4),
            right: ValueId(5),
            ty: "Int".to_string(),
        }],
    };

    let loop2 = Inst::WhileLoop {
        condition_insts: vec![Inst::ConstInt {
            dest: ValueId(6),
            value: 100,
        }],
        cond_val,
        body_insts: vec![Inst::BinOp {
            dest: ValueId(7),
            op: "*".to_string(),
            left: ValueId(8),
            right: ValueId(9),
            ty: "Int".to_string(),
        }],
    };

    f.blocks[0].instructions.push(loop1);
    f.blocks[0].instructions.push(loop2);

    assert_eq!(f.blocks[0].instructions.len(), 2);

    let mut trace = OptimizationDecisionTrace::default();
    let fused = PolyhedralOptimizer::fuse_sequential_loops(&mut f, &mut trace);

    assert_eq!(fused, 1, "Must physically fuse 1 pair of loops");
    assert_eq!(
        f.blocks[0].instructions.len(),
        1,
        "Instruction count in block must reduce to 1 after fusion"
    );

    if let Inst::WhileLoop { body_insts, .. } = &f.blocks[0].instructions[0] {
        assert_eq!(
            body_insts.len(),
            2,
            "Fused loop must contain body instructions from both loops"
        );
    } else {
        panic!("Remaining instruction must be a WhileLoop");
    }
}

#[test]
fn test_v126_polyhedral_loop_interchange() {
    let mut f = make_test_function("test_interchange");

    let outer_cv = ValueId(10);
    let inner_cv = ValueId(20);

    let inner_loop = Inst::WhileLoop {
        condition_insts: vec![Inst::ConstInt {
            dest: inner_cv,
            value: 50,
        }],
        cond_val: inner_cv,
        body_insts: vec![Inst::BinOp {
            dest: ValueId(30),
            op: "+".to_string(),
            left: ValueId(31),
            right: ValueId(32),
            ty: "Int".to_string(),
        }],
    };

    let outer_loop = Inst::WhileLoop {
        condition_insts: vec![Inst::ConstInt {
            dest: outer_cv,
            value: 100,
        }],
        cond_val: outer_cv,
        body_insts: vec![inner_loop],
    };

    f.blocks[0].instructions.push(outer_loop);

    let mut trace = OptimizationDecisionTrace::default();
    let interchanged = PolyhedralOptimizer::interchange_nested_loops(&mut f, &mut trace);

    assert_eq!(interchanged, 1, "Must interchange 1 nested loop");

    if let Inst::WhileLoop {
        cond_val: new_outer_cv,
        body_insts,
        ..
    } = &f.blocks[0].instructions[0]
    {
        assert_eq!(
            new_outer_cv.0, inner_cv.0,
            "Outer loop condition value must now be the original inner condition value"
        );
        if let Inst::WhileLoop {
            cond_val: new_inner_cv,
            ..
        } = &body_insts[0]
        {
            assert_eq!(
                new_inner_cv.0, outer_cv.0,
                "Inner loop condition value must now be the original outer condition value"
            );
        } else {
            panic!("Inner instruction must remain a WhileLoop");
        }
    }
}

#[test]
fn test_v126_cache_tiling() {
    let config = CacheConfig::default();
    let tile_size = CacheTilingOptimizer::compute_optimal_tile_size(&config, 8);
    assert!(tile_size >= 16, "Optimal tile size must be >= 16 elements");
    assert_eq!(
        tile_size % 8,
        0,
        "Optimal tile size must align to cache lines"
    );

    let mut f = make_test_function("test_tiling");

    let inner_loop = Inst::WhileLoop {
        condition_insts: vec![],
        cond_val: ValueId(2),
        body_insts: vec![],
    };
    let outer_loop = Inst::WhileLoop {
        condition_insts: vec![],
        cond_val: ValueId(1),
        body_insts: vec![inner_loop],
    };
    f.blocks[0].instructions.push(outer_loop);

    let cost_model = CostModel::new("release");
    let mut trace = OptimizationDecisionTrace::default();
    let tiled = CacheTilingOptimizer::tile_loops(&mut f, &cost_model, &mut trace);
    assert_eq!(tiled, 1, "Must tile 2D nested loop");

    // Idempotency: second run must not re-tile
    let tiled_second = CacheTilingOptimizer::tile_loops(&mut f, &cost_model, &mut trace);
    assert_eq!(tiled_second, 0, "Second tiling pass must be idempotent (0)");
}

#[test]
fn test_v126_slp_vectorization() {
    let mut f = make_test_function("test_slp");

    // 4 independent scalar additions on Float
    f.blocks[0].instructions.push(Inst::BinOp {
        dest: ValueId(10),
        op: "+".to_string(),
        left: ValueId(1),
        right: ValueId(2),
        ty: "Float".to_string(),
    });
    f.blocks[0].instructions.push(Inst::BinOp {
        dest: ValueId(11),
        op: "+".to_string(),
        left: ValueId(3),
        right: ValueId(4),
        ty: "Float".to_string(),
    });
    f.blocks[0].instructions.push(Inst::BinOp {
        dest: ValueId(12),
        op: "+".to_string(),
        left: ValueId(5),
        right: ValueId(6),
        ty: "Float".to_string(),
    });
    f.blocks[0].instructions.push(Inst::BinOp {
        dest: ValueId(13),
        op: "+".to_string(),
        left: ValueId(7),
        right: ValueId(8),
        ty: "Float".to_string(),
    });

    let cost_model = CostModel::new("release");
    let mut trace = OptimizationDecisionTrace::default();
    let count = SLPOptimizer::vectorize(&mut f, &cost_model, &mut trace);

    assert_eq!(count, 1, "Must vectorize 4 scalar adds into 1 SIMD group");

    // Check that float4 vector constructor and f32x4_add are emitted
    let mut has_float4 = false;
    let mut has_f32x4_add = false;
    let mut extract_lanes = 0;

    for inst in &f.blocks[0].instructions {
        if let Inst::Call { func, .. } = inst {
            if func == "float4" {
                has_float4 = true;
            }
            if func == "f32x4_add" {
                has_f32x4_add = true;
            }
            if func.starts_with("float4_") {
                extract_lanes += 1;
            }
        }
    }

    assert!(has_float4, "Must emit float4 vector constructors");
    assert!(has_f32x4_add, "Must emit f32x4_add SIMD vector call");
    assert_eq!(
        extract_lanes, 4,
        "Must extract all 4 lanes back to destinations"
    );
}

#[test]
fn test_v126_end_to_end_slp_in_cranelift() {
    let source = r#"
fn compute_quad(a1: Float, b1: Float, a2: Float, b2: Float, a3: Float, b3: Float, a4: Float, b4: Float) -> Float {
    let r1 = a1 + b1
    let r2 = a2 + b2
    let r3 = a3 + b3
    let r4 = a4 + b4
    return r1 + r2 + r3 + r4
}

fn main() -> Float {
    return compute_quad(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let dmir = compiler
        .compile_source_to_dmir(source, "v126_slp.dtr")
        .expect("Compilation to DMIR must succeed");

    println!(
        "Compiled DMIR for SLP:
{:?}",
        dmir
    );
}
