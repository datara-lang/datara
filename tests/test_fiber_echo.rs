//! E3 ACCEPTANCE (repro #14 CLOSED): the full fiber echo + HTTP round trip
//! run OUTSIDE gdb on the AOT tier.
//!
//! Root cause of the old 5/5 segfault: datara_rt_outcome_build stored the
//! value/msg pointers VERBATIM — fiber_net_recv/accept passed STACK buffers
//! whose pointers were dead on return (garbage payloads, "socket recv
//! error", wild-pointer crashes). The builder now copies both strings into
//! the TLS scratch ring, and socket.dtr's TcpStream flips connected sockets
//! non-blocking + parks on recv (a blocking recv froze the whole scheduler
//! thread — the second half of the deadlock).

use forgen::driver::ForgenCompiler;

fn run_expect(src: &str, name: &str, expected: &str) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, name, None);
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
    assert_eq!(code, 0, "no crash: the echo flow must complete cleanly");
    assert_eq!(stdout.trim(), expected);
}

#[test]
fn fiber_echo_full_round_trip() {
    let src = r#"
use stdlib.net.net

fn serve() {
    val bound = tcp_listen(45341)
    if bound.is_success {
        val acc = tcp_accept(bound.value)
        if acc.is_success {
            val req = tcp_recv(acc.value, 4096, 5000)
            if req.is_success {
                val reply = "echo:" + req.value
                tcp_send(acc.value, reply)
            }
            tcp_close(acc.value)
        }
        tcp_close(bound.value)
    }
}

fn client_work() {
    fiber_sleep_ms(100)
    val c = tcp_connect("127.0.0.1", 45341)
    if c.is_success {
        tcp_send(c.value, "hello")
        val resp = tcp_recv(c.value, 4096, 5000)
        if resp.is_success {
            out fmt"GOT {resp.value}"
        } else {
            out fmt"RECV-ERR {resp.error_msg}"
        }
        tcp_close(c.value)
    }
}

fn main() {
    unsafe(justification: "echo E3 acceptance test") {
        fiber_spawn(serve, 0)
        fiber_spawn(client_work, 0)
        fiber_run_until_done()
        out "ECHO-DONE"
    }
}
"#;
    run_expect(src, "echo_e3.dtr", "GOT echo:hello\nECHO-DONE");
}

#[test]
fn http_client_server_round_trip() {
    // The full HttpClient -> fiber server flow: connect, send, park on
    // recv, wake on the response, parse status + body, drain on close.
    let src = r#"
use stdlib.http.client
use stdlib.net.net

fn serve() {
    val bound = tcp_listen(45342)
    if bound.is_success {
        val acc = tcp_accept(bound.value)
        if acc.is_success {
            val req = tcp_recv(acc.value, 4096, 5000)
            if req.is_success {
                val body = "{\"ok\":true,\"n\":42}"
                let header = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\n\r\n"
                tcp_send(acc.value, header + body)
            }
            tcp_close(acc.value)
        }
        tcp_close(bound.value)
    }
}

fn client_work() {
    fiber_sleep_ms(100)
    val client = HttpClient.new()
    val resp = client.get("127.0.0.1", 45342, "/api/health")
    if resp.is_success {
        out fmt"status={resp.value.status} body={resp.value.body}"
    } else {
        out fmt"ERR {resp.error_msg}"
    }
}

fn main() {
    unsafe(justification: "http round trip E3 test") {
        fiber_spawn(serve, 0)
        fiber_spawn(client_work, 0)
        fiber_run_until_done()
        out "HTTP-DONE"
    }
}
"#;
    run_expect(
        src,
        "http_roundtrip.dtr",
        "status=200 body={\"ok\":true,\"n\":42}\nHTTP-DONE",
    );
}
