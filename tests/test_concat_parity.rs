//! Loop-carried concatenation parity: the LoopUnroll pass (loops/engine_v2)
//! duplicates even-trip-count loop bodies by factor 2, and the duplicate must
//! consume the first copy's updated loop-carried values.
//!
//! Regression context: the duplicated body used to re-read the incoming
//! header block param for the induction variable, so `i` advanced once per
//! unrolled iteration while the body did twice the work. Every side effect
//! doubled -- `s = s + "x"` over an even trip count n appended 2n chars
//! (e.g. n=4 printed "xxxxxxxx"), while odd trip counts skipped the pass and
//! stayed correct. Even counts 4/6/30_000, odd counts 3/5, varying RHS,
//! constant prepend, and integer accumulation are all pinned here, on both
//! the AOT (native executable) and the Cranelift in-memory JIT path.

use forgen::driver::ForgenCompiler;

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

/// Compile + run natively (AOT), return trimmed stdout.
fn run_native(name: &str, source: &str) -> String {
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
    stdout.trim().to_string()
}

/// Compile + run through the Cranelift in-memory JIT, return trimmed stdout.
fn run_jit_source(source: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let module = compiler
        .compile_source_to_dmir(source, "concat_parity_jit.dtr")
        .expect("JIT lowering must succeed");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_jit(&module, &[], true)
        .expect("JIT execution must succeed");
    assert_eq!(code, 0, "JIT run failed: {}", stderr);
    stdout.trim().to_string()
}

/// Source: append a constant string once per loop iteration, then report the
/// length, a `str_repeat` content comparison, and (small n only) a literal
/// content comparison.
fn const_append_source(n: usize) -> String {
    let literal_check = if n <= 6 {
        format!(
            "\n    if s == \"{}\" {{\n        out 2\n    }} else {{\n        out 0 - 2\n    }}",
            "x".repeat(n)
        )
    } else {
        String::new()
    };
    format!(
        r#"fn main() {{
    mut s = ""
    mut i = 0
    while i < {n} {{
        s = s + "x"
        i = i + 1
    }}
    out s.len()
    if s == str_repeat("x", {n}) {{
        out 1
    }} else {{
        out 0 - 1
    }}{literal_check}
}}
"#
    )
}

fn const_append_expected(n: usize) -> String {
    if n <= 6 {
        format!("{n}\n1\n2")
    } else {
        format!("{n}\n1")
    }
}

#[test]
fn concat_const_rhs_even_trip_counts_aot() {
    for n in [4usize, 6, 30_000] {
        let name = format!("concat_parity_even_{n}.dtr");
        let out = run_native(&name, &const_append_source(n));
        assert_eq!(out, const_append_expected(n), "even n={n} AOT");
    }
}

#[test]
fn concat_const_rhs_odd_trip_counts_aot() {
    for n in [3usize, 5] {
        let name = format!("concat_parity_odd_{n}.dtr");
        let out = run_native(&name, &const_append_source(n));
        assert_eq!(out, const_append_expected(n), "odd n={n} AOT");
    }
}

#[test]
fn concat_const_rhs_even_trip_counts_jit() {
    for n in [4usize, 6] {
        let out = run_jit_source(&const_append_source(n));
        assert_eq!(out, const_append_expected(n), "even n={n} JIT");
    }
}

#[test]
fn concat_const_rhs_large_even_trip_count_jit() {
    let out = run_jit_source(&const_append_source(30_000));
    assert_eq!(out, const_append_expected(30_000), "even n=30000 JIT");
}

#[test]
fn concat_const_rhs_odd_trip_counts_jit() {
    for n in [3usize, 5] {
        let out = run_jit_source(&const_append_source(n));
        assert_eq!(out, const_append_expected(n), "odd n={n} JIT");
    }
}

/// The varying-RHS shape (`s = s + int_to_str(i)`) was always correct; it
/// must stay correct, and it proves per-iteration content ordering.
#[test]
fn concat_varying_rhs_stays_correct_aot() {
    for n in [3usize, 4] {
        let content: String = (0..n).map(|i| i.to_string()).collect();
        let source = format!(
            r#"fn main() {{
    mut s = ""
    mut i = 0
    while i < {n} {{
        s = s + int_to_str(i)
        i = i + 1
    }}
    out s.len()
    out s
}}
"#
        );
        let name = format!("concat_parity_varying_{n}.dtr");
        let out = run_native(&name, &source);
        assert_eq!(
            out,
            format!("{}\n{content}", content.len()),
            "varying n={n} AOT"
        );
    }
}

#[test]
fn concat_varying_rhs_stays_correct_jit() {
    let source = r#"fn main() {
    mut s = ""
    mut i = 0
    while i < 4 {
        s = s + int_to_str(i)
        i = i + 1
    }
    out s.len()
    out s
}
"#;
    let out = run_jit_source(source);
    assert_eq!(out, "4\n0123", "varying n=4 JIT");
}

/// Same bug shape with the constant on the left (`s = "x" + s`).
#[test]
fn concat_prepend_const_rhs_parity_aot() {
    for n in [4usize, 5] {
        let source = format!(
            r#"fn main() {{
    mut s = ""
    mut i = 0
    while i < {n} {{
        s = "x" + s
        i = i + 1
    }}
    out s.len()
    if s == "{}" {{
        out 1
    }} else {{
        out 0 - 1
    }}
}}
"#,
            "x".repeat(n)
        );
        let name = format!("concat_parity_prepend_{n}.dtr");
        let out = run_native(&name, &source);
        assert_eq!(out, format!("{n}\n1"), "prepend n={n} AOT");
    }
}

#[test]
fn concat_prepend_const_rhs_parity_jit() {
    let source = r#"fn main() {
    mut s = ""
    mut i = 0
    while i < 6 {
        s = "x" + s
        i = i + 1
    }
    out s.len()
    out s
}
"#;
    let out = run_jit_source(source);
    assert_eq!(out, "6\nxxxxxx", "prepend n=6 JIT");
}

/// Integer accumulation in the same loop shape: promoted (SSA block-param)
/// loop-carried variables must advance exactly once per original iteration.
#[test]
fn int_accumulator_even_trip_counts_aot() {
    for (n, step, expected) in [
        (4usize, 1usize, 4usize),
        (6, 1, 6),
        (30_000, 1, 30_000),
        (4, 2, 8),
    ] {
        let source = format!(
            r#"fn main() {{
    mut acc = 0
    mut i = 0
    while i < {n} {{
        acc = acc + {step}
        i = i + 1
    }}
    out acc
}}
"#
        );
        let name = format!("concat_parity_int_{n}_{step}.dtr");
        let out = run_native(&name, &source);
        assert_eq!(out, expected.to_string(), "int acc n={n} step={step} AOT");
    }
}

#[test]
fn int_accumulator_even_trip_counts_jit() {
    for (n, expected) in [(4usize, 4usize), (6, 6)] {
        let source = format!(
            r#"fn main() {{
    mut acc = 0
    mut i = 0
    while i < {n} {{
        acc = acc + 1
        i = i + 1
    }}
    out acc
}}
"#
        );
        let out = run_jit_source(&source);
        assert_eq!(out, expected.to_string(), "int acc n={n} JIT");
    }
}
