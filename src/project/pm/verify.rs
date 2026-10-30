use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub digest: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationStatus {
    Valid,
    Mismatch,
    Missing,
    /// Directory present in packages/ but absent from datara.lock.
    Untracked,
}

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub name: String,
    pub version: String,
    pub status: VerificationStatus,
    pub message: String,
}

/// Detached Ed25519 signature status for a package bundle (PLAN_V151 N3).
/// A bundle is signed when a `<bundle>.sig` file sits next to it; the trust
/// decision comes from `$DATARA_HOME/trust/<pk>.pk` presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureStatus {
    /// No .sig file — unsigned publish (legacy behavior).
    Unsigned,
    /// Signature present, key trusted, signature valid over the bundle.
    Valid(String),
    /// Signature present but the key is not in the trust-store.
    Untrusted(String),
    /// Signature present, key trusted, but verification FAILED — tamper.
    Invalid(String),
}
