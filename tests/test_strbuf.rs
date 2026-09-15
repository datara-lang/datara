//! v1.4.0 `StrBuf` prelude class and the L1401 loop-concatenation lint.
//!
//! * `StrBuf` must produce byte-identical results to naive concatenation.
//! * The builder must scale: 100 000 appends complete and produce exactly
//!   the expected string.
//! * Timing is asserted loosely (completion only); the measured numbers are
//!   printed so the run log carries real data.
//! * `forgen lint` warns L1401 on `s = s + ...` inside a loop and stays
//!   silent on the StrBuf form.

use forgen::driver::ForgenCompiler;
use forgen::lint::lint_source;
use std::path::PathBuf;

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
}

fn write_source(name: &str, source: &str) -> PathBuf {
    let path = PathBuf::from(name);
    std::fs::write(&path, source).expect("must write test source");
    path
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("exe"));
    let _ = std::fs::remove_file(path.with_extension("pdb"));
    let _ = std::fs::remove_file(path.with_extension("obj"));
}

fn run(path: &std::path::Path, source: &str) -> String {
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(path, None);
    assert!(
        res.success,
        "compilation failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    assert_eq!(code, 0, "run failed for {}: {}", source, stderr);
    stdout
}

/// StrBuf must produce byte-identical results to an independently
/// constructed string of the same parts.
///
/// NOTE: the original plan compared against `s = s + "ab"` inside a loop.
/// That exposed a PRE-EXISTING v1.3.4 optimizer defect (confirmed against
/// the released 1.3.4 binary): loop unrolling duplicates loop-carried
/// string concatenation for even trip counts (naive(2000).len() reports
/// 8000 instead of 4000; odd trip counts take the scalar path and are
/// correct). The pre-existing bug is out of scope for v1.4.0 and is NOT
/// asserted here; `str_repeat` is the independent reference instead, plus
/// a direct multi-append sequence at n=5 (odd, unbugged path).
#[test]
fn strbuf_matches_independent_reference() {
    let source = r#"
fn buffered(n: Int) -> Str {
    let sb = StrBuf { }
    for i in 0..n {
        sb.push("ab")
    }
    return sb.join()
}

fn direct_appends() -> Str {
    let sb = StrBuf { }
    sb.push("ab")
    sb.push("cd")
    sb.push("ef")
    sb.push("gh")
    sb.push("ij")
    return sb.join()
}

fn main() {
    let n = 2000
    let b = buffered(n)
    let reference = str_repeat("ab", n)
    let d = direct_appends()
    if b == reference && d == "abcdefghij" {
        out b.len()
    } else {
        out 0 - 1
    }
}
"#;
    let path = write_source("v140_strbuf_equal.dtr", source);
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&path, None);
    assert!(
        res.success,
        "compilation failed: {:?}
{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native executable");
    assert_eq!(code, 0, "run failed: {}", stderr);
    assert_eq!(
        stdout.trim(),
        "4000",
        "StrBuf output must match the independently constructed reference"
    );
    cleanup(&path);
}

/// 100 000 appends complete and produce exactly the expected string length.
/// The expected content is built independently with `str_repeat`, so the
/// comparison is strict without paying the naive O(n^2) cost.
#[test]
fn strbuf_100k_parts() {
    let source = r#"
fn buffered(n: Int) -> Str {
    let sb = StrBuf { }
    for i in 0..n {
        sb.push("xy")
    }
    return sb.join()
}

fn main() {
    let n = 100000
    let got = buffered(n)
    let expected = str_repeat("xy", n)
    if got == expected {
        out got.len()
    } else {
        out 0 - 1
    }
}
"#;
    let path = write_source("v140_strbuf_100k.dtr", source);
    let stdout = run(&path, "strbuf 100k");
    assert_eq!(
        stdout.trim(),
        "200000",
        "100k StrBuf parts must join to 200000 bytes exactly"
    );
    cleanup(&path);
}

/// Mixed `push` / `push_int` parts match the naive equivalent, and the
/// builder keeps accepting appends after a `join()`.
#[test]
fn strbuf_mixed_parts_and_reuse() {
    let source = r##"
fn naive(n: Int) -> Str {
    mut s = ""
    for i in 0..n {
        s = s + "["
        s = s + int_to_str(i)
        s = s + "]"
    }
    return s
}

fn buffered(n: Int) -> Str {
    let sb = StrBuf { }
    for i in 0..n {
        sb.push("[")
        sb.push_int(i)
        sb.push("]")
    }
    let first = sb.join()
    sb.push("#")
    let second = sb.join()
    if second.len() == first.len() + 1 {
        return first
    }
    return "reuse-failed"
}

fn main() {
    let n = 500
    let a = naive(n)
    let b = buffered(n)
    if a == b {
        out a.len()
    } else {
        out 0 - 1
    }
}
"##;
    let path = write_source("v140_strbuf_mixed.dtr", source);
    let stdout = run(&path, "strbuf mixed parts");
    let expected_len: i64 = (0..500i64).map(|i| 2 + i.to_string().len() as i64).sum();
    assert_eq!(
        stdout.trim(),
        expected_len.to_string(),
        "mixed push/push_int output must match the naive build"
    );
    cleanup(&path);
}

/// Timing is reported, not enforced beyond completion: the builder must
/// finish 100 000 parts while the naive path does the same work at a size
/// where it is still affordable. Both numbers are printed for the run log.
#[test]
fn strbuf_reports_timing() {
    let source = r#"
fn buffered(n: Int) -> Int {
    let sb = StrBuf { }
    for i in 0..n {
        sb.push_int(i)
    }
    return sb.join().len()
}

fn naive(n: Int) -> Int {
    mut s = ""
    for i in 0..n {
        s = s + int_to_str(i)
    }
    return s.len()
}

fn main() {
    let n = 20000
    let t0 = now_ns()
    let lb = buffered(n)
    let t1 = now_ns()
    let ln = naive(n)
    let t2 = now_ns()
    if lb == ln {
        println("strbuf_ns_per_call=" + int_to_str((t1 - t0) / n))
        println("naive_ns_per_call=" + int_to_str((t2 - t1) / n))
        out lb
    } else {
        out 0 - 1
    }
}
"#;
    let path = write_source("v140_strbuf_timing.dtr", source);
    let stdout = run(&path, "strbuf timing");
    println!("[strbuf timing] {}", stdout.trim().replace('\n', " | "));
    let expected_len: i64 = (0..20000i64).map(|i| i.to_string().len() as i64).sum();
    assert!(
        stdout.contains(&expected_len.to_string()),
        "both builders must agree on the length: {}",
        stdout
    );
    assert!(
        stdout.contains("strbuf_ns_per_call=") && stdout.contains("naive_ns_per_call="),
        "timing lines must be present: {}",
        stdout
    );
    cleanup(&path);
}

/// `forgen lint` warns L1401 on loop-carried string concatenation.
#[test]
fn lint_warns_on_loop_concat() {
    let sample = r#"
fn build(n: Int) -> Str {
    mut s = ""
    for i in 0..n {
        s = s + "part,"
    }
    return s
}
"#;
    let diags = lint_source(sample, "sample_concat.dtr").expect("must lint");
    assert!(
        diags.iter().any(|d| d.code == "L1401"),
        "L1401 must fire on loop concatenation, got: {:?}",
        diags.iter().map(|d| d.code).collect::<Vec<_>>()
    );
    let l1401 = diags.iter().find(|d| d.code == "L1401").unwrap();
    assert!(
        l1401.help.as_deref().unwrap_or("").contains("StrBuf"),
        "the suggestion must point at StrBuf"
    );
}

/// The StrBuf form must not trigger L1401 (nothing is concatenated in a
/// loop), and a plain integer accumulation loop must stay silent too.
#[test]
fn lint_is_silent_on_strbuf_and_int_accumulation() {
    let strbuf_sample = r#"
fn build(n: Int) -> Str {
    let sb = StrBuf { }
    for i in 0..n {
        sb.push("part,")
    }
    return sb.join()
}
"#;
    let diags = lint_source(strbuf_sample, "sample_strbuf.dtr").expect("must lint");
    assert!(
        !diags.iter().any(|d| d.code == "L1401"),
        "StrBuf usage must not warn, got: {:?}",
        diags.iter().map(|d| d.code).collect::<Vec<_>>()
    );

    let int_sample = r#"
fn sum(n: Int) -> Int {
    mut acc = 0
    for i in 0..n {
        acc = acc + 1
    }
    return acc
}
"#;
    let diags = lint_source(int_sample, "sample_int.dtr").expect("must lint");
    assert!(
        !diags.iter().any(|d| d.code == "L1401"),
        "integer accumulation must not warn, got: {:?}",
        diags.iter().map(|d| d.code).collect::<Vec<_>>()
    );
}
