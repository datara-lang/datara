//! v1.5.0 §5 regression: Int-keyed maps.
//!
//! History: the checker accepted `Map<Int, ...>` and `m.insert(1, v)` on any
//! map, but the C runtime's key word was ALWAYS interpreted as `char*`.
//! `Map.new()` defaulted to `Map<Str, Int>`, so `m.insert(1, 10)` passed an
//! integer 1 to `strlen` and segfaulted (exit 139). The fix has four parts:
//!   * runtime: a key-kind flag in the map header, int hashing/comparison and
//!     `*_i64` entrypoints, with the flag preserved across growth/free;
//!   * lowering: int-keyed maps route insert/get/contains/index/literals
//!     through those entrypoints (annotation and parameter types kept);
//!   * checker: `Map.new()` adopts the annotation, key/value argument types
//!     are validated, and unsupported key types are rejected up front;
//!   * LLVM: f64 payloads are bitcast into/out of the all-i64 collection ABI.

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

fn run_llvm(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release").with_llvm(true);
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "LLVM compilation must succeed: {:?}\n{}",
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

const INT_KEY_MAP: &str = r#"
fn main() {
    val m: Map<Int, Int> = {1: 10, 2: 20, 3: 30}
    out m[1]
    out m.get(2) or -1
    out m.contains(3)
    m.insert(2, 22)
    out m.get(2) or -1
    out m.get(99) or -1
    out m.len()
    mut i = 0
    while i < 1000 {
        m.insert(i, i * 3)
        i = i + 1
    }
    out m.len()
    out m.get(999) or -1
    out m.contains(777)
}
"#;

#[test]
fn int_key_map_literal_and_growth() {
    let (stdout, code) = run_native(INT_KEY_MAP, "map_int_keys.dtr");
    assert_eq!(code, 0);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["10", "20", "true", "22", "-1", "3", "1000", "2997", "true"]
    );
}

#[test]
fn map_new_adopts_annotation_key_type() {
    let src = r#"
fn main() {
    val m: Map<Int, Int> = Map.new()
    m.insert(1, 10)
    m.insert(2, 20)
    out m.len()
    out m.get(1) or -1
    m.insert(1, 11)
    out m.get(1) or -1
    out m.len()
}
"#;
    let (stdout, code) = run_native(src, "map_new_int_keys.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["2", "10", "11", "2"]);
}

#[test]
fn int_key_map_parameters_keep_key_type() {
    let src = r#"
fn lookup(m: Map<Int, Int>, key: Int) -> Int {
    return m.get(key) or -1
}

fn main() {
    val m: Map<Int, Int> = {1: 10, 2: 20}
    out lookup(m, 2)
    out lookup(m, 5)
}
"#;
    let (stdout, code) = run_native(src, "map_int_param.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["20", "-1"]);
}

#[test]
fn int_key_map_str_and_float_values() {
    let src = r#"
fn main() {
    val names: Map<Int, Str> = {1: "one", 2: "two"}
    out names.get(2) or "?"
    out names[1]
    val scores: Map<Int, Float> = {1: 1.5, 2: 2.25}
    out scores.get(1) or 0.0
    out scores[2]
    scores.insert(3, 3.75)
    out scores.get(3) or 0.0
}
"#;
    let (stdout, code) = run_native(src, "map_int_value_kinds.dtr");
    assert_eq!(code, 0);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["two", "one", "1.5", "2.25", "3.75"]
    );
}

#[test]
fn llvm_int_key_map_float_payloads() {
    // Regression for the LLVM collection ABI: map constructors/inserts take
    // value words as i64, and the raw accessor returns i64. Without the
    // f64<->i64 bitcasts the values landed in the wrong register class
    // (NaN on read).
    let src = r#"
fn main() {
    val m: Map<Int, Float> = {1: 1.5, 2: 2.25}
    out m[1]
    out m.get(2) or 0.0
    m.insert(3, 3.75)
    out m.get(3) or 0.0
    val s: Map<Str, Float> = {"a": 4.5}
    out s["a"]
    out s.get("a") or 0.0
}
"#;
    let (stdout, code) = run_llvm(src, "map_int_float_llvm.dtr");
    assert_eq!(code, 0);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["1.5", "2.25", "3.75", "4.5", "4.5"]
    );
}

#[test]
fn str_key_map_still_works() {
    let src = r#"
fn main() {
    val m: Map<Str, Int> = {"a": 10, "b": 20}
    out m.len()
    out m.get("b") or -1
    out m.contains("a")
    m.insert("c", 30)
    out m.get("c") or -1
}
"#;
    let (stdout, code) = run_native(src, "map_str_keys_regression.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["2", "20", "true", "30"]);
}

#[test]
fn wrong_key_kind_is_a_compile_error() {
    let compiler = ForgenCompiler::new("release");
    let src = r#"
fn main() {
    val m: Map<Str, Int> = {"a": 1}
    m.insert(1, 10)
}
"#;
    let res = compiler.check_source(src, "map_wrong_key.dtr");
    assert!(!res.success, "an Int key on a Map<Str, Int> must be rejected");
    let diags = res.diagnostics.clone();
    assert!(
        diags.contains("map.insert key type mismatch"),
        "diagnostic must name the mismatch: {diags}"
    );
}

#[test]
fn unsupported_key_type_is_a_compile_error() {
    let compiler = ForgenCompiler::new("release");
    let src = r#"
fn main() {
    val m: Map<Float, Int> = {1.5: 1}
}
"#;
    let res = compiler.check_source(src, "map_bad_key_type.dtr");
    assert!(!res.success, "Float map keys are unsupported");
    let diags = res.diagnostics.clone();
    assert!(
        diags.contains("unsupported map key type"),
        "diagnostic must explain the key contract: {diags}"
    );
}

#[test]
fn wrong_value_kind_is_a_compile_error() {
    let compiler = ForgenCompiler::new("release");
    let src = r#"
fn main() {
    val m: Map<Int, Int> = {1: 1}
    m.insert(2, "two")
}
"#;
    let res = compiler.check_source(src, "map_wrong_value.dtr");
    assert!(!res.success, "a Str value in a Map<Int, Int> must be rejected");
    let diags = res.diagnostics.clone();
    assert!(
        diags.contains("map.insert value type mismatch"),
        "diagnostic must name the value mismatch: {diags}"
    );
}
