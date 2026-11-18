//! PLAN_V153 A2: stdlib.net machinery contract (native/AOT tier).
//!
//! VERIFIED (this test): tcp_listen (non-blocking flip), tcp_accept
//! PARKS the fiber and times out cleanly at its deadline, tcp_close
//! works, the scheduler drains.
//!
//! E3/repro #14 CLOSED (2026-11-18): the full echo now passes OUTSIDE
//! gdb 5/5 on the AOT tier - root cause was datara_rt_outcome_build
//! storing stack-buffer string pointers verbatim (dangling payloads),
//! plus socket.dtr's TcpStream blocking the scheduler thread on recv.
//! The full echo + HTTP round trip are pinned in tests/test_fiber_echo.rs.
//!
//! The test below pins the RELIABLE contract (accept + timeout drain);
//! the echo shape is the E3 acceptance criterion.

use forgen::driver::ForgenCompiler;

#[test]
fn net_accept_parks_and_times_out() {
    let src = r#"
use stdlib.net.net

fn probe(l: Int) {
    val acc = tcp_accept(l)
    if acc.is_success {
        out "ACCEPTED"
        tcp_close(acc.value)
    } else {
        out fmt"TIMEOUT msg={acc.error_msg}"
    }
    tcp_close(l)
}

fn main() {
    unsafe(justification: "stdlib.net accept test") {
        val bound = tcp_listen(45201)
        if bound.is_success {
            fiber_spawn(probe, bound.value)
            fiber_run_until_done()
            out "NET_DONE"
        }
    }
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "stdlib_net_accept.dtr", None);
    assert!(
        res.success,
        "compilation must succeed: {:?}
{}",
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
    assert_eq!(code, 0, "no hang: the accept deadline fires and drains");
    assert_eq!(
        stdout.trim(),
        "TIMEOUT msg=timeout
NET_DONE"
    );
}
