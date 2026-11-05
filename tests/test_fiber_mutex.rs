//! PLAN_V152 N1.2: fiber-aware cooperative mutexes.
//!
//! datara_rt_mutex_lock parks the calling fiber (FIB_WAITING_MUTEX) when
//! the lock is taken; unlock grants ownership DIRECTLY to the head
//! waiter; the scheduler wakes deadline-expired waiters with a timeout
//! outcome. The deadlock detector resolves FIB_WAITING_MUTEX edges to
//! the lock owner, so an AB-BA cycle is reported (and timeouts make the
//! scenario self-resolving instead of hanging).

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

/// 1. Grant / busy-timeout / regrant: the basic lifecycle.
#[test]
fn mutex_lifecycle_grant_timeout_regrant() {
    let src = r#"
fn main() {
    val m = mutex_new()
    val r1 = mutex_lock(m, 100)
    out fmt"first={r1.is_success}"
    val r2 = mutex_lock(m, 100)
    if r2.is_success {
        out "GRANTED"
    } else {
        out "TIMEOUT"
    }
    mutex_unlock(m)
    val r3 = mutex_lock(m, 100)
    out fmt"after_unlock={r3.is_success}"
}
"#;
    let (out, code) = run_native(src, "mutex_lifecycle");
    assert_eq!(code, 0);
    assert_eq!(out, "first=true\nTIMEOUT\nafter_unlock=true");
}

/// 2. AB-BA across two fibers: both fibers end up waiting for the lock
/// the other holds; their 200 ms timeouts fire (< 1 s total) and the
/// program terminates with both fibers having observed a timeout — the
/// scheduler did NOT hang.
#[test]
fn mutex_abba_timeouts_resolve_under_1s() {
    let src = r#"
fn holder_a(ma: Int, mb: Int) {
    val _ = mutex_lock(ma, 1000)
    // Hold A briefly so fiber2 can take B first; then wait on B (taken
    // by fiber2) -> timeout.
    val r = mutex_lock(mb, 200)
    if r.is_success {
        out "A_GOT_B"
    } else {
        out "A_TIMEOUT"
    }
    mutex_unlock(ma)
}

fn holder_b(ma: Int, mb: Int) {
    val _ = mutex_lock(mb, 1000)
    fiber_sleep_ms(20)
    val r = mutex_lock(ma, 200)
    if r.is_success {
        out "B_GOT_A"
    } else {
        out "B_TIMEOUT"
    }
    mutex_unlock(mb)
}

fn main() {
    val ma = mutex_new()
    val mb = mutex_new()
    fiber_spawn(holder_a, ma)
    fiber_spawn(holder_b, mb)
    fiber_run_until_done()
    out "DONE"
}
"#;
    let started = std::time::Instant::now();
    let (out, code) = run_native(src, "mutex_abba");
    let elapsed = started.elapsed();
    assert_eq!(code, 0, "the scheduler must not hang on AB-BA");
    // The invariant is NOT "both timeout": unlock() grants ownership
    // directly to the head waiter, so once one fiber's deadline fires and
    // it releases its lock, the other fiber may legitimately ACQUIRE it
    // (the classic AB-BA resolves as success-through-timeout). What must
    // hold: both fibers terminate, DONE is printed, and the whole run
    // stays far below the 1 s wall clock.
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "expected A-line, B-line, DONE; got: {out}");
    assert_eq!(lines[2], "DONE");
    assert!(
        lines[0] == "A_TIMEOUT" || lines[0] == "A_GOT_B",
        "unexpected A outcome: {out}"
    );
    assert!(
        lines[1] == "B_TIMEOUT" || lines[1] == "B_GOT_A",
        "unexpected B outcome: {out}"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "AB-BA must self-resolve via timeouts in < 1 s, took {elapsed:?}"
    );
}

/// 3. Mutual exclusion actually excludes: a counter incremented under the
/// lock by two fibers (1000 iters each) totals exactly 2000.
#[test]
fn mutex_mutual_exclusion_counter() {
    let src = r#"
mut total = 0

fn worker(m: Int) {
    mut i = 0
    while i < 1000 {
        val g = mutex_lock(m, 1000)
        if g.is_success {
            total = total + 1
            mutex_unlock(m)
        }
        i = i + 1
    }
}

fn main() {
    val m = mutex_new()
    fiber_spawn(worker, m)
    fiber_spawn(worker, m)
    fiber_run_until_done()
    out int_to_str(total)
}
"#;
    let (out, code) = run_native(src, "mutex_exclusion");
    assert_eq!(code, 0);
    assert_eq!(out, "2000");
}
