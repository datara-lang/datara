//! Datara & Forgen v1.2.4 Test Suite: Polyhedral Loops, Cache Tiling, TBAA & Prefetch
//!
//! Validates:
//! 1. Polyhedral loop fusion combines adjacent loops with identical trip counts.
//! 2. Polyhedral loop interchange restructures nested loops for row-major cache access.
//! 3. Cache-aware tiling calculates optimal tile dimensions and partitions iteration space.
//! 4. LLVM TBAA and `noalias` metadata emission enables aggressive SIMD vectorization.
//! 5. Hardware `@llvm.prefetch` intrinsics are emitted for sequential memory reads.

use forgen::codegen::llvm::alias_analysis::{
    emit_tbaa_metadata_definitions, format_param_noalias, get_tbaa_tag_for_type,
};
use forgen::codegen::llvm::prefetch::{
    PrefetchLocality, emit_llvm_prefetch_declaration, emit_prefetch_inst,
};
use forgen::dmir::*;
use forgen::optimizer::cache_tiling::{CacheConfig, CacheTilingOptimizer};
use forgen::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use forgen::optimizer::polyhedral::PolyhedralOptimizer;

#[test]
fn test_v124_polyhedral_loop_fusion() {
    let mut func = Function {
        name: "process_arrays".into(),
        params: vec![("n".into(), "Int".into(), ValueId(1))],
        return_type: "Int".into(),
        entry_block: BasicBlockId(0),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            label: "entry".into(),
            instructions: vec![
                // Loop 1: i < n
                Inst::WhileLoop {
                    condition_insts: vec![],
                    cond_val: ValueId(10),
                    body_insts: vec![Inst::ConstInt {
                        dest: ValueId(11),
                        value: 42,
                    }],
                },
                // Loop 2: i < n (identical condition)
                Inst::WhileLoop {
                    condition_insts: vec![],
                    cond_val: ValueId(10),
                    body_insts: vec![Inst::ConstInt {
                        dest: ValueId(12),
                        value: 84,
                    }],
                },
            ],
            terminator: Terminator::Return {
                value: Some(ValueId(12)),
            },
            params: Vec::new(),
        }],
        ..Default::default()
    };

    let mut trace = OptimizationDecisionTrace::new();
    let fused = PolyhedralOptimizer::fuse_sequential_loops(&mut func, &mut trace);
    assert!(
        fused >= 1,
        "Must detect and fuse sequential loops over identical condition"
    );

    let has_fusion_record = trace
        .records
        .iter()
        .any(|r| r.pass == "PolyhedralLoopFusion" && r.decision == "Applied");
    assert!(
        has_fusion_record,
        "Trace must record applied polyhedral loop fusion"
    );
}

#[test]
fn test_v124_polyhedral_loop_interchange() {
    let mut func = Function {
        name: "matrix_multiply".into(),
        params: vec![("size".into(), "Int".into(), ValueId(1))],
        return_type: "Int".into(),
        entry_block: BasicBlockId(0),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            label: "entry".into(),
            instructions: vec![
                // Outer loop
                Inst::WhileLoop {
                    condition_insts: vec![],
                    cond_val: ValueId(10),
                    body_insts: vec![
                        // Nested inner loop
                        Inst::WhileLoop {
                            condition_insts: vec![],
                            cond_val: ValueId(20),
                            body_insts: vec![Inst::ConstInt {
                                dest: ValueId(21),
                                value: 1,
                            }],
                        },
                    ],
                },
            ],
            terminator: Terminator::Return { value: None },
            params: Vec::new(),
        }],
        ..Default::default()
    };

    let mut trace = OptimizationDecisionTrace::new();
    let interchanged = PolyhedralOptimizer::interchange_nested_loops(&mut func, &mut trace);
    assert!(
        interchanged >= 1,
        "Must detect and interchange nested loops"
    );

    let has_interchange_record = trace
        .records
        .iter()
        .any(|r| r.pass == "PolyhedralLoopInterchange" && r.decision == "Applied");
    assert!(
        has_interchange_record,
        "Trace must record applied polyhedral loop interchange"
    );
}

#[test]
fn test_v124_cache_tiling_computation() {
    let config = CacheConfig::default();
    assert_eq!(config.l1_data_cache_bytes, 32 * 1024);
    assert_eq!(config.cache_line_bytes, 64);

    let tile_size = CacheTilingOptimizer::compute_optimal_tile_size(&config, 8);
    assert!(
        (16..=64).contains(&tile_size),
        "Tile size must be cache-friendly: {}",
        tile_size
    );
    assert_eq!(
        tile_size % 8,
        0,
        "Tile size must align with cache line elements"
    );

    // Test loop tiling pass
    let mut func = Function {
        name: "tile_candidate".into(),
        params: Vec::new(),
        return_type: "void".into(),
        entry_block: BasicBlockId(0),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            label: "entry".into(),
            instructions: vec![Inst::WhileLoop {
                condition_insts: vec![],
                cond_val: ValueId(1),
                body_insts: vec![Inst::WhileLoop {
                    condition_insts: vec![],
                    cond_val: ValueId(2),
                    body_insts: vec![],
                }],
            }],
            terminator: Terminator::Return { value: None },
            params: Vec::new(),
        }],
        ..Default::default()
    };

    let cost_model = CostModel::new("release");
    let mut trace = OptimizationDecisionTrace::new();
    let tiled = CacheTilingOptimizer::tile_loops(&mut func, &cost_model, &mut trace);
    assert_eq!(tiled, 1, "Must tile nested 2D loop");

    let has_tiling_record = trace
        .records
        .iter()
        .any(|r| r.pass == "CacheTiling" && r.decision == "Applied");
    assert!(has_tiling_record);
}

#[test]
fn test_v124_llvm_tbaa_and_noalias() {
    // Test format_param_noalias
    let param_immut = format_param_noalias("ptr", "input_slice", true);
    assert_eq!(param_immut, "ptr noalias %input_slice");

    let param_val = format_param_noalias("i64", "count", false);
    assert_eq!(param_val, "i64 %count");

    // Test TBAA metadata emission
    let tbaa_def = emit_tbaa_metadata_definitions();
    assert!(tbaa_def.contains("!\"Datara Root TBAA\""));
    assert!(tbaa_def.contains("!\"Datara Int\""));
    assert!(tbaa_def.contains("!\"Datara Float\""));

    // Test TBAA tags
    assert_eq!(get_tbaa_tag_for_type("Int"), ", !tbaa !6");
    assert_eq!(get_tbaa_tag_for_type("Float"), ", !tbaa !7");
    assert_eq!(get_tbaa_tag_for_type("List"), ", !tbaa !8");
    assert_eq!(get_tbaa_tag_for_type("Unknown"), "");
}

#[test]
fn test_v124_llvm_prefetch_intrinsic() {
    let decl = emit_llvm_prefetch_declaration();
    assert_eq!(decl, "declare void @llvm.prefetch(ptr, i32, i32, i32)\n");

    let prefetch_read = emit_prefetch_inst("%addr", false, PrefetchLocality::High);
    assert_eq!(
        prefetch_read,
        "  call void @llvm.prefetch(ptr %addr, i32 0, i32 3, i32 1)\n"
    );

    let prefetch_write = emit_prefetch_inst("%dest_ptr", true, PrefetchLocality::Moderate);
    assert_eq!(
        prefetch_write,
        "  call void @llvm.prefetch(ptr %dest_ptr, i32 1, i32 2, i32 1)\n"
    );
}
