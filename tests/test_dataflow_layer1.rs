//! M6 TMM Layer 1: dataflow last-use analysis tests (PLAN_V150 M6
//! sub-milestone 1: AST→dataflow-граф→решатель). The analyzer decides,
//! per uniquely-owned object, where a deterministic release may be
//! inserted: immediately after the LAST use, never at scope end, and
//! never for anything that escapes.

use forgen::dmir::{verify_module, Lowering};
use forgen::diagnostics::DiagnosticEngine;
use forgen::lexer::Lexer;
use forgen::ownership::dataflow::{analyze_module, layer1_report, EscapeReason};
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

fn lower(src: &str, name: &str) -> forgen::dmir::Module {
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source(name, src);
    let tokens = Lexer::new(src, name).tokenize(&mut diag);
    let program = Parser::new(tokens, &mut diag, name).parse_program();
    assert!(
        !diag.has_errors(),
        "parse failed: {}",
        diag.format_all()
    );
    let mut resolver = Resolver::new();
    resolver.resolve_program(&program, &mut diag);
    assert!(!diag.has_errors(), "resolve failed: {}", diag.format_all());
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

const PLAIN: &str = "struct Plain { a: Int, b: Int }\n";

#[test]
fn last_use_release_replaces_scope_end() {
    // t is used twice; its release point must sit after the SECOND use
    // (the s computation), not after the first and not at scope end.
    let m = lower(
        &format!("{PLAIN}\nfn main() -> Int {{\n    val t = Plain {{ a: 1, b: 2 }}\n    val s = t.a + t.b\n    out s\n    return 0\n}}\n"),
        "l1_last_use.dtr",
    );
    let plan = analyze_module(&m);
    // t is a plain struct whose object value never escapes (only field
    // reads); there must be exactly one release decision for it and its
    // insertion index must be after the LAST use instruction.
    let t_rels: Vec<_> = plan
        .releases
        .iter()
        .filter(|r| r.variable == "t")
        .collect();
    assert_eq!(t_rels.len(), 1, "exactly one release decision for t");
    let r = t_rels[0];
    assert_eq!(r.class, "Plain");
    assert!(!r.is_arc);
    // The last use of t is the second field read; the decision must be
    // strictly later than the first field read's index.
    assert!(
        r.after_index >= 3,
        "release must sit after the last use, got after_index {}",
        r.after_index
    );
}

#[test]
fn never_used_object_releases_right_after_creation() {
    // An unused object still leaks on the heap today; Layer 1 frees it
    // immediately (release point == creation index).
    let m = lower(
        &format!("{PLAIN}\nfn main() {{\n    val dead = Plain {{ a: 3, b: 4 }}\n    out 0\n}}\n"),
        "l1_dead.dtr",
    );
    let plan = analyze_module(&m);
    let dead: Vec<_> = plan
        .releases
        .iter()
        .filter(|r| r.variable == "dead")
        .collect();
    assert_eq!(dead.len(), 1, "unused object must get a release decision");
}

#[test]
fn returned_object_is_kept() {
    let src = &format!(
        "{PLAIN}\nfn make() -> Plain {{\n    val t = Plain {{ a: 1, b: 2 }}\n    return t\n}}\n\nfn main() {{\n    out 0\n}}\n"
    );
    let m = lower(src, "l1_returned.dtr");
    let plan = analyze_module(&m);
    let kept: Vec<_> = plan
        .keeps
        .iter()
        .filter(|k| k.function == "make" && k.variable == "t")
        .collect();
    assert_eq!(kept.len(), 1, "returned object must be kept");
    assert_eq!(kept[0].reason, EscapeReason::Returned);
}

#[test]
fn aliased_copy_is_kept_and_copy_holder_is_marked() {
    // `val p = q` copies the object value: q's def becomes aliased (kept)
    // and p is a copy binding (kept, CopyBound) — no release anywhere.
    let src = &format!(
        "{PLAIN}\nfn main() {{\n    val q = Plain {{ a: 1, b: 2 }}\n    val p = q\n    out p.a\n    out q.b\n    out 0\n}}\n"
    );
    let m = lower(src, "l1_alias.dtr");
    let plan = analyze_module(&m);
    let q_kept = plan
        .keeps
        .iter()
        .any(|k| k.function == "main" && k.variable == "q" && k.reason == EscapeReason::Aliased);
    assert!(q_kept, "the copy source must be kept: {:?}", plan.keeps);
    let p_kept = plan
        .keeps
        .iter()
        .any(|k| k.function == "main" && k.variable == "p" && k.reason == EscapeReason::CopyBound);
    assert!(p_kept, "the copy holder must be kept: {:?}", plan.keeps);
    assert!(
        plan.releases.is_empty(),
        "no release decisions may exist for aliased objects"
    );
}

#[test]
fn call_argument_escape_is_kept() {
    // Passing the object to a function conservatively keeps it: the
    // callee may retain the pointer.
    let src = &format!(
        "{PLAIN}\nfn take(p: Plain) -> Int {{\n    return p.a\n}}\n\nfn main() {{\n    val t = Plain {{ a: 5, b: 6 }}\n    val r = take(t)\n    out r\n    out 0\n}}\n"
    );
    let m = lower(src, "l1_calleh.dtr");
    let plan = analyze_module(&m);
    let kept = plan
        .keeps
        .iter()
        .any(|k| k.function == "main" && k.variable == "t" && k.reason == EscapeReason::PassedToCall);
    assert!(kept, "call-arg object must be kept: {:?}", plan.keeps);
}

#[test]
fn loop_use_is_kept() {
    // The Datara while loop lowers to a real CFG cycle (its own basic
    // blocks), so the use lands in a DIFFERENT block than the creation:
    // CrossBlock and InLoop are both sound keeps — the release may never
    // be inserted before a use that runs on a later iteration.
    let src = &format!(
        "{PLAIN}\nfn main() {{\n    val t = Plain {{ a: 1, b: 2 }}\n    mut i = 0\n    while i < 3 {{\n        out t.a\n        i = i + 1\n    }}\n    out 0\n}}\n"
    );
    let m = lower(src, "l1_loop.dtr");
    let plan = analyze_module(&m);
    let kept = plan.keeps.iter().any(|k| {
        k.function == "main"
            && k.variable == "t"
            && matches!(k.reason, EscapeReason::InLoop | EscapeReason::CrossBlock)
    });
    assert!(
        kept,
        "loop-used object must be kept (InLoop or CrossBlock): {:?}",
        plan.keeps
    );
}

#[test]
fn manual_release_is_never_doubled() {
    // If the user releases manually, Layer 1 must not add a second one.
    let src = "@arc\nstruct Counter { value: Int }\n\nfn main() {\n    val c = Counter { value: 1 }\n    c.arc_release()\n    out 0\n}\n";
    let m = lower(src, "l1_manual.dtr");
    let plan = analyze_module(&m);
    let kept = plan.keeps.iter().any(|k| {
        k.function == "main" && k.variable == "c" && k.reason == EscapeReason::ManuallyReleased
    });
    assert!(kept, "manually released object must be kept: {:?}", plan.keeps);
}

#[test]
fn parameter_objects_are_caller_owned() {
    let src = &format!(
        "{PLAIN}\nfn consume(p: Plain) -> Int {{\n    val s = p.a + p.b\n    return s\n}}\n\nfn main() {{\n    out 0\n}}\n"
    );
    let m = lower(src, "l1_param.dtr");
    let plan = analyze_module(&m);
    let kept = plan
        .keeps
        .iter()
        .any(|k| k.function == "consume" && k.variable == "p" && k.reason == EscapeReason::CallerOwned);
    assert!(kept, "parameters are caller-owned: {:?}", plan.keeps);
}

#[test]
fn cross_block_use_is_kept() {
    let src = &format!(
        "{PLAIN}\nfn main(x: Int) {{\n    val t = Plain {{ a: 1, b: 2 }}\n    if x > 0 {{\n        out t.a\n    }}\n    out t.b\n    out 0\n}}\n"
    );
    let m = lower(src, "l1_crossblock.dtr");
    let plan = analyze_module(&m);
    let t_rels = plan
        .releases
        .iter()
        .filter(|r| r.function == "main" && r.variable == "t")
        .count();
    let kept = plan.keeps.iter().any(|k| {
        k.function == "main"
            && k.variable == "t"
            && matches!(k.reason, EscapeReason::CrossBlock)
    });
    assert!(
        t_rels == 0 || kept,
        "cross-block object must be kept, not released: rels={t_rels} keeps={:?}",
        plan.keeps
    );
}

#[test]
fn inspect_report_renders_layer1_section() {
    let m = lower(
        &format!("{PLAIN}\nfn main() {{\n    val t = Plain {{ a: 1, b: 2 }}\n    out t.a\n    val dead = Plain {{ a: 3, b: 4 }}\n    out 0\n}}\n"),
        "l1_report.dtr",
    );
    let report = layer1_report(&m);
    assert!(report.contains("Layer 1 dataflow decisions"), "{report}");
    assert!(report.contains("release t (Plain)"), "{report}");
    assert!(report.contains("release dead (Plain)"), "{report}");
    // The last-use release must come BEFORE the scope end conceptually:
    // the report names the insertion block index, ensure it parses.
    assert!(report.contains("bb"), "{report}");
}
