//! Detached Ed25519 bundle signing (PLAN_V151 N3) — extracted from
//! registry.rs for the 60 KB release size gate. Publish signs both bundle
//! copies when a signing key resolves; verify checks signatures against
//! the trust-store (`<store>/../trust/<pk>.pk`).

use super::crypto::{pubkey_from_sk, sign_ed25519_message, verify_ed25519_signature};
use super::registry::HyperGridRegistry;
use super::verify::SignatureStatus;
use std::fs;
use std::path::{Path, PathBuf};

impl HyperGridRegistry {
    /// Signing key resolution (PLAN_V151 N3), in order:
    /// 1. `DATARA_SIGN_KEY` env (hex secret key), `DATARA_SIGN_KEY_ID` names it;
    /// 2. the first `*.sk` hex file in `$DATARA_HOME/keys/` (its stem is the
    ///    key id, e.g. `datara-core-2026-v2.sk`).
    ///
    /// None → unsigned publish (legal, verify reports Unsigned).
    pub fn resolve_signing_key() -> Option<(String, String)> {
        if let Ok(sk) = std::env::var("DATARA_SIGN_KEY")
            && !sk.trim().is_empty()
        {
            let key_id = std::env::var("DATARA_SIGN_KEY_ID").unwrap_or_else(|_| "env".to_string());
            return Some((key_id, sk.trim().to_string()));
        }
        let home = std::env::var("DATARA_HOME").ok()?;
        let keys_dir = std::path::PathBuf::from(home).join("keys");
        let mut candidates: Vec<PathBuf> = fs::read_dir(&keys_dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("sk"))
            .collect();
        candidates.sort();
        let first = candidates.into_iter().next()?;
        let key_id = first
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("key")
            .to_string();
        let sk = fs::read_to_string(&first).ok()?;
        let sk = sk.trim().to_string();
        if sk.is_empty() {
            return None;
        }
        Some((key_id, sk))
    }

    /// Sign a bundle file in place: writes `<path>.sig` with the detached,
    /// self-describing signature line `ed25519:<pk-hex>:<sig-hex>`. Returns
    /// the signing public key hex.
    pub fn sign_bundle_file(bundle_path: &Path, sk_hex: &str) -> Result<String, String> {
        let bundle = fs::read(bundle_path)
            .map_err(|e| format!("cannot read bundle {}: {}", bundle_path.display(), e))?;
        let pk_hex = pubkey_from_sk(sk_hex)?;
        let sig_hex = sign_ed25519_message(&bundle, sk_hex)?;
        let sig_path = bundle_path.with_extension("dtr-pkg.sig");
        fs::write(&sig_path, format!("ed25519:{}:{}\n", pk_hex, sig_hex))
            .map_err(|e| format!("cannot write signature {}: {}", sig_path.display(), e))?;
        Ok(pk_hex)
    }

    /// Verify the detached signature of a bundle against the trust-store
    /// (`$DATARA_HOME/trust/<pk>.pk` presence = trust anchor).
    pub fn verify_bundle_signature(bundle_path: &Path, trust_dir: &Path) -> SignatureStatus {
        let sig_path = bundle_path.with_extension("dtr-pkg.sig");
        if !sig_path.exists() {
            return SignatureStatus::Unsigned;
        }
        let sig_line = match fs::read_to_string(&sig_path) {
            Ok(l) => l,
            Err(e) => {
                return SignatureStatus::Invalid(format!(
                    "unreadable signature {}: {}",
                    sig_path.display(),
                    e
                ));
            }
        };
        let parts: Vec<&str> = sig_line.trim().split(':').collect();
        if parts.len() != 3 || parts[0] != "ed25519" {
            return SignatureStatus::Invalid("malformed signature line".into());
        }
        let (pk_hex, sig_hex) = (parts[1], parts[2]);
        if !trust_dir.join(format!("{}.pk", pk_hex)).exists() {
            return SignatureStatus::Untrusted(pk_hex.to_string());
        }
        let bundle = match fs::read(bundle_path) {
            Ok(b) => b,
            Err(e) => {
                return SignatureStatus::Invalid(format!("unreadable bundle: {}", e));
            }
        };
        match verify_ed25519_signature(&bundle, sig_hex, pk_hex) {
            Ok(()) => SignatureStatus::Valid(pk_hex.to_string()),
            Err(e) => SignatureStatus::Invalid(e),
        }
    }

    /// The trust-store directory: derived from the store layout —
    /// `<home>/store` → `<home>/trust` — so an explicitly overridden
    /// store_path (tests, portable installs) keeps its own trust anchors.
    pub(crate) fn trust_store_dir(&self) -> PathBuf {
        self.store_path
            .parent()
            .map(|p| p.join("trust"))
            .unwrap_or_else(|| PathBuf::from("trust"))
    }
}
