//! M10 NatVar: native fiber runtime — end-to-end tests (PLAN_V150 M10).
//!
//! Slice 1 semantics: one scheduler thread runs stackful fibers
//! (Windows CreateFiberEx / POSIX ucontext). `fiber_spawn(f, arg[, stack_kb])`
//! registers a fiber whose body is a void(Int) Datara function (code pointer
//! via GetFuncAddr); `fiber_run_until_done()` drains the scheduler;
//! `fiber_yield()` round-robins; `fiber_sleep_ms(ms)` parks the fiber on a
//! timer wheel without blocking other fibers; `fiber_join(h)` waits for a
//! peer's exit code; `fiber_chan_*` are rendezvous channels. Timing-order
//! assertions use only guaranteed happens-before edges (spawn→run→done),
//! never wall-clock scheduling races.

use forgen::driver::ForgenCompiler;

// The NatVar C runtime holds ONE global scheduler state per process, and
// Windows fibers are thread-bound: all fiber programs in this suite must
// run serialized on the same thread. The mutex enforces that under the
// default parallel test harness.
static FIBER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run_native(source: &str, name: &str) -> String {
    let _guard = FIBER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
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
    assert_eq!(code, 0, "native run exited {}: {}", code, stdout);
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

fn run_jit(source: &str, name: &str) -> String {
    let _guard = FIBER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

fn assert_both_tiers(source: &str, name: &str, expected: &str) {
    assert_eq!(run_native(source, name), expected, "native tier");
    assert_eq!(
        run_jit(source, &format!("{}_jit", name)),
        expected,
        "jit tier"
    );
}

/// Native-only variant: the fiber channels' main-thread drain and the
/// cross-program fiber counters are slice-1 semantics proven on the native
/// tier; the JIT host re-uses the same C runtime but its host-thread fiber
/// binding needs a dedicated pass (see PLAN_V150 M10 slice-1 notes).
fn assert_native(source: &str, name: &str, expected: &str) {
    assert_eq!(run_native(source, name), expected, "native tier");
}

/// 1. Basic spawn + drain: two fibers execute and complete.
#[test]
fn natvar_spawn_and_drain() {
    let src = r#"
mut total = 0
fn add(x: Int) {
    total = total + x
}
fn main() {
    val h1 = fiber_spawn(add, 10)
    val h2 = fiber_spawn(add, 20)
    val done = fiber_run_until_done()
    out int_to_str(done)
    out int_to_str(total)
    out int_to_str(fiber_join(h1))
    out int_to_str(fiber_join(h2))
}
"#;
    assert_both_tiers(src, "natvar_spawn", "2\n30\n0\n0");
}

/// 2. Yield round-robin: fibers interleave through a shared order log.
#[test]
fn natvar_yield_interleaves() {
    let src = r#"
mut log = ""
fn step_a(x: Int) {
    log = log + "a1"
    fiber_yield()
    log = log + "a2"
}
fn step_b(x: Int) {
    log = log + "b1"
    fiber_yield()
    log = log + "b2"
}
fn main() {
    fiber_spawn(step_a, 0)
    fiber_spawn(step_b, 0)
    fiber_run_until_done()
    out log
}
"#;
    // Two fibers alternate: a1, b1, a2, b2 (FIFO ready queue).
    assert_both_tiers(src, "natvar_yield", "a1b1a2b2");
}

/// 3. Sleep parks only the sleeping fiber: the other keeps running.
#[test]
fn natvar_sleep_parks_one() {
    let src = r#"
mut order = ""
fn sleeper(x: Int) {
    order = order + "S"
    fiber_sleep_ms(30)
    order = order + "E"
}
fn quick(x: Int) {
    order = order + "Q"
}
fn main() {
    fiber_spawn(sleeper, 0)
    fiber_spawn(quick, 0)
    fiber_run_until_done()
    out order
}
"#;
    // Q runs while the sleeper is parked; "E" lands after the wake.
    assert_both_tiers(src, "natvar_sleep", "SQE");
}

/// 4. Sleep ordering: later deadline wakes later (timer wheel monotonicity).
#[test]
fn natvar_sleep_order() {
    let src = r#"
mut order = ""
fn slowpoke(x: Int) {
    fiber_sleep_ms(50)
    order = order + "A"
}
fn quickling(x: Int) {
    fiber_sleep_ms(10)
    order = order + "B"
}
fn main() {
    fiber_spawn(slowpoke, 0)
    fiber_spawn(quickling, 0)
    fiber_run_until_done()
    out order
}
"#;
    // The 10 ms fiber wakes before the 50 ms one.
    assert_both_tiers(src, "natvar_sleep_order", "BA");
}

/// 5. Join from main after drain returns exit codes.
#[test]
fn natvar_join_after_drain() {
    let src = r#"
fn noop(x: Int) {}
fn main() {
    val h = fiber_spawn(noop, 7)
    fiber_run_until_done()
    out int_to_str(fiber_join(h))
}
"#;
    assert_both_tiers(src, "natvar_join", "0");
}

/// 6. Fiber counters report spawn/done counts.
#[test]
fn natvar_counters() {
    let src = r#"
fn noop(x: Int) {}
fn main() {
    val before = fiber_count()
    fiber_spawn(noop, 1)
    fiber_spawn(noop, 2)
    fiber_spawn(noop, 3)
    val spawned = fiber_count() - before
    fiber_run_until_done()
    out int_to_str(spawned)
    out int_to_str(fiber_done_count())
}
"#;
    assert_native(src, "natvar_counters", "3\n3");
}

/// 7. Channels: send→recv rendezvous delivers the value.
#[test]
fn natvar_chan_basic() {
    let src = r#"
mut got = 0
fn sender(ch: Int) {
    fiber_chan_send(ch, 4242)
}
fn main() {
    val ch = fiber_chan_create()
    fiber_spawn(sender, ch)
    got = fiber_chan_recv(ch)
    fiber_run_until_done()
    out int_to_str(got)
}
"#;
    assert_native(src, "natvar_chan", "4242");
}

/// 8. Channel ping-pong between two fibers (multiple messages, order kept).
#[test]
fn natvar_chan_ping_pong() {
    let src = r#"
mut seq = ""
fn sender(ch: Int) {
    fiber_chan_send(ch, 1)
    fiber_chan_send(ch, 2)
    fiber_chan_send(ch, 3)
}
fn main() {
    val ch = fiber_chan_create()
    fiber_spawn(sender, ch)
    mut v = fiber_chan_recv(ch)
    seq = seq + int_to_str(v)
    v = fiber_chan_recv(ch)
    seq = seq + int_to_str(v)
    v = fiber_chan_recv(ch)
    seq = seq + int_to_str(v)
    fiber_run_until_done()
    out seq
}
"#;
    assert_native(src, "natvar_pingpong", "123");
}

/// 9. Custom stack size is accepted (spawn with explicit stack_kb).
#[test]
fn natvar_custom_stack() {
    let src = r#"
fn deep(x: Int) {
    mut acc = x
    mut i = 0
    while i < 100 {
        acc = acc + 1
        i = i + 1
    }
    gtotal = gtotal + acc
}
mut gtotal = 0
fn main() {
    fiber_spawn(deep, 1, 64)
    fiber_run_until_done()
    out int_to_str(gtotal)
}
"#;
    assert_both_tiers(src, "natvar_stack", "101");
}

/// 10. Many fibers: 2000 fibers all complete (scheduler scalability smoke).
#[test]
fn natvar_many_fibers() {
    let src = r#"
mut sum = 0
fn bump(x: Int) {
    sum = sum + 1
}
fn main() {
    mut i = 0
    while i < 2000 {
        fiber_spawn(bump, i)
        i = i + 1
    }
    fiber_run_until_done()
    out int_to_str(sum)
}
"#;
    assert_both_tiers(src, "natvar_many", "2000");
}

/// 11. yield() outside a fiber is a safe no-op (main is the scheduler).
#[test]
fn natvar_yield_outside_is_noop() {
    let src = r#"
fn main() {
    fiber_yield()
    out "alive"
}
"#;
    assert_both_tiers(src, "natvar_yield_outside", "alive");
}

/// 12. Join on an unknown handle returns -1 (fail-closed, no crash).
#[test]
fn natvar_join_unknown_handle() {
    let src = r#"
fn main() {
    out int_to_str(fiber_join(99999))
}
"#;
    assert_both_tiers(src, "natvar_join_unknown", "-1");
}

/// 13. Fiber body calling another Datara function (indirect via body).
#[test]
fn natvar_body_calls_helper() {
    let src = r#"
fn helper(a: Int, b: Int) -> Int {
    return a * b
}
fn body(x: Int) {
    gval = helper(x, x + 1)
}
mut gval = 0
fn main() {
    fiber_spawn(body, 6)
    fiber_run_until_done()
    out int_to_str(gval)
}
"#;
    assert_both_tiers(src, "natvar_helper", "42");
}

/// 14. Sleep in a loop: repeated parking keeps the ordering stable.
#[test]
fn natvar_repeated_sleep() {
    let src = r#"
mut ticks = 0
fn ticker(x: Int) {
    mut i = 0
    while i < 3 {
        fiber_sleep_ms(5)
        ticks = ticks + 1
        i = i + 1
    }
}
fn main() {
    fiber_spawn(ticker, 0)
    fiber_run_until_done()
    out int_to_str(ticks)
}
"#;
    assert_both_tiers(src, "natvar_repeat_sleep", "3");
}

/// 15. JIT tier smoke: spawn+drain+join through the JIT backend.
#[test]
fn natvar_jit_smoke() {
    let src = r#"
mut total = 0
fn add(x: Int) {
    total = total + x
}
fn main() {
    fiber_spawn(add, 5)
    fiber_spawn(add, 6)
    fiber_run_until_done()
    out int_to_str(total)
}
"#;
    assert_eq!(run_jit(src, "natvar_jit_smoke"), "11");
}

/// 16. Fiber API survives a scheduler reset between main invocations is not
///     needed within one program — but two sequential drain rounds must work.
#[test]
fn natvar_two_drain_rounds() {
    let src = r#"
mut acc = 0
fn add(x: Int) {
    acc = acc + x
}
fn main() {
    fiber_spawn(add, 1)
    fiber_run_until_done()
    val first = acc
    fiber_spawn(add, 10)
    fiber_run_until_done()
    out int_to_str(first)
    out int_to_str(acc)
}
"#;
    assert_both_tiers(src, "natvar_rounds", "1\n11");
}
