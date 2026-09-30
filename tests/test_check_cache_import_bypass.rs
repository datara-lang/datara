use forgen::driver::check_cache::CheckCache;
use forgen::driver::ForgenCompiler;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct CacheSandbox(PathBuf);

impl CacheSandbox {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forgen_check_cache_imports_{}_{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&root).expect("create cache sandbox");
        Self(root)
    }
}

impl Drop for CacheSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// M4: a root source with module imports is cached under a dependency
/// fingerprint. The first check primes the fingerprinted entry; a repeat
/// check of the same unchanged project is a cache hit (sub-ms, same
/// diagnostics); editing a dependency file remaps the key, so the next
/// check re-runs the pipeline and observes the edit.
#[test]
fn imported_roots_are_cached_with_dependency_fingerprints_and_invalidated_on_edit() {
    let sandbox = CacheSandbox::new();
    let src = sandbox.0.join("src");
    fs::create_dir_all(&src).expect("create source directory");
    let main = src.join("main.dtr");
    let helper = src.join("helper.dtr");
    let source = "use helper.answer\nfn main() { out answer() }\n";
    fs::write(&main, source).expect("write root source");
    fs::write(&helper, "pub fn answer() -> Int => 40\n").expect("write helper v1");

    let compiler = ForgenCompiler::new("check");
    let first = compiler.check_file(&main);
    assert!(
        first.success,
        "initial imported project check failed: {}",
        first.diagnostics
    );

    // Entry exists under the fingerprinted key.
    let fingerprint = compiler
        .module_dependency_fingerprint_quick(source, main.to_str().unwrap())
        .expect("fingerprint must be derivable for a resolvable import");
    let tier = format!("{:?}", compiler.opt_tier);
    assert!(
        CheckCache::get_with_deps(
            source,
            main.to_str().unwrap(),
            compiler.struct_return_abi(),
            &tier,
            fingerprint
        )
        .is_some(),
        "root source with a module import must be cached under its dependency fingerprint"
    );

    // Unchanged repeat is a hit and reports the same success.
    let repeat = compiler.check_file(&main);
    assert!(repeat.success, "unchanged repeat check failed: {}", repeat.diagnostics);
    assert_eq!(repeat.success, first.success);

    // Direct dependency edit must be observed by check (key remaps).
    // The signature stays `-> Int` while the body returns a Str literal:
    // a type error inside the dependency itself.
    fs::write(&helper, "pub fn answer() -> Int => \"wrong\"\n")
        .expect("write invalid helper");
    let changed = compiler.check_file(&main);
    assert!(!changed.success, "dependency edit must be observed by check");
    assert!(changed.diagnostics.to_lowercase().contains("type"));

    // Transitive dependency edit (helper <- mid <- main) must also be
    // observed: the fingerprint walks nested imports of loaded modules.
    fs::write(&helper, "pub fn answer() -> Int => 40\n").expect("restore helper");
    let mid = src.join("mid.dtr");
    fs::write(&mid, "use helper.answer\npub fn double() -> Int => answer() * 2\n")
        .expect("write mid");
    let main_source = "use mid.double\nfn main() { out double() }\n";
    fs::write(&main, main_source).expect("write main v2");
    let ok = compiler.check_file(&main);
    assert!(ok.success, "transitive project check failed: {}", ok.diagnostics);

    // Restore helper to a valid-but-different value: mid's semantics change,
    // main's own text does not — the fingerprint must still remap the key.
    let fp_before = compiler
        .module_dependency_fingerprint_quick(main_source, main.to_str().unwrap())
        .expect("fingerprint derivable");
    fs::write(&helper, "pub fn answer() -> Int => 400\n").expect("write helper v3");
    let fp_after = compiler
        .module_dependency_fingerprint_quick(main_source, main.to_str().unwrap())
        .expect("fingerprint derivable");
    assert_ne!(
        fp_before, fp_after,
        "a transitive dependency edit must change the fingerprint"
    );
    let changed_transitively = compiler.check_file(&main);
    assert!(
        changed_transitively.success,
        "edited transitive dep must still typecheck: {}",
        changed_transitively.diagnostics
    );
}

/// M4 parity: a cache-served result must carry byte-identical diagnostics
/// to a fresh pipeline run of the same unchanged project — success flag,
/// rendered string, every structured record (code, severity, message, span,
/// help) included. This is the correctness contract of the fingerprinted
/// cache: a hit that drops or reorders a diagnostic is a silent false-pass.
#[test]
fn cached_diagnostics_parity_with_fresh_run_bit_for_bit() {
    // Error project: the structured records must survive a cache round-trip
    // exactly (this is where dropping records would be most damaging).
    let sandbox = CacheSandbox::new();
    let src = sandbox.0.join("src");
    fs::create_dir_all(&src).expect("create source directory");
    let main = src.join("parity.dtr");
    let helper = src.join("parity_helper.dtr");
    fs::write(&helper, "pub fn answer() -> Int => 1\n").expect("write helper");
    // Root references a missing function from the helper: E-RESOLVE path.
    let source = "use parity_helper.answer\nfn main() { out answer_two() }\n";
    fs::write(&main, source).expect("write root");

    let compiler = ForgenCompiler::new("check");
    let fresh = compiler.check_file(&main);
    assert!(!fresh.success, "project must fail: {}", fresh.diagnostics);
    // check_file put the failed result into the cache — a second run hits it.
    let cached = compiler.check_file(&main);
    assert!(!cached.success, "cached result must preserve failure");

    assert_eq!(
        fresh.success, cached.success,
        "success flag parity"
    );
    assert_eq!(
        fresh.diagnostics, cached.diagnostics,
        "rendered diagnostics must be byte-identical between fresh and cached"
    );
    assert_eq!(
        fresh.diagnostic_records.len(),
        cached.diagnostic_records.len(),
        "record count parity"
    );
    for (f, c) in fresh.diagnostic_records.iter().zip(&cached.diagnostic_records) {
        assert_eq!(f.code, c.code, "record code parity");
        assert_eq!(f.severity, c.severity, "record severity parity");
        assert_eq!(f.message, c.message, "record message parity");
        assert_eq!(f.span, c.span, "record span parity");
        assert_eq!(f.help, c.help, "record help parity");
    }

    // Clean project too: the success path must round-trip identically.
    let ok_source = "use parity_helper.answer\nfn main() { out answer() }\n";
    fs::write(&main, ok_source).expect("rewrite root clean");
    let fresh_ok = compiler.check_file(&main);
    assert!(fresh_ok.success, "clean project must pass: {}", fresh_ok.diagnostics);
    let cached_ok = compiler.check_file(&main);
    assert!(cached_ok.success, "cached clean result must pass");
    assert_eq!(fresh_ok.diagnostics, cached_ok.diagnostics, "clean diagnostics parity");
    assert_eq!(
        fresh_ok.diagnostic_records.len(),
        cached_ok.diagnostic_records.len(),
        "clean record count parity"
    );
}

#[test]
fn cacheable_comment_containing_use_is_not_mistaken_for_a_module_import() {
    let compiler = ForgenCompiler::new("check");
    let source = "// use helper.answer\nfn main() {}\n";
    let file = format!(
        "cache_comment_probe_{}_{}.dtr",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos()
    );
    let result = compiler.check_source(source, &file);
    assert!(result.success, "comment-only source check failed: {}", result.diagnostics);
    assert!(
        CheckCache::get(source, &file, compiler.struct_return_abi()).is_some(),
        "a comment containing `use` must not disable caching"
    );
    let _ = fs::remove_file(CheckCache::compute_key(source, &file, compiler.struct_return_abi()).1);
}

#[test]
fn unresolvable_import_is_never_cached() {
    let sandbox = CacheSandbox::new();
    let src = sandbox.0.join("src");
    fs::create_dir_all(&src).expect("create source directory");
    let main = src.join("broken.dtr");
    let source = "use missing_module.thing\nfn main() {}\n";
    fs::write(&main, source).expect("write broken root");

    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_file(&main);
    assert!(!res.success, "unresolvable import must fail the check");
    assert!(
        compiler
            .module_dependency_fingerprint_quick(source, main.to_str().unwrap())
            .is_none(),
        "unresolvable imports must yield no fingerprint (nothing cacheable)"
    );
}
