use forgen::driver::check_cache::{CachedCheckRecord, CheckCache, CHECKER_SEMANTIC_VERSION};
use forgen::driver::ForgenCompiler;
use std::fs;

#[test]
fn test_cache_invalidated_on_semantic_version_mismatch() {
    let compiler = ForgenCompiler::new("check");
    let abi = compiler.struct_return_abi();
    let source = "fn add(a: Int, b: Int) -> Int => a + b\nfn main() {}";
    let file = "probe_semantic_cache.dtr";

    // 1. Initial check: populates cache
    let res = compiler.check_source(source, file);
    assert!(res.success, "Initial check must succeed");

    // 2. Locate cache entry on disk
    let (_, cache_path) = CheckCache::compute_key(source, file, abi);
    assert!(
        cache_path.exists(),
        "Cache file must be created at {}",
        cache_path.display()
    );

    // 3. Verify record has current semantic version
    let content = fs::read_to_string(&cache_path).expect("read cache file");
    let record: CachedCheckRecord = serde_json::from_str(&content).expect("parse cache json");
    assert_eq!(
        record.checker_semantic_version, CHECKER_SEMANTIC_VERSION,
        "Record must match current CHECKER_SEMANTIC_VERSION"
    );

    // 4. Cache hit with matching semantic version
    let hit = CheckCache::get(source, file, abi);
    assert!(hit.is_some(), "Cache get must hit when semantic version matches");

    // 5. Mutate cache record to simulate older compiler version / altered checker semantics
    let mut stale_record = record;
    stale_record.checker_semantic_version = "old-outdated-semantics-v0".to_string();
    let stale_json = serde_json::to_string(&stale_record).expect("serialize stale record");
    fs::write(&cache_path, stale_json).expect("write stale record");

    // 6. Verification: CheckCache::get must reject stale record and purge file!
    let miss = CheckCache::get(source, file, abi);
    assert!(
        miss.is_none(),
        "Stale cache with outdated semantic version must MISS"
    );
    assert!(
        !cache_path.exists(),
        "Stale cache file must be automatically removed on mismatch"
    );

    // 7. Recheck must succeed and write fresh record with current semantic version
    let fresh_res = compiler.check_source(source, file);
    assert!(fresh_res.success, "Fresh check after cache invalidation must succeed");
    assert!(cache_path.exists(), "Cache file must be regenerated");
    let fresh_content = fs::read_to_string(&cache_path).expect("read fresh cache file");
    let fresh_record: CachedCheckRecord = serde_json::from_str(&fresh_content).expect("parse fresh cache");
    assert_eq!(fresh_record.checker_semantic_version, CHECKER_SEMANTIC_VERSION);

    // Cleanup
    let _ = fs::remove_file(&cache_path);
}
