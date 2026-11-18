//! PLAN_V155 A2b: stdlib.http.client contract.
//!
//! The module's parse_response + request-building machinery is pinned here
//! without real sockets (the fiber echo path is the E3 item). Compile-time
//! contract: the Result-based API (status/body/headers_raw fields, no
//! sentinel strings) type-checks and drives the pure parser end-to-end.
//!
//! NOTE: builtin arity is strict — tcp_recv takes (sock, max_bytes,
//! timeout_ms); a wrong-arity builtin call currently aborts codegen with a
//! Cranelift ABI assert instead of a resolver error (tracked separately).

use forgen::driver::ForgenCompiler;

#[test]
fn http_response_parsing() {
    let src = r#"
use stdlib.http.client

fn main() {
    val pr = parse_response("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}")
    if pr.is_success {
        out fmt"s={pr.value.status} b={pr.value.body}"
        out fmt"h={pr.value.headers_raw}"
    } else {
        out fmt"err={pr.error_msg}"
    }
    val bad = parse_response("garbage")
    if bad.is_success {
        out "BAD-PARSED"
    } else {
        out fmt"reject={bad.error_msg}"
    }
    val no_status = parse_response("HTTP/1.1 zz\r\n\r\nx")
    if no_status.is_success {
        out "BAD-STATUS-PARSED"
    } else {
        out fmt"reject2={no_status.error_msg}"
    }
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "http_parse_test.dtr", None);
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
    assert_eq!(code, 0);
    assert_eq!(
        stdout.trim(),
        "s=200 b={\"ok\":true}\nh=Content-Type: application/json\nreject=http: malformed response (no header terminator)\nreject2=http: bad status code"
    );
}
