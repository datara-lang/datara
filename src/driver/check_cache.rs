//! Incremental check cache for `forgen check` (v1.4.4 Track 3).
//!
//! Caches diagnostic results in `.forgen_cache/check/` keyed by source hash,
//! file path, compiler version, and ABI configuration.
//! Provides sub-millisecond clean verification for unchanged source code.

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
}

pub struct CheckCache;

impl CheckCache {
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
        std::env::var("FORGEN_NO_CACHE")
            .map_or(false, |v| v == "1" || v.eq_ignore_ascii_case("true"))
    }

    pub fn get(source: &str, file: &str, abi: StructReturnAbi) -> Option<CompilationResult> {
        if Self::is_disabled() || source.contains("import c") || source.contains("import \"") {
            return None;
        }

        let start = Instant::now();
        let (source_hash, cache_file) = Self::compute_key(source, file, abi);
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
        if Self::is_disabled() || source.contains("import c") || source.contains("import \"") {
            return;
        }

        let (source_hash, cache_file) = Self::compute_key(source, file, abi);
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
