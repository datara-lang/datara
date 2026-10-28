//! M10 slice 2: multi-threaded fiber scheduler (PLAN_V151 N2).
//! Cross-worker correctness via single-writer slots; FIBER_LOCK serializes
//! process-global MT state under the parallel test harness.

use forgen::driver::ForgenCompiler;

static MT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run_native(source: &str, name: &str) -> String {
    let _g = MT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compile failed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("native exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("run exe");
    assert_eq!(code, 0, "exit {}: {}", code, stdout);
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

/// 10k tasks on 8 workers: every slot written exactly once by its own task.
#[test]
fn mt_10k_tasks_all_correct() {
    let src = r#"
mut slots: List<Int> = List.new(10000)
fn work(x: Int) {
    slots[x] = x + 1
}
fn main() {
    val workers = fiber_mt_start(8)
    out int_to_str(workers)
    mut i = 0
    while i < 10000 {
        fiber_mt_spawn(work, i)
        i = i + 1
    }
    fiber_mt_wait_idle()
    out int_to_str(fiber_mt_done_count())
    mut ok = 0
    mut j = 0
    while j < 10000 {
        if slots[j] == j + 1 {
            ok = ok + 1
        }
        j = j + 1
    }
    out int_to_str(ok)
}
"#;
    let out = run_native(src, "natvar_mt_10k.dtr");
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines[0], "8");
    assert_eq!(lines[1], "10000", "done count");
    assert_eq!(lines[2], "10000", "all slots correct");
}

/// Spawn after a completed batch works (two sequential MT rounds).
#[test]
fn mt_two_rounds() {
    let src = r#"
mut slots: List<Int> = List.new(4)
fn work(x: Int) {
    slots[x] = x + 1
}
fn main() {
    fiber_mt_start(4)
    mut r = 0
    while r < 2 {
        mut i = 0
        while i < 2 {
            fiber_mt_spawn(work, r * 2 + i)
            i = i + 1
        }
        fiber_mt_wait_idle()
        r = r + 1
    }
    out int_to_str(slots[0] + slots[1] + slots[2] + slots[3])
}
"#;
    // Slots receive x+1 for x in 0..4 across two rounds: 1+2+3+4 = 10.
    let out = run_native(src, "natvar_mt_rounds.dtr");
    assert_eq!(out, "10");
}

/// MT spawn count reporting.
#[test]
fn mt_spawned_counter() {
    let src = r#"
fn noop(x: Int) {}
fn main() {
    fiber_mt_start(2)
    mut i = 0
    while i < 50 {
        fiber_mt_spawn(noop, i)
        i = i + 1
    }
    fiber_mt_wait_idle()
    out int_to_str(fiber_mt_spawned())
}
"#;
    let out = run_native(src, "natvar_mt_spawned.dtr");
    assert_eq!(out, "50");
}
