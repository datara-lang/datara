//! v1.4.1 Full List<T> API tests.
//!
//! Contract notes:
//! - sort is a stable in-place merge sort; elem_kind is inferred from the
//!   list's element repr. Stability on duplicates is verified only for
//!   equal-element runs (values with tags would need pair structures, which
//!   Datara lists do not carry -- see the report's limitations section).
//! - pop/first/last return the checked Outcome<T> object ("empty list" on an
//!   empty receiver) and require the stdlib Outcome import for
//!   unwrap/is_err/err, exactly like the checked-I/O builtins.
//! - remove_at/remove_value return Int (1 = removed, 0 = nothing changed);
//!   index_of returns -1 when absent; contains/is_empty return Bool.
//! - insert_at may reallocate (idx == count appends); the receiver handle
//!   is reassigned by the compiler like push/set.

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

fn run_datara(source: &str, name: &str) -> String {
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}",
        name, res.error
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "{} exited with {}", name, code);

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

fn run_jit(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

const OUTCOME_IMPORT: &str = "use stdlib.result.result.Outcome\n";

// ---------------------------------------------------------------------------
// sort
// ---------------------------------------------------------------------------

#[test]
fn test_sort_ints_in_place() {
    let out = run_datara(
        r#"
fn main() {
    let xs = [5, 3, 9, 1, 7]
    xs.sort()
    out xs.len()
    out xs[0]
    out xs[1]
    out xs[2]
    out xs[3]
    out xs[4]
}
"#,
        "list_sort_ints",
    );
    assert_eq!(out, "5\n1\n3\n5\n7\n9");
}

#[test]
fn test_sort_floats() {
    let out = run_datara(
        r#"
fn main() {
    let xs = [2.5, 1.5, 3.5, 0.5]
    xs.sort()
    out xs[0]
    out xs[3]
}
"#,
        "list_sort_floats",
    );
    assert_eq!(out, "0.5\n3.5");
}

#[test]
fn test_sort_strings() {
    let out = run_datara(
        r#"
fn main() {
    let ws = ["pear", "apple", "orange"]
    ws.sort()
    out ws[0]
    out ws[1]
    out ws[2]
}
"#,
        "list_sort_strs",
    );
    assert_eq!(out, "apple\norange\npear");
}

#[test]
fn test_sort_duplicate_values_unchanged() {
    // Stability limit: with identical values there is no observable
    // permutation, so a stable sort must leave the list untouched. Ordering
    // tagged duplicates would require pair structures that lists do not
    // carry (documented limitation).
    let out = run_datara(
        r#"
fn main() {
    let xs = [7, 7, 7, 7]
    xs.sort()
    out xs.len()
    out xs[0]
    out xs[3]
}
"#,
        "list_sort_dup",
    );
    assert_eq!(out, "4\n7\n7");
}

#[test]
fn test_sort_single_and_after_clear() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [2, 1]
    xs.clear()
    xs.sort()
    out xs.len()
    xs.push(1)
    xs.sort()
    out xs[0]
}
"#,
        "list_sort_edge",
    );
    assert_eq!(out, "0\n1");
}

#[test]
fn test_sort_returns_receiver() {
    // The mutators return the (unchanged) receiver handle, so the result
    // stays a list: printing it prints list contents, not an int.
    let out = run_datara(
        r#"
fn main() {
    let xs = [3, 1, 2]
    let sorted = xs.sort()
    out sorted[0]
    out sorted.len()
}
"#,
        "list_sort_ret",
    );
    assert_eq!(out, "1\n3");
}

// ---------------------------------------------------------------------------
// remove_at / remove_value
// ---------------------------------------------------------------------------

#[test]
fn test_remove_at_valid() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [10, 20, 30]
    out xs.remove_at(1)
    out xs.len()
    out xs[0]
    out xs[1]
}
"#,
        "list_remove_at",
    );
    assert_eq!(out, "1\n2\n10\n30");
}

#[test]
fn test_remove_at_oob_is_zero() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [10, 20, 30]
    out xs.remove_at(5)
    out xs.remove_at(-1)
    out xs.len()
}
"#,
        "list_remove_at_oob",
    );
    assert_eq!(out, "0\n0\n3");
}

#[test]
fn test_remove_value_first_occurrence() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [4, 2, 4, 2]
    out xs.remove_value(2)
    out xs.len()
    out xs[0]
    out xs[1]
    out xs[2]
}
"#,
        "list_remove_value",
    );
    // The first 2 (index 1) is removed; the second 2 (index 3) survives.
    assert_eq!(out, "1\n3\n4\n4\n2");
}

#[test]
fn test_remove_value_missing_is_zero() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [1, 2, 3]
    out xs.remove_value(99)
    out xs.len()
}
"#,
        "list_remove_value_missing",
    );
    assert_eq!(out, "0\n3");
}

// ---------------------------------------------------------------------------
// insert_at
// ---------------------------------------------------------------------------

#[test]
fn test_insert_at_begin_mid_end() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [20, 30]
    xs.insert_at(0, 10)
    xs.insert_at(2, 25)
    xs.insert_at(4, 40)
    out xs[0]
    out xs[1]
    out xs[2]
    out xs[3]
    out xs[4]
}
"#,
        "list_insert_at_all",
    );
    assert_eq!(out, "10\n20\n25\n30\n40");
}

#[test]
fn test_insert_at_oob_is_noop() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [10, 20]
    xs.insert_at(9, 99)
    out xs.len()
}
"#,
        "list_insert_at_oob",
    );
    assert_eq!(out, "2");
}

#[test]
fn test_insert_at_grows_past_capacity() {
    // Force the full-capacity growth path: a literal of exactly the
    // initial capacity then one more insert.
    let out = run_datara(
        r#"
fn main() {
    mut xs = [1, 2, 3, 4, 5, 6, 7, 8]
    xs.insert_at(0, 0)
    out xs.len()
    out xs[0]
    out xs[1]
    out xs[8]
}
"#,
        "list_insert_at_grow",
    );
    assert_eq!(out, "9\n0\n1\n8");
}

// ---------------------------------------------------------------------------
// contains / index_of / is_empty
// ---------------------------------------------------------------------------

#[test]
fn test_contains_bool_result() {
    let out = run_datara(
        r#"
fn main() {
    let xs = [10, 20, 30]
    out xs.contains(20)
    out xs.contains(99)
}
"#,
        "list_contains",
    );
    assert_eq!(out, "true\nfalse");
}

#[test]
fn test_index_of_found_and_missing() {
    let out = run_datara(
        r#"
fn main() {
    let xs = [10, 20, 30]
    out xs.index_of(20)
    out xs.index_of(99)
}
"#,
        "list_index_of",
    );
    assert_eq!(out, "1\n-1");
}

#[test]
fn test_is_empty() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [1]
    out xs.is_empty()
    xs.clear()
    out xs.is_empty()
}
"#,
        "list_is_empty",
    );
    assert_eq!(out, "false\ntrue");
}

#[test]
fn test_contains_strings() {
    let out = run_datara(
        r#"
fn main() {
    let ws = ["alpha", "beta"]
    out ws.contains("beta")
    out ws.contains("gamma")
}
"#,
        "list_contains_str",
    );
    assert_eq!(out, "true\nfalse");
}

// ---------------------------------------------------------------------------
// reverse / clear
// ---------------------------------------------------------------------------

#[test]
fn test_reverse_odd_even_empty() {
    let out = run_datara(
        r#"
fn main() {
    mut a = [1, 2, 3]
    a.reverse()
    out a[0]
    out a[2]
    mut b = [1, 2, 3, 4]
    b.reverse()
    out b[0]
    out b[3]
    mut c = [9]
    c.clear()
    c.reverse()
    out c.len()
}
"#,
        "list_reverse",
    );
    assert_eq!(out, "3\n1\n4\n1\n0");
}

#[test]
fn test_clear_resets_count() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [1, 2, 3]
    xs.clear()
    out xs.len()
    out xs.is_empty()
}
"#,
        "list_clear",
    );
    assert_eq!(out, "0\ntrue");
}

// ---------------------------------------------------------------------------
// slice (method form; index-range x[a..b] is the older, separate path)
// ---------------------------------------------------------------------------

#[test]
fn test_slice_partial_and_clamp() {
    let out = run_datara(
        r#"
fn main() {
    let xs = [10, 20, 30, 40]
    let mid = xs.slice(1, 3)
    out mid.len()
    out mid[0]
    out mid[1]
    let clamped = xs.slice(2, 99)
    out clamped.len()
    let empty = xs.slice(3, 1)
    out empty.len()
}
"#,
        "list_slice",
    );
    assert_eq!(out, "2\n20\n30\n2\n0");
}

// ---------------------------------------------------------------------------
// first / last / pop: checked Outcome<T> accessors
// ---------------------------------------------------------------------------

#[test]
fn test_first_last_ok() {
    let out = run_datara(
        &format!(
            "{}\n{}",
            OUTCOME_IMPORT,
            r#"
fn main() {
    let xs = [10, 20, 30]
    out xs.first().unwrap()
    out xs.last().unwrap()
    out xs.len()
}
"#
        ),
        "list_first_last",
    );
    // Non-mutating: length is unchanged.
    assert_eq!(out, "10\n30\n3");
}

#[test]
fn test_first_last_empty_are_err() {
    let out = run_datara(
        &format!(
            "{}\n{}",
            OUTCOME_IMPORT,
            r#"
fn main() {
    mut xs = [1]
    xs.clear()
    let f = xs.first()
    let l = xs.last()
    if f.is_err() {
        out "F_ERR"
    }
    if l.is_err() {
        out "L_ERR"
    }
    out f.err()
}
"#
        ),
        "list_first_last_err",
    );
    assert_eq!(out, "F_ERR\nL_ERR\nempty list");
}

#[test]
fn test_pop_outcome_ok() {
    let out = run_datara(
        &format!(
            "{}\n{}",
            OUTCOME_IMPORT,
            r#"
fn main() {
    mut xs = [10, 20, 30]
    let a = xs.pop()
    let b = xs.pop()
    out a.unwrap()
    out b.unwrap()
    out xs.len()
}
"#
        ),
        "list_pop_ok",
    );
    assert_eq!(out, "30\n20\n1");
}

#[test]
fn test_pop_empty_is_err_with_message() {
    let out = run_datara(
        &format!(
            "{}\n{}",
            OUTCOME_IMPORT,
            r#"
fn main() {
    mut xs = [1]
    xs.clear()
    let o = xs.pop()
    if o.is_err() {
        out o.err()
    }
}
"#
        ),
        "list_pop_err",
    );
    assert_eq!(out, "empty list");
}

#[test]
fn test_question_propagates_pop_error() {
    // `?` works on the checked accessors: an empty-list error propagates
    // out of the helper instead of crashing on a bogus element.
    let out = run_datara(
        &format!(
            "{}\n{}",
            OUTCOME_IMPORT,
            r#"
fn grab_last(xs: List<Int>) -> Outcome<Int> {
    let v = xs.last()?
    return Outcome<Int> { is_success: true, value: v + 1, error_msg: "" }
}
fn main() {
    mut good = [41, 42]
    let ok = grab_last(good)
    out ok.unwrap()
    mut bad = [0]
    bad.clear()
    let failed = grab_last(bad)
    if failed.is_err() {
        out failed.err()
    }
}
"#
        ),
        "list_pop_question",
    );
    assert_eq!(out, "43\nempty list");
}

// ---------------------------------------------------------------------------
// String/Float element specifics
// ---------------------------------------------------------------------------

#[test]
fn test_string_remove_and_contains() {
    let out = run_datara(
        r#"
fn main() {
    mut ws = ["alpha", "beta", "alpha"]
    out ws.remove_value("alpha")
    out ws.len()
    out ws[0]
    out ws[1]
    out ws.contains("alpha")
    out ws.index_of("beta")
}
"#,
        "list_str_ops",
    );
    assert_eq!(out, "1\n2\nbeta\nalpha\ntrue\n0");
}

#[test]
fn test_float_contains_and_remove() {
    let out = run_datara(
        r#"
fn main() {
    mut xs = [1.5, 2.5, 3.5]
    out xs.contains(2.5)
    out xs.remove_value(2.5)
    out xs.len()
    out xs.contains(2.5)
}
"#,
        "list_float_ops",
    );
    assert_eq!(out, "true\n1\n2\nfalse");
}

// ---------------------------------------------------------------------------
// JIT parity: sort and pop through the in-memory JIT (no disk artifacts).
// ---------------------------------------------------------------------------

#[test]
fn test_jit_sort_and_pop() {
    let sort_src = r#"
use stdlib.result.result.Outcome
fn main() {
    let xs = [5, 3, 9, 1, 7]
    xs.sort()
    out xs[0]
    out xs[4]
    mut ws = ["pear", "apple"]
    ws.sort()
    out ws[0]
    mut ys = [1]
    let o = ys.pop()
    out o.unwrap()
}
"#;
    assert_eq!(run_jit(sort_src, "jit_list_sort"), "1\n9\napple\n1");

    let pop_src = r#"
use stdlib.result.result.Outcome
fn main() {
    mut xs = [7, 8]
    let a = xs.pop()
    let b = xs.pop()
    out a.unwrap()
    out b.unwrap()
    let c = xs.pop()
    if c.is_err() {
        out c.err()
    }
}
"#;
    assert_eq!(run_jit(pop_src, "jit_list_pop"), "8\n7\nempty list");
}
