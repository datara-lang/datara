use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    pub entry: Option<String>,
    pub authors: Option<Vec<String>>,
    pub description: Option<String>,
    pub edition: Option<String>,
    pub license: Option<String>,
}

use crate::rust_bridge::config::RustCrateConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencyConfig {
    Simple(String),
    Detailed {
        version: Option<String>,
        path: Option<String>,
        git: Option<String>,
    },
    RustGroup(HashMap<String, RustCrateConfig>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetConfig {
    pub bin_name: Option<String>,
    pub arch: Option<String>,
    pub os: Option<String>,
    pub opt_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    pub opt_level: Option<String>,
    pub debug_info: Option<bool>,
    pub pgo: Option<bool>,
    pub lto: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataraManifest {
    pub package: PackageMeta,
    #[serde(default)]
    pub dependencies: HashMap<String, DependencyConfig>,
    #[serde(default, rename = "rust_dependencies")]
    pub explicit_rust_deps: HashMap<String, RustCrateConfig>,
    pub target: Option<TargetConfig>,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
    #[serde(default)]
    pub capabilities: Option<CapabilitiesConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    FsRead,
    FsWrite,
    NetListen,
    NetConnect,
    Env,
    Exec,
}

impl CapabilityKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::FsRead => "fs-read",
            Self::FsWrite => "fs-write",
            Self::NetListen => "net-listen",
            Self::NetConnect => "net-connect",
            Self::Env => "env",
            Self::Exec => "exec",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilitiesConfig {
    #[serde(default, alias = "fs_read", rename = "fs-read")]
    pub fs_read: Vec<String>,
    #[serde(default, alias = "fs_write", rename = "fs-write")]
    pub fs_write: Vec<String>,
    #[serde(default, alias = "net_listen", rename = "net-listen")]
    pub net_listen: Vec<String>,
    #[serde(default, alias = "net_connect", rename = "net-connect")]
    pub net_connect: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub exec: Vec<String>,
}

impl CapabilitiesConfig {
    pub fn has_category(&self, kind: CapabilityKind) -> bool {
        !self.patterns(kind).is_empty()
    }

    pub fn patterns(&self, kind: CapabilityKind) -> &[String] {
        match kind {
            CapabilityKind::FsRead => &self.fs_read,
            CapabilityKind::FsWrite => &self.fs_write,
            CapabilityKind::NetListen => &self.net_listen,
            CapabilityKind::NetConnect => &self.net_connect,
            CapabilityKind::Env => &self.env,
            CapabilityKind::Exec => &self.exec,
        }
    }

    pub fn matches(&self, kind: CapabilityKind, target: &str) -> bool {
        let patterns = self.patterns(kind);
        if patterns.is_empty() {
            return false;
        }
        for pat in patterns {
            if matches_glob(pat, target, kind) {
                return true;
            }
        }
        false
    }
}

pub fn matches_glob(pattern: &str, target: &str, kind: CapabilityKind) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    let is_path = matches!(kind, CapabilityKind::FsRead | CapabilityKind::FsWrite);
    let p_norm = if is_path {
        normalize_path(pattern)
    } else {
        pattern.to_string()
    };
    let t_norm = if is_path {
        normalize_path(target)
    } else {
        target.to_string()
    };

    if p_norm == t_norm {
        return true;
    }

    if kind == CapabilityKind::Exec {
        let prog = target.split_whitespace().next().unwrap_or(target);
        if pattern == prog || wildcard_match(pattern, prog) {
            return true;
        }
    }

    if is_path {
        if let Some(prefix) = p_norm.strip_suffix("/**") {
            if t_norm.starts_with(prefix) {
                let rem = &t_norm[prefix.len()..];
                if rem.is_empty() || rem.starts_with('/') {
                    return true;
                }
            }
        }
        if let Some(prefix) = p_norm.strip_suffix("/*") {
            if t_norm.starts_with(prefix) {
                let rem = &t_norm[prefix.len()..];
                if rem.starts_with('/') && !rem[1..].contains('/') {
                    return true;
                }
            }
        }
    }

    wildcard_match(&p_norm, &t_norm)
}

fn normalize_path(p: &str) -> String {
    let replaced = p.replace('\\', "/");
    let trimmed = replaced.trim_start_matches("./");
    trimmed.to_string()
}

fn wildcard_match(pat: &str, s: &str) -> bool {
    let pat_chars: Vec<char> = pat.chars().collect();
    let s_chars: Vec<char> = s.chars().collect();
    let mut p_idx = 0;
    let mut s_idx = 0;
    let mut star_idx = None;
    let mut match_idx = 0;

    while s_idx < s_chars.len() {
        if p_idx < pat_chars.len()
            && (pat_chars[p_idx] == '?' || pat_chars[p_idx] == s_chars[s_idx])
        {
            p_idx += 1;
            s_idx += 1;
        } else if p_idx < pat_chars.len() && pat_chars[p_idx] == '*' {
            star_idx = Some(p_idx);
            p_idx += 1;
            match_idx = s_idx;
        } else if let Some(star) = star_idx {
            p_idx = star + 1;
            match_idx += 1;
            s_idx = match_idx;
        } else {
            return false;
        }
    }

    while p_idx < pat_chars.len() && pat_chars[p_idx] == '*' {
        p_idx += 1;
    }

    p_idx == pat_chars.len()
}

pub fn validate_semver(version: &str) -> Result<(), String> {
    if version.is_empty() {
        return Err("Package version cannot be empty".to_string());
    }

    // Split build metadata
    let (core_and_pre, _build) = match version.split_once('+') {
        Some((c, b)) => {
            if b.is_empty()
                || !b.split('.').all(|id| {
                    !id.is_empty() && id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
                })
            {
                return Err(format!(
                    "Invalid semver build metadata '+{}' in '{}'",
                    b, version
                ));
            }
            (c, Some(b))
        }
        None => (version, None),
    };

    // Split prerelease
    let (core, _prerelease) = match core_and_pre.split_once('-') {
        Some((c, p)) => {
            if p.is_empty() {
                return Err(format!("Invalid empty semver prerelease in '{}'", version));
            }
            for id in p.split('.') {
                if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
                    return Err(format!(
                        "Invalid semver prerelease identifier '{}' in '{}'",
                        id, version
                    ));
                }
                if id.chars().all(|ch| ch.is_ascii_digit()) && id.len() > 1 && id.starts_with('0') {
                    return Err(format!(
                        "Leading zero in numeric semver prerelease '{}' in '{}'",
                        id, version
                    ));
                }
            }
            (c, Some(p))
        }
        None => (core_and_pre, None),
    };

    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return Err(format!(
            "Package version '{}' is not valid semver: expected MAJOR.MINOR.PATCH (e.g. '0.1.0')",
            version
        ));
    }

    for (part_name, part) in [
        ("major", parts[0]),
        ("minor", parts[1]),
        ("patch", parts[2]),
    ] {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "Invalid semver {} component '{}' in '{}' (must be a non-negative integer)",
                part_name, part, version
            ));
        }
        if part.len() > 1 && part.starts_with('0') {
            return Err(format!(
                "Invalid semver {} component '{}' in '{}': leading zeroes are forbidden",
                part_name, part, version
            ));
        }
    }

    Ok(())
}

pub fn validate_edition(edition: &str) -> Result<(), String> {
    match edition {
        "2024" | "2025" | "2026" => Ok(()),
        other => Err(format!(
            "Unsupported edition '{}'. Supported editions: '2024', '2025', '2026'",
            other
        )),
    }
}

impl DataraManifest {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read manifest '{}': {}", path.display(), e))?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, String> {
        let mut manifest: Self = toml::from_str(content)
            .map_err(|e| format!("Invalid datara.toml manifest format: {}", e))?;

        // Enforce semver validation on package version
        validate_semver(&manifest.package.version)?;

        // Enforce edition validation
        if let Some(ref ed) = manifest.package.edition {
            validate_edition(ed)?;
        } else {
            manifest.package.edition = Some("2026".to_string());
        }

        if let Ok(value) = toml::from_str::<toml::Value>(content) {
            if let Some(deps) = value.get("dependencies").and_then(|d| d.as_table()) {
                if let Some(rust_val) = deps.get("rust") {
                    if let Ok(rust_group) = rust_val
                        .clone()
                        .try_into::<HashMap<String, RustCrateConfig>>()
                    {
                        manifest.explicit_rust_deps.extend(rust_group);
                    }
                }
            }
            if let Some(rust_val) = value.get("rust_dependencies") {
                if let Ok(rust_group) = rust_val
                    .clone()
                    .try_into::<HashMap<String, RustCrateConfig>>()
                {
                    manifest.explicit_rust_deps.extend(rust_group);
                }
            }
        }

        Ok(manifest)
    }

    /// Extract all Rust crate dependencies from [dependencies.rust] or [rust_dependencies].
    pub fn rust_dependencies(&self) -> HashMap<String, RustCrateConfig> {
        let mut rust_deps = self.explicit_rust_deps.clone();
        if let Some(DependencyConfig::RustGroup(group)) = self.dependencies.get("rust") {
            for (k, v) in group {
                rust_deps.insert(k.clone(), v.clone());
            }
        }
        rust_deps
    }

    pub fn default_template(name: &str) -> String {
        format!(
            r#"[package]
name = "{}"
version = "1.0.0"
entry = "src/main.dtr"
edition = "2026"
description = "A high-performance Datara application"

[dependencies]
# core = "1.0.0"

[target]
# bin_name = "{}"
# opt_level = "domain"

[profiles.release]
opt_level = "3"
lto = true

[profiles.domain]
opt_level = "domain"
pgo = true
lto = true
"#,
            name, name
        )
    }
}
