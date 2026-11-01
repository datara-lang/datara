//! PLAN_V152 N1.2: AtomicInt — MT shared-counter primitives. The whole
//! point: plain `+=` on a shared cell loses updates under races; atomic
//! fetch_add must land exactly on the expected total across 8 workers.

use forgen::driver::ForgenCompiler;

const MT_ATOMIC: &str = r#"
fn worker(cell: Int, id: Int) {
    mut i = 0
    while i < 1250 {
        atomic_fetch_add(cell, 1)
        i = i + 1
    }
}

fn main() {
    fiber_mt_start(8)
    val cell = atomic_new(0)
    mut w = 0
    while w < 8 {
        fiber_mt_spawn(worker, cell)
        w = w + 1
    }
    fiber_mt_wait_idle()
    out int_to_str(atomic_load(cell))
    atomic_store(cell, 5)
    out int_to_str(atomic_load(cell))
    val old = atomic_cas(cell, 5, 500)
    out int_to_str(old)
    out int_to_str(atomic_load(cell))
}
"#;

fn run_tier(extra: &[&str], name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(MT_ATOMIC, name, None);
    assert!(
        res.success,
        "compilation must succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0);
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    let _ = extra;
    stdout.trim().to_string()
}

#[test]
fn test_atomic_mt_fetch_add_exact_and_cas() {
    let out = run_tier(&[], "atomic_mt_cranelift.dtr");
    assert_eq!(
        out, "10000\n5\n5\n500",
        "8x1250 fetch_add must be exactly 10000; cas must return the old value"
    );
}
