//! PLAN_V152 N2 repro #8: `m[k] = v` with an Int key on a bare `{}` map
//! literal segfaulted — the lowering routed every map index-set/get
//! through the STR entrypoint, which strlen()s the key (strlen on the
//! raw Int 10 → segfault, AOT + JIT).
//!
//! Fix: a statically-Int key always routes through the _i64 entrypoints
//! (datara_rt_map_insert_i64 / datara_rt_map_get_outcome_i64); the
//! runtime adopts the int key kind on first insert. A Str-keyed map
//! receiving an Int key is the checker's job to reject.
//!
//! Known remaining edge (repro #11, NOT covered here): the TSU `is Int`
//! narrowing on a map index-get RESULT misreads the box tag ("expected
//! Int, got Float" panic) — separate issue from the segfault.

use forgen::driver::ForgenCompiler;

fn run_native(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation must succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    if code != 0 {
        eprintln!("stderr: {stderr}");
    }
    (stdout.trim().replace("\r\n", "\n"), code)
}

#[test]
fn bare_map_int_index_set_get_roundtrip() {
    let src = r#"
fn main() {
    val m = {}
    m[10] = 100
    m[20] = 200
    out int_to_str(m.len())
    val ks = map_keys_int(m)
    mut acc = 0
    mut i = 0
    while i < ks.len() {
        acc = acc + ks.get(i)
        i = i + 1
    }
    out int_to_str(acc)
}
"#;
    let (out, code) = run_native(src, "bare_map_int_index.dtr");
    assert_eq!(code, 0, "Int-keyed index-set/get must not segfault");
    // len=2, key sum=30
    assert_eq!(out, "2\n30");
}

/// Control: the Str-key path still works unchanged.
#[test]
fn bare_map_str_index_still_works() {
    let src = r#"
fn main() {
    val m = {}
    m["x"] = 100
    out int_to_str(m.len())
}
"#;
    let (out, code) = run_native(src, "bare_map_str_index.dtr");
    assert_eq!(code, 0);
    assert_eq!(out, "1");
}
