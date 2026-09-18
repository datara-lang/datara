//! v1.4.4 Concurrency, Channels, Parallel Loops & Autonomous Transient Memory Tests.

use forgen::driver::ForgenCompiler;

fn run_datara(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}\n{}",
        name, res.error, res.diagnostics
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    if code != 0 {
        eprintln!("STDERR: {}", stderr);
    }
    (stdout.trim().replace("\r\n", "\n"), code)
}

#[test]
fn test_scratchpad_arena_lifecycle() {
    let src = r#"
fn main() -> Int {
    let wm = scratch_enter()
    let p1 = scratch_alloc(128, 8)
    let p2 = scratch_alloc(256, 8)
    scratch_exit(wm)
    print("scratch ok")
    return 0
}
"#;
    let (stdout, code) = run_datara(src, "test_scratch.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "scratch ok");
}

#[test]
fn test_channel_send_recv_lifecycle() {
    let src = r#"
fn main() -> Int {
    let ch = channel_create(10)
    channel_send(ch, 42)
    channel_send(ch, 99)
    let v1 = channel_recv(ch)
    let v2 = channel_recv(ch)
    print(v1)
    print(v2)
    channel_close(ch)
    return 0
}
"#;
    let (stdout, code) = run_datara(src, "test_channel.dtr");
    assert_eq!(code, 0);
    assert!(stdout.contains("42"));
    assert!(stdout.contains("99"));
}

#[test]
fn test_channel_methods() {
    let src = r#"
fn main() -> Int {
    let ch = channel_create(5)
    ch.send(100)
    ch.send(200)
    let len = ch.len()
    let v1 = ch.recv()
    let v2 = ch.recv()
    ch.close()
    ch.free()
    print(len)
    print(v1)
    print(v2)
    return 0
}
"#;
    let (stdout, code) = run_datara(src, "test_channel_m.dtr");
    assert_eq!(code, 0);
    assert!(stdout.contains("2"));
    assert!(stdout.contains("100"));
    assert!(stdout.contains("200"));
}

#[test]
fn test_scratchpad_promote() {
    let src = r#"
fn main() -> Int {
    let wm = scratch_enter()
    let p = scratch_alloc(64, 8)
    let promoted = scratch_promote(p, 64)
    scratch_exit(wm)
    print("promote ok")
    return 0
}
"#;
    let (stdout, code) = run_datara(src, "test_promote.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "promote ok");
}

#[test]
fn test_thread_spawn_and_join() {
    let src = r#"
fn compute() -> Int {
    return 12345
}

fn main() -> Int {
    let th = spawn(compute)
    let res = join(th)
    print(res)
    return 0
}
"#;
    let (stdout, code) = run_datara(src, "test_spawn.dtr");
    assert_eq!(code, 0);
    assert!(stdout.contains("12345"));
}


