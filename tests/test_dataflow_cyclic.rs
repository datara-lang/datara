//! M6.2 auto-region: cyclic ownership structures become region-resident
//! (PLAN_V150, M6.2 item "Тесты").
//!
//! Scenarios:
//! 1. Layer-1 classification: classes in a strong ownership cycle
//!    (mutual field edges A↔B, or a container self-loop `next: List<Node>`)
//!    produce `Keep { reason: CyclicStructure }` decisions and NEVER a
//!    release decision — the region owns the lifetime instead.
//! 2. In-frame cycle built via field assignment (the historical probe
//!    cycle2.dtr shape) and a cycle returned from a callee (escape_ret.dtr
//!    shape) keep both objects region-resident.
//! 3. `@arc` classes keep their own Layer-2 lifetime: no CyclicStructure
//!    keep decision is emitted for them.
//! 4. End-to-end: a program that builds a cycle in `mk()` and returns it
//!    compiles natively and exits cleanly (regression for the SEGFAULT
//!    caused by the first-cut structural escape pass missing the LoadVar
//!    alias flow).

use forgen::dmir::{verify_module, Lowering};
use forgen::diagnostics::DiagnosticEngine;
use forgen::driver::ForgenCompiler;
use forgen::lexer::Lexer;
use forgen::ownership::dataflow::{analyze_module, EscapeReason};
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

const CYCLE_CLASSES: &str = "struct A { v: Int, peer: B }\n\nstruct B { v: Int, peer: A }\n";

#[test]
fn in_frame_cycle_is_region_resident_without_releases() {
    // The historical cycle2.dtr shape: the cycle is closed in main via
    // `a.peer = b; b.peer = a`. Both objects must be KEPT with the
    // CyclicStructure reason and no release decision may exist for them.
    let m = lower(
        &format!(
            "{CYCLE_CLASSES}\nfn main() {{\n    mut a = A {{ v: 1 }}\n    mut b = B {{ v: 2 }}\n    a.peer = b\n    b.peer = a\n    out a.v\n    out a.peer.v\n    return\n}}\n"
        ),
        "l2_in_frame.dtr",
    );
    assert!(
        m.cyclic_classes.contains("A") && m.cyclic_classes.contains("B"),
        "A and B must be detected as cyclic classes, got {:?}",
        m.cyclic_classes
    );
    let plan = analyze_module(&m);
    let a_keeps: Vec<_> = plan
        .keeps
        .iter()
        .filter(|k| k.class == "A")
        .collect();
    let b_keeps: Vec<_> = plan
        .keeps
        .iter()
        .filter(|k| k.class == "B")
        .collect();
    assert!(
        a_keeps
            .iter()
            .any(|k| k.reason == EscapeReason::CyclicStructure),
        "A must be kept as a cyclic structure: {:?}",
        plan.keeps
    );
    assert!(
        b_keeps
            .iter()
            .any(|k| k.reason == EscapeReason::CyclicStructure),
        "B must be kept as a cyclic structure: {:?}",
        plan.keeps
    );
    let cyclic_releases: Vec<_> = plan
        .releases
        .iter()
        .filter(|r| r.class == "A" || r.class == "B")
        .collect();
    assert!(
        cyclic_releases.is_empty(),
        "cyclic-class objects must never receive Layer-1 releases: {:?}",
        cyclic_releases
    );
}

#[test]
fn returned_cycle_keeps_region_residency_via_alias_flow() {
    // The escape_ret.dtr shape: the cycle is built inside mk() and RETURNED
    // through a LoadVar copy. The alias-flow escape analysis must still keep
    // both objects region-resident — the region outlives the frame via the
    // caller, so no release may fire inside mk().
    let m = lower(
        &format!(
            "{CYCLE_CLASSES}\nfn mk() -> A {{\n    mut a = A {{ v: 1 }}\n    mut b = B {{ v: 2 }}\n    a.peer = b\n    b.peer = a\n    return a\n}}\n\nfn main() {{\n    val a = mk()\n    out a.v\n    return\n}}\n"
        ),
        "l2_returned.dtr",
    );
    assert!(m.cyclic_classes.contains("A") && m.cyclic_classes.contains("B"));
    let plan = analyze_module(&m);
    let mk_keeps: Vec<_> = plan
        .keeps
        .iter()
        .filter(|k| k.function == "mk" && k.reason == EscapeReason::CyclicStructure)
        .collect();
    assert!(
        mk_keeps.iter().any(|k| k.class == "A")
            && mk_keeps.iter().any(|k| k.class == "B"),
        "returned cycle must keep BOTH objects region-resident in mk(): {:?}",
        plan.keeps
    );
    let mk_releases: Vec<_> = plan
        .releases
        .iter()
        .filter(|r| r.function == "mk" && (r.class == "A" || r.class == "B"))
        .collect();
    assert!(
        mk_releases.is_empty(),
        "no release may fire inside mk() for the returned cycle: {:?}",
        mk_releases
    );
}

#[test]
fn acyclic_class_keeps_historical_release_path() {
    // A struct that is NOT part of an ownership cycle keeps the historical
    // Layer-1 lifetime (released at last use) — the relaxation must not
    // leak onto non-cyclic classes.
    let m = lower(
        "struct P { x: Int, y: Int }\n\nfn main() {\n    val t = P { x: 1, y: 2 }\n    val s = t.x + t.y\n    out s\n    return\n}\n",
        "l2_acyclic.dtr",
    );
    assert!(
        m.cyclic_classes.is_empty(),
        "P is acyclic: {:?}",
        m.cyclic_classes
    );
    let plan = analyze_module(&m);
    assert!(
        !plan.keeps
            .iter()
            .any(|k| k.class == "P" && k.reason == EscapeReason::CyclicStructure),
        "acyclic P must not be kept with CyclicStructure: {:?}",
        plan.keeps
    );
}

#[test]
fn arc_classes_are_not_region_converted() {
    // @arc classes own their lifetime through the refcount header (Layer 2);
    // a strong cycle is rejected by the TMM checker and a weak back-edge
    // breaks it — either way the region relaxation must not claim them.
    let src = "@arc\nstruct A { v: Int, b: B }\n\n@arc\nstruct B { v: Int, a: Weak<A> }\n\nfn main() {\n    out 1\n    return\n}\n";
    let m = lower(src, "l2_arc_weak.dtr");
    assert!(
        !m.cyclic_classes.contains("A") && !m.cyclic_classes.contains("B"),
        "@arc classes must stay out of the cyclic-region set: {:?}",
        m.cyclic_classes
    );
    let plan = analyze_module(&m);
    assert!(
        !plan.keeps
            .iter()
            .any(|k| (k.class == "A" || k.class == "B")
                && k.reason == EscapeReason::CyclicStructure),
        "@arc objects must not be region-converted: {:?}",
        plan.keeps
    );
}

#[test]
fn returned_cycle_compiles_and_runs_natively() {
    // End-to-end regression for the escape_ret.dtr SEGFAULT: the returned
    // cycle must compile (Cranelift) and exit 0 with the expected output.
    let src = "struct A {\n    v: Int\n    peer: B\n}\n\nstruct B {\n    v: Int\n    peer: A\n}\n\nfn mk() -> A {\n    mut a = A { v: 1 }\n    mut b = B { v: 2 }\n    a.peer = b\n    b.peer = a\n    return a\n}\n\nfn main() {\n    val a = mk()\n    out a.v\n    out a.peer.v\n    out a.peer.peer.v\n}\n";
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "test_dataflow_cyclic_ret.dtr", None);
    assert!(
        res.success,
        "returned-cycle program must compile: {:?} / {}",
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
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.first().copied(), Some("1"));
    assert_eq!(lines.get(1).copied(), Some("2"));
    assert_eq!(lines.get(2).copied(), Some("1"));
}
