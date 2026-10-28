use forgen::diagnostics::DiagnosticEngine;
use forgen::dmir::{Lowering, verify_module};
use forgen::lexer::Lexer;
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

#[test]
fn value_ids_are_local_to_each_lowered_function() {
    let source = r#"
fn first(a: Int) -> Int => a + 1
fn second(b: Int) -> Int => b * 2
fn third(c: Int) -> Int => c - 3
"#;
    let mut diagnostics = DiagnosticEngine::new("en");
    diagnostics.set_source("local_ids.dtr", source);

    let tokens = Lexer::new(source, "local_ids.dtr").tokenize(&mut diagnostics);
    let program = Parser::new(tokens, &mut diagnostics, "local_ids.dtr").parse_program();
    assert!(
        !diagnostics.has_errors(),
        "parse failed: {}",
        diagnostics.format_all()
    );

    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diagnostics);
    assert!(
        !diagnostics.has_errors(),
        "resolve failed: {}",
        diagnostics.format_all()
    );

    let mut type_checker = TypeChecker::new(&resolver);
    type_checker.check_program(&program, &mut diagnostics);
    assert!(
        !diagnostics.has_errors(),
        "typecheck failed: {}",
        diagnostics.format_all()
    );

    let module = Lowering::new(&resolver, &type_checker).lower_program(&program, "local_ids");
    verify_module(&module).expect("lowered module must satisfy DMIR invariants");

    for name in ["first", "second", "third"] {
        let function = module.functions.get(name).expect("function was lowered");
        assert_eq!(
            function.params[0].2.0, 0,
            "ValueIds must start at zero in function {name}"
        );
    }
}
