//! Version-sync guard (B5).
//!
//! The version lives in three places that historically drifted apart:
//! the `VERSION` file, `Cargo.toml`, and the human-readable README badges
//! (English and Russian). This test fails the suite when any of them lags
//! behind the release version, so the drift is caught by CI instead of by
//! users reading stale docs.

use std::path::Path;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

/// Extracts all `version-X.Y.Z-blue.svg` (or the localized Russian
/// `версия-X.Y.Z-blue.svg`) badge versions from a README.
fn badge_versions(readme: &str) -> Vec<String> {
    readme
        .lines()
        .filter_map(|line| {
            let marker = ["version-", "версия-"]
                .iter()
                .find_map(|m| line.find(m).map(|i| (i, m.len())))?;
            let rest = &line[marker.0 + marker.1..];
            let end = rest.find("-blue.svg")?;
            Some(rest[..end].to_string())
        })
        .collect()
}

#[test]
fn version_file_matches_cargo_toml() {
    let version_file = read(&repo_root().join("VERSION"));
    let version = version_file.trim();
    assert!(
        !version.is_empty(),
        "VERSION file must contain the release version"
    );

    let cargo = read(&repo_root().join("Cargo.toml"));
    let cargo_version = cargo
        .lines()
        .find_map(|l| l.strip_prefix("version = "))
        .map(|v| v.trim().trim_matches('"').to_string())
        .expect("Cargo.toml must define a package version");

    assert_eq!(
        version, cargo_version,
        "VERSION file and Cargo.toml package version are out of sync"
    );
}

#[test]
fn readme_badges_match_version() {
    let version_file = read(&repo_root().join("VERSION"));
    let version = version_file.trim();

    for readme in ["README.md", "README_RU.md"] {
        let content = read(&repo_root().join(readme));
        let badges = badge_versions(&content);
        assert!(
            !badges.is_empty(),
            "{} must carry at least one version badge",
            readme
        );
        for badge in &badges {
            assert_eq!(
                badge, version,
                "{} version badge `{}` is out of sync with VERSION (`{}`)",
                readme, badge, version
            );
        }
    }
}
