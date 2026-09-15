//! v1.3.4 `@inline` attribute tests: validation (E0956/E0957), forced
//! inlining of `@inline(always)` callees under `--opt speed`, and behavior
//! equivalence across tiers.
//!
//! Scope notes:
//! * Validation is tier-independent and runs in both the compile pipeline
//!   and `forgen check`; the tests drive `validate_inline_attributes`
//!   directly plus one end-to-end `check_file` run for E0956.
//! * Forced inlining is gated behind `--opt speed` (see the report). The
//!   tier difference is asserted through the decision trace: the
//!   `InlineAlways` pass emits records only when it runs.

use forgen::codegen::cranelift::CraneliftBackend;
use forgen::diagnostics::DiagnosticEngine;
use forgen::dmir::{Inst, Lowering};
use forgen::driver::ForgenCompiler;
use forgen::lexer::Lexer;
use forgen::optimizer::{OptTier, Optimizer};
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;
use std::fs;
use std::path::PathBuf;

fn parse(src: &str, diag: &mut DiagnosticEngine) -> forgen::ast::Program {
    let mut lexer = Lexer::new(src, "inline_attr.dtr");
    let tokens = lexer.tokenize(diag);
    let mut parser = Parser::new(tokens, diag, "inline_attr.dtr");
    parser.parse_program()
}

fn lower(src: &str) -> forgen::dmir::Module {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("inline_attr.dtr", src);

    let mut lexer = Lexer::new(src, "inline_attr.dtr");
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, "inline_attr.dtr");
    let program = parser.parse_program();
    assert!(!diag.has_errors(), "parse errors: {:?}", diag.diagnostics);

    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);
    assert!(!diag.has_errors(), "resolve errors: {:?}", diag.diagnostics);

    let mut tc = TypeChecker::new(&resolver);
    tc.check_program(&program, &mut diag);
    assert!(
        !diag.has_errors(),
        "typecheck errors: {:?}",
        diag.diagnostics
    );

    let mut lowering = Lowering::new(&resolver, &tc);
    lowering.lower_program(&program, "inline_attr")
}

fn optimize(src: &str, tier: OptTier) -> (forgen::dmir::Module, Optimizer) {
    let mut module = lower(src);
    let mut opt = Optimizer::new("release");
    opt.opt_tier = tier;
    opt.optimize_module(&mut module)
        .expect("optimizer must not fail");
    (module, opt)
}

fn compile_and_run(module: &forgen::dmir::Module, tag: &str) -> String {
    let be = CraneliftBackend::for_host();
    let exe = PathBuf::from(format!("target/inline_attr_{}.exe", tag));
    let p = be
        .compile_native(module, &exe)
        .unwrap_or_else(|e| panic!("[{}] native compilation failed: {}", tag, e));
    let (out, err, code, _) = be
        .run_executable(&p, &[])
        .unwrap_or_else(|e| panic!("[{}] execution failed: {}", tag, e));
    assert_eq!(code, 0, "[{}] non-zero exit; stderr={}", tag, err);
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[test]
fn unknown_inline_argument_is_rejected_with_e0957() {
    let src = r#"
@inline(sometimes)
fn f(x: Int) -> Int {
    return x + 1
}
fn main() {
    out f(1)
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    let program = parse(src, &mut diag);
    assert!(!diag.has_errors(), "the attribute must parse");

    let mut vdiag = DiagnosticEngine::new("en");
    forgen::optimizer::inline_attr::validate_inline_attributes(&program, &mut vdiag);
    assert!(
        vdiag.has_errors(),
        "unknown inline argument must be an error"
    );
    assert!(
        vdiag.diagnostics.iter().any(|d| d.code == "E0957"),
        "expected E0957, got: {:?}",
        vdiag.diagnostics
    );
}

#[test]
fn duplicate_inline_attribute_is_rejected_with_e0957() {
    let src = r#"
@inline
@inline(always)
fn f(x: Int) -> Int {
    return x + 1
}
fn main() {
    out f(1)
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    let program = parse(src, &mut diag);
    assert!(!diag.has_errors());

    let mut vdiag = DiagnosticEngine::new("en");
    forgen::optimizer::inline_attr::validate_inline_attributes(&program, &mut vdiag);
    assert!(
        vdiag.diagnostics.iter().any(|d| d.code == "E0957"),
        "duplicate @inline must be E0957, got: {:?}",
        vdiag.diagnostics
    );
}

#[test]
fn inline_always_on_extern_is_rejected_with_e0956() {
    let src = r#"
@inline(always)
extern "C" fn ext_add(a: Int, b: Int) -> Int
fn main() {
    out 1
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    let program = parse(src, &mut diag);
    assert!(
        !diag.has_errors(),
        "extern with attribute must parse: {:?}",
        diag.diagnostics
    );

    let mut vdiag = DiagnosticEngine::new("en");
    forgen::optimizer::inline_attr::validate_inline_attributes(&program, &mut vdiag);
    assert!(
        vdiag.diagnostics.iter().any(|d| d.code == "E0956"),
        "expected E0956, got: {:?}",
        vdiag.diagnostics
    );
}

#[test]
fn inline_never_on_extern_is_not_rejected() {
    // Only `@inline(always)` is meaningless-and-rejected on extern; the
    // weaker hints stay accepted.
    let src = r#"
@inline(never)
extern "C" fn ext_add(a: Int, b: Int) -> Int
fn main() {
    out 1
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    let program = parse(src, &mut diag);
    let mut vdiag = DiagnosticEngine::new("en");
    forgen::optimizer::inline_attr::validate_inline_attributes(&program, &mut vdiag);
    assert!(
        !vdiag.has_errors(),
        "@inline(never) on extern must not be an error: {:?}",
        vdiag.diagnostics
    );
}

#[test]
fn check_file_reports_e0956_end_to_end() {
    let path = PathBuf::from("target/inline_attr_e0956.dtr");
    fs::write(
        &path,
        "@inline(always)\nextern \"C\" fn ext_add(a: Int, b: Int) -> Int\nfn main() {\n    out 1\n}\n",
    )
    .expect("write temp source");

    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_file(&path);
    assert!(
        !res.success,
        "forgen check must fail on @inline(always) extern"
    );
    let text = res.diagnostics.to_string();
    assert!(
        text.contains("E0956"),
        "diagnostics must mention E0956, got: {}",
        text
    );
    let _ = fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// Hint storage + forced inlining
// ---------------------------------------------------------------------------

#[test]
fn hints_are_stored_on_dmir_functions() {
    let src = r#"
@inline(always)
fn always_fn(x: Int) -> Int {
    return x + 1
}
@inline(never)
fn never_fn(x: Int) -> Int {
    return x + 2
}
@inline
fn plain_fn(x: Int) -> Int {
    return x + 3
}
fn no_attr(x: Int) -> Int {
    return x + 4
}
fn main() {
    out always_fn(1)
    out never_fn(1)
    out plain_fn(1)
    out no_attr(1)
}
"#;
    let module = lower(src);
    use forgen::dmir::InlineHint;
    assert_eq!(
        module.functions["always_fn"].inline_hint,
        InlineHint::Always
    );
    assert_eq!(module.functions["never_fn"].inline_hint, InlineHint::Never);
    assert_eq!(module.functions["plain_fn"].inline_hint, InlineHint::Inline);
    assert_eq!(module.functions["no_attr"].inline_hint, InlineHint::None);
}

const INLINE_SRC: &str = r#"
@inline(always)
fn add_one(x: Int) -> Int {
    return x + 1
}
fn main() {
    mut i = 0
    mut s = 0
    while i < 10 {
        s = s + add_one(i)
        i = i + 1
    }
    out s
}
"#;

#[test]
fn inline_always_callee_is_spliced_at_speed_tier() {
    let (module, opt) = optimize(INLINE_SRC, OptTier::Speed);

    assert!(
        opt.trace
            .records
            .iter()
            .any(|r| r.pass == "InlineAlways" && r.decision == "Applied"),
        "speed tier must run the forced inliner and record it: {:?}",
        opt.trace
            .records
            .iter()
            .filter(|r| r.pass == "InlineAlways")
            .collect::<Vec<_>>()
    );

    // No call to the hint-marked callee may remain anywhere in the module.
    for (fname, f) in &module.functions {
        for b in &f.blocks {
            for inst in &b.instructions {
                if let Inst::Call { func, .. } = inst {
                    assert_ne!(
                        func, "add_one",
                        "call to @inline(always) callee survived in function '{}'",
                        fname
                    );
                }
            }
        }
    }
}

#[test]
fn forced_inliner_does_not_run_at_default_tier() {
    let (_module, opt) = optimize(INLINE_SRC, OptTier::Default);
    assert!(
        !opt.trace.records.iter().any(|r| r.pass == "InlineAlways"),
        "the forced inliner must not run under the default tier"
    );
}

#[test]
fn inlined_program_behaves_identically_at_both_tiers() {
    // sum of (i + 1) for i in 0..10 = 55
    let (default_module, _) = optimize(INLINE_SRC, OptTier::Default);
    assert_eq!(
        compile_and_run(&default_module, "default"),
        "55",
        "default tier result changed"
    );

    let (speed_module, _) = optimize(INLINE_SRC, OptTier::Speed);
    assert_eq!(
        compile_and_run(&speed_module, "speed"),
        "55",
        "inlining changed the program result"
    );
}

#[test]
fn impure_inline_always_body_is_refused_not_truncated() {
    // The body has an `out`, which the straight-line splicer cannot copy.
    // It must be rejected with a trace record, never silently truncated —
    // the program (and its output) must stay intact.
    let src = r#"
@inline(always)
fn noisy(x: Int) -> Int {
    out x
    return x + 1
}
fn main() {
    out noisy(7)
}
"#;
    let (module, opt) = optimize(src, OptTier::Speed);
    assert!(
        opt.trace
            .records
            .iter()
            .any(|r| r.pass == "InlineAlways" && r.decision == "Rejected"),
        "effectful @inline(always) body must be rejected explicitly"
    );
    assert_eq!(
        compile_and_run(&module, "noisy"),
        "7\n8",
        "the effectful callee must still run as a call"
    );
}

#[test]
fn inline_never_blocks_every_inliner() {
    // `never_fn` is a small pure single-block function: the cost-model
    // inliner (<= 20 insts) and IPO (<= 15 insts) would both inline it, and
    // the specializer would clone it for the constant call. The attribute
    // vetoes all of them at both tiers.
    let src = r#"
@inline(never)
fn add_two(x: Int) -> Int {
    return x + 2
}
fn main() {
    out add_two(40)
}
"#;
    for tier in [OptTier::Default, OptTier::Speed] {
        let (module, _opt) = optimize(src, tier);
        let call_exists = module.functions.values().any(|f| {
            f.blocks.iter().any(|b| {
                b.instructions
                    .iter()
                    .any(|i| matches!(i, Inst::Call { func, .. } if func.starts_with("add_two")))
            })
        });
        assert!(
            call_exists,
            "[{:?}] '@inline(never)' function must keep outline dispatch: a call must survive",
            tier
        );
        assert_eq!(compile_and_run(&module, "never"), "42");
    }
}
