//! PLAN_V153 A2: stdlib.net machinery contract (native/AOT tier).
//!
//! VERIFIED (this test): tcp_listen (non-blocking flip), tcp_accept
//! PARKS the fiber and times out cleanly at its deadline, tcp_close
//! works, the scheduler drains.
//!
//! KNOWN OPEN (repro #14 / E3 dedicated pass): the FULL echo (serve
//! accepts the pending connection and echoes) does not complete — the
//! serve fiber's accept wake on listener-readable is the remaining
//! fiber-binding work, same family as the documented JIT-host
//! limitation (test_natvar header). The deadline machinery itself is
//! proven by the timeout shape below.

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
