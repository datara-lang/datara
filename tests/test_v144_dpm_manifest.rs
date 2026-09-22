//! v1.4.4: Unified multi-language dependency manifest (dpm.toml) tests.

use forgen::project::dpm::DpmManifest;

#[test]
fn test_v144_dpm_manifest_parsing_all_ecosystems() {
    let toml_str = r#"
[package]
name = "hyper_service"
version = "1.4.4"
description = "Universal polyglot microservice"

[dependencies]
stdlib = "1.4.4"

[c-dependencies]
sqlite3 = { version = "3.45", windows = "sqlite3.lib", linux = "libsqlite3.a" }

[cpp-dependencies]
fast_simd = { header = "simd.hpp", link = "fast_simd.lib" }

[rust-dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }

[python-dependencies]
numpy = ">=1.24"
torch = ">=2.0"

[npm-dependencies]
express = "^4.18"

[go-dependencies]
cryptoutil = { path = "./go/cryptoutil.go", buildmode = "c-shared" }

[dotnet-dependencies]
fastmath = { path = "./dotnet/FastMath.csproj", aot = true }
"#;

    let manifest: DpmManifest = toml::from_str(toml_str).expect("parse dpm.toml");
    assert_eq!(manifest.package.name, "hyper_service");
    assert_eq!(manifest.package.version, "1.4.4");

    // Verify all ecosystem dependency tables
    assert!(manifest.c_dependencies.contains_key("sqlite3"));
    assert!(manifest.cpp_dependencies.contains_key("fast_simd"));
    assert!(manifest.rust_dependencies.contains_key("serde"));
    assert!(manifest.rust_dependencies.contains_key("tokio"));
    assert!(manifest.python_dependencies.contains_key("numpy"));
    assert!(manifest.npm_dependencies.contains_key("express"));
    assert!(manifest.go_dependencies.contains_key("cryptoutil"));
    assert!(manifest.dotnet_dependencies.contains_key("fastmath"));

    let tokio_dep = manifest
        .rust_dependencies
        .get("tokio")
        .and_then(|v| v.detailed())
        .expect("expected detailed tokio dependency");
    assert_eq!(tokio_dep.version.as_deref(), Some("1.35"));
    assert_eq!(tokio_dep.features.as_deref(), Some(&["full".to_string()][..]));

    let go_dep = manifest
        .go_dependencies
        .get("cryptoutil")
        .and_then(|v| v.detailed())
        .expect("expected detailed go dependency");
    assert_eq!(go_dep.buildmode.as_deref(), Some("c-shared"));
}
