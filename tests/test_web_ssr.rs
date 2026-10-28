//! M12 Web groundwork: stdlib.web.ssr — escaped-by-default SSR helpers.

use forgen::driver::ForgenCompiler;

fn run_native(source: &str, name: &str) -> String {
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
    assert_eq!(code, 0);
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

/// XSS-гейт: интерполяция пользовательских данных экранируется по умолчанию.
#[test]
fn ssr_escapes_by_default() {
    let src = r#"
use stdlib.web.ssr

fn main() {
    val page = render_page("T & T <script>", render_h1("Hello <b>x</b>") + render_ul(["<img>", "ok"]))
    out str_contains(page, "&lt;script&gt;")
    out str_contains(page, "&lt;img&gt;")
    out str_contains(page, "<h1>Hello &lt;b&gt;x&lt;/b&gt;</h1>")
}
"#;
    assert_eq!(run_native(src, "ssr_xss.dtr"), "true\ntrue\ntrue");
}

/// Raw-интерполяция — явный выход: render_raw НЕ экранирует.
#[test]
fn ssr_raw_is_explicit() {
    let src = r#"
use stdlib.web.ssr

fn main() {
    out str_contains(render_raw("<b>bold</b>"), "<b>")
    out str_contains(render_h1("<b>"), "<b>")
}
"#;
    assert_eq!(run_native(src, "ssr_raw.dtr"), "true\nfalse");
}

/// URL-эскейп и response-обёртка.
#[test]
fn ssr_url_escape_and_response() {
    let src = r#"
use stdlib.web.ssr

fn main() {
    out url_escape("a b&c")
    out str_starts_with(render_response("200 OK", "text/html", render_p("hi")), "HTTP/1.1 200 OK")
}
"#;
    assert_eq!(run_native(src, "ssr_url.dtr"), "a%20b%26c\ntrue");
}

/// Регресс: stdlib.text импортер получает Outcome-класс автоматически
/// (text_is_float через is_ok — e2e на кодогене).
#[test]
fn ssr_text_import_pulls_result_class() {
    let src = r#"
use stdlib.web.ssr

fn main() {
    out html_escape("plain")
}
"#;
    assert_eq!(run_native(src, "ssr_textpull.dtr"), "plain");
}
