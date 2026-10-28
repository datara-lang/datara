use forgen::dmir::{BasicBlock, BasicBlockId, Function, Inst, Terminator, ValueId};
use forgen::optimizer::cost_model::{CostModel, OptimizationDecisionTrace};
use forgen::optimizer::scalar::ScalarOptimizer;

fn function_with_binops(operations: &[(usize, usize, usize)]) -> Function {
    let instructions = operations
        .iter()
        .map(|&(left, right, dest)| Inst::BinOp {
            dest: ValueId(dest),
            op: "+".to_string(),
            left: ValueId(left),
            right: ValueId(right),
            ty: "Int".to_string(),
        })
        .collect();

    Function {
        name: "cse_fast_path".to_string(),
        params: vec![
            ("a".to_string(), "Int".to_string(), ValueId(0)),
            ("b".to_string(), "Int".to_string(), ValueId(1)),
            ("c".to_string(), "Int".to_string(), ValueId(2)),
        ],
        param_refinements: Vec::new(),
        requires: Vec::new(),
        return_type: "Int".to_string(),
        entry_block: BasicBlockId(0),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            label: "entry".to_string(),
            params: Vec::new(),
            instructions,
            terminator: Terminator::Return {
                value: operations.last().map(|operation| ValueId(operation.2)),
            },
        }],
        ..Default::default()
    }
}

#[test]
fn no_expressions_skips_the_dominance_graph() {
    let mut function = function_with_binops(&[]);
    let mut trace = OptimizationDecisionTrace::new();

    let eliminated = ScalarOptimizer::eliminate_common_subexpressions(
        &mut function,
        &CostModel::new("release"),
        &mut trace,
    );

    assert_eq!(eliminated, 0);
    assert!(trace.records.is_empty());
}

#[test]
fn unique_expressions_are_preserved_by_the_dominance_based_pass() {
    let mut function = function_with_binops(&[(0, 1, 3), (1, 2, 4)]);
    let before = format!("{:?}", function.blocks[0].instructions);
    let mut trace = OptimizationDecisionTrace::new();

    let eliminated = ScalarOptimizer::eliminate_common_subexpressions(
        &mut function,
        &CostModel::new("release"),
        &mut trace,
    );

    assert_eq!(eliminated, 0);
    assert_eq!(format!("{:?}", function.blocks[0].instructions), before);
    assert!(trace.records.is_empty());
}

#[test]
fn repeated_expressions_still_use_global_cse() {
    let mut function = function_with_binops(&[(0, 1, 3), (0, 1, 4)]);
    let mut trace = OptimizationDecisionTrace::new();

    let eliminated = ScalarOptimizer::eliminate_common_subexpressions(
        &mut function,
        &CostModel::new("release"),
        &mut trace,
    );

    assert_eq!(eliminated, 1);
    assert!(matches!(
        &function.blocks[0].instructions[1],
        Inst::UnOp { op, operand, .. } if op == "copy" && *operand == ValueId(3)
    ));
    assert_eq!(
        trace
            .records
            .iter()
            .filter(|record| record.pass == "CSE" && record.decision == "Applied")
            .count(),
        1
    );
}
