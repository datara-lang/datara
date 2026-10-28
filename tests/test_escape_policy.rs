//! M6 escape policy (PLAN_V150, item "Escape-политика владения"):
//! ownership of objects crossing function boundaries.
//!
//! Rules under test:
//! 1. A function whose EVERY return transfers a freshly created object of
//!    its declared return class is "fresh" — ownership transfers to the
//!    caller: the CALLER receives the release decision (datara_rt_free at
//!    the object's last use) and the CALLEE receives none.
//! 2. A function whose return value is not provably fresh (a parameter, a
//!    field copy, an identity chain bottoming out anywhere else) stays out
//!    of the fresh set — its callers keep the pre-policy conservative
//!    behavior: no release, the object escapes as before. Never a wrong
//!    release.
//! 3. Storing an object into a module global escapes it with
//!    `GlobalStore` — the global outlives the frame and owns the lifetime.
//! 4. The release splice is idempotent: re-running insert_releases over
//!    post-insert IR must not duplicate a free.

use forgen::dmir::{verify_module, Inst, Lowering};
use forgen::diagnostics::DiagnosticEngine;
use forgen::driver::ForgenCompiler;
use forgen::lexer::Lexer;
use forgen::ownership::dataflow::{
    analyze_module, compute_fresh_set, insert_releases, EscapeReason,
};
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

fn lower(src: &str, name: &str) -> forgen::dmir::Module {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source(name, src);
    let tokens = Lexer::new(src, name).tokenize(&mut diag);
    let program = Parser::new(tokens, &mut diag, name).parse_program();
    assert!(!diag.has_errors(), "parse failed: {}", diag.format_all());
    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);
    assert!(
        !diag.has_errors(),
        "resolve failed: {}",
        diag.format_all()
    );
    let mut tc = TypeChecker::new(&resolver);
    tc.check_program(&program, &mut diag);
    assert!(
        !diag.has_errors(),
        "typecheck failed: {}",
        diag.format_all()
    );
    let module = Lowering::new(&resolver, &tc).lower_program(&program, "m");
    verify_module(&module).expect("DMIR invariants");
    module
}

fn count_free_calls(m: &forgen::dmir::Module, fname: &str) -> usize {
    m.functions
        .get(fname)
        .map(|f| {
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter())
                .filter(|i| {
                    matches!(
                        i,
                        Inst::Call { func, args, .. }
                            if func == "datara_rt_free" && args.len() == 1
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

const FRESH_RET_SRC: &str = "struct A { v: Int }\n\nfn mk() -> A {\n    val a = A { v: 42 }\n    return a\n}\n\nfn main() {\n    val a = mk()\n    out a.v\n    return\n}\n";

#[test]
fn returned_fresh_object_is_released_in_caller() {
    let m = lower(FRESH_RET_SRC, "fresh_ret.dtr");

    // Layer 0: the fresh-set fixpoint classifies mk as fresh, main is not.
    let fresh = compute_fresh_set(&m);
    assert_eq!(
        fresh.get("mk").map(String::as_str),
        Some("A"),
        "mk must be fresh (its only return transfers a fresh A): {:?}",
        fresh
    );
    assert!(
        !fresh.contains_key("main"),
        "main returns nothing and must never be fresh: {:?}",
        fresh
    );

    // Layer 1: ownership moved to the caller — main releases, mk does not.
    let plan = analyze_module(&m);
    let mk_rels: Vec<_> = plan
        .releases
        .iter()
        .filter(|r| r.function == "mk")
        .collect();
    assert!(
        mk_rels.is_empty(),
        "callee must not release the returned fresh object: {:?}",
        mk_rels
    );
    let main_rels: Vec<_> = plan
        .releases
        .iter()
        .filter(|r| r.function == "main" && r.class == "A")
        .collect();
    assert_eq!(
        main_rels.len(),
        1,
        "caller must own exactly one release for the fresh object: {:?}",
        plan.releases
    );
    assert!(
        plan.keeps.iter().any(|k| k.function == "mk"
            && k.class == "A"
            && k.reason == EscapeReason::Returned),
        "the object inside mk must be kept with the Returned reason: {:?}",
        plan.keeps
    );
}

#[test]
fn fresh_return_splice_frees_in_caller_and_is_idempotent() {
    let mut m = lower(FRESH_RET_SRC, "fresh_ret_splice.dtr");
    insert_releases(&mut m);
    verify_module(&m).expect("post-splice DMIR invariants");

    let main_frees = count_free_calls(&m, "main");
    let mk_frees = count_free_calls(&m, "mk");
    assert_eq!(
        main_frees, 1,
        "the spliced datara_rt_free must land in the CALLER (main)"
    );
    assert_eq!(
        mk_frees, 0,
        "no datara_rt_free may be spliced into the fresh callee (mk)"
    );

    // Idempotency (M6 diff-test guarantee: 0 double-free): re-analyzing
    // post-insert IR decides the same single release and skips the splice.
    insert_releases(&mut m);
    assert_eq!(
        count_free_calls(&m, "main"),
        main_frees,
        "re-running insert_releases must not duplicate the free"
    );
    verify_module(&m).expect("post-idempotency DMIR invariants");
}

#[test]
fn non_fresh_return_keeps_conservative_caller_behavior() {
    // echo returns its PARAMETER — the returned value is not a fresh
    // creation, so echo stays out of the fresh set and main keeps the
    // pre-policy behavior: no release anywhere (leak as before, never a
    // wrong release).
    let src = "struct A { v: Int }\n\nfn echo(a: A) -> A {\n    return a\n}\n\nfn main() {\n    mut a = A { v: 1 }\n    val b = echo(a)\n    out b.v\n    return\n}\n";
    let m = lower(src, "non_fresh_ret.dtr");

    let fresh = compute_fresh_set(&m);
    assert!(
        !fresh.contains_key("echo"),
        "a param-returning function is not fresh: {:?}",
        fresh
    );

    let plan = analyze_module(&m);
    assert!(
        !plan
            .releases
            .iter()
            .any(|r| r.function == "main" && r.class == "A"),
        "caller must keep pre-policy behavior for a non-fresh callee: {:?}",
        plan.releases
    );
    assert!(
        !plan.releases.iter().any(|r| r.function == "echo"),
        "the non-fresh callee gets no release either: {:?}",
        plan.releases
    );
    assert!(
        plan.keeps.iter().any(|k| k.function == "echo"
            && k.class == "A"
            && k.reason == EscapeReason::CallerOwned),
        "the parameter is caller-owned in echo: {:?}",
        plan.keeps
    );
    assert!(
        plan.keeps.iter().any(|k| k.function == "main"
            && k.class == "A"
            && k.reason == EscapeReason::PassedToCall),
        "passing `a` into echo escapes it in main: {:?}",
        plan.keeps
    );
}

#[test]
fn global_store_escapes_object_to_the_global() {
    // An object stored into a module global outlives the frame: the
    // global owns it (other functions read it later), so the storing
    // binding escapes with GlobalStore and NO in-frame release fires.
    let src = "struct A { v: Int }\n\nmut g: A = A { v: 0 }\n\nfn main() {\n    g = A { v: 7 }\n    out g.v\n    return\n}\n";
    let m = lower(src, "global_store.dtr");
    assert!(
        m.globals.contains_key("g"),
        "the module must expose `g` as a global"
    );

    let plan = analyze_module(&m);
    assert!(
        plan.keeps.iter().any(|k| k.function == "main"
            && k.variable == "g"
            && k.class == "A"
            && k.reason == EscapeReason::GlobalStore),
        "storing into the global must keep the object with GlobalStore: {:?}",
        plan.keeps
    );
    assert!(
        !plan
            .releases
            .iter()
            .any(|r| r.function == "main" && r.class == "A"),
        "no release may fire for a globally stored object: {:?}",
        plan.releases
    );

    let mut spliced = lower(src, "global_store_splice.dtr");
    insert_releases(&mut spliced);
    assert_eq!(
        count_free_calls(&spliced, "main"),
        0,
        "the splice must not free a globally stored object"
    );
}

#[test]
fn fresh_return_runs_natively() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(FRESH_RET_SRC, "test_escape_policy_fresh.dtr", None);
    assert!(
        res.success,
        "fresh-return program must compile: {:?} / {}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "program must exit cleanly, stderr: {stderr}");
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn global_store_runs_natively() {
    let src = "struct A { v: Int }\n\nmut g: A = A { v: 0 }\n\nfn main() {\n    g = A { v: 7 }\n    out g.v\n    return\n}\n";
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "test_escape_policy_global.dtr", None);
    assert!(
        res.success,
        "global-store program must compile: {:?} / {}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "program must exit cleanly, stderr: {stderr}");
    assert_eq!(stdout.trim(), "7");
}
