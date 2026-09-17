use forgen::driver::ForgenCompiler;
use std::fs;
use std::path::{Path, PathBuf};

struct TempDir(PathBuf);
impl TempDir {
    fn new(name: &str) -> Self {
        static CNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = CNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("forgen_test_cap2_{}_{}_{}", name, std::process::id(), id));
        let _ = fs::create_dir_all(&p);
        TempDir(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_capabilities2_fs_read_glob_allow_bypasses_unsafe() {
    let dir = TempDir::new("fs_read_glob");
    let toml = r#"
[package]
name = "cap_test"
version = "0.1.0"

[capabilities]
fs-read = ["./data/**", "./config/*"]
"#;
    fs::write(dir.path().join("datara.toml"), toml).expect("write toml");

    let code = r#"
fn main() {
    let content = file_read("./data/input.txt")
}
"#;
    let src_path = dir.path().join("main.dtr");
    fs::write(&src_path, code).expect("write src");

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, src_path.to_str().unwrap(), None);
    assert!(
        res.success,
        "Allowed glob path ./data/input.txt must bypass unsafe. Error: {:?}\nDiag: {}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_capabilities2_fs_read_glob_violation_e_cap_002() {
    let dir = TempDir::new("cap_test");
    let toml = r#"
[package]
name = "cap_test"
version = "0.1.0"

[capabilities]
fs-read = ["./data/**"]
"#;
    fs::write(dir.path().join("datara.toml"), toml).expect("write toml");

    let code = r#"
fn main() {
    let content = file_read("./secret.txt")
}
"#;
    let src_path = dir.path().join("main.dtr");
    fs::write(&src_path, code).expect("write src");

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, src_path.to_str().unwrap(), None);
    assert!(
        !res.success,
        "Accessing secret.txt outside allowed glob must fail"
    );
    assert!(
        res.diagnostics.contains("E-CAP-002"),
        "Diagnostics must contain E-CAP-002: {}",
        res.diagnostics
    );
}

#[test]
fn test_capabilities2_missing_permission_e_cap_001() {
    let dir = TempDir::new("cap_test");
    let toml = r#"
[package]
name = "cap_test"
version = "0.1.0"

[capabilities]
fs-read = ["./data/**"]
"#;
    fs::write(dir.path().join("datara.toml"), toml).expect("write toml");

    let code = r#"
fn main() {
    file_write("./data/out.txt", "payload")
}
"#;
    let src_path = dir.path().join("main.dtr");
    fs::write(&src_path, code).expect("write src");

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, src_path.to_str().unwrap(), None);
    assert!(
        !res.success,
        "Undeclared fs-write capability must fail with E-CAP-001"
    );
    assert!(
        res.diagnostics.contains("E-CAP-001"),
        "Diagnostics must contain E-CAP-001: {}",
        res.diagnostics
    );
}

#[test]
fn test_capabilities2_env_glob_match_and_violation() {
    let dir = TempDir::new("cap_test");
    let toml = r#"
[package]
name = "cap_test"
version = "0.1.0"

[capabilities]
env = ["APP_*", "PORT"]
"#;
    fs::write(dir.path().join("datara.toml"), toml).expect("write toml");

    let code_ok = r#"
fn main() {
    let k = env_get("APP_KEY")
}
"#;
    let src_ok = dir.path().join("ok.dtr");
    fs::write(&src_ok, code_ok).expect("write src");

    let compiler = ForgenCompiler::new("release");
    let res_ok = compiler.compile_source(code_ok, src_ok.to_str().unwrap(), None);
    assert!(
        res_ok.success,
        "env_get for APP_KEY must be allowed without unsafe. Error: {:?}\nDiag: {}",
        res_ok.error, res_ok.diagnostics
    );

    let code_bad = r#"
fn main() {
    let s = env_get("DB_PASSWORD")
}
"#;
    let src_bad = dir.path().join("bad.dtr");
    fs::write(&src_bad, code_bad).expect("write src");

    let res_bad = compiler.compile_source(code_bad, src_bad.to_str().unwrap(), None);
    assert!(
        !res_bad.success,
        "env_get for DB_PASSWORD must fail with E-CAP-002"
    );
    assert!(
        res_bad.diagnostics.contains("E-CAP-002"),
        "Diagnostics must contain E-CAP-002: {}",
        res_bad.diagnostics
    );
}

#[test]
fn test_capabilities2_escalated_exec_requires_unsafe_on_top_of_manifest() {
    let dir = TempDir::new("cap_test");
    let toml = r#"
[package]
name = "cap_test"
version = "0.1.0"

[capabilities]
exec = ["python3", "git"]
"#;
    fs::write(dir.path().join("datara.toml"), toml).expect("write toml");

    let code_no_unsafe = r#"
fn main() {
    let _ = exec("python3 script.py")
}
"#;
    let src_no_unsafe = dir.path().join("no_unsafe.dtr");
    fs::write(&src_no_unsafe, code_no_unsafe).expect("write src");

    let compiler = ForgenCompiler::new("release");
    let res_no_unsafe =
        compiler.compile_source(code_no_unsafe, src_no_unsafe.to_str().unwrap(), None);
    assert!(
        !res_no_unsafe.success,
        "Escalated exec must fail without unsafe even if declared in manifest"
    );

    let code_with_unsafe = r#"
fn main() {
    unsafe(justification: "test escalated exec") {
        let _ = exec("python3 script.py")
    }
}
"#;
    let src_with_unsafe = dir.path().join("with_unsafe.dtr");
    fs::write(&src_with_unsafe, code_with_unsafe).expect("write src");

    let res_with_unsafe =
        compiler.compile_source(code_with_unsafe, src_with_unsafe.to_str().unwrap(), None);
    assert!(
        res_with_unsafe.success,
        "Escalated exec with manifest permission AND unsafe block must succeed. Error: {:?}\nDiag: {}",
        res_with_unsafe.error, res_with_unsafe.diagnostics
    );
}

#[test]
fn test_capabilities2_escalated_socket_requires_unsafe_on_top_of_manifest() {
    let dir = TempDir::new("cap_test");
    let toml = r#"
[package]
name = "cap_test"
version = "0.1.0"

[capabilities]
net-connect = ["127.0.0.1:*"]
"#;
    fs::write(dir.path().join("datara.toml"), toml).expect("write toml");

    let code_no_unsafe = r#"
fn main() {
    let s = socket_connect(1, "127.0.0.1", 8080)
}
"#;
    let src_no_unsafe = dir.path().join("sock_no_unsafe.dtr");
    fs::write(&src_no_unsafe, code_no_unsafe).expect("write src");

    let compiler = ForgenCompiler::new("release");
    let res_no_unsafe =
        compiler.compile_source(code_no_unsafe, src_no_unsafe.to_str().unwrap(), None);
    assert!(
        !res_no_unsafe.success,
        "socket_connect must fail without unsafe block"
    );

    let code_with_unsafe = r#"
fn main() {
    unsafe(justification: "connect to local service") {
        let s = socket_connect(1, "127.0.0.1", 8080)
    }
}
"#;
    let src_with_unsafe = dir.path().join("sock_with_unsafe.dtr");
    fs::write(&src_with_unsafe, code_with_unsafe).expect("write src");

    let res_with_unsafe =
        compiler.compile_source(code_with_unsafe, src_with_unsafe.to_str().unwrap(), None);
    assert!(
        res_with_unsafe.success,
        "socket_connect with manifest permission AND unsafe block must succeed. Error: {:?}\nDiag: {}",
        res_with_unsafe.error, res_with_unsafe.diagnostics
    );
}

#[test]
fn test_capabilities2_backward_compatible_absent_manifest() {
    let dir = TempDir::new("cap_test");
    let code_no_unsafe = r#"
fn main() {
    let c = file_read("test.txt")
}
"#;
    let src = dir.path().join("absent_manifest.dtr");
    fs::write(&src, code_no_unsafe).expect("write src");

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code_no_unsafe, src.to_str().unwrap(), None);
    assert!(
        !res.success,
        "Without manifest capabilities, file_read without unsafe must fail with E0940"
    );
    assert!(
        res.diagnostics.contains("E0940"),
        "Diagnostics must contain E0940: {}",
        res.diagnostics
    );

    let code_with_unsafe = r#"
fn main() {
    unsafe(justification: "read file in backward compat mode") {
        let c = file_read("test.txt")
    }
}
"#;
    let res_ok = compiler.compile_source(code_with_unsafe, src.to_str().unwrap(), None);
    assert!(
        res_ok.success,
        "Without manifest capabilities, file_read with unsafe must succeed. Error: {:?}\nDiag: {}",
        res_ok.error, res_ok.diagnostics
    );
}
