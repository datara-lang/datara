//! M9 supply chain: dpm lock + sha256/merkle digest + tamper detection +
//! reproducible lock (PLAN_V150 M9).

use forgen::project::pm::manifest::{DataraLock, HyperGridPackage};
use forgen::project::pm::registry::HyperGridRegistry;
use forgen::project::pm::verify::VerificationStatus;
use std::collections::HashMap;
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forgen_supply_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_pkg(name: &str, version: &str, body: &str) -> HyperGridPackage {
    let mut files = HashMap::new();
    files.insert(
        "main.dtr".to_string(),
        format!("fn helper() -> Int {{ return {} }}\n", body),
    );
    files.insert("package.json".to_string(), format!("{{\"name\":\"{}\"}}", name));
    HyperGridPackage {
        name: name.to_string(),
        version: version.to_string(),
        description: "test".into(),
        author: "test".into(),
        license: "MIT".into(),
        digest: String::new(),
        capabilities: vec![],
        dependencies: vec![],
        entry: "main.dtr".into(),
        files,
    }
}

/// [1] Reproducible lock: two clean installs in SEPARATE project roots with
///    identical package content produce IDENTICAL digests in datara.lock
///    (digest is content-derived, not path/time-derived).
#[test]
fn lock_reproducible_two_installs() {
    let registry = HyperGridRegistry::new();
    let pkg = make_pkg("repro_pkg", "1.0.0", "7");

    let root_a = temp_root("a");
    let root_b = temp_root("b");
    registry.install_with_source(&pkg, &root_a, "local://repro_pkg").expect("install A");
    registry.install_with_source(&pkg, &root_b, "local://repro_pkg").expect("install B");

    let lock_a = DataraLock::load(&root_a).expect("lock A");
    let lock_b = DataraLock::load(&root_b).expect("lock B");
    let da = lock_a.packages.get("repro_pkg").expect("entry A").digest.clone();
    let db = lock_b.packages.get("repro_pkg").expect("entry B").digest.clone();
    assert!(!da.is_empty(), "digest must be recorded");
    assert_eq!(da, db, "identical content must yield identical digests");

    let _ = std::fs::remove_dir_all(&root_a);
    let _ = std::fs::remove_dir_all(&root_b);
}

/// [2] Tamper detection: flipping ONE byte of an installed package file makes
/// `dpm verify` report a Mismatch (fail-closed).
#[test]
fn verify_detects_tampered_byte() {
    let registry = HyperGridRegistry::new();
    let pkg = make_pkg("tamper_pkg", "1.0.0", "3");
    let root = temp_root("tamper");
    registry.install_with_source(&pkg, &root, "local://tamper_pkg").expect("install");

    // Clean verify passes
    let results = registry.verify(&root).expect("verify runs");
    assert!(results.iter().all(|r| matches!(
        r.status,
        VerificationStatus::Valid
    )),
    "fresh install must verify clean: {:?}",
    results.iter().map(|r| (r.name.clone(), r.status.clone())).collect::<Vec<_>>());

    // Tamper: flip one byte (append a char changes digest)
    let target = root.join("packages").join("tamper_pkg").join("main.dtr");
    let content = std::fs::read_to_string(&target).unwrap();
    std::fs::write(&target, content.replace("return 3", "return 4")).unwrap();

    let results = registry.verify(&root).expect("verify runs");
    let tampered = results.iter().find(|r| r.name == "tamper_pkg").unwrap();
    assert_eq!(
        tampered.status,
        VerificationStatus::Mismatch,
        "one flipped byte must be detected"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// [3] Tamper detection at INSTALL time: the registry refuses a package whose
/// recorded sha256 doesn't match its bytes.
#[test]
fn install_refuses_checksum_mismatch() {
    let data = b"authentic package payload";
    let good_digest = format!("sha256:{}", forgen::project::pm::crypto::sha256_hexdigest(data));
    HyperGridRegistry::verify_sha256(data, &good_digest)
        .expect("matching digest must pass");
    let err = HyperGridRegistry::verify_sha256(b"tampered payload", &good_digest)
        .expect_err("mismatched digest must be rejected");
    assert!(err.contains("mismatch"), "error must name the mismatch: {}", err);
}

/// [4] Untracked package under packages/ (no lock entry) is reported —
/// a tampered package cannot hide by removing its lock entry.
#[test]
fn verify_reports_untracked_package() {
    let registry = HyperGridRegistry::new();
    let pkg = make_pkg("tracked_pkg", "1.0.0", "1");
    let root = temp_root("untracked");
    registry.install_with_source(&pkg, &root, "local://tracked_pkg").expect("install");

    // Plant an untracked directory
    let ghost = root.join("packages").join("ghost_pkg");
    std::fs::create_dir_all(&ghost).unwrap();
    std::fs::write(ghost.join("evil.dtr"), "fn evil() {}").unwrap();

    let results = registry.verify(&root).expect("verify runs");
    let untracked = results
        .iter()
        .find(|r| r.name == "ghost_pkg")
        .expect("untracked package must be reported");
    assert!(matches!(
        untracked.status,
        VerificationStatus::Untracked
    ));

    let _ = std::fs::remove_dir_all(&root);
}

/// 5. Missing package directory is reported as Missing.
#[test]
fn verify_reports_missing_package() {
    let registry = HyperGridRegistry::new();
    let pkg = make_pkg("missing_pkg", "1.0.0", "2");
    let root = temp_root("missing");
    registry.install_with_source(&pkg, &root, "local://missing_pkg").expect("install");

    let dir = root.join("packages").join("missing_pkg");
    std::fs::remove_dir_all(&dir).unwrap();

    let results = registry.verify(&root).expect("verify runs");
    let missing = results.iter().find(|r| r.name == "missing_pkg").unwrap();
    assert!(matches!(
        missing.status,
        VerificationStatus::Missing
    ));

    let _ = std::fs::remove_dir_all(&root);
}

/// [6] Lock file is stable across saves (BTreeMap ordering → reproducible
/// toml bytes for the same content).
#[test]
fn lock_serialization_deterministic() {
    let mut lock_a = DataraLock::default();
    let mut lock_b = DataraLock::default();
    let entries = [
        ("alpha", "1.0.0", "sha256:aaa"),
        ("beta", "2.0.0", "sha256:bbb"),
        ("gamma", "0.1.0", "sha256:ccc"),
    ];
    for (n, v, d) in entries {
        lock_a.insert_or_update(n, v, d, "local", Vec::new());
        // Insert in REVERSE order into lock_b
    }
    for (n, v, d) in entries.iter().rev() {
        lock_b.insert_or_update(n, v, d, "local", Vec::new());
    }
    let a = toml::to_string_pretty(&lock_a).unwrap();
    let b = toml::to_string_pretty(&lock_b).unwrap();
    assert_eq!(a, b, "lock serialization must be order-independent");
}

