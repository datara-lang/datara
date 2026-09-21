//! Integration test suite for compile-time execution (v1.4.5)
//!
//! Covers:
//! 1. test_comptime_arithmetic
//! 2. test_comptime_strings
//! 3. test_comptime_list_literals
//! 4. test_comptime_control_flow
//! 5. test_comptime_recursion_fib
//! 6. test_comptime_mutual_calls
//! 7. test_comptime_dmir_inspect
//! 8. test_comptime_recursion_limit_err
//! 9. test_comptime_forbidden_io_err
//! 10. test_comptime_crc32_table

use forgen::ast::*;
use forgen::comptime::{ComptimeEvaluator, ComptimeValue};
use forgen::diagnostics::DiagnosticEngine;
use forgen::lexer::Lexer;
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

fn parse_program(src: &str) -> Program {
    let mut diag = DiagnosticEngine::new("en");
    let mut lexer = Lexer::new(src, "test.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "test.dtr");
    parser.parse_program()
}

#[test]
fn test_comptime_arithmetic() {
    let src = r#"
comptime fn calc() -> Int {
    let a = 10 + 20 * 3
    let b = (a >> 2) & 0x0F
    return b ^ 5
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval
        .call_fn("calc", vec![], &SourceSpan::default())
        .unwrap();
    // 10 + 60 = 70. 70 >> 2 = 17. 17 & 0x0F = 1. 1 ^ 5 = 4.
    assert_eq!(res, ComptimeValue::Int(4));
}

#[test]
fn test_comptime_strings() {
    let src = r#"
comptime fn greet(name: Str) -> Str {
    return "Hello, " + name
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval
        .call_fn(
            "greet",
            vec![ComptimeValue::Str("Datara".into())],
            &SourceSpan::default(),
        )
        .unwrap();
    assert_eq!(res, ComptimeValue::Str("Hello, Datara".into()));
}

#[test]
fn test_comptime_list_literals() {
    let src = r#"
comptime fn make_list() -> Int {
    let xs = [10, 20, 30, 40]
    return xs[2]
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval
        .call_fn("make_list", vec![], &SourceSpan::default())
        .unwrap();
    assert_eq!(res, ComptimeValue::Int(30));
}

#[test]
fn test_comptime_control_flow() {
    let src = r#"
comptime fn sum_odd(n: Int) -> Int {
    mut total = 0
    mut i = 0
    while i < n {
        if (i % 2) != 0 {
            total = total + i
        }
        i = i + 1
    }
    return total
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval
        .call_fn(
            "sum_odd",
            vec![ComptimeValue::Int(10)],
            &SourceSpan::default(),
        )
        .unwrap();
    // 1 + 3 + 5 + 7 + 9 = 25
    assert_eq!(res, ComptimeValue::Int(25));
}

#[test]
fn test_comptime_recursion_fib() {
    let src = r#"
comptime fn fib(n: Int) -> Int {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval
        .call_fn("fib", vec![ComptimeValue::Int(10)], &SourceSpan::default())
        .unwrap();
    assert_eq!(res, ComptimeValue::Int(55));
}

#[test]
fn test_comptime_mutual_calls() {
    let src = r#"
comptime fn is_even(n: Int) -> Bool {
    if n == 0 {
        return true
    }
    return is_odd(n - 1)
}

comptime fn is_odd(n: Int) -> Bool {
    if n == 0 {
        return false
    }
    return is_even(n - 1)
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res1 = eval
        .call_fn(
            "is_even",
            vec![ComptimeValue::Int(8)],
            &SourceSpan::default(),
        )
        .unwrap();
    let res2 = eval
        .call_fn(
            "is_odd",
            vec![ComptimeValue::Int(8)],
            &SourceSpan::default(),
        )
        .unwrap();
    assert_eq!(res1, ComptimeValue::Bool(true));
    assert_eq!(res2, ComptimeValue::Bool(false));
}

#[test]
fn test_comptime_dmir_inspect() {
    let src = r#"
comptime fn magic_num() -> Int {
    return 42 * 2
}

fn main() -> Int {
    let x = magic_num()
    return x
}
"#;
    let prog = parse_program(src);
    let mut resolver = Resolver::new();
    let mut diag = DiagnosticEngine::new("en");
    resolver.resolve_program(&prog, &mut diag);
    let mut type_checker = TypeChecker::new(&resolver);
    type_checker.check_program(&prog, &mut diag);

    let mut lowering = forgen::dmir::lowering::Lowering::new(&resolver, &type_checker);
    let module = lowering.lower_program(&prog, "main");

    let main_func = module
        .functions
        .get("main")
        .expect("main function exists in DMIR");
    let mut has_const_84 = false;
    let mut has_call_magic = false;
    for block in &main_func.blocks {
        for inst in &block.instructions {
            match inst {
                forgen::dmir::ir::Inst::ConstInt { value: 84, .. } => {
                    has_const_84 = true;
                }
                forgen::dmir::ir::Inst::Call { func, .. } if func == "magic_num" => {
                    has_call_magic = true;
                }
                _ => {}
            }
        }
    }
    assert!(
        has_const_84,
        "DMIR should contain ConstInt 84 evaluated at comptime"
    );
    assert!(
        !has_call_magic,
        "DMIR should not contain a runtime Call to magic_num"
    );
}

#[test]
fn test_comptime_recursion_limit_err() {
    let src = r#"
comptime fn infinite_recursion(n: Int) -> Int {
    return infinite_recursion(n + 1)
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval.call_fn(
        "infinite_recursion",
        vec![ComptimeValue::Int(0)],
        &SourceSpan::default(),
    );
    assert!(res.is_err());
    match res.unwrap_err() {
        forgen::comptime::ComptimeError::RecursionLimitExceeded(_) => {}
        other => panic!("Expected RecursionLimitExceeded, got {:?}", other),
    }
}

#[test]
fn test_comptime_forbidden_io_err() {
    let src = r#"
comptime fn bad_io() {
    println("Leaking to stdout!")
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval.call_fn("bad_io", vec![], &SourceSpan::default());
    assert!(res.is_err());
    match res.unwrap_err() {
        forgen::comptime::ComptimeError::ForbiddenEffect(msg, _) => {
            assert_eq!(msg, "println");
        }
        other => panic!("Expected ForbiddenEffect, got {:?}", other),
    }
}

#[test]
fn test_comptime_crc32_table() {
    let src = r#"
comptime fn make_crc_table() -> List<Int> {
    mut table: List<Int> = []
    mut i = 0
    while i < 256 {
        mut c = i
        mut j = 0
        while j < 8 {
            if (c & 1) != 0 {
                c = 0xEDB88320 ^ (c >> 1)
            } else {
                c = c >> 1
            }
            j = j + 1
        }
        table.push(c)
        i = i + 1
    }
    return table
}
"#;
    let prog = parse_program(src);
    let mut eval = ComptimeEvaluator::new();
    for decl in &prog.declarations {
        if let Decl::Function(f) = decl {
            eval.register_function(f.clone());
        }
    }
    let res = eval
        .call_fn("make_crc_table", vec![], &SourceSpan::default())
        .unwrap();
    if let ComptimeValue::List(table) = res {
        assert_eq!(table.len(), 256, "CRC32 table must have 256 entries");
        // Check standard CRC-32 polynomial IEEE 802.3 table values
        // Index 0 is 0
        assert_eq!(table[0], ComptimeValue::Int(0));
        // Index 1 is 0x77073096 (1996959894)
        assert_eq!(table[1], ComptimeValue::Int(0x77073096));
        // Index 2 is 0xEE0E612C (3993919788)
        assert_eq!(table[2], ComptimeValue::Int(0xEE0E612C));
        // Index 255 is 0x2D02EF8D (755189645)
        assert_eq!(table[255], ComptimeValue::Int(0x2D02EF8D));
    } else {
        panic!("Expected List result from make_crc_table");
    }
}
