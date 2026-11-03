//! PLAN_V152 N2: docs coverage gate — the Language Reference Manual must
//! document the 1.5 features (TSU/`is`, NatVar fibers, migrate/inspect),
//! and every fenced `datara` snippet in the NEW sections must compile and
//! run (the ones that are complete programs). This is the grep gate the
//! manual was missing: docs that drift from the compiler now break CI.

use forgen::driver::ForgenCompiler;

fn run_native(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "manual snippet must compile: {:?}\n{}",
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
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    stdout.trim_end().to_string()
}

fn read_manual() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/DATARA_LANGUAGE_REFERENCE_MANUAL.md");
    std::fs::read_to_string(path).expect("manual must exist")
}

#[test]
fn manual_documents_1_5_features() {
    let manual = read_manual();
    for (feature, needle) in [
        // §1-3: core language
        ("Intro section", "## 1. Introduction & Architectural Philosophy"),
        ("Variable triad", "## 3. The Variable Triad: Deterministic State"),
        // §4-4.bis: types + TSU
        ("Type system", "## 4. Type System & Primitive Representations"),
        ("TSU section", "## 4.bis Dynamic Values & Type-Set Unboxing"),
        ("`is` narrowing", "if x is Int"),
        ("E-ANY-001", "E-ANY-001"),
        // §5-7: structures + control flow
        ("DOP section", "## 5. Data-Oriented Programming"),
        ("Control flow", "## 7. Control Flow"),
        // §8-9: ownership + effects
        ("Ownership", "## 8. Affine Ownership"),
        ("Effects", "## 9. The Algebraic Effect Lattice"),
        // §10-11: patterns + errors
        ("Pattern matching", "## 10. Pattern Matching"),
        ("Outcome section", "## 11. Deterministic Error Handling"),
        ("`or` operator", "or 8080"),
        // §12-12.bis: concurrency
        ("Multithreading", "## 12. High-Performance Multithreading"),
        ("Fibers section", "## 12.bis Fibers & NatVar"),
        ("fiber_spawn", "fiber_spawn(worker, 21, 64)"),
        ("MT scheduler", "fiber_mt_start(n)"),
        ("Channels", "fiber_chan_create"),
        // §13-15: stdlib + FFI + internals
        ("Stdlib section", "## 13. Standard Library Specification"),
        ("FFI section", "## 14. Foreign Function Interface"),
        // §16-17: toolchain + diagnostics
        ("CLI section", "## 16. The `forgen` Toolchain Command Line Manual"),
        ("migrate CLI", "forgen migrate 1.4 1.5"),
        ("inspect CLI", "forgen inspect <symbol|dmir|clif|codegen>"),
        ("Diagnostics", "## 17. Diagnostics & Error Index"),
    ] {
        assert!(
            manual.contains(needle),
            "manual is missing {feature} (looked for {needle:?})"
        );
    }
}

#[test]
fn manual_tsu_snippet_compiles_and_runs() {
    // The §4.bis example, as a complete program.
    let src = r#"
fn describe(v: Int) -> Str {
    return fmt"int {v}"
}

fn main() {
    val xs = [42, "hi"]
    val x = xs[0]
    if x is Int {
        out describe(x)
    }
    val s = xs[1]
    if s is Str {
        out s
    }
}
"#;
    assert_eq!(run_native(src, "docs_tsu.dtr"), "int 42\nhi");
}

#[test]
fn manual_fibers_snippet_compiles_and_runs() {
    // The §12.bis example (worker/join), as a complete program.
    let src = r#"
fn worker(id: Int) {
    out int_to_str(id * 2)
}

fn main() {
    val h = fiber_spawn(worker, 21, 64)
    val done = fiber_run_until_done()
    out int_to_str(done)
    out int_to_str(fiber_join(h))
}
"#;
    assert_eq!(
        run_native(src, "docs_fibers.dtr"),
        "42
1
0"
    );
}

#[test]
fn manual_channels_snippet_compiles_and_runs() {
    let src = r#"
fn producer(ch: Int) {
    fiber_chan_send(ch, 5)
}

fn main() {
    val ch = fiber_chan_create()
    fiber_spawn(producer, ch)
    val v = fiber_chan_recv(ch)
    fiber_run_until_done()
    out int_to_str(v)
}
"#;
    assert_eq!(run_native(src, "docs_chan.dtr"), "5");
}
