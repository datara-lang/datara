//! PLAN_V153 PE3: async sugar — wait_timeout + drain_all.
//! wait_timeout drives the scheduler via fiber_run_until_done (the
//! main function runs ON the scheduler fiber — a fiber_sleep_ms in
//! main is a no-op; only run_until_done lets spawned fibers execute).

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
fn wait_timeout_and_drain() {
    let src = r#"
use stdlib.async.sugar

fn worker(x: Int) {
    fiber_sleep_ms(5)
    out "WORKER_DONE"
}

fn main() {
    val h = fiber_handle(fiber_spawn(worker, 0))
    val done = wait_timeout(h.handle, 1000)
    out fmt"done={done}"
    val drained = drain_all()
    out int_to_str(drained)
    out fmt"final={fiber_is_done(h.handle)}"
}
"#;
    let (out, code) = run_native(src, "async_sugar.dtr");
    assert_eq!(code, 0);
    assert!(
        out.contains("done=true"),
        "wait_timeout must see completion: {out}"
    );
    assert!(out.contains("WORKER_DONE"));
    assert!(out.contains("final=true"));
}
