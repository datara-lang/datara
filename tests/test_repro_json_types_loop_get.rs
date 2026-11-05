//! PLAN_V152 N2 repro #9 (2026-11-05): `use stdlib.json.types` + List.get
//! inside a while-loop returns garbage (a pointer, not the byte).
//!
//! Bisect matrix (all `forgen run`, Cranelift native; AOT exe crashes the
//! same way; JIT host identical):
//!   import only                      -> OK        (M39)
//!   single get, no loop              -> OK (97)   (M38)
//!   loop + len, no get               -> OK        (M37)
//!   loop + get, no json.types import -> OK        (M22/M34-style)
//!   loop + get + json.types          -> GARBAGE   (M40: acc=0x20C… instead
//!                                                  of 2847; M42 with a
//!                                                  CONSTANT index also
//!                                                  garbage => the loop
//!                                                  phi of the LIST value
//!                                                  itself is corrupted,
//!                                                  not the index)
//!   + if-branch around the get       -> SEGFAULT in strcmp(98, -1)  (M35)
//!   unrelated module imports (set)   -> OK        (M36)
//!
//! Mechanism notes: method dispatch is NOT the culprit — trace shows
//! is_map=false / is_list=true, rt_list_get_id is chosen; the value that
//! reaches the loop body via the block-param/phi machinery is already
//! wrong. The presence of stdlib.json.types in the module graph shifts
//! something in the loop header's parameter construction. gdb: the
//! eventual crash is strcmp(0x62, -1) — raw Int values used as pointers
//! downstream of the corrupted value.
//!
//! FIXED 2026-11-05: the root cause was lowering/mod.rs registering every
//! class method ALSO under its bare name in the global
//! function_return_types table; JsonValue.get(key: Str) -> Str then
//! typed `list.get(i)` as Str for ANY receiver (the bare-name fallback
//! in expr_call.rs fired before the protocol heuristic). The value was
//! treated as a string pointer inside loops -> garbage accumulation and
//! strcmp segfaults. Fix: bare-name method registrations removed
//! (class-scoped keys remain); the expr_call fallback now applies only
//! when the receiver's class is unknown.

use forgen::driver::ForgenCompiler;

#[test]
fn repro_json_types_loop_get_garbage() {
    let src = r#"
use stdlib.json.types

fn main() {
    val b = str_bytes("abcdefghijklmnopqrstuvwxyz")
    mut i = 0
    mut acc = 0
    while i < 26 {
        acc = acc + b.get(i)
        i = i + 1
    }
    out int_to_str(acc)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "repro_json_types_loop_get.dtr", None);
    assert!(res.success, "compile: {:?}", res.diagnostics);
    let exe = res.exe_path.expect("exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("run");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "must not crash");
    // 26 letters 'a'..'z' -> sum 2847
    assert_eq!(stdout.trim(), "2847");
}

/// The original symptom shape: a behavior method (JsonParser.get_int)
/// called from inside a byte-scan loop over a large document — crashed
/// before the fix (repro #7 in PLAN_V152 N2), now bit-exact.
#[test]
fn repro_method_call_in_scan_loop() {
    let src = r#"
use stdlib.json.parser

fn main() {
    val jp = JsonParser { source: "" }
    mut k = 0
    mut acc = 0
    while k < 100 {
        acc = acc + jp.get_int("{\"id\": 5}", "id")
        k = k + 1
    }
    out int_to_str(acc)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(src, "repro_method_scan_loop.dtr", None);
    assert!(res.success, "compile: {:?}", res.diagnostics);
    let exe = res.exe_path.expect("exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("run");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "must not crash");
    assert_eq!(stdout.trim(), "500");
}
