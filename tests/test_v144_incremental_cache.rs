//! v1.4.4 Track 3: Incremental Check Cache Integration Tests.

use forgen::driver::{CheckCache, ForgenCompiler};
use std::time::Instant;

#[test]
fn test_incremental_check_cache_lifecycle() {
    // 1. Clean cache state
    CheckCache::clear();

    let compiler = ForgenCompiler::new("check");
    let source1 = r#"
fn add(a: Int, b: Int) -> Int {
    return a + b
}

fn main() {
    let x: Int = add(10, 20)
}
"#;

    // 2. Cold check (cache miss)
    let t0 = Instant::now();
    let res1 = compiler.check_source(source1, "test_cache.dtr");
    let cold_duration = t0.elapsed();
    assert!(res1.success, "Cold check must succeed: {}", res1.diagnostics);

    // 3. Warm check (cache hit)
    let t1 = Instant::now();
    let res2 = compiler.check_source(source1, "test_cache.dtr");
    let warm_duration = t1.elapsed();
    assert!(res2.success, "Warm check must succeed");

    // The warm check must be dramatically faster (cache hit reading JSON vs full lex/parse/typecheck/borrow)
    println!("Cold check: {:?}, Warm check: {:?}", cold_duration, warm_duration);
    assert!(warm_duration < cold_duration, "Warm check must be faster than cold check");

    // 4. Content modification invalidates cache
    let source2 = r#"
fn add(a: Int, b: Int) -> Int {
    return a + b + 1
}

fn main() {
    let x: Int = add(10, 20)
}
"#;
    let res3 = compiler.check_source(source2, "test_cache.dtr");
    assert!(res3.success, "Modified check must succeed");

    // 5. Error caching and replay
    let bad_source = r#"
fn bad() -> Int {
    return "type mismatch"
}
"#;
    let res_bad1 = compiler.check_source(bad_source, "test_bad.dtr");
    assert!(!res_bad1.success, "Initial bad check must fail");

    let res_bad2 = compiler.check_source(bad_source, "test_bad.dtr");
    assert!(!res_bad2.success, "Cached bad check must also report failure");
    assert_eq!(res_bad1.diagnostics, res_bad2.diagnostics, "Cached error output must match");

    // 6. Cache bypass with FORGEN_NO_CACHE=1
    unsafe {
        std::env::set_var("FORGEN_NO_CACHE", "1");
    }
    assert!(CheckCache::is_disabled());
    let res_bypass = compiler.check_source(source1, "test_cache.dtr");
    assert!(res_bypass.success);
    unsafe {
        std::env::remove_var("FORGEN_NO_CACHE");
    }
    assert!(!CheckCache::is_disabled());

    // 7. Clear cache
    CheckCache::clear();
}
