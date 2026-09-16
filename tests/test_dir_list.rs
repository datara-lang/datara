//! Directory listing builtins: dir_list (sorted entry names) and path_exists
//! (files AND directories), both capability-gated like file_read --
//! compile-time E0940 without unsafe(justification:)/Capability<FileRead>,
//! plus the runtime DATARA_CAP_FS_READ trap.

use forgen::driver::ForgenCompiler;

static MSVC_ENV: std::sync::Once = std::sync::Once::new();
fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| unsafe {
        if std::env::var_os("ProgramFiles(x86)").is_none() {
            std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
        }
        if std::env::var_os("ProgramFiles").is_none() {
            std::env::set_var("ProgramFiles", r"C:\Program Files");
        }
    });
}

fn write_source(dir: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("failed to write test source");
    path
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("exe"));
    let _ = std::fs::remove_file(path.with_extension("pdb"));
    let _ = std::fs::remove_file(path.with_extension("obj"));
}

fn run_stdout(path: &std::path::Path) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(path, None);
    assert!(
        res.success,
        "Compilation should succeed:\n{}",
        res.diagnostics
    );
    let (stdout, stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    if code != 0 {
        eprintln!("stderr: {}", stderr);
    }
    (stdout, code)
}

fn temp_subdir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn dtr_path(p: &std::path::Path) -> String {
    p.to_str().unwrap().replace('\\', "/")
}

#[test]
fn test_dir_list_returns_sorted_names() {
    ensure_msvc_env();
    let dir = temp_subdir("datara_dir_list_sorted");
    // Created out of order on purpose: dir_list must sort alphabetically.
    std::fs::write(dir.join("gamma.txt"), "g").unwrap();
    std::fs::write(dir.join("alpha.txt"), "a").unwrap();
    std::fs::write(dir.join("beta.txt"), "b").unwrap();

    let src = format!(
        r#"fn main() {{
    unsafe(justification: "test dir listing") {{
        let names = dir_list("{d}")
        mut i = 0
        while i < names.len() {{
            println(names[i])
            i = i + 1
        }}
    }}
}}
"#,
        d = dtr_path(&dir)
    );
    let path = write_source(&std::env::temp_dir(), "dir_list_sorted.dtr", &src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines,
        vec!["alpha.txt", "beta.txt", "gamma.txt"],
        "dir_list must return exactly the sorted entry names"
    );
    cleanup(&path);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_dir_list_empty_directory() {
    ensure_msvc_env();
    let dir = temp_subdir("datara_dir_list_empty");
    let src = format!(
        r#"fn main() {{
    unsafe(justification: "test dir listing") {{
        let names = dir_list("{d}")
        println(int_to_str(names.len()))
    }}
}}
"#,
        d = dtr_path(&dir)
    );
    let path = write_source(&std::env::temp_dir(), "dir_list_empty.dtr", &src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "0", "empty dir must yield an empty list");
    cleanup(&path);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_path_exists_file_is_true() {
    ensure_msvc_env();
    let dir = temp_subdir("datara_path_exists_file");
    let file = dir.join("present.txt");
    std::fs::write(&file, "x").unwrap();
    let src = format!(
        r#"fn main() {{
    unsafe(justification: "test existence") {{
        println(bool_to_str(path_exists("{f}")))
    }}
}}
"#,
        f = dtr_path(&file)
    );
    let path = write_source(&std::env::temp_dir(), "path_exists_file.dtr", &src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "true");
    cleanup(&path);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_path_exists_directory_is_true() {
    ensure_msvc_env();
    let dir = temp_subdir("datara_path_exists_dir");
    let src = format!(
        r#"fn main() {{
    unsafe(justification: "test existence") {{
        println(bool_to_str(path_exists("{d}")))
    }}
}}
"#,
        d = dtr_path(&dir)
    );
    let path = write_source(&std::env::temp_dir(), "path_exists_dir.dtr", &src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "true");
    cleanup(&path);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_path_exists_missing_is_false() {
    ensure_msvc_env();
    let src = r#"fn main() {
    unsafe(justification: "test existence") {
        println(bool_to_str(path_exists("D:/datara/definitely/not/here/xyz_123")))
    }
}
"#;
    let path = write_source(&std::env::temp_dir(), "path_exists_missing.dtr", src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "false");
    cleanup(&path);
}

/// Capability gate: dir_list / path_exists must be rejected at compile time
/// without unsafe(justification:) or a granted Capability<FileRead>, exactly
/// like file_read (E0940, src/security/capabilities.rs).
#[test]
fn test_dir_list_requires_capability() {
    ensure_msvc_env();
    for (name, call) in [
        ("gate_dir_list.dtr", "dir_list(\"D:/tmp\")"),
        ("gate_path_exists.dtr", "path_exists(\"D:/tmp\")"),
    ] {
        let src = format!("fn main() {{\n    let r = {}\n}}\n", call);
        let path = write_source(&std::env::temp_dir(), name, &src);
        let compiler = ForgenCompiler::new("release");
        let res = compiler.compile_file(&path, None);
        assert!(
            !res.success,
            "{} without unsafe justification must fail to compile",
            call
        );
        let diag = res.diagnostics;
        assert!(
            diag.contains("E0940"),
            "expected E0940 for {}, got:\n{}",
            call,
            diag
        );
        assert!(
            diag.contains("Capability<FileRead>"),
            "expected Capability<FileRead> requirement for {}, got:\n{}",
            call,
            diag
        );
        cleanup(&path);
    }
}
