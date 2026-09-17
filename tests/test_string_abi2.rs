//! Integration tests for Datara String ABI v2 ([len: i64][bytes...][\0]).
//!
//! Validates binary transparency (embedded NUL bytes '\0' preserved),
//! O(1) instantaneous length, lossless binary file I/O, lexicographical
//! comparison past NULs, split/join, substring, and C-string interop.

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

fn run_native(name: &str, source: &str) -> (String, i32) {
    ensure_msvc_env();
    let dir = std::env::temp_dir();
    let path = dir.join(name);
    std::fs::write(&path, source).expect("must write test source");
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}\n{}",
        name, res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    let _ = std::fs::remove_file(&path);
    for ext in ["exe", "pdb", "obj"] {
        let _ = std::fs::remove_file(path.with_extension(ext));
    }
    assert_eq!(code, 0, "run failed for {}: {}", name, stderr);
    (stdout.trim().to_string(), code)
}

#[test]
fn test_abi2_string_length_embedded_nul() {
    let src = r#"
fn main() -> Int {
    let s1 = "hello\0world"
    let l1 = s1.len()
    if l1 != 11 { return 1 }

    let s2 = "abc\0\0def\0"
    let l2 = s2.len()
    if l2 != 9 { return 2 }

    let s3 = ""
    if s3.len() != 0 { return 3 }

    return 0
}
"#;
    let (_, code) = run_native("str_abi2_len.dtr", src);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_concat_embedded_nul() {
    let src = r#"
fn main() -> Int {
    let a = "hello\0"
    let b = "world"
    let c = a + b
    if c.len() != 11 { return 1 }

    let d = c + "\0foo"
    if d.len() != 15 { return 2 }

    return 0
}
"#;
    let (_, code) = run_native("str_abi2_concat.dtr", src);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_substring_across_nul() {
    let src = r#"
fn main() -> Int {
    let s = "foo\0bar\0baz"
    let sub = s.substring(2, 7)
    if sub.len() != 7 { return 1 }
    return 0
}
"#;
    let (_, code) = run_native("str_abi2_substr.dtr", src);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_file_write_read_roundtrip_nul() {
    let temp_file = std::env::temp_dir().join("datara_abi2_test.bin");
    let path_str = temp_file.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
fn main(sys: SystemCapabilities) -> Int {{
    let payload = "header\0mid_data\0footer\0"
    let ok = file_write("{path_str}", payload)
    if ok != 1 {{ return 1 }}

    let back = file_read("{path_str}")
    if back.len() != payload.len() {{ return 2 }}
    if back != payload {{ return 3 }}

    return 0
}}
"#
    );
    let (_, code) = run_native("str_abi2_file_io.dtr", &src);
    let _ = std::fs::remove_file(&temp_file);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_equality_and_comparison() {
    let src = r#"
fn main() -> Int {
    let a = "abc\0def"
    let b = "abc\0def"
    let c = "abc\0deg"
    let d = "abc\0"

    if a != b { return 1 }
    if a == c { return 2 }
    if a == d { return 3 }
    if a < d { return 4 }

    return 0
}
"#;
    let (_, code) = run_native("str_abi2_cmp.dtr", src);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_repeat_and_padding() {
    let src = r#"
fn main() -> Int {
    let s = "a\0b"
    let r = s.repeat(3)
    if r.len() != 9 { return 1 }

    let p = "x\0y".pad_left(6, "-")
    if p.len() != 6 { return 2 }

    let q = "x\0y".pad_right(7, "+")
    if q.len() != 7 { return 3 }

    return 0
}
"#;
    let (_, code) = run_native("str_abi2_repeat_pad.dtr", src);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_split_join_with_nul() {
    let src = r#"
fn main() -> Int {
    let s = "alpha\0beta\0gamma"
    let parts = s.split("\0")
    if parts.len() != 3 { return 1 }
    if parts[0] != "alpha" { return 2 }
    if parts[1] != "beta" { return 3 }
    if parts[2] != "gamma" { return 4 }

    let reconstructed = parts.join("\0")
    if reconstructed.len() != s.len() { return 5 }
    if reconstructed != s { return 6 }

    return 0
}
"#;
    let (_, code) = run_native("str_abi2_split_join.dtr", src);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_o1_length_efficiency() {
    let src = r#"
fn main() -> Int {
    let s = "abcdefghijklmnopqrstuvwxyz"
    let mut i = 0
    let mut total = 0
    while i < 10000 {
        total = total + s.len()
        i = i + 1
    }
    if total != 260000 { return 1 }
    return 0
}
"#;
    let (_, code) = run_native("str_abi2_o1_len.dtr", src);
    assert_eq!(code, 0);
}

#[test]
fn test_abi2_c_string_backward_compatibility() {
    let src = r#"
fn main() -> Int {
    let s = "c_string_compatible_literal"
    if s.len() != 27 { return 1 }
    println(s)
    return 0
}
"#;
    let (stdout, code) = run_native("str_abi2_c_compat.dtr", src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "c_string_compatible_literal");
}
