//! Incremental check cache for `forgen check` (v1.4.4 Track 3).
//!
//! Caches diagnostic results in `.forgen_cache/check/` keyed by source hash,
//! file path, compiler version, and ABI configuration.
//! Provides sub-millisecond clean verification for unchanged source code.
//!
//! v1.5.0 (M4): sources with module `use` imports are cached too, via a
//! dependency fingerprint folded into the cache key. The fingerprint covers
//! the root import set plus the (path, content) pairs of every transitively
//! reached module file, so editing any dependency remaps the key instead of
//! serving stale diagnostics. The legacy dependency-blind path stays for
//! import-free sources only; foreign imports (`import c "..."`) are never
//! cached because header expansion reads files outside the fingerprint.

use crate::cimport::StructReturnAbi;
use crate::diagnostics::Diagnostic;
use crate::driver::pipeline::{CompilationResult, CompilationTimings};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

pub const CHECKER_SEMANTIC_VERSION: &str = match option_env!("FORGEN_BUILD_SEMANTIC_HASH") {
    Some(h) => h,
    None => "1.4.4-sem-v1",
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCheckRecord {
    pub success: bool,
    pub diagnostics: String,
    pub diagnostic_records: Vec<Diagnostic>,
    pub timings: CompilationTimings,
    pub source_hash: u64,
    pub abi: String,
    pub compiler_version: String,
    #[serde(default)]
    pub checker_semantic_version: String,
    /// v1.5.0 (M4): opt-tier the check ran under (Speed's aggressive passes
    /// can surface additional diagnostics, so results must not be shared
    /// across tiers).
    #[serde(default)]
    pub opt_tier: String,
    /// v1.5.0 (M4): fingerprint folded into the key. 0 = dependency-blind
    /// legacy entry (sources without module imports never carry one).
    #[serde(default)]
    pub dependency_fingerprint: u64,
}

pub struct CheckCache;

impl CheckCache {
    fn source_has_module_import(source: &str) -> bool {
        source.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("use ") || line.starts_with("use\t")
        })
    }

    fn source_has_foreign_import(source: &str) -> bool {
        source.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("import c")
                || line
                    .strip_prefix("import ")
                    .is_some_and(|rest| rest.starts_with('\"'))
        })
    }

    /// Legacy gate: only import-free sources may use the dependency-blind
    /// key (nothing outside the root source can influence their result).
    fn is_cacheable_source(source: &str) -> bool {
        !Self::source_has_module_import(source) && !Self::source_has_foreign_import(source)
    }

    /// Fingerprinted gate: module imports are fine (they are fingerprinted),
    /// but foreign imports are not (C/Rust headers are read during pipeline
    /// expansion and are outside the fingerprint).
    fn is_fingerprint_cacheable_source(source: &str) -> bool {
        !Self::source_has_foreign_import(source)
    }

    /// FNV-1a 64-bit hash for fast, deterministic, cross-platform hashing.
    pub fn hash_bytes(data: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for &b in data {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3u64);
        }
        hash
    }

    pub fn compute_key(source: &str, file: &str, abi: StructReturnAbi) -> (u64, PathBuf) {
        Self::compute_key_impl(source, file, abi, "", 0)
    }

    /// Key for a check that already knows its dependency fingerprint and
    /// optimization tier (the module-aware fast path in `check_source`).
    pub fn compute_key_with_deps(
        source: &str,
        file: &str,
        abi: StructReturnAbi,
        opt_tier: &str,
        dependency_fingerprint: u64,
    ) -> (u64, PathBuf) {
        Self::compute_key_impl(source, file, abi, opt_tier, dependency_fingerprint)
    }

    fn compute_key_impl(
        source: &str,
        file: &str,
        abi: StructReturnAbi,
        opt_tier: &str,
        dependency_fingerprint: u64,
    ) -> (u64, PathBuf) {
        let mut key_data = Vec::with_capacity(source.len() + file.len() + 128);
        key_data.extend_from_slice(source.as_bytes());
        key_data.push(0);
        key_data.extend_from_slice(file.as_bytes());
        key_data.push(0);
        key_data.extend_from_slice(format!("{:?}", abi).as_bytes());
        key_data.push(0);
        key_data.extend_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
        key_data.push(0);
        key_data.extend_from_slice(CHECKER_SEMANTIC_VERSION.as_bytes());
        // The tier and the transitive dependency fingerprint participate in
        // the key itself, so a dependency edit remaps the entry instead of
        // pointing the old one at diagnostics from a different dependency set.
        key_data.push(0);
        key_data.extend_from_slice(opt_tier.as_bytes());
        key_data.push(0);
        key_data.extend_from_slice(&dependency_fingerprint.to_le_bytes());

        // Include datara.toml if present in cwd to track dependency changes
        if let Ok(manifest) = fs::read("datara.toml") {
            key_data.push(0);
            key_data.extend_from_slice(&manifest);
        }

        let hash = Self::hash_bytes(&key_data);
        let cache_dir = Self::cache_dir();
        let path = cache_dir.join(format!("{:016x}.json", hash));
        (hash, path)
    }

    pub fn cache_dir() -> PathBuf {
        if let Ok(cwd) = std::env::current_dir() {
            cwd.join(".forgen_cache").join("check")
        } else {
            PathBuf::from(".forgen_cache").join("check")
        }
    }

    pub fn is_disabled() -> bool {
        std::env::var("FORGEN_NO_CACHE").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    }

    pub fn get(source: &str, file: &str, abi: StructReturnAbi) -> Option<CompilationResult> {
        if Self::is_disabled() || !Self::is_cacheable_source(source) {
            return None;
        }
        Self::get_impl(source, file, abi, "", 0)
    }

    /// v1.5.0 (M4): cache lookup for the module-aware fast path in
    /// `check_source`. Sources with foreign imports are never cached (their
    /// header expansion reads files outside the dependency fingerprint).
    pub fn get_with_deps(
        source: &str,
        file: &str,
        abi: StructReturnAbi,
        opt_tier: &str,
        dependency_fingerprint: u64,
    ) -> Option<CompilationResult> {
        if Self::is_disabled() || !Self::is_fingerprint_cacheable_source(source) {
            return None;
        }
        Self::get_impl(source, file, abi, opt_tier, dependency_fingerprint)
    }

    fn get_impl(
        source: &str,
        file: &str,
        abi: StructReturnAbi,
        opt_tier: &str,
        dependency_fingerprint: u64,
    ) -> Option<CompilationResult> {
        let start = Instant::now();
        let (source_hash, cache_file) =
            Self::compute_key_impl(source, file, abi, opt_tier, dependency_fingerprint);
        if !cache_file.exists() {
            return None;
        }

        let content = fs::read_to_string(&cache_file).ok()?;
        let record: CachedCheckRecord = serde_json::from_str(&content).ok()?;

        if record.compiler_version != env!("CARGO_PKG_VERSION")
            || record.checker_semantic_version != CHECKER_SEMANTIC_VERSION
            || record.source_hash != source_hash
        {
            let _ = fs::remove_file(&cache_file);
            return None;
        }

        let mut timings = record.timings;
        timings.total_ms = start.elapsed().as_millis();

        let program = if record.success {
            let mut dummy_diag = crate::diagnostics::DiagnosticEngine::new("en");
            crate::driver::pipeline::parse_single_source(
                source,
                file,
                &mut dummy_diag,
                crate::driver::pipeline::CompilationTimings::default(),
                start,
            )
            .ok()
            .map(|(p, _)| p)
        } else {
            None
        };

        Some(CompilationResult {
            success: record.success,
            exe_path: None,
            error: if !record.success {
                Some(record.diagnostics.clone())
            } else {
                None
            },
            program,
            semantic_graph: None,
            dmir_module: None,
            optimization_report: None,
            schedule_proof: None,
            diagnostics: record.diagnostics,
            diagnostic_records: record.diagnostic_records,
            clif_source: None,
            llvm_source: None,
            timings,
        })
    }

    pub fn put(source: &str, file: &str, abi: StructReturnAbi, result: &CompilationResult) {
        if Self::is_disabled() || !Self::is_cacheable_source(source) {
            return;
        }
        Self::put_impl(source, file, abi, result, "", 0);
    }

    /// v1.5.0 (M4): cache store for the module-aware fast path. See
    /// [`Self::get_with_deps`] for the foreign-import exclusion.
    pub fn put_with_deps(
        source: &str,
        file: &str,
        abi: StructReturnAbi,
        result: &CompilationResult,
        opt_tier: &str,
        dependency_fingerprint: u64,
    ) {
        if Self::is_disabled() || !Self::is_fingerprint_cacheable_source(source) {
            return;
        }
        Self::put_impl(source, file, abi, result, opt_tier, dependency_fingerprint);
    }

    fn put_impl(
        source: &str,
        file: &str,
        abi: StructReturnAbi,
        result: &CompilationResult,
        opt_tier: &str,
        dependency_fingerprint: u64,
    ) {
        let (source_hash, cache_file) =
            Self::compute_key_impl(source, file, abi, opt_tier, dependency_fingerprint);
        if let Some(parent) = cache_file.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let record = CachedCheckRecord {
            success: result.success,
            diagnostics: result.diagnostics.clone(),
            diagnostic_records: result.diagnostic_records.clone(),
            timings: result.timings.clone(),
            source_hash,
            abi: format!("{:?}", abi),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            checker_semantic_version: CHECKER_SEMANTIC_VERSION.to_string(),
            opt_tier: opt_tier.to_string(),
            dependency_fingerprint,
        };

        if let Ok(json) = serde_json::to_string(&record) {
            let _ = fs::write(&cache_file, json);
        }
    }

    pub fn clear() {
        let dir = Self::cache_dir();
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}
