//! PLAN_V152 N1.1: fiber_net_recv — non-blocking socket recv that parks
//! the calling fiber and is woken by the scheduler's select() slice.
//! A Rust-side std::net::TcpListener serves as the echo peer; the Datara
//! program connects to it and uses fiber_net_recv to read the reply.

use forgen::driver::ForgenCompiler;
use std::io::{Read, Write};

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn test_fiber_net_recv_parks_and_reads() {
    let port = free_port();
    let src = format!(
        r#"
import python

fn main() {{
    unsafe(justification: "io test: local socket to the test peer") {{
        val sock = socket_create(1)
        val rc = socket_connect(sock, "127.0.0.1", {port})
        if rc != 0 {{
            out "CONN_FAIL"
            return
        }}
        socket_send(sock, "ping")
        // fiber_net_recv: would-block parks this fiber; the scheduler's
        // select() slice wakes it when the echo arrives.
        val res = fiber_net_recv(sock, 4096, 5000)
        if res.is_success {{
            val v = res.value
        out fmt"got {{v}}"
        }} else {{
            out "TIMEOUT"
        }}
        socket_close(sock)
    }}
}}
"#
    );
    // Spawn the echo peer first.
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    let server = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let mut buf = [0u8; 64];
        let n = conn.read(&mut buf).unwrap_or(0);
        let _ = conn.write_all(&buf[..n]);
    });

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(&src, "fiber_net_test.dtr", None);
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
    server.join().unwrap();
    assert_eq!(code, 0, "stdout: {stdout}");
    assert_eq!(
        stdout.trim(),
        "got ping",
        "echo via fiber_net_recv: {stdout}"
    );
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
}
