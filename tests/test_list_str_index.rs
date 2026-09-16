//! List<Str> element indexing tests.
//!
//! Regression tests for the silent-wrong-data bug where an inline
//! `parts[0]` index expression printed the raw element pointer as an
//! integer (e.g. 1580880920640) instead of the string value, because the
//! element type was never inferred for unannotated `let parts =
//! str_split(...)` bindings and string list literals, so the index
//! lowering classified the element as Int. Read through an explicitly
//! typed (`List<Str>`) helper parameter worked, which is the asymmetry
//! these tests pin down.

use forgen::driver::ForgenCompiler;

fn run_source(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, name, None);
    assert!(
        res.success,
        "Compilation should succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.unwrap();
    let (stdout, stderr, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    if code != 0 {
        eprintln!("stderr: {}", stderr);
    }
    (stdout, code)
}

/// The reviewer's exact repro: `str_split("a=b=c", "=")` then inline
/// `parts[0..2]` must print a/b/c, never pointer integers.
const SPLIT_INLINE: &str = r#"
fn main() -> Int {
    let parts = str_split("a=b=c", "=")
    println(parts.len())
    println(parts[0])
    println(parts[1])
    println(parts[2])
    if parts.len() != 3 { return 1 }
    if parts[0] != "a" { return 2 }
    if parts[1] != "b" { return 3 }
    if parts[2] != "c" { return 4 }
    return 0
}
"#;

#[test]
fn test_str_split_inline_index_prints_values() {
    let (stdout, code) = run_source(SPLIT_INLINE, "list_str_split_inline.dtr");
    assert_eq!(code, 0, "inline List<Str> indexing must be correct");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("3"),
        "len must be 3: {}",
        stdout
    );
    assert!(
        stdout.contains('a') && stdout.contains('b') && stdout.contains('c'),
        "elements must print as strings, got: {}",
        stdout
    );
    // A raw pointer would be a long digit run; the values are single chars.
    assert!(
        !stdout.contains("000000") && !stdout.contains("00000"),
        "must not print pointer integers: {}",
        stdout
    );
}

/// List<Str> literals must infer their element type too.
#[test]
fn test_list_str_literal_index_prints_value() {
    let source = r#"
fn main() -> Int {
    let lit = ["x", "y", "z"]
    println(lit[1])
    if lit[1] != "y" { return 1 }
    if lit[0] != "x" { return 2 }
    if lit[2] != "z" { return 3 }
    return 0
}
"#;
    let (stdout, code) = run_source(source, "list_str_literal.dtr");
    assert_eq!(code, 0, "literal List<Str> indexing must be correct");
    assert!(stdout.contains('y'), "lit[1] must be y, got: {}", stdout);
}

/// Elements read through an explicitly typed helper parameter (the path
/// that always worked) must keep working alongside the inline path.
#[test]
fn test_list_str_element_through_helper_param() {
    let source = r#"
fn peek(parts: List<Str>) -> Str {
    return parts[0]
}

fn peek_int(parts: List<Str>) -> Int {
    println(parts[1])
    return 0
}

fn main() -> Int {
    let parts = str_split("a=b=c", "=")
    if peek(parts) != "a" { return 1 }
    if peek_int(parts) != 0 { return 2 }
    if peek(parts) != "a" { return 3 }
    return 0
}
"#;
    let (stdout, code) = run_source(source, "list_str_helper.dtr");
    assert_eq!(code, 0, "helper parameter indexing must be correct");
    assert!(stdout.contains('b'), "helper must print b, got: {}", stdout);
}

/// Stringification inside a format string and a concatenation must also
/// see the element as a string, not an integer.
#[test]
fn test_list_str_element_in_format_and_print() {
    let source = r#"
fn main() -> Int {
    let parts = str_split("alpha-beta", "-")
    let joined = fmt"{parts[0]}|{parts[1]}"
    println(joined)
    if joined != "alpha|beta" { return 1 }
    return 0
}
"#;
    let (stdout, code) = run_source(source, "list_str_format.dtr");
    assert_eq!(code, 0, "format interpolation must use string values");
    assert!(
        stdout.contains("alpha|beta"),
        "formatted output must contain the strings, got: {}",
        stdout
    );
}

/// Bounds checks must stay in force: a statically known out-of-range
/// literal index is rejected at compile time with E0947.
#[test]
fn test_out_of_bounds_literal_index_still_errors() {
    let source = r#"
fn main() -> Int {
    let lit = ["x", "y", "z"]
    println(lit[5])
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, "list_str_oob.dtr", None);
    assert!(
        !res.success,
        "out-of-bounds literal index must fail compilation"
    );
    assert!(
        res.diagnostics.contains("E0947"),
        "diagnostics must carry E0947, got: {}",
        res.diagnostics
    );
}

/// Negative literal index is likewise rejected.
#[test]
fn test_negative_literal_index_still_errors() {
    let source = r#"
fn main() -> Int {
    let lit = ["x", "y", "z"]
    println(lit[-3])
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, "list_str_neg.dtr", None);
    assert!(!res.success, "negative literal index must fail compilation");
    assert!(
        res.diagnostics.contains("E0947"),
        "diagnostics must carry E0947, got: {}",
        res.diagnostics
    );
}

/// A runtime-known out-of-range index on a split result must stay
/// bounds-checked (fail-safe zero), never read out of the allocation.
#[test]
fn test_runtime_out_of_bounds_index_stays_bounds_checked() {
    let source = r#"
fn main() -> Int {
    let parts = str_split("a=b=c", "=")
    let idx = parts.len()
    let v = parts[idx]
    print("OOBLEN:")
    println(str_len(v))
    if str_len(v) == 0 {
        return 0
    }
    return 1
}
"#;
    let (stdout, code) = run_source(source, "list_str_runtime_oob.dtr");
    assert_eq!(code, 0, "runtime OOB must stay bounds-checked");
    assert!(
        stdout.contains("OOBLEN:0"),
        "OOB read must yield an empty (zero) value, got: {}",
        stdout
    );
}

/// Non-string element types keep their integer path: List<Int> indexing
/// must still print numbers (no string classification regression).
#[test]
fn test_list_int_index_still_prints_numbers() {
    let source = r#"
fn main() -> Int {
    let nums = [10, 20, 30]
    println(nums[1])
    if nums[1] != 20 { return 1 }
    return 0
}
"#;
    let (stdout, code) = run_source(source, "list_int_index.dtr");
    assert_eq!(code, 0, "List<Int> indexing must keep working");
    assert!(stdout.contains("20"), "nums[1] must be 20, got: {}", stdout);
}
