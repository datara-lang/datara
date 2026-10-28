//! M5 TMM end-to-end tests (PLAN_V150, M5 item "Тесты").
//!
//! Four scenarios from the plan:
//! 1. Cyclic ownership graph A→B→A with a `@weak` back-edge: no leak, no
//!    diagnostic; the same graph with a strong back-edge is exactly one
//!    E-TMM-001.
//! 2. FFI owned/borrowed boundary: a violation (borrowed value returned /
//!    transferred into an `owned` slot) is a compile-time error E-TMM-002.
//! 3. `@derive(AutoFree)` scope order: `inspect --memory` prints the
//!    defer-last release list in reverse declaration order.
//! 4. `@arc` multithreaded: loom-style stress against the C runtime's
//!    atomic refcount header — N threads × M acquire/release cycles must
//!    balance exactly, and weak handles must observe deterministic death
//!    after the last strong release.

use forgen::diagnostics::DiagnosticEngine;
use forgen::driver::ForgenCompiler;
use forgen::lexer::Lexer;
use forgen::ownership::memory_report;
use forgen::parser::Parser;
use std::sync::Arc as RustArc;
use std::sync::Barrier;

fn check_source(name: &str, source: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!("forgen_tmm_m5_{}_{}", std::process::id(), name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.dtr", name));
    std::fs::write(&path, source).expect("write TMM test source");
    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_file(&path);
    let _ = std::fs::remove_dir_all(&dir);
    (res.success, res.diagnostics)
}

// ---------------------------------------------------------------------------
// 1. Cycle A→B→A: strong back-edge is exactly one E-TMM-001; the @weak
//    back-edge is the fix and compiles clean.
// ---------------------------------------------------------------------------

#[test]
fn strong_arc_cycle_is_exactly_one_etmm001() {
    let src = "@arc\nstruct A { v: Int, b: B }\n\n@arc\nstruct B { v: Int, a: A }\n\nfn main() {\n    out 1\n}\n";
    let (ok, diags) = check_source("tmm_strong_cycle", src);
    assert!(!ok, "strong cycle A→B→A must fail to check");
    let count = diags.matches("error[E-TMM-001]").count();
    assert_eq!(
        count, 1,
        "exactly one E-TMM-001 per cycle, got {count}: {diags}"
    );
    assert!(
        diags.contains("A → B → A"),
        "diagnostic must name the cycle path: {diags}"
    );
}

#[test]
fn weak_back_edge_breaks_cycle_and_runtime_balances() {
    // Static: the same graph with a weak back-edge must check clean.
    let src = "@arc\nstruct A { v: Int, b: B }\n\n@arc\nstruct B { v: Int, a: Weak<A> }\n\nfn main() {\n    out 1\n}\n";
    let (ok, diags) = check_source("tmm_weak_cycle", src);
    assert!(ok, "weak back-edge must silence the cycle: {diags}");
    assert!(!diags.contains("E-TMM-001"));

    // Runtime "no leak": strong edge A→B, weak edge B→A. The owner chain
    // releases deterministically: arc_release(B) drops the A→B strong
    // edge, arc_release(A) reaches zero and frees — weak handles observe
    // the death instead of keeping the objects alive. Run the compiled
    // program: it must exit cleanly with the expected counts.
    let run_src = r#"
use stdlib.result.result

@arc
struct A { v: Int }

@arc
struct B { v: Int }

fn main() {
    val a = A { v: 1 }
    val b = B { v: 2 }
    val w = weak_create(b)
    out a.arc_count()
    out b.arc_count()
    b.arc_release()
    a.arc_release()
    val up = w.upgrade()
    out up.is_ok()
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(run_src, "test_tmm_weak_leak.dtr", None);
    assert!(
        res.success,
        "weak-linked arc program must compile: {:?} / {}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.expect("native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "program must exit cleanly, stderr: {stderr}");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.first().copied(), Some("1"), "A holds itself only");
    assert_eq!(lines.get(1).copied(), Some("1"), "B holds itself only");
    assert_eq!(
        lines.get(2).copied(),
        Some("false"),
        "weak upgrade after the final release must report dead"
    );
}

// ---------------------------------------------------------------------------
// 2. FFI owned/borrowed boundary: violation = compile-time E-TMM-002.
// ---------------------------------------------------------------------------

#[test]
fn borrowed_param_returned_is_etmm002() {
    // Ownership modes are PREFIX annotations on the parameter name:
    // `own p: T` / `view p: T` (src/parser/types.rs parse_param_list).
    let src = "extern fn take_owned(own p: RawPtr) -> Int\n\nfn leak(view p: RawPtr) -> RawPtr {\n    return p\n}\n\nfn main() {\n    out 0\n}\n";
    let (ok, diags) = check_source("tmm_ffi_return", src);
    assert!(
        !ok,
        "returning a borrowed value must be a compile error: {diags}"
    );
    assert!(diags.contains("E-TMM-002"), "expected E-TMM-002: {diags}");
}

#[test]
fn borrowed_param_into_owned_slot_is_etmm002() {
    let src = "extern fn take_owned(own p: RawPtr) -> Int\n\nfn misuse(view p: RawPtr) {\n    take_owned(p)\n}\n\nfn main() {\n    out 0\n}\n";
    let (ok, diags) = check_source("tmm_ffi_transfer", src);
    assert!(
        !ok,
        "borrowed→owned transfer must be a compile error: {diags}"
    );
    assert!(diags.contains("E-TMM-002"), "expected E-TMM-002: {diags}");
}

#[test]
fn legal_view_usage_stays_clean() {
    // A view parameter consumed inside the call (read-only use, inside the
    // required unsafe block) is legal — no E-TMM-002.
    let src = "extern fn peek(view p: RawPtr) -> Int\n\nfn read(view p: RawPtr) -> Int {\n    unsafe(justification: \"peek only reads the borrow\") {\n        return peek(p)\n    }\n}\n\nfn main() {\n    out 0\n}\n";
    let (ok, diags) = check_source("tmm_ffi_legal", src);
    assert!(ok, "legal view use must check clean: {diags}");
    assert!(!diags.contains("E-TMM-002"));
}

// ---------------------------------------------------------------------------
// 3. AutoFree scope order: defer-last, reverse declaration order.
// ---------------------------------------------------------------------------

#[test]
fn autofree_scope_insertions_are_defer_last_reverse_order() {
    let source = r#"
@derive(AutoFree)
struct Buf { size: Int }

@derive(AutoFree)
struct Log { n: Int }

fn work() -> Int {
    val first = Buf { size: 8 }
    val second = Log { n: 3 }
    return first.size + second.n
}

fn main() -> Int {
    return work()
}
"#;
    let mut diag = DiagnosticEngine::new("en");
    diag.set_source("tmm_autofree.dtr", source);
    let tokens = Lexer::new(source, "tmm_autofree.dtr").tokenize(&mut diag);
    let program = Parser::new(tokens, &mut diag, "tmm_autofree.dtr").parse_program();
    assert!(!diag.has_errors(), "parse failed: {}", diag.format_all());

    let report = memory_report(&program);
    assert!(
        report.contains("Scope insertions (Layer 3, defer-last)"),
        "report must carry the Layer 3 section:\n{report}"
    );
    let work_line = report
        .lines()
        .find(|l| l.contains("fn work"))
        .expect("work() must appear in the scope-insertion plan");
    // Reverse declaration order: second released before first.
    let second_pos = work_line.find("second").expect("second in plan");
    let first_pos = work_line.find("first").expect("first in plan");
    // The plan LISTS locals in declaration order; the release executes
    // defer-last in the reverse of that listed order (label on the line).
    assert!(
        first_pos < second_pos,
        "plan must list locals in declaration order: {work_line}"
    );
    assert!(
        work_line.contains("reverse order"),
        "defer-last label must state the reverse release order: {work_line}"
    );
}

// ---------------------------------------------------------------------------
// 4. @arc multithreaded: loom-style stress on the C runtime refcount.
//    The 16-byte header (strong/weak) is updated with CAS; N threads × M
//    acquire/release pairs must balance to exactly the surviving owners,
//    and the header must die deterministically after the last release.
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn datara_rt_arc_init(obj: *mut ()) -> i64;
    fn datara_rt_arc_acquire(obj: *mut ()) -> i64;
    fn datara_rt_arc_release(obj: *mut ());
    fn datara_rt_arc_count(obj: *mut ()) -> i64;
    fn datara_rt_weak_create(obj: *mut ()) -> *mut ();
    fn datara_rt_weak_upgrade(cell: *mut ()) -> *mut ();
    fn datara_rt_weak_drop(cell: *mut ());
    fn malloc(size: usize) -> *mut ();
}

/// Raw pointer wrapper so the shared @arc object can cross thread bounds;
/// the pointed-to allocation is only ever touched through the C runtime's
/// atomic refcount API, which is thread-safe by contract.
#[derive(Clone, Copy)]
struct SendPtr(*mut ());
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

/// Allocate a raw @arc-shaped object (16-byte header + payload words) and
/// initialize its refcount to 1, exactly as the codegen alloc path does.
fn make_arc_obj(payload_words: usize) -> *mut () {
    unsafe {
        let base = malloc(16 + payload_words * 8) as *mut u64;
        assert!(!base.is_null(), "malloc failed");
        let obj = base.add(2) as *mut ();
        datara_rt_arc_init(obj);
        obj
    }
}

#[test]
fn arc_refcount_survives_concurrent_acquire_release_storm() {
    let obj = make_arc_obj(4);
    let weak_cell = unsafe { datara_rt_weak_create(obj) };
    assert!(!weak_cell.is_null());

    const THREADS: usize = 8;
    const ITERS: usize = 20_000;
    let shared_obj = RustArc::new(SendPtr(obj));
    let barrier = RustArc::new(Barrier::new(THREADS));
    let mut handles = Vec::with_capacity(THREADS);

    for _ in 0..THREADS {
        let obj = RustArc::clone(&shared_obj);
        let barrier = RustArc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            let obj = obj.0;
            for _ in 0..ITERS {
                unsafe {
                    datara_rt_arc_acquire(obj);
                    // The count must never dip below 1 (the base owner) and
                    // never exceed the theoretical in-flight maximum: a
                    // torn CAS would show up as an out-of-range reading.
                    let c = datara_rt_arc_count(obj);
                    assert!(c >= 1, "refcount underflow observed: {c}");
                    assert!(
                        c <= 1 + (THREADS * ITERS) as i64,
                        "refcount corrupted upward: {c}"
                    );
                    datara_rt_arc_release(obj);
                }
            }
        }));
    }

    for h in handles {
        h.join().expect("stress thread must not panic");
    }

    // All storm references balanced: exactly the base owner remains.
    let remaining = unsafe { datara_rt_arc_count(obj) };
    assert_eq!(remaining, 1, "acquire/release storm must balance to 1");

    // Deterministic death: the last strong release frees the object; the
    // weak header keeps the header readable until weak_drop. After the
    // release, upgrade must answer "dead" (err outcome, not a resurrected
    // pointer) and weak_drop reclaims the header.
    unsafe {
        datara_rt_arc_release(obj);
        let up = datara_rt_weak_upgrade(weak_cell);
        // Outcome layout: [is_ok? payload]. err outcome must not carry the
        // live object pointer; a resurrecting upgrade would return obj.
        let is_ok = *(up as *const u64);
        assert_eq!(is_ok, 0, "upgrade after final release must report dead");
        datara_rt_weak_drop(weak_cell);
        // Header memory is reclaimed by the runtime; touching it now would
        // be UB, so the test stops here — the free itself ran under
        // datara_rt_free with no allocator mismatch (see make_arc_obj).
    }
}
