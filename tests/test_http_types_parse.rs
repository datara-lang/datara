//! PLAN_V155 D-stdlib: HttpRequest.parse contract — the rewritten parser
//! extracts method / path / query / body instead of the old shape where
//! `body` carried the whole raw request and query was always empty.

use forgen::driver::ForgenCompiler;

#[test]
fn http_request_parse_fields() {
    let src = r#"
use stdlib.http.types

fn main() {
    let raw = "POST /api/items?limit=5 HTTP/1.1\r\nHost: x\r\n\r\n{\"name\":\"alpha\"}"
    let r = HttpRequest.parse(raw)
    out fmt"method={r.method}"
    out fmt"path={r.path}"
    out fmt"query={r.query}"
    out fmt"body={r.body}"
    let g = HttpRequest.parse("GET /health HTTP/1.1\r\n\r\n")
    out fmt"g={g.method} {g.path} body=[{g.body}] q=[{g.query}]"
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "http_types_test.dtr", None);
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
        "method=POST\npath=/api/items\nquery=limit=5\nbody={\"name\":\"alpha\"}\ng=GET /health body=[] q=[]"
    );
}
