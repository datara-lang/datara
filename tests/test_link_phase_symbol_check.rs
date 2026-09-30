//! M4 (PLAN_V150): link-phase duplicate symbol detection with E-LINK-001.
//!
//! Before this phase, `lower_program` inserted into `module.functions` via
//! `HashMap::insert`, which silently overwrote on a name collision: a
//! top-level `fn Foo_bar` and the mangled name of `Foo.bar` (a method)
//! resolved to one link symbol and one body was lost. The check pipeline now
//! reports the collision with E-LINK-001 instead of producing a wrong binary
//! or an opaque backend failure.

use forgen::driver::{Interner, SymbolId, SymbolTable};
use forgen::driver::symbols::{check_module_link, mangle_method, mangle_specialization};
use forgen::diagnostics::DiagnosticEngine;
use forgen::dmir::ir::Module;

#[test]
fn interner_returns_stable_dense_ids() {
    let mut i = Interner::with_capacity(8);
    let a: SymbolId = i.intern("alpha");
    let b = i.intern("beta");
    let a2 = i.intern("alpha");
    assert_eq!(a, a2);
    assert_ne!(a, b);
    assert_eq!(i.resolve(a), Some("alpha"));
    assert_eq!(i.resolve(b), Some("beta"));
    assert_eq!(i.len(), 2);
    // An ID from a different interner resolves to None (defensive).
    let other = Interner::new();
    assert_eq!(other.resolve(a), None);
}

#[test]
fn mangling_is_stable_and_colliding_by_construction() {
    // The documented v1.5.0 mangling scheme: Type_method and fn_T1_T2.
    assert_eq!(mangle_method("Vec2", "len"), "Vec2_len");
    assert_eq!(mangle_specialization("pair_swap", &["Int", "Str"]), "pair_swap_Int_Str");
    // A top-level function with exactly this name collides by construction.
    assert_eq!(mangle_method("Vec2", "len"), "Vec2_len");
}

#[test]
fn duplicate_symbol_yields_elink001_diagnostic() {
    let mut table = SymbolTable::new();
    table.define(
        "Foo_bar",
        "function",
        Some(forgen::diagnostics::SourceSpan::point(1, 1, "a.dtr".into())),
    );
    table.define(
        "Foo_bar",
        "method",
        Some(forgen::diagnostics::SourceSpan::point(9, 5, "b.dtr".into())),
    );
    let module = Module::new("t");
    let mut diag = DiagnosticEngine::new("en");
    let n = check_module_link(&module, &table, &mut diag);
    assert_eq!(n, 1, "collision must be reported exactly once");
    assert!(diag.has_errors());
    let d = &diag.diagnostics[0];
    assert_eq!(d.code, "E-LINK-001");
    assert!(d.message.contains("Foo_bar"), "message names the symbol");
    assert!(d.message.contains("a.dtr:1"), "message anchors the first definition");
    assert!(d.help.is_some());
}

#[test]
fn alias_reinsert_is_not_reported() {
    // Behavior lowering inserts the same body under `Type_m` and `B_Type_m`;
    // a same-name/same-span re-insert is an alias, not a collision.
    let span = forgen::diagnostics::SourceSpan::point(3, 5, "m.dtr".into());
    let mut table = SymbolTable::new();
    table.define("Type_m", "method", Some(span.clone()));
    table.define("Type_m", "method", Some(span));
    let module = Module::new("t");
    let mut diag = DiagnosticEngine::new("en");
    assert_eq!(check_module_link(&module, &table, &mut diag), 0);
    assert!(!diag.has_errors());
}

#[test]
fn unique_program_yields_no_diagnostics() {
    let mut table = SymbolTable::new();
    table.define("main", "function", None);
    table.define("Vec2_len", "method", None);
    table.define("pair_swap_Int", "specialization", None);
    let module = Module::new("t");
    let mut diag = DiagnosticEngine::new("en");
    assert_eq!(check_module_link(&module, &table, &mut diag), 0);
    assert!(!diag.has_errors());
}

/// End-to-end: a source file where a top-level function's name equals the
/// mangled name of a class method must fail `check` with E-LINK-001 (and
/// not compile to a binary that silently lost one body).
#[test]
fn end_to_end_collision_fails_check_with_elink001() {
    let dir = std::env::temp_dir().join(format!("forgen_elink_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("collision.dtr");
    std::fs::write(
        &file,
        "fn Foo_bar(x: Int) -> Int {\n    return x + 1\n}\n\nstruct Foo {\n    value: Int\n}\n\nbehavior Foo {\n    fn bar(x: Int) -> Int {\n        return x + 1\n    }\n}\n\nfn main() -> Int {\n    val f = Foo { value: 5 }\n    return Foo_bar(1) + f.bar(2)\n}\n",
    )
    .unwrap();

    let compiler = forgen::driver::ForgenCompiler::new("check");
    let result = compiler.check_file(&file);
    assert!(
        !result.success,
        "collision program must not pass check"
    );
    assert!(
        result.diagnostics.contains("E-LINK-001"),
        "expected E-LINK-001 in check diagnostics, got:\n{}",
        result.diagnostics
    );

    // The unique program still passes: no false positives.
    let ok_file = dir.join("unique.dtr");
    std::fs::write(
        &ok_file,
        "fn foo_bar_free(x: Int) -> Int {\n    return x + 1\n}\n\nstruct Foo {\n    value: Int\n}\n\nbehavior Foo {\n    fn bar(x: Int) -> Int {\n        return x + 1\n    }\n}\n\nfn main() -> Int {\n    val f = Foo { value: 5 }\n    return foo_bar_free(1) + f.bar(2)\n}\n",
    )
    .unwrap();
    let result_ok = compiler.check_file(&ok_file);
    assert!(
        result_ok.success,
        "unique program must pass check, got:\n{}",
        result_ok.diagnostics
    );

    let _ = std::fs::remove_dir_all(&dir);
}
