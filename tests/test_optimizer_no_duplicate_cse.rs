use forgen::dmir::{
    BasicBlock, BasicBlockId, Function, Inst, Module, Terminator, ValueId,
};
use forgen::optimizer::Optimizer;

fn function(name: &str, value: i64) -> Function {
    Function {
        name: name.to_string(),
        return_type: "Int".to_string(),
        entry_block: BasicBlockId(0),
        blocks: vec![BasicBlock {
            id: BasicBlockId(0),
            label: "entry".to_string(),
            params: vec![],
            instructions: vec![Inst::ConstInt {
                dest: ValueId(0),
                value,
            }],
            terminator: Terminator::Return {
                value: Some(ValueId(0)),
            },
        }],
        ..Default::default()
    }
}

#[test]
fn same_local_value_ids_in_different_functions_remain_function_scoped() {
    let mut module = Module::new("duplicate_cse_scope_test");
    module.functions.insert("main".into(), function("main", 3));
    module.functions.insert("helper".into(), function("helper", 17));

    let mut optimizer = Optimizer::new("check");
    optimizer.optimize_module(&mut module).expect("module must optimize");

    let main_return = module.functions["main"].blocks[0].terminator.clone();
    let helper_return = module.functions["helper"].blocks[0].terminator.clone();
    assert_eq!(main_return, Terminator::Return { value: Some(ValueId(0)) });
    assert_eq!(helper_return, Terminator::Return { value: Some(ValueId(0)) });
    assert!(matches!(module.functions["main"].blocks[0].instructions[0], Inst::ConstInt { value: 3, .. }));
    assert!(matches!(module.functions["helper"].blocks[0].instructions[0], Inst::ConstInt { value: 17, .. }));
}
