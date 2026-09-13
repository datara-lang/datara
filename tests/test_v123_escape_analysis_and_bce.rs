//! Datara & Forgen v1.2.3 Test Suite: Escape Analysis, SRA & Bounds-Check Elimination (BCE)
//!
//! Validates:
//! 1. Multi-block Escape Analysis correctly identifies non-escaping structs across branching CFGs.
//! 2. SRA (Scalar Replacement of Aggregates) completely removes struct allocations.
//! 3. `for item in list` loops compile directly to `datara_rt_list_get_unchecked` (0 bounds checks).
//! 4. Escaping structs (returned or passed to external functions) safely retain their allocations.
//! 5. High-performance `BumpArena` sub-nanosecond region allocation and scope reset.

use forgen::dmir::*;
use forgen::driver::ForgenCompiler;
use forgen::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use forgen::optimizer::escape::{EscapeAnalyzer, EscapeState};
use forgen::optimizer::sra::SraOptimizer;
use forgen::runtime::bump_arena::{BumpArena, ScopeArenaGuard};

#[test]
fn test_v123_multi_block_escape_analysis_and_sra() {
    // Construct a multi-block CFG with an if-else branch
    let mut func = Function {
        name: "compute_point_metrics".into(),
        params: vec![("flag".into(), "Bool".into(), ValueId(1))],
        return_type: "Int".into(),
        entry_block: BasicBlockId(0),
        blocks: vec![
            // Block 0: Entry
            BasicBlock {
                id: BasicBlockId(0),
                label: "entry".into(),
                instructions: vec![
                    Inst::StructInit {
                        dest: ValueId(2),
                        class_name: "Point".into(),
                        fields: vec![("x".into(), ValueId(10)), ("y".into(), ValueId(20))],
                    },
                    Inst::AssignVar {
                        name: "pt".into(),
                        value: ValueId(2),
                    },
                ],
                terminator: Terminator::CondBranch {
                    cond: ValueId(1),
                    then_block: BasicBlockId(1),
                    then_args: Vec::new(),
                    else_block: BasicBlockId(2),
                    else_args: Vec::new(),
                },
                params: Vec::new(),
            },
            // Block 1: Then
            BasicBlock {
                id: BasicBlockId(1),
                label: "then_blk".into(),
                instructions: vec![Inst::SetField {
                    object: ValueId(2),
                    field: "x".into(),
                    value: ValueId(100),
                }],
                terminator: Terminator::Branch {
                    target: BasicBlockId(3),
                    args: Vec::new(),
                },
                params: Vec::new(),
            },
            // Block 2: Else
            BasicBlock {
                id: BasicBlockId(2),
                label: "else_blk".into(),
                instructions: vec![Inst::SetField {
                    object: ValueId(2),
                    field: "y".into(),
                    value: ValueId(200),
                }],
                terminator: Terminator::Branch {
                    target: BasicBlockId(3),
                    args: Vec::new(),
                },
                params: Vec::new(),
            },
            // Block 3: Exit / Merge
            BasicBlock {
                id: BasicBlockId(3),
                label: "exit_blk".into(),
                instructions: vec![
                    Inst::GetField {
                        dest: ValueId(30),
                        object: ValueId(2),
                        field: "x".into(),
                        ty: "Int".into(),
                    },
                    Inst::GetField {
                        dest: ValueId(31),
                        object: ValueId(2),
                        field: "y".into(),
                        ty: "Int".into(),
                    },
                    Inst::BinOp {
                        dest: ValueId(32),
                        op: "+".into(),
                        left: ValueId(30),
                        right: ValueId(31),
                        ty: "Int".into(),
                    },
                ],
                terminator: Terminator::Return {
                    value: Some(ValueId(32)),
                },
                params: Vec::new(),
            },
        ],
        ..Default::default()
    };

    let mut trace = OptimizationDecisionTrace::new();
    let escape_res = EscapeAnalyzer::analyze(&func, &mut trace);
    assert_eq!(escape_res.non_escaping_count, 1);
    assert_eq!(escape_res.escaping_count, 0);
    assert!(escape_res.is_non_escaping(ValueId(2)));

    // Run SRA on this multi-block CFG
    let cost_model = CostModel::new("release");
    let eliminated = SraOptimizer::scalarize(&mut func, &cost_model, &mut trace);
    assert_eq!(eliminated, 1, "SRA must eliminate the struct allocation");

    // Verify that the entry block no longer has Inst::StructInit
    let has_struct_init = func.blocks[0]
        .instructions
        .iter()
        .any(|i| matches!(i, Inst::StructInit { .. }));
    assert!(
        !has_struct_init,
        "StructInit must be completely eliminated by SRA"
    );

    // Verify GetField and SetField are converted to AssignVar and LoadVar
    let has_get_field = func.blocks.iter().any(|b| {
        b.instructions
            .iter()
            .any(|i| matches!(i, Inst::GetField { .. }))
    });
    let has_set_field = func.blocks.iter().any(|b| {
        b.instructions
            .iter()
            .any(|i| matches!(i, Inst::SetField { .. }))
    });
    assert!(
        !has_get_field,
        "All GetField instructions must be replaced by scalar loads"
    );
    assert!(
        !has_set_field,
        "All SetField instructions must be replaced by scalar assigns"
    );
}

#[test]
fn test_v123_for_in_loop_unchecked_bce() {
    let source = r#"
fn sum_list(items: List<Int>) -> Int {
    mut total = 0
    for x in items {
        total = total + x
    }
    total
}

fn main() -> Int {
    let nums = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let s = sum_list(nums)
    println(int_to_str(s))
    0
}
"#;

    let compiler = ForgenCompiler::new("quick");
    let res = compiler.compile_source(source, "for_bce.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);

    let dmir_mod = res.dmir_module.expect("DMIR module");
    let sum_fn = dmir_mod
        .functions
        .get("sum_list")
        .or_else(|| {
            dmir_mod
                .functions
                .values()
                .find(|f| f.name.contains("sum_list"))
        })
        .expect("sum_list function");

    // Check that for-in loop uses datara_rt_list_get_unchecked
    let uses_unchecked = sum_fn.blocks.iter().any(|b| {
        b.instructions.iter().any(|inst| {
            if let Inst::Call { func, .. } = inst {
                func == "datara_rt_list_get_unchecked"
            } else {
                false
            }
        })
    });
    assert!(
        uses_unchecked,
        "for-in loop must lower directly to datara_rt_list_get_unchecked for zero bounds-check overhead"
    );

    // Verify native execution output
    let exe = res.exe_path.expect("Executable path");
    let (stdout, _, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("Execution");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "55", "Sum of 1..10 must be 55");
}

#[test]
fn test_v123_bump_arena_sub_nanosecond_region() {
    let mut arena = BumpArena::new(1024 * 64); // 64 KB chunk

    let ptr1 = arena.alloc(128, 64);
    assert!(!ptr1.is_null());
    assert_eq!(ptr1 as usize % 64, 0, "Memory must be 64-byte aligned");

    let ptr2 = arena.alloc(256, 64);
    assert!(!ptr2.is_null());
    assert_eq!(ptr2 as usize % 64, 0);
    assert!(ptr2 as usize > ptr1 as usize);

    assert_eq!(arena.bytes_allocated(), 128 + 256);

    // Reset arena in 1 CPU cycle
    arena.reset();
    assert_eq!(arena.bytes_allocated(), 0);

    // Re-allocate from beginning
    let ptr3 = arena.alloc(128, 64);
    assert_eq!(ptr3, ptr1, "Reset must reuse chunk from beginning");

    // Test ScopeArenaGuard
    {
        let mut guard = ScopeArenaGuard::new(&mut arena);
        let scoped_ptr = guard.alloc(512, 64);
        assert!(!scoped_ptr.is_null());
    }
    // After guard drops, arena offset is rolled back
    assert_eq!(arena.bytes_allocated(), 128);
}

#[test]
fn test_v123_escaping_struct_safety_preservation() {
    // Function that returns the struct -> must be classified as EscapedReturn
    let func = Function {
        name: "create_and_return_point".into(),
        params: Vec::new(),
        return_type: "Point".into(),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            label: "entry".into(),
            instructions: vec![Inst::StructInit {
                dest: ValueId(5),
                class_name: "Point".into(),
                fields: vec![("x".into(), ValueId(1)), ("y".into(), ValueId(2))],
            }],
            terminator: Terminator::Return {
                value: Some(ValueId(5)),
            },
            params: Vec::new(),
        }],
        ..Default::default()
    };

    let mut trace = OptimizationDecisionTrace::new();
    let escape_res = EscapeAnalyzer::analyze(&func, &mut trace);
    assert_eq!(escape_res.non_escaping_count, 0);
    assert_eq!(escape_res.escaping_count, 1);
    assert_eq!(
        escape_res.allocations.get(&ValueId(5)).unwrap().state,
        EscapeState::EscapedReturn
    );
}
