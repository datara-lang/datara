//! PLAN_V151 N3: `forgen bridge check` — static audit of foreign-function
//! bridges. Runs the real CLI binary (CARGO_BIN_EXE) over temp fixtures:
//! a valid bridge set must pass (exit 0), a wrong-arity C call must fail
//! with an E-BRIDGE-001 diagnostic at the call site, and a missing header
//! must fail with E-BRIDGE-002 at the import span.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forgen_bridge_check_{}_{}_{}",
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

fn run_bridge_check(dir: &std::path::Path, file: &str) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_forgen"))
        .args(["bridge", "check", file])
        .current_dir(dir)
        .output()
        .expect("bridge check must run");
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (text, out.status.code().unwrap_or(-1))
}

fn write(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

#[test]
fn test_bridge_check_passes_valid_bridges() {
    let dir = temp_dir("good");
    write(
        &dir,
        "good.h",
        "int add2(int a, int b);\ndouble scale(double x);\n",
    );
    write(
        &dir,
        "good.dtr",
        r#"import c "good.h" with link("good.lib");

bridge py::math {
    fn py_sqrt(x: Float) -> Float
}

fn main() {
    out int_to_str(add2(2, 3))
    out float_to_str(py_sqrt(4.0))
}
"#,
    );
    let (text, code) = run_bridge_check(&dir, "good.dtr");
    assert_eq!(code, 0, "valid bridges must pass: {text}");
    assert!(text.contains("Bridge check passed"), "{text}");
    assert!(
        text.contains("py::math"),
        "surface must list the bridge: {text}"
    );
    assert!(
        text.contains("good.h"),
        "surface must list the header: {text}"
    );
}

#[test]
fn test_bridge_check_flags_arity_mismatch_at_call_site() {
    let dir = temp_dir("arity");
    write(&dir, "good.h", "int add2(int a, int b);\n");
    write(
        &dir,
        "bad.dtr",
        r#"import c "good.h" with link("good.lib");

fn main() {
    out int_to_str(add2(1))
}
"#,
    );
    let (text, code) = run_bridge_check(&dir, "bad.dtr");
    assert_ne!(code, 0, "wrong arity must fail: {text}");
    assert!(
        text.contains("E-BRIDGE-001"),
        "must carry the E-BRIDGE-001 code: {text}"
    );
    assert!(
        text.contains("expects 2 argument(s), call passes 1"),
        "must state the arity: {text}"
    );
    // Exact location: line 4, the call site column.
    assert!(
        text.contains("bad.dtr:4:"),
        "must point at the call: {text}"
    );
}

#[test]
fn test_bridge_check_flags_missing_header() {
    let dir = temp_dir("missing");
    write(
        &dir,
        "missing.dtr",
        r#"import c "nope.h" with link("x.lib");

fn main() {
    out "hi"
}
"#,
    );
    let (text, code) = run_bridge_check(&dir, "missing.dtr");
    assert_ne!(code, 0, "missing header must fail: {text}");
    assert!(text.contains("E-BRIDGE-002"), "{text}");
    assert!(text.contains("nope.h"), "{text}");
}
