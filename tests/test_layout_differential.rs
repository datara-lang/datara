//! Differential correctness test for the v1.3.2 layout-sensitivity gate
//! (E-OPT-001): the adaptive layout optimizer must not change the observable
//! behavior of a program, whether or not it reorders fields.
//!
//! Case 1 (uniform 8-byte fields): reordering is provably safe (offsets are
//!   idx*8 regardless of order), so the transform applies and the program
//!   must behave identically to the same source compiled with the optimizer
//!   disabled (quick mode preserves verbatim IR).
//! Case 2 (mixed-size fields): reordering cannot be proven equivalent, so
//!   the gate skips the transform, emits an E-OPT-001 warning, and keeps
//!   the source field order while the program still prints the same values
//!   as the unoptimized build.

use forgen::driver::ForgenCompiler;

fn compile_and_run(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(res.success, "compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "program must exit cleanly, stderr: {}", stderr);
    stdout
}

fn compile_and_run_quick(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("quick");
    let res = compiler.compile_source_native(source, name, None);
    assert!(res.success, "compilation must succeed: {:?}", res.error);
    let exe = res.exe_path.expect("must produce a native executable");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    assert_eq!(code, 0, "program must exit cleanly, stderr: {}", stderr);
    stdout
}

const UNIFORM_SOA: &str = r#"
@layout(soa)
struct Particle {
    x: Float
    y: Float
    z: Float
    vx: Float
    vy: Float
    vz: Float
}

fn main() {
    let p = Particle { x: 1.0, y: 2.0, z: 3.0, vx: 0.5, vy: 0.5, vz: 0.5 }
    out p.x
    out p.y
    out p.z
    out p.vx
    out p.vy
    out p.vz
}
"#;

const MIXED_FIELDS: &str = r#"
@layout(soa)
struct Mixed {
    small: Bool
    mid: Int32
    big: Int
    other: Float
}

fn main() {
    let m = Mixed { small: true, mid: 7, big: 42, other: 1.5 }
    out m.small
    out m.mid
    out m.big
    out m.other
}
"#;

#[test]
fn test_layout_soa_uniform_fields_behavior_unchanged() {
    let optimized = compile_and_run(UNIFORM_SOA, "layout_diff_uniform_opt");
    let baseline = compile_and_run_quick(UNIFORM_SOA, "layout_diff_uniform_base");
    assert_eq!(
        optimized, baseline,
        "SoA/uniform-field program must print identical output with and without optimization"
    );
}

#[test]
fn test_layout_mixed_fields_behavior_unchanged_and_warned() {
    let optimized = compile_and_run(MIXED_FIELDS, "layout_diff_mixed_opt");
    let baseline = compile_and_run_quick(MIXED_FIELDS, "layout_diff_mixed_base");
    assert_eq!(
        optimized, baseline,
        "Mixed-size-field program must print identical output with and without optimization"
    );

    // The gate must have emitted the E-OPT-001 advisory.
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(MIXED_FIELDS, "layout_diff_mixed_diag", None);
    assert!(res.success);
    assert!(
        res.diagnostics.contains("E-OPT-001"),
        "mixed-size layout must trigger E-OPT-001 warning, diagnostics: {}",
        res.diagnostics
    );
}
