//! Interprocedural ownership solver tests.
//!
//! Every compilation runs inside a dedicated 64 MiB-stack thread so deep
//! compiler recursion (parser, type checker, ownership solver) cannot
//! overflow the ~1 MiB default stack of a spawned test thread on Windows.
//! Root-cause analysis: commit bcfac07.

use forgen::driver::{CompilationResult, ForgenCompiler};

fn compile(mode: &'static str, source: String, file: &'static str) -> CompilationResult {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || ForgenCompiler::new(mode).compile_source(&source, file, None))
        .expect("failed to spawn compiler thread")
        .join()
        .expect("compiler thread panicked")
}

#[test]
fn test_ownership_interprocedural_view_parameter_preserved() {
    let source = r#"
class Document {
    title: String
    content: String
}

fn inspect_doc(doc: Document) {
    out doc.title
}

fn main() {
    let doc = Document { title: "Architecture", content: "Native Compiler" }
    // Viewing doc across function call preserves doc validity in main
    let v = view(doc)
    inspect_doc(v)

    // doc is still valid here
    out doc.title
}
"#;

    let res = compile("debug", source.to_string(), "interproc_view.dtr");
    assert!(
        res.success,
        "Interprocedural view must preserve caller ownership: {:?}\n{}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_ownership_interprocedural_transfer_return_ownership() {
    let source = r#"
class Buffer {
    capacity: Int
}

fn allocate_buffer(size: Int) -> Buffer {
    let buf = Buffer { capacity: size }
    return buf
}

fn main() {
    // Caller receives full ownership of returned buffer
    let my_buf = allocate_buffer(1024)
    out fmt"Allocated: {my_buf.capacity}"
}
"#;

    let res = compile("debug", source.to_string(), "interproc_return.dtr");
    assert!(
        res.success,
        "Transferring ownership via return value must succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_ownership_interprocedural_negative_use_after_move() {
    let source = r#"
class Connection {
    host: String
}

fn close_and_destroy(conn: Connection) {
    destroy(conn)
}

fn main() {
    let conn = Connection { host: "127.0.0.1" }
    close_and_destroy(conn)

    // Error: cannot view or use conn after it was moved/destroyed
    let dangling = view(conn)
    out dangling.host
}
"#;

    let res = compile("debug", source.to_string(), "interproc_use_after_move.dtr");
    assert!(
        !res.success,
        "Using value after move across function call must fail compilation"
    );
}

#[test]
fn test_ownership_interprocedural_multiple_functions_pure_pipeline() {
    let source = r#"
class Payload {
    value: Int
}

fn step1(p: Payload) -> Int {
    return p.value * 2
}

fn step2(val: Int) -> Int {
    return val + 10
}

fn main() {
    let p = Payload { value: 25 }
    let v = view(p)
    let intermediate = step1(v)
    let final_res = step2(intermediate)
    out fmt"Final: {final_res}"
}
"#;

    let res = compile("release", source.to_string(), "interproc_pipeline.dtr");
    assert!(
        res.success,
        "Multi-step pipeline with borrowed payloads must succeed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let compiler = ForgenCompiler::new("release");
    let (stdout, _stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.expect("must produce an executable"), &[])
        .unwrap();
    assert_eq!(code, 0);
    assert!(stdout.contains("Final: 60"));
}
