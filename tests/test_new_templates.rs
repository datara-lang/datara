//! PLAN_V153 D1: `forgen new <name> --template cli|server|script` —
//! template projects generate, type-check, and the cli/script bodies run.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forgen_tpl_{}_{}_{}",
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

fn forgen_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forgen"))
}

/// Points the child forgen at the REPO stdlib: a stale installed copy in
/// AppData/Programs would otherwise shadow the embedded modules (the
/// installed clock.dtr predates fmt_date).
fn repo_stdlib() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("stdlib")
        .to_string_lossy()
        .to_string()
}

#[test]
fn template_cli_generates_and_runs() {
    let dir = temp_dir("cli");
    let out = Command::new(forgen_bin())
        .args(["new", "tpl_cli", "--template", "cli"])
        .current_dir(&dir)
        .env("DATARA_STDLIB", repo_stdlib())
        .output()
        .expect("spawn forgen new");
    assert!(out.status.success(), "forgen new failed: {:?}", out.stderr);
    let main_dtr = dir.join("tpl_cli/src/main.dtr");
    assert!(main_dtr.exists(), "template main.dtr must exist");

    // The generated CLI tool parses --name and greets.
    let run = Command::new(forgen_bin())
        .args(["run", "src/main.dtr", "--", "--name", "World"])
        .current_dir(dir.join("tpl_cli"))
        .env("DATARA_STDLIB", repo_stdlib())
        .output()
        .expect("spawn forgen run");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(stdout.trim(), "Hello, World");
}

#[test]
fn template_script_generates_and_runs() {
    let dir = temp_dir("script");
    let out = Command::new(forgen_bin())
        .args(["new", "tpl_script", "--template", "script"])
        .current_dir(&dir)
        .env("DATARA_STDLIB", repo_stdlib())
        .output()
        .expect("spawn forgen new");
    assert!(out.status.success());
    let run = Command::new(forgen_bin())
        .args(["run", "src/main.dtr"])
        .current_dir(dir.join("tpl_script"))
        .env("DATARA_STDLIB", repo_stdlib())
        .output()
        .expect("spawn forgen run");
    assert!(
        run.status.success(),
        "script run failed: {}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    // fmt_date output: "YYYY-MM-DD hh:mm:ss"
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(stdout.trim().len(), 19, "date shape: {stdout}");
}

#[test]
fn template_server_generates_and_checks() {
    let dir = temp_dir("server");
    let out = Command::new(forgen_bin())
        .args(["new", "tpl_server", "--template", "server"])
        .current_dir(&dir)
        .env("DATARA_STDLIB", repo_stdlib())
        .output()
        .expect("spawn forgen new");
    assert!(out.status.success());
    let check = Command::new(forgen_bin())
        .args(["check", "tpl_server/src/main.dtr"])
        .current_dir(&dir)
        .env("DATARA_STDLIB", repo_stdlib())
        .output()
        .expect("spawn forgen check");
    let stderr = String::from_utf8_lossy(&check.stderr);
    let stdout = String::from_utf8_lossy(&check.stdout);
    assert!(
        stdout.contains("100% OK") || stderr.contains("100% OK"),
        "server template must type-check: {stdout}{stderr}"
    );
}

#[test]
fn template_unknown_falls_back() {
    let dir = temp_dir("bogus");
    let out = Command::new(forgen_bin())
        .args(["new", "tpl_bogus", "--template", "bogus"])
        .current_dir(&dir)
        .env("DATARA_STDLIB", repo_stdlib())
        .output()
        .expect("spawn forgen new");
    assert!(
        out.status.success(),
        "unknown template falls back, not fails"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        (stdout.to_string() + &stderr).contains("Unknown template 'bogus'"),
        "must name the unknown template"
    );
}
