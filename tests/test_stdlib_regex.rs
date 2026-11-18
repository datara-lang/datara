//! PLAN_V155 A1: stdlib.text.regex contract (native/AOT tier).
//!
//! The module is a pure-Datara backtracking VM over a compiled program.
//! These tests pin the public API surface: literals, classes, quantifiers,
//! anchors, alternation, capture groups, replace with $N refs, split, and
//! compile-error reporting. They also regression-guard two compiler bugs
//! found while building it:
//!  - repro #16: struct-field List<Str> element reads lost their Str
//!    tagging after a struct crossed a fn return (is_expr_list /
//!    list_elem_repr had no MemberAccess arm);
//!  - the parser/VM split desync: single-branch alternation used to emit
//!    optional-body scaffolding (zero-length matches), and the backtrack
//!    stack appended instead of overwriting popped slots.

use forgen::driver::ForgenCompiler;

fn run_expect(src: &str, expected: &str) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "regex_anchors.dtr", None);
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
    assert_eq!(code, 0, "program must exit cleanly");
    assert_eq!(stdout.trim(), expected, "unexpected output");
}

#[test]
fn regex_match_and_anchors() {
    let src = r#"
use text.regex

fn main() {
    val p1 = "h.llo"
    val t1 = "hello"
    val t2 = "hxllo"
    val t3 = "yellow"
    val p2 = "^he"
    val p3 = "lo$"
    val p4 = "^hello$"
    out fmt"a={regex_is_match(p1, t1)}"
    out fmt"b={regex_is_match(p1, t2)}"
    out fmt"c={regex_is_match(p1, t3)}"
    out fmt"d={regex_is_match(p2, t1)}"
    out fmt"e={regex_is_match(p2, t3)}"
    out fmt"f={regex_is_match(p3, t1)}"
    out fmt"g={regex_match_full(p4, t1)}"
    out fmt"h={regex_match_full(p2, t1)}"
}
"#;
    run_expect(
        src,
        "a=true\nb=true\nc=false\nd=true\ne=false\nf=true\ng=true\nh=false",
    );
}

#[test]
fn regex_find_classes_and_quantifiers() {
    let src = r#"
use text.regex

fn main() {
    val p1 = "[0-9]+"
    val t1 = "abc123def456"
    val f = regex_find(p1, t1)
    if f.is_success {
        out fmt"find={f.value}"
    } else {
        out fmt"err={f.error_msg}"
    }
    val all = regex_find_all(p1, t1)
    out fmt"n={all.len()} a0={all.get(0)} a1={all.get(1)}"
    val p2 = "a(b+)c"
    val t2 = "xxabbbcyy"
    val f2 = regex_find(p2, t2)
    if f2.is_success {
        out fmt"find2={f2.value}"
    }
    val gs = regex_find_groups(p2, t2)
    if gs.is_success {
        out fmt"g1={gs.value.get(1)}"
    }
}
"#;
    run_expect(src, "find=123\nn=2 a0=123 a1=456\nfind2=abbbc\ng1=bbb");
}

#[test]
fn regex_alternation_and_replacement() {
    let src = r#"
use text.regex

fn main() {
    val p1 = "cat|dog"
    val t1 = "hotdog"
    out fmt"alt={regex_is_match(p1, t1)}"
    val p2 = "o"
    val t2 = "foo boo"
    val z = "0"
    out fmt"rep={regex_replace(p2, t2, z)}"
    val p3 = "(a+)(b+)"
    val t3 = "xaabby"
    val r = "-$2$1-"
    out fmt"rep2={regex_replace(p3, t3, r)}"
    val p4 = ","
    val t4 = "a,b,c"
    val sp = regex_split(p4, t4)
    out fmt"sp={sp.len()} s1={sp.get(1)}"
    val bad = "a("
    out fmt"err=[{regex_error(bad)}]"
}
"#;
    run_expect(
        src,
        "alt=true\nrep=f00 b00\nrep2=x-bbaa-y\nsp=3 s1=b\nerr=[regex: missing ')']",
    );
}

#[test]
fn regex_escapes_and_word_class() {
    let src = r#"
use text.regex

fn main() {
    val p1 = "(\w+)@(\w+)"
    val t1 = "bob@mail"
    val gs = regex_find_groups(p1, t1)
    if gs.is_success {
        out fmt"g0={gs.value.get(0)}"
        out fmt"g1={gs.value.get(1)}"
        out fmt"g2={gs.value.get(2)}"
    }
    val p2 = "a\.b"
    val t2 = "a.b"
    val t3 = "axb"
    out fmt"esc={regex_is_match(p2, t2)} noesc={regex_is_match(p2, t3)}"
    val p3 = "[^0-9]+"
    val t4 = "12ab34"
    val f = regex_find(p3, t4)
    if f.is_success {
        out fmt"neg={f.value}"
    }
}
"#;
    run_expect(
        src,
        "g0=bob@mail\ng1=bob\ng2=mail\nesc=true noesc=false\nneg=ab",
    );
}

#[test]
fn repro16_struct_return_field_list_read() {
    // repro #16: `r.names.get(0)` on a struct that crossed a fn return
    // used to print the raw char* — the MemberAccess chain had no list
    // element type resolution.
    let src = r#"
struct Holder {
    names: List<Str>
    n: Int
}

fn make() -> Holder {
    mut h = Holder { names: [], n: 0 }
    h.names = h.names.push("alpha")
    h.names = h.names.push("beta")
    h.n = 2
    return h
}

fn main() {
    val r = make()
    out fmt"len={r.names.len()} a={r.names.get(0)} b={r.names.get(1)} n={r.n}"
    mut r2 = make()
    r2.names = r2.names.push("gamma")
    out fmt"ext={r2.names.get(2)} base={r.names.len()}"
}
"#;
    run_expect(src, "len=2 a=alpha b=beta n=2\next=gamma base=2");
}
