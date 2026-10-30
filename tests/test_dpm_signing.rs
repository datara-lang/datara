//! PLAN_V151 N3: detached Ed25519 bundle signatures. `dpm publish` signs
//! the bundle when a signing key is configured (env or
//! `$DATARA_HOME/keys/*.sk`); `dpm verify` checks the signature against the
//! trust-store (`$DATARA_HOME/trust/<pk>.pk`).

use forgen::project::pm::crypto::{
    create_ed25519_keypair, pubkey_from_sk, sign_ed25519_message, verify_ed25519_signature,
};
use forgen::project::pm::registry::HyperGridRegistry;
use forgen::project::pm::{DataraLock, LockedPackage, SignatureStatus, VerificationStatus};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forgen_dpm_sign_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_keypair_roundtrip_and_pubkey_derivation() {
    let (pk, sk) = create_ed25519_keypair();
    assert_eq!(pk.len(), 64, "public key must be 32 bytes hex");
    assert_eq!(sk.len(), 128, "secret key must be 64 bytes (seed+pk) hex");
    assert_eq!(pubkey_from_sk(&sk).unwrap(), pk, "pk must derive from sk");

    let msg = b"bundle-json-bytes";
    let sig = sign_ed25519_message(msg, &sk).unwrap();
    assert!(verify_ed25519_signature(msg, &sig, &pk).is_ok());
    // A single flipped bit must fail verification.
    let tampered_msg = b"bundle-json-bytes!";
    assert!(verify_ed25519_signature(tampered_msg, &sig, &pk).is_err());
}

#[test]
fn test_sign_and_verify_bundle_paths() {
    let dir = temp_dir("e2e");
    let trust_dir = dir.join("trust");
    fs::create_dir_all(&trust_dir).unwrap();
    let (pk, sk) = create_ed25519_keypair();

    let bundle = dir.join("pkg-0.1.0.dtr-pkg");
    fs::write(&bundle, "{\"name\":\"pkg\",\"files\":{}}").unwrap();

    // Unsigned before signing.
    assert_eq!(
        HyperGridRegistry::verify_bundle_signature(&bundle, &trust_dir),
        SignatureStatus::Unsigned
    );

    // Sign: detached .sig appears; verification is untrusted until the key
    // enters the trust-store.
    HyperGridRegistry::sign_bundle_file(&bundle, &sk).unwrap();
    let sig_path = bundle.with_extension("dtr-pkg.sig");
    assert!(sig_path.exists(), "detached .sig must be written");
    assert_eq!(
        HyperGridRegistry::verify_bundle_signature(&bundle, &trust_dir),
        SignatureStatus::Untrusted(pk.clone()),
        "signed but not yet trusted"
    );

    // Trust the key: valid signature now.
    fs::write(trust_dir.join(format!("{}.pk", pk)), format!("{}\n", pk)).unwrap();
    assert_eq!(
        HyperGridRegistry::verify_bundle_signature(&bundle, &trust_dir),
        SignatureStatus::Valid(pk.clone())
    );

    // Tamper the BUNDLE (keep the old .sig): verification must fail.
    let sig_bytes = fs::read(&sig_path).unwrap();
    fs::write(
        &bundle,
        "{\"name\":\"pkg\",\"files\":{\"lib.dtr\":\"evil\"}}",
    )
    .unwrap();
    fs::write(&sig_path, sig_bytes).unwrap();
    match HyperGridRegistry::verify_bundle_signature(&bundle, &trust_dir) {
        SignatureStatus::Invalid(_) => {}
        other => panic!("tampered bundle must be Invalid, got {:?}", other),
    }
}

#[test]
fn test_registry_verify_escalates_invalid_signature() {
    // Full-registry path: a trusted-but-invalid .sig escalates the locked
    // package to VerificationStatus::Mismatch even with a matching digest.
    let dir = temp_dir("registry");
    let trust_dir = dir.join("trust");
    fs::create_dir_all(&trust_dir).unwrap();
    let (pk, sk) = create_ed25519_keypair();
    fs::write(trust_dir.join(format!("{}.pk", pk)), format!("{}\n", pk)).unwrap();

    let mut registry = HyperGridRegistry::new();
    registry.store_path = dir.join("store");
    let bundles = registry.store_path.join("bundles");
    fs::create_dir_all(&bundles).unwrap();

    // Project with one locked package whose files match the bundle.
    let proj = dir.join("proj");
    let pkg_dir = proj.join("packages").join("pkg");
    fs::create_dir_all(&pkg_dir).unwrap();
    let lib_src = "fn hello() {}\n";
    fs::write(pkg_dir.join("lib.dtr"), lib_src).unwrap();

    let mut files: HashMap<String, String> = HashMap::new();
    files.insert("lib.dtr".to_string(), lib_src.to_string());
    let digest = HyperGridRegistry::compute_merkle_root(&files);
    let bundle_json = serde_json::json!({
        "name": "pkg", "version": "0.1.0", "description": "t",
        "author": "t", "license": "MIT", "digest": digest,
        "capabilities": [], "dependencies": [], "entry": "lib.dtr",
        "files": files,
    })
    .to_string();
    let bundle = bundles.join("pkg-0.1.0.dtr-pkg");
    fs::write(&bundle, &bundle_json).unwrap();

    let mut lock = DataraLock::default();
    lock.packages.insert(
        "pkg".to_string(),
        LockedPackage {
            version: "0.1.0".to_string(),
            digest: digest.clone(),
            source: "local".to_string(),
            dependencies: Vec::new(),
        },
    );
    lock.save(&proj).unwrap();

    // Valid signature → Valid, message mentions the signature.
    HyperGridRegistry::sign_bundle_file(&bundle, &sk).unwrap();
    let results = registry.verify(&proj).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, VerificationStatus::Valid);
    assert!(
        results[0].message.contains("ed25519 signature verified"),
        "message must mention the signature: {}",
        results[0].message
    );

    // Corrupt the signature → Mismatch escalation.
    let sig_line = fs::read_to_string(bundle.with_extension("dtr-pkg.sig")).unwrap();
    let parts: Vec<&str> = sig_line.trim().split(':').collect();
    let bad_sig = "ab".repeat(64);
    fs::write(
        bundle.with_extension("dtr-pkg.sig"),
        format!("ed25519:{}:{}\n", parts[1], bad_sig),
    )
    .unwrap();
    let results = registry.verify(&proj).unwrap();
    assert_eq!(results[0].status, VerificationStatus::Mismatch);
    assert!(results[0].message.contains("SIGNATURE INVALID"));
}
