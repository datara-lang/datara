//! v1.4.0 `forgen doctor --bridges` interop audit harness.
//!
//! Runs the audit in-process: every bridge probe is a real compile + run
//! through the normal pipeline. The gate asserts:
//! * the C bridge scenarios 1-3 (roundtrip, primitives, aggregate) pass
//!   natively on this machine (MSVC linker discovery needs
//!   `ProgramFiles(x86)`, restored here when the harness strips it),
//! * Python and JS scenarios are SKIP-or-PASS, never FAIL - the gate must
//!   not depend on interpreters being installed.

use forgen::driver::ForgenCompiler;
use forgen::interop_audit::{Verdict, run_bridge_audit};

/// Windows linker discovery reads `ProgramFiles(x86)` to locate
/// vswhere/BuildTools. Some harnesses (sandboxed shells, stripped CI
/// containers) drop the variable entirely; restore the conventional
/// defaults so the machine paths resolve. Mirrors the hardcoded fallback
/// the other native tests use.
static MSVC_ENV: std::sync::Once = std::sync::Once::new();

fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| {
        // SAFETY: `set_var` is process-global; the call happens once,
        // only when the variable is absent, and before any linker
        // subprocess is spawned by this test process.
        unsafe {
            if std::env::var_os("ProgramFiles(x86)").is_none() {
                std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
            }
            if std::env::var_os("ProgramFiles").is_none() {
                std::env::set_var("ProgramFiles", r"C:\Program Files");
            }
        }
    });
    // Touch the compiler once so the linker cache and runtime lib paths are
    // initialized in the same environment the probes will see.
    let _ = ForgenCompiler::new("release");
}

#[test]
fn c_bridge_core_scenarios_pass() {
    ensure_msvc_env();
    let scenarios = run_bridge_audit();
    for name in ["roundtrip", "primitives", "aggregate"] {
        let s = scenarios
            .iter()
            .find(|s| s.bridge == "C" && s.scenario == name)
            .unwrap_or_else(|| panic!("C scenario '{}' missing from audit", name));
        assert_eq!(
            s.verdict,
            Verdict::Pass,
            "C scenario '{}' must pass, detail: {}",
            name,
            s.detail
        );
    }
}

#[test]
fn c_bridge_error_transfer_and_leak_smoke_pass() {
    ensure_msvc_env();
    let scenarios = run_bridge_audit();
    for name in ["error_transfer", "leak_smoke"] {
        let s = scenarios
            .iter()
            .find(|s| s.bridge == "C" && s.scenario == name)
            .unwrap_or_else(|| panic!("C scenario '{}' missing from audit", name));
        assert_eq!(
            s.verdict,
            Verdict::Pass,
            "C scenario '{}' must pass, detail: {}",
            name,
            s.detail
        );
    }
}

#[test]
fn python_bridge_is_skip_or_pass() {
    ensure_msvc_env();
    let scenarios = run_bridge_audit();
    let py: Vec<_> = scenarios.iter().filter(|s| s.bridge == "PY").collect();
    assert!(!py.is_empty(), "python bridge scenarios must be present");
    for s in &py {
        assert_ne!(
            s.verdict,
            Verdict::Fail,
            "python scenario '{}' must not FAIL when the interpreter is absent (detail: {})",
            s.scenario,
            s.detail
        );
    }
}

#[test]
fn js_bridge_is_skip_or_pass() {
    ensure_msvc_env();
    let scenarios = run_bridge_audit();
    let js: Vec<_> = scenarios.iter().filter(|s| s.bridge == "JS").collect();
    assert!(!js.is_empty(), "js bridge scenarios must be present");
    for s in &js {
        assert_ne!(
            s.verdict,
            Verdict::Fail,
            "js scenario '{}' must not FAIL when the engine is absent (detail: {})",
            s.scenario,
            s.detail
        );
    }
}

#[test]
fn rust_bridge_toolchain_is_skip_or_pass() {
    let scenarios = run_bridge_audit();
    let toolchain = scenarios
        .iter()
        .find(|s| s.bridge == "RUST" && s.scenario == "toolchain")
        .expect("rust toolchain scenario must be present");
    assert_ne!(
        toolchain.verdict,
        Verdict::Fail,
        "rust toolchain probe must not FAIL, detail: {}",
        toolchain.detail
    );
}

#[test]
fn audit_covers_the_full_matrix() {
    let scenarios = run_bridge_audit();
    for bridge in ["C", "PY", "JS", "RUST"] {
        let count = scenarios.iter().filter(|s| s.bridge == bridge).count();
        assert!(
            count >= 2,
            "bridge {} must report at least 2 scenarios, got {}",
            bridge,
            count
        );
    }
}
