//! M6 incremental lifetime cache (PLAN_V150 M6 item "Инкрементальность:
//! lifetime-граф кэшируется по модулям, перестройка только изменённых").
//!
//! The cache is a per-function memo keyed by the structural fingerprint of
//! the exact analysis read surface (name, alloc_hint, params, block
//! structure) plus the module class sets. Any change — a global ValueId
//! shift included — remaps the key, so only changed functions are rebuilt.
//!
//! The `bench_analyze_*` tests are #[ignore]d: run them manually for the
//! A/B evidence (they print timings and must not assert on wall time):
//!   cargo test --release --test test_lifetime_cache -- --ignored --nocapture
//!   DATARA_NO_LIFETIME_CACHE=1 cargo test --release --test test_lifetime_cache -- --ignored --nocapture

use forgen::dmir::{verify_module, Lowering};
use forgen::diagnostics::DiagnosticEngine;
use forgen::lexer::Lexer;
use forgen::ownership::{
    analyze_module, insert_releases, lifetime_cache_reset, lifetime_cache_stats,
};
use forgen::parser::Parser;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

const TWO_FN_SRC: &str = "struct P { a: Int, b: Int }\n\nfn first() -> Int {\n    val t = P { a: 10, b: 20 }\n    val s = t.a + t.b\n    out s\n    return s\n}\n\nfn second() -> Int {\n    val u = P { a: 1, b: 2 }\n    val w = u.a + u.b\n    out w\n    return w\n}\n";

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

#[test]
fn identical_module_reuses_function_plans() {
    let m1 = lower(TWO_FN_SRC, "lc_same_1.dtr");
    let m2 = lower(TWO_FN_SRC, "lc_same_2.dtr");
    // Deterministic lowering: identical sources must produce identical IR
    // (the cache key depends on it).
    assert_eq!(m1.functions.len(), m2.functions.len());
    let (hits0, _) = lifetime_cache_stats();

    let plan1 = analyze_module(&m1);
    let (hits1, misses1) = lifetime_cache_stats();
    let plan2 = analyze_module(&m2);
    let (hits2, misses2) = lifetime_cache_stats();

    // First analysis primes the cache, second analysis must be ALL hits.
    assert_eq!(
        misses1, misses2,
        "second identical analysis must add no misses"
    );
    assert!(
        hits2 >= hits1 + m2.functions.len() as u64,
        "every function must be served from the memo: before {hits1}/{misses1}, after {hits2}/{misses2}"
    );
    // Cached plans must be byte-identical to the primed ones.
    assert_eq!(plan1.releases.len(), plan2.releases.len());
    assert_eq!(plan1.keeps.len(), plan2.keeps.len());
    for (a, b) in plan1.releases.iter().zip(plan2.releases.iter()) {
        assert_eq!(a.function, b.function);
        assert_eq!(a.block, b.block);
        assert_eq!(a.after_index, b.after_index);
        assert_eq!(a.value, b.value);
        assert_eq!(a.variable, b.variable);
        assert_eq!(a.class, b.class);
        assert_eq!(a.is_arc, b.is_arc);
    }
    let _ = hits0;
}

#[test]
fn changed_function_is_rebuilt_unchanged_reused() {
    let before = lower(TWO_FN_SRC, "lc_chg_before.dtr");
    analyze_module(&before);
    let (h0, m0) = lifetime_cache_stats();

    // Change ONLY the second function (it is lowered after `first`, so the
    // first function's global ValueIds are untouched).
    let modified = TWO_FN_SRC.replace(
        "fn second() -> Int {\n    val u = P { a: 1, b: 2 }",
        "fn second() -> Int {\n    val u = P { a: 1, b: 2 }\n    val extra = u.a * 2\n    out extra",
    );
    assert_ne!(modified, TWO_FN_SRC);
    let after = lower(&modified, "lc_chg_after.dtr");
    analyze_module(&after);
    let (h1, m1) = lifetime_cache_stats();

    assert!(
        h1 > h0,
        "unchanged `first` must be a cache hit: {h0}/{m0} -> {h1}/{m1}"
    );
    assert!(
        m1 > m0,
        "changed `second` must be rebuilt: {h0}/{m0} -> {h1}/{m1}"
    );
}

#[test]
fn cached_plans_equal_uncached_analysis() {
    lifetime_cache_reset();
    let m = lower(TWO_FN_SRC, "lc_equal.dtr");
    let cached = analyze_module(&m);
    // Uncached reference: the public per-function API.
    let mut refs = std::collections::BTreeMap::new();
    for (name, f) in &m.functions {
        let fl = forgen::ownership::dataflow::analyze_function(
            f,
            &m.arc_classes,
            &m.cyclic_classes,
        );
        refs.insert(name.clone(), fl);
    }
    for r in &cached.releases {
        let fl = &refs[&r.function];
        assert!(
            fl.releases
                .iter()
                .any(|x| x.block == r.block && x.after_index == r.after_index && x.value == r.value),
            "release decision must be identical with and without the memo"
        );
    }
    for k in &cached.keeps {
        let fl = &refs[&k.function];
        assert!(
            fl.keeps.iter().any(|x| x.variable == k.variable
                && x.class == k.class
                && x.reason == k.reason),
            "keep decision must be identical with and without the memo"
        );
    }
}

#[test]
fn double_insert_releases_stays_single_release_with_cache() {
    // The idempotency guarantee of M6.3 must survive the memo: after the
    // first insert the IR changed (release calls added), so every function
    // fingerprint differs and the second pass re-analyzes from scratch —
    // and still must not double-release.
    let mut m = lower(TWO_FN_SRC, "lc_idem.dtr");
    insert_releases(&mut m);
    let count_first: usize = m
        .functions
        .values()
        .map(|f| {
            f.blocks
                .iter()
                .map(|b| {
                    b.instructions
                        .iter()
                        .filter(|i| {
                            matches!(i, forgen::dmir::Inst::Call { func, .. }
                                if func == "datara_rt_free"
                                    || func == "datara_rt_arc_release")
                        })
                        .count()
                })
                .sum::<usize>()
        })
        .sum();
    assert!(count_first > 0, "the source must produce releases");
    insert_releases(&mut m);
    let count_second: usize = m
        .functions
        .values()
        .map(|f| {
            f.blocks
                .iter()
                .map(|b| {
                    b.instructions
                        .iter()
                        .filter(|i| {
                            matches!(i, forgen::dmir::Inst::Call { func, .. }
                                if func == "datara_rt_free"
                                    || func == "datara_rt_arc_release")
                        })
                        .count()
                })
                .sum::<usize>()
        })
        .sum();
    assert_eq!(
        count_first, count_second,
        "second insert must be a no-op even with the memo active"
    );
}

// ---------------------------------------------------------------------------
// A/B benchmark evidence for the plan checkbox. #[ignore]d by default.
// ---------------------------------------------------------------------------

fn bench_once(source: &str, rounds: usize) -> u128 {
    let m = lower(source, "lc_bench.dtr");
    let start = std::time::Instant::now();
    for _ in 0..rounds {
        let _ = analyze_module(&m);
    }
    start.elapsed().as_micros()
}

/// Generates a module with `n` functions over a local struct (each owning
/// one release decision) — the shape the memo is designed for.
fn wide_module(n: usize) -> String {
    let mut s = String::from("struct P { a: Int, b: Int }\n\n");
    for i in 0..n {
        s.push_str(&format!(
            "fn gen_{i}() -> Int {{\n    val t = P {{ a: {i}, b: {i} }}\n    val s = t.a + t.b\n    out s\n    return s\n}}\n\n"
        ));
    }
    s.push_str("fn main() -> Int {\n    return gen_0()\n}\n");
    s
}

#[test]
#[ignore]
fn bench_analyze_hot_vs_cold() {
    let src = wide_module(40);
    let rounds = 200;

    lifetime_cache_reset();
    let cold_us = bench_once(&src, rounds);
    let hot_us = bench_once(&src, rounds);
    let (hits, misses) = lifetime_cache_stats();
    println!(
        "[lifetime-cache] warm: {hot_us} us for {rounds} rounds ({hits} hits / {misses} misses)"
    );
    println!("[lifetime-cache] fresh (after reset, still memoized): {cold_us} us");
}
