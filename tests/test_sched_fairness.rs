//! PLAN_V152 N1.3 scheduler fairness: a fiber that polls (yield +
//! is_done) around a SLEEPING peer used to hang the scheduler forever —
//! the timer wheel was consulted only when the ready queue emptied, and
//! the continuously-yielding poller kept it non-empty. The first fix
//! attempt also dropped the promoted fiber on the floor (timer_pop_due
//! return discarded) and aliased &last_now as the out-param.

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
fn poller_around_sleeping_worker_terminates() {
    let src = r#"
use stdlib.async.async

fn worker(x: Int) {
    fiber_sleep_ms(10)
}

fn poller(h_handle: Int) {
    val h = fiber_handle(h_handle)
    while !h.is_done() {
        fiber_yield()
    }
    out "POLLER_SAW_DONE"
}

fn main() {
    val h = fiber_spawn(worker, 1)
    fiber_spawn(poller, h)
    fiber_run_until_done()
    out "MAIN_DONE"
}
"#;
    let started = std::time::Instant::now();
    let (out, code) = run_native(src, "sched_fairness");
    assert_eq!(code, 0, "the scheduler must not hang");
    assert_eq!(out, "POLLER_SAW_DONE\nMAIN_DONE");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the poll must observe the 10 ms sleep quickly, took {:?}",
        started.elapsed()
    );
}

/// FiberHandle API: is_done flips after the drain; join returns the exit
/// code.
#[test]
fn fiber_handle_is_done_and_join() {
    let src = r#"
use stdlib.async.async

fn quick(x: Int) {
    out "WORKER_RAN"
}

fn main() {
    val h = fiber_handle(fiber_spawn(quick, 1))
    fiber_run_until_done()
    out fmt"done={h.is_done()}"
    out int_to_str(h.join())
}
"#;
    let (out, code) = run_native(src, "fiber_handle_api");
    assert_eq!(code, 0);
    assert_eq!(out, "WORKER_RAN\ndone=true\n0");
}
