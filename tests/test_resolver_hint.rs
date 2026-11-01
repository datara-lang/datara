//! PLAN_V152 N2: resolver-hint — an undefined symbol that exists inside a
//! project dependency (packages/<pkg>) must produce the hint
//! "add `use <pkg>` (found in package '<pkg>')" instead of the generic
//! "declared with val/mut" advice.

#[test]
fn resolver_hints_missing_use_from_packages() {
    let dir = std::env::temp_dir().join(format!(
        "forgen_hint_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let pkg = dir.join("packages/mathx");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("lib.dtr"),
        "fn super_unique_helper(v: Int) -> Int {\n    return v\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.dtr"),
        "fn main() {\n    out int_to_str(super_unique_helper(5))\n}\n",
    )
    .unwrap();

    // check_source has no cwd parameter — spawn the CLI binary with cwd
    // set to the temp project so `packages/` is discoverable.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_forgen"))
        .args(["check", "main.dtr"])
        .current_dir(&dir)
        .output()
        .expect("check must run");
    let diag =
        String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    assert!(
        !out.status.success(),
        "undefined symbol must fail the check: {diag}"
    );
    assert!(
        diag.contains("add `use mathx` (found in package 'mathx')"),
        "hint must name the package: {diag}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
