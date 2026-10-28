//! Gate for docs/STDLIB_ON_DATARA.md — "part of the stdlib is written IN
//! Datara" (PLAN_V150 M1).
//!
//! Two halves:
//! 1. Purity: the listed Datara-written stdlib modules must exist, be
//!    non-trivial, and contain no `import c` / `unsafe` (pure Datara over
//!    compiler builtins). `stdlib/sys/sys.dtr` is allowed exactly its
//!    documented `unsafe(justification:)` wrapper for `sys_exec`.
//! 2. End-to-end: a probe program using text + sort + path + diagnostics
//!    together is compiled and run; runtime results are asserted.

use forgen::driver::ForgenCompiler;

/// Pure-Datara stdlib modules (no FFI, no unsafe, no extern fns).
const PURE_DATARA_MODULES: &[&str] = &[
    "stdlib/text/text.dtr",
    "stdlib/collections/sort.dtr",
    "stdlib/collections/iter.dtr",
    "stdlib/io/path.dtr",
    "stdlib/diagnostics/diagnostic.dtr",
];

/// Wrapper-layer modules: Datara code standing on the C runtime builtins.
/// `sys.dtr` carries the single documented `unsafe(justification:)` for the
/// escalated `sys_exec` capability; `fs.dtr` is the guarded capability layer.
const WRAPPER_DATARA_MODULES: &[&str] = &["stdlib/io/fs.dtr", "stdlib/sys/sys.dtr"];

fn repo_root() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is the crate root (= repo root for this project).
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_module(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("stdlib module {} must exist: {}", rel, e))
}

#[test]
fn pure_datara_modules_exist_and_are_non_trivial() {
    for rel in PURE_DATARA_MODULES {
        let src = read_module(rel);
        let lines = src.lines().count();
        assert!(
            lines > 50,
            "{} must be a real module (>50 lines), got {}",
            rel,
            lines
        );
    }
}

#[test]
fn pure_datara_modules_contain_no_ffi() {
    for rel in PURE_DATARA_MODULES {
        let src = read_module(rel);
        assert!(
            !src.contains("import c"),
            "{} must not import C headers (pure Datara module)",
            rel
        );
        assert!(
            !src.contains("unsafe("),
            "{} must not contain unsafe blocks",
            rel
        );
        assert!(
            !src.lines().any(|l| l.trim_start().starts_with("extern fn")),
            "{} must not declare extern fn bindings",
            rel
        );
    }
}

#[test]
fn wrapper_modules_are_datara_with_documented_unsafe_only() {
    for rel in WRAPPER_DATARA_MODULES {
        let src = read_module(rel);
        let lines = src.lines().count();
        assert!(lines > 20, "{} must be non-trivial, got {}", rel, lines);
        assert!(
            !src.contains("import c"),
            "{} must not import C headers (builtins only)",
            rel
        );
    }
    // sys.dtr: every unsafe( must be a documented escalation wrapper.
    let sys = read_module("stdlib/sys/sys.dtr");
    let unsafe_count = sys.matches("unsafe(").count();
    assert!(
        unsafe_count >= 1,
        "stdlib/sys/sys.dtr must wrap its escalated ops in unsafe(justification:)"
    );
    for line in sys.lines() {
        if line.contains("unsafe(") {
            assert!(
                line.contains("justification:"),
                "every unsafe in stdlib/sys/sys.dtr must carry a justification: {}",
                line.trim()
            );
        }
    }
}

/// End-to-end: one program using text + sort + path + diagnostics together.
const PROBE: &str = r#"
use stdlib.text.text
use stdlib.collections.sort
use stdlib.io.path
use stdlib.diagnostics.diagnostic

fn main() -> Int {
    // checked parse with or-fallback (stdlib.text)
    val n = text_parse_int("41") or -1
    if n != 41 { return 1 }
    val bad = text_parse_int("zzz") or -1
    if bad != -1 { return 2 }

    // UTF-8 codepoint slice (stdlib.text)
    val w = text_slice_cp("привет", 0, 3)
    if w != "при" { return 3 }

    // stable sort over generics (stdlib.collections.sort)
    val xs = [5, 3, 9, 1]
    val sorted = sort_stable(xs)
    if sorted.get(0) != 1 || sorted.get(3) != 9 { return 4 }
    if !is_sorted(sorted) { return 5 }
    if binary_search(sorted, 9) != 3 { return 6 }

    // path decomposition (stdlib.io.path)
    if path_basename("src/main.dtr") != "main.dtr" { return 7 }
    if path_parent("src/main.dtr") != "src" { return 8 }
    if path_extension("a/b.txt") != "txt" { return 9 }

    // diagnostics render in the compiler format (stdlib.diagnostics)
    val d = Diagnostic.warning("W-TEST-001", "synthetic warning")
    if d.is_error() { return 10 }
    val e = Diagnostic.error_at("E-TEST-001", "synthetic error", 3, 7, "do the right thing")
    val r = e.render()
    if str_contains(r, "3:7") != 1 || str_contains(r, "error[E-TEST-001]") != 1 { return 11 }

    return 0
}
"#;

#[test]
fn probe_program_uses_all_proof_modules_end_to_end() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(PROBE, "stdlib_on_datara_probe.dtr", None);
    assert!(
        res.success,
        "probe must compile: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.unwrap();
    let (stdout, stderr, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    if code != 0 {
        eprintln!("stdout: {}\nstderr: {}", stdout, stderr);
    }
    assert_eq!(code, 0, "probe must run cleanly; stdout: {}", stdout);
}
