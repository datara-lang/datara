use forgen::driver::ForgenCompiler;
use forgen::project::run_dpm_cli_args;
use std::fs;
use std::path::{Path, PathBuf};

static DPM_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct TempDir(PathBuf);
impl TempDir {
    fn new(name: &str) -> Self {
        static CNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = CNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("forgen_test_dpm_{}_{}_{}", name, std::process::id(), id));
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
fn test_dpm_add_bridge_package_creates_files_and_manifest_entry() {
    let _lock = DPM_TEST_MUTEX.lock().unwrap();
    let dir = TempDir::new("add");
    let orig_dir = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(dir.path()).expect("cd temp");

    let toml = "[package]\nname = \"my_app\"\nversion = \"0.1.0\"\n\n[dependencies]\n";
    fs::write("datara.toml", toml).expect("write toml");

    run_dpm_cli_args(&["dpm".into(), "add".into(), "py:math@1.0.0".into()]);

    assert!(
        fs::metadata("dpm_packages/math/bridge.toml").is_ok(),
        "bridge.toml must be created"
    );
    assert!(
        fs::metadata("dpm_packages/math/bridge.dtr").is_ok(),
        "bridge.dtr must be created"
    );

    let manifest_content = fs::read_to_string("datara.toml").expect("read toml");
    assert!(
        manifest_content.contains("\"py:math\""),
        "datara.toml must record py:math dependency"
    );

    std::env::set_current_dir(orig_dir).expect("restore dir");
}

#[test]
fn test_dpm_publish_dry_run_validates_bridge_package() {
    let _lock = DPM_TEST_MUTEX.lock().unwrap();
    let dir = TempDir::new("publish");
    let orig_dir = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(dir.path()).expect("cd temp");

    let bridge_toml = r#"
[bridge]
name = "geometry"
lang = "py"
version = "0.2.0"
"#;
    let bridge_dtr = r#"
use py::geometry

bridge py::geometry {
    fn hypot(a: Float, b: Float) -> Float
}
"#;
    fs::write("bridge.toml", bridge_toml).expect("write bridge.toml");
    fs::write("bridge.dtr", bridge_dtr).expect("write bridge.dtr");

    run_dpm_cli_args(&["dpm".into(), "publish".into(), "--dry-run".into()]);

    std::env::set_current_dir(orig_dir).expect("restore dir");
}

#[test]
fn test_dpm_compiler_auto_scanning_dpm_packages() {
    let _lock = DPM_TEST_MUTEX.lock().unwrap();
    let dir = TempDir::new("scan");
    let orig_dir = std::env::current_dir().expect("cwd");

    let dpm_pkg_dir = dir.path().join("dpm_packages").join("fastmath");
    fs::create_dir_all(&dpm_pkg_dir).expect("create dpm pkg dir");

    let bridge_toml = r#"
[bridge]
name = "fastmath"
lang = "py"
version = "0.1.0"
"#;
    let bridge_dtr = r#"
use py::fastmath

bridge py::fastmath {
    fn sqr(x: Float) -> Float
}
"#;
    fs::write(dpm_pkg_dir.join("bridge.toml"), bridge_toml).expect("write bridge.toml");
    fs::write(dpm_pkg_dir.join("bridge.dtr"), bridge_dtr).expect("write bridge.dtr");

    let main_dtr = r#"
fn main() {
    let y = fastmath.sqr(5.0)
}
"#;
    let main_path = dir.path().join("main.dtr");
    fs::write(&main_path, main_dtr).expect("write main");

    std::env::set_current_dir(dir.path()).expect("cd temp");

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(main_dtr, main_path.to_str().unwrap(), None);

    std::env::set_current_dir(orig_dir).expect("restore dir");

    assert!(
        res.success,
        "Compiler must automatically discover and resolve bridge in dpm_packages. Diag: {}",
        res.diagnostics
    );
}

#[test]
fn test_dpm_remove_bridge_package() {
    let _lock = DPM_TEST_MUTEX.lock().unwrap();
    let dir = TempDir::new("remove");
    let orig_dir = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(dir.path()).expect("cd temp");

    let dpm_pkg_dir = dir.path().join("dpm_packages").join("numpy");
    fs::create_dir_all(&dpm_pkg_dir).expect("create dpm pkg dir");
    fs::write(dpm_pkg_dir.join("bridge.toml"), "[bridge]\nname=\"numpy\"\nlang=\"py\"\nversion=\"1.0.0\"\n").unwrap();
    fs::write(dpm_pkg_dir.join("bridge.dtr"), "use py::numpy\nbridge py::numpy {}\n").unwrap();

    let toml = "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n\"py:numpy\" = \"1.0.0\"\n";
    fs::write("datara.toml", toml).unwrap();

    run_dpm_cli_args(&["dpm".into(), "remove".into(), "py:numpy".into()]);

    assert!(
        !dir.path().join("dpm_packages").join("numpy").exists(),
        "dpm_packages/numpy must be deleted"
    );

    let manifest = fs::read_to_string("datara.toml").unwrap();
    assert!(
        !manifest.contains("\"py:numpy\""),
        "datara.toml must no longer contain py:numpy"
    );

    std::env::set_current_dir(orig_dir).expect("restore dir");
}
