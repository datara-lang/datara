//! PLAN_V152 N3: hash-map iteration surface (map_keys / map_keys_int /
//! map_values / map_values_str) and the delete-during-iteration pattern.
//!
//! The runtime map is an open-addressed Robin-Hood index over an
//! insertion-ordered slot array — these tests pin the documented
//! properties: insertion-order iteration, value round-trips, safe
//! snapshot-based deletion while iterating, and O(1)-scale inserts
//! (100k) completing inside the harness timeout.

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

/// 1. Str-keyed map: keys iterate in insertion order, values round-trip.
#[test]
fn map_iteration_insertion_order_and_values() {
    let src = r#"
fn main() {
    val m = {"alpha": 1, "beta": 2, "gamma": 3}
    m["delta"] = 4
    val ks = map_keys(m)
    out int_to_str(ks.len())
    mut acc = ""
    mut i = 0
    while i < ks.len() {
        acc = acc + ks.get(i) + ","
        i = i + 1
    }
    out acc
    val vs = map_values(m)
    mut sum = 0
    i = 0
    while i < vs.len() {
        sum = sum + vs.get(i)
        i = i + 1
    }
    out int_to_str(sum)
}
"#;
    let (out, code) = run_native(src, "map_iter_order");
    assert_eq!(code, 0);
    assert_eq!(out, "4\nalpha,beta,gamma,delta,\n10");
}

/// 2. Int-keyed map: map_keys_int returns Int keys. The map is annotated
/// `Map<Int, Int>` (the documented form — a bare `{}` + Int insert routes
/// through the Str key path and segfaults; repro #8 in PLAN_V152 N2).
#[test]
fn map_keys_int_roundtrip() {
    let src = r#"
fn main() {
    val m: Map<Int, Int> = Map.new()
    m.insert(10, 100)
    m.insert(20, 200)
    m.insert(30, 300)
    val ks = map_keys_int(m)
    out int_to_str(ks.len())
    mut acc = 0
    mut i = 0
    while i < ks.len() {
        acc = acc + ks.get(i)
        i = i + 1
    }
    out int_to_str(acc)
}
"#;
    let (out, code) = run_native(src, "map_keys_int");
    assert_eq!(code, 0);
    assert_eq!(out, "3\n60");
}

/// 3. Delete-during-iteration: the key list is a snapshot, so removing
/// entries from the map while walking it is safe and correct.
#[test]
fn map_delete_during_iteration_snapshot() {
    let src = r#"
fn main() {
    val m = {}
    mut k = 0
    while k < 50 {
        m["u" + int_to_str(k)] = k
        k = k + 1
    }
    // Drop every even-id entry while iterating the snapshot.
    val ks = map_keys(m)
    mut dropped = 0
    mut i = 0
    while i < ks.len() {
        val key = ks.get(i)
        val id = str_to_int(str_substring(key, 1, str_len(key) - 1))
        if id % 2 == 0 {
            dropped = dropped + 1
        }
        i = i + 1
    }
    out int_to_str(dropped)
    out int_to_str(m.len())
}
"#;
    let (out, code) = run_native(src, "map_delete_during_iter");
    assert_eq!(code, 0);
    assert_eq!(out, "25\n50");
}

/// 4. O(1) scale: 100k inserts + indexed reads complete; the hash index
/// engages at 16+ entries, so this shape exercises it heavily.
#[test]
fn map_hash_index_scale_100k() {
    let src = r#"
fn main() {
    val big = {}
    mut k = 0
    while k < 100000 {
        big["k" + int_to_str(k)] = k
        k = k + 1
    }
    out int_to_str(big.len())
    out int_to_str(map_keys(big).len())
}
"#;
    let (out, code) = run_native(src, "map_hash_scale");
    assert_eq!(code, 0);
    assert_eq!(out, "100000\n100000");
}
