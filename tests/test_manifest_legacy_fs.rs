use forgen::project::manifest::{CapabilitiesConfig, CapabilityKind, DataraManifest};

#[test]
fn legacy_fs_shorthand_grants_read() {
    let m = DataraManifest::parse(
        "[package]\nname = \"t\"\nversion = \"0.1.0\"\n\n[capabilities]\nfs = [\"read\"]\n",
    )
    .expect("parse must succeed");
    let caps: &CapabilitiesConfig = m.capabilities.as_ref().expect("caps present");
    assert!(
        caps.has_category(CapabilityKind::FsRead),
        "legacy fs = [\"read\"] must grant fs-read"
    );
    assert!(
        !caps.has_category(CapabilityKind::FsWrite),
        "read-only shorthand must NOT grant fs-write"
    );
}

#[test]
fn legacy_fs_wildcard_grants_both() {
    let m = DataraManifest::parse(
        "[package]\nname = \"t\"\nversion = \"0.1.0\"\n\n[capabilities]\nfs = [\"*\"]\n",
    )
    .expect("parse must succeed");
    let caps = m.capabilities.as_ref().expect("caps present");
    assert!(caps.has_category(CapabilityKind::FsRead));
    assert!(caps.has_category(CapabilityKind::FsWrite));
}

#[test]
fn legacy_fs_write_grants_write_only() {
    let m = DataraManifest::parse(
        "[package]\nname = \"t\"\nversion = \"0.1.0\"\n\n[capabilities]\nfs = [\"write\"]\n",
    )
    .expect("parse must succeed");
    let caps = m.capabilities.as_ref().expect("caps present");
    assert!(!caps.has_category(CapabilityKind::FsRead));
    assert!(caps.has_category(CapabilityKind::FsWrite));
}

#[test]
fn canonical_keys_still_work() {
    let m = DataraManifest::parse(
        "[package]\nname = \"t\"\nversion = \"0.1.0\"\n\n[capabilities]\nfs-read = [\"*\"]\nfs-write = [\"*\"]\nenv = [\"*\"]\n",
    )
    .expect("parse must succeed");
    let caps = m.capabilities.as_ref().expect("caps present");
    assert!(caps.has_category(CapabilityKind::FsRead));
    assert!(caps.has_category(CapabilityKind::FsWrite));
    assert!(caps.has_category(CapabilityKind::Env));
    // legacy bucket must stay empty for canonical manifests
    assert!(caps.legacy_fs.is_empty());
}
