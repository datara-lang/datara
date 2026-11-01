//! PLAN_V152 N2: `forgen migrate` — mechanical 1.4→1.5 modernization.
//! Round-trip contract: migrating a legacy file produces source that
//! COMPILES and runs with the same output, `--dry-run` writes nothing.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forgen_migrate_{}_{}_{}",
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

const LEGACY: &str = r#"fn greet(name: String) -> String {
    println("hello")
    return name
}

fn main() {
    let x = 5
    let mut y = 10
    y = y + x
    let msg = greet("world")
    println(msg)
    println(int_to_str(y))
}
"#;

fn run_migrate(dir: &std::path::Path, extra: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_forgen"))
        .args(["migrate", "1.4", "1.5", "legacy.dtr"])
        .args(extra)
        .current_dir(dir)
        .output()
        .expect("migrate must run");
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (text, out.status.code().unwrap_or(-1))
}

fn run_legacy(dir: &std::path::Path) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_forgen"))
        .args(["run", "legacy.dtr"])
        .current_dir(dir)
        .output()
        .expect("run must start");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn migrate_rewrites_and_result_still_runs() {
    let dir = temp_dir("rewrite");
    fs::write(dir.join("legacy.dtr"), LEGACY).unwrap();
    let (text, code) = run_migrate(&dir, &[]);
    assert_eq!(code, 0, "{text}");
    let migrated = fs::read_to_string(dir.join("legacy.dtr")).unwrap();
    // All four rules applied:
    assert!(migrated.contains("val x = 5"), "let -> val: {migrated}");
    assert!(
        migrated.contains("mut y = 10"),
        "let mut -> mut: {migrated}"
    );
    assert!(
        migrated.contains("name: Str") && !migrated.contains("name: String"),
        "String -> Str: {migrated}"
    );
    assert!(
        migrated.contains("out \"hello\"") && migrated.contains("out msg"),
        "println -> out: {migrated}"
    );
    assert!(!migrated.contains("let "), "no `let` remains: {migrated}");
    // Round-trip: the migrated program runs and prints the same output as
    // the original did before migration.
    let (stdout, run_code) = run_legacy(&dir);
    assert_eq!(run_code, 0, "{stdout}");
    assert_eq!(stdout.trim(), "hello\nworld\n15", "{stdout}");
}

#[test]
fn migrate_dry_run_writes_nothing() {
    let dir = temp_dir("dry");
    fs::write(dir.join("legacy.dtr"), LEGACY).unwrap();
    let (text, code) = run_migrate(&dir, &["--dry-run"]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("dry-run: nothing written"),
        "dry-run must be announced: {text}"
    );
    assert!(
        text.contains("line 1: String -> Str"),
        "report must list edits: {text}"
    );
    // The file must be UNTOUCHED.
    assert_eq!(fs::read_to_string(dir.join("legacy.dtr")).unwrap(), LEGACY);
}

#[test]
fn migrate_reports_missing_path() {
    let dir = temp_dir("missing");
    let (text, code) = run_migrate(&dir, &[]);
    assert_ne!(code, 0, "missing path must fail: {text}");
    assert!(text.contains("Path does not exist"), "{text}");
}
