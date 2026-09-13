use forgen::diagnostics::DiagnosticEngine;
use forgen::lexer::Lexer;
use forgen::parser::Parser;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_lexer_never_panics(s in ".*") {
        let mut diag = DiagnosticEngine::new("en");
        let mut lexer = Lexer::new(&s, "fuzz_input.dtr");
        let _ = lexer.tokenize(&mut diag);
    }

    #[test]
    fn prop_parser_never_panics_on_arbitrary_input(s in ".*") {
        let mut diag = DiagnosticEngine::new("en");
        let mut lexer = Lexer::new(&s, "fuzz_input.dtr");
        let tokens = lexer.tokenize(&mut diag);
        let mut parser = Parser::new(tokens, &mut diag, "fuzz_input.dtr");
        let _ = parser.parse_program();
    }

    #[test]
    fn prop_nesting_depth_boundary(depth in 1usize..80) {
        let (has_errors, parsed) = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let s = "(".repeat(depth) + "42" + &")".repeat(depth);
                let mut diag = DiagnosticEngine::new("en");
                let mut lexer = Lexer::new(&s, "fuzz_depth.dtr");
                let tokens = lexer.tokenize(&mut diag);
                let mut parser = Parser::new(tokens, &mut diag, "fuzz_depth.dtr");
                let expr = parser.parse_expression();
                (diag.has_errors(), expr.is_some())
            })
            .unwrap()
            .join()
            .unwrap();

        if depth > 64 {
            prop_assert!(has_errors, "Depth {} > 64 must produce diagnostic error", depth);
        } else {
            prop_assert!(!has_errors, "Depth {} <= 64 must parse cleanly", depth);
            prop_assert!(parsed, "Depth {} <= 64 must produce Some(Expr)", depth);
        }
    }

    #[test]
    fn prop_unicode_never_panics(s in "\\PC*") {
        let mut diag = DiagnosticEngine::new("en");
        let mut lexer = Lexer::new(&s, "fuzz_unicode.dtr");
        let tokens = lexer.tokenize(&mut diag);
        let mut parser = Parser::new(tokens, &mut diag, "fuzz_unicode.dtr");
        let _ = parser.parse_program();
    }
}
