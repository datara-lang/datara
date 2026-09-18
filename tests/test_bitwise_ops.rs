//! End-to-end tests for the infix bitwise operators added in v1.3.2:
//! `&` (and), `|` (or), `^` (xor), `<<` (shift left), `>>` (arithmetic
//! shift right).
//!
//! README_RU ("Побитовые операции и аппаратные интринсики") promises these
//! operators; the hardware intrinsics `and(...)`, `xor(...)`, `shl(...)`,
//! `math_shr(...)` remain available alongside them.
//!
//! Operator precedence (high to low), documented in README.md:
//! `*` `/` `%`  >  `+` `-`  >  `<<` `>>`  >  `&`  >  `^`  >  `|`  >
//! `==` `!=` `<` `<=` `>` `>=`  >  `&&`  >  `||`.
//!
//! Constant shift counts outside `0..64` are rejected at compile time with
//! E0947 (fail-closed: `Int` is a signed 64-bit integer, so any other count
//! has no defined result). The literal case is caught by the type checker,
//! the variable case (constant propagated through a binding) by the
//! constant-folding pass.

use forgen::dmir::{BasicBlock, BasicBlockId, Function, Inst, Module, Terminator, ValueId};
use forgen::driver::ForgenCompiler;
use forgen::optimizer::Optimizer;

fn compile_and_run(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(res.success, "compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "program must exit cleanly, stderr: {}", stderr);
    stdout
}

#[test]
fn test_bitwise_operator_values() {
    let src = r#"
fn main() {
    out 12 & 10
    out 12 | 10
    out 12 ^ 10
    out 1 << 10
    out -16 >> 2
}
"#;
    let stdout = compile_and_run(src, "test_bitwise_values.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["8", "14", "6", "1024", "-4"]);
}

#[test]
fn test_bitwise_precedence() {
    // `2 | 4 ^ 1 & 3` groups by precedence (no parentheses):
    //   `&` binds tightest:  1 & 3 = 1
    //   then `^`:            4 ^ 1 = 5
    //   then `|`:            2 | 5 = 7
    // A left-to-right or wrong-level parse could not produce 7.
    let src = r#"
fn main() {
    out 2 | 4 ^ 1 & 3
}
"#;
    let stdout = compile_and_run(src, "test_bitwise_precedence.dtr");
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn test_bitwise_const_fold_proven_in_dmir() {
    // Two proof layers:
    //
    // (1) Lowering-level: `out 12 & 10` reaches the DMIR already folded —
    //     the module contains ConstInt(8) and no `&` BinOp.
    //
    // (2) Pass-level: a synthetic straight-line function with
    //     ConstInt(12), ConstInt(10), BinOp("&") must be folded to
    //     ConstInt(8) by the constant-folding pass, and the report must
    //     count it. This is the same code path that folds bitwise ops
    //     whose operands become constant only inside the optimizer
    //     (for example through block-local constant propagation).
    let src = r#"
fn main() {
    out 12 & 10
}
"#;
    let compiler = ForgenCompiler::new("release");
    let module = compiler
        .compile_source_to_dmir(src, "test_bitwise_fold.dtr")
        .expect("must lower to DMIR");
    let main_fn = module
        .functions
        .get("main")
        .expect("main must exist in DMIR");
    let mut const_values = Vec::new();
    let mut bitwise_binops = 0;
    for block in &main_fn.blocks {
        for inst in &block.instructions {
            match inst {
                Inst::ConstInt { value, .. } => const_values.push(*value),
                Inst::BinOp { op, .. } if op == "&" || op == "<<" => bitwise_binops += 1,
                _ => {}
            }
        }
    }
    assert!(
        const_values.contains(&8),
        "12 & 10 must be folded to ConstInt(8), got {:?}",
        const_values
    );
    assert_eq!(
        bitwise_binops, 0,
        "constant bitwise BinOps must be replaced by folded constants"
    );

    // Pass-level proof on a synthetic function (no variables involved,
    // so the folding must happen in the constant-folding pass itself).
    let mut module = Module::new("bitwise_fold_proof");
    let mut function = Function {
        name: "f".into(),
        params: vec![],
        return_type: "Int".into(),
        entry_block: BasicBlockId(0),
        blocks: Vec::new(),
        ..Default::default()
    };
    function.blocks.push(BasicBlock {
        id: BasicBlockId(0),
        label: "entry".into(),
        params: vec![],
        instructions: vec![
            Inst::ConstInt {
                dest: ValueId(1),
                value: 12,
            },
            Inst::ConstInt {
                dest: ValueId(2),
                value: 10,
            },
            Inst::BinOp {
                dest: ValueId(3),
                op: "&".into(),
                left: ValueId(1),
                right: ValueId(2),
                ty: "Int".into(),
            },
        ],
        terminator: Terminator::Return {
            value: Some(ValueId(3)),
        },
    });
    module.functions.insert("f".to_string(), function);

    let mut optimizer = Optimizer::new("release");
    optimizer
        .optimize_module(&mut module)
        .expect("optimization must succeed");
    assert!(
        optimizer.report.constants_folded >= 1,
        "constant folding must report folded constants"
    );
    let f = module.functions.get("f").expect("f must survive");
    let mut folded_values = Vec::new();
    let mut remaining_bitwise = 0;
    for block in &f.blocks {
        for inst in &block.instructions {
            match inst {
                Inst::ConstInt { value, .. } => folded_values.push(*value),
                Inst::BinOp { op, .. } if op == "&" => remaining_bitwise += 1,
                _ => {}
            }
        }
    }
    assert_eq!(remaining_bitwise, 0, "BinOp(&) must be folded away");
    assert!(
        folded_values.contains(&8),
        "BinOp(12 & 10) must fold to ConstInt(8), got {:?}",
        folded_values
    );
}

#[test]
fn test_shift_out_of_range_literal_rejected() {
    // Type-checker path: a literal shift count >= 64 has no defined result.
    for src in [
        "fn main() {\n    out 1 << 64\n}\n",
        "fn main() {\n    out 1 >> 64\n}\n",
    ] {
        let res = compile_and_expect_failure(src, "test_shift64.dtr");
        assert!(
            res.contains("E0947") || res.contains("out of range"),
            "shift of 64 must be rejected with E0947, got: {}",
            res
        );
    }
    // A count inside 0..64 stays legal (even when the high bits shift out).
    let ok = r#"
fn main() {
    out 1 << 63
}
"#;
    let stdout = compile_and_run(ok, "test_shift63_ok.dtr");
    assert_eq!(stdout.trim(), "-9223372036854775808");
}

#[test]
fn test_shift_out_of_range_variable_rejected() {
    // Constant-folding path: the shift count reaches the optimizer as a
    // DMIR constant propagated through a variable, not as a literal.
    let neg = "fn main() {\n    mut k = -1\n    out 1 >> k\n}\n";
    let res = compile_and_expect_failure(neg, "test_shift_neg.dtr");
    assert!(
        res.contains("E0947") || res.contains("out of range"),
        "negative variable shift must be rejected with E0947, got: {}",
        res
    );

    let big = "fn main() {\n    mut k = 64\n    out 1 << k\n}\n";
    let res = compile_and_expect_failure(big, "test_shift_var64.dtr");
    assert!(
        res.contains("E0947") || res.contains("out of range"),
        "variable shift by 64 must be rejected with E0947, got: {}",
        res
    );
}

#[test]
fn test_float_bitwise_rejected() {
    // No implicit numeric conversion (SPEC_V1 Gate 7): bitwise operators
    // are defined on Int only.
    let res = compile_and_expect_failure(
        "fn main() {\n    out 1.5 & 2\n}\n",
        "test_float_bitwise.dtr",
    );
    assert!(
        res.contains("E-TYPE-004") || res.contains("cannot be applied"),
        "1.5 & 2 must be a type error, got: {}",
        res
    );

    // Bool is not Int either: strict typing rejects `true & false`.
    let res = compile_and_expect_failure(
        "fn main() {\n    out true & false\n}\n",
        "test_bool_bitwise.dtr",
    );
    assert!(
        res.contains("E-TYPE-004") || res.contains("cannot be applied"),
        "true & false must be a type error, got: {}",
        res
    );
}

#[test]
fn test_hardware_intrinsics_still_work() {
    // The function-form intrinsics must keep behaving exactly as before.
    let src = r#"
fn main() {
    out and(12, 10)
    out math_or(12, 10)
    out xor(12, 10)
    out shl(1, 10)
    out math_shr(-16, 2)
}
"#;
    let stdout = compile_and_run(src, "test_bitwise_intrinsics.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["8", "14", "6", "1024", "-4"]);
}

#[test]
fn test_nested_generics_still_parse_with_shr_token() {
    // The lexer emits `>>` as one Shr token; in type position it must
    // still close two nested generic argument lists.
    let src = r#"
fn main() {
    let xs: List<List<Int>> = [[1, 2], [3, 4]]
    out xs[0][1]
    out xs[1][0]
}
"#;
    let stdout = compile_and_run(src, "test_nested_generics_shr.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["2", "3"]);
}

#[test]
fn test_logical_operators_unchanged() {
    // `&&` / `||` keep their logical (short-circuit) meaning.
    let src = r#"
fn main() {
    out true && false
    out true || false
    out true && true
}
"#;
    let stdout = compile_and_run(src, "test_logical_unchanged.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["false", "true", "true"]);
}

#[test]
fn test_shifts_compose_with_arithmetic() {
    // Shift binds tighter than comparison, looser than `+`:
    // `1 + 2 << 3` == `(1 + 2) << 3` == 24, and `4 << 1 == 8` is
    // `((4 << 1) == 8)` because shift outranks comparison.
    let src = r#"
fn main() {
    out 1 + 2 << 3
    out 4 << 1 == 8
}
"#;
    let stdout = compile_and_run(src, "test_shift_compose.dtr");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["24", "true"]);
}

fn compile_and_expect_failure(source: &str, name: &str) -> String {
    let res = compile_native(source, name);
    assert!(
        !res.success,
        "compilation must fail, but succeeded; diagnostics: {}",
        res.diagnostics
    );
    let err = res.error.unwrap_or_default();
    let rendered = format!("{}{}", err, res.diagnostics);
    assert!(!rendered.is_empty(), "failure must carry a diagnostic");
    rendered
}

fn compile_native(source: &str, name: &str) -> forgen::driver::CompilationResult {
    ForgenCompiler::new("release").compile_source_native(source, name, None)
}
