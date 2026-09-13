//! Comprehensive Integration Test Suite for Datara & Forgen v1.3.0
//! Universal Polyglot Zero-Latency Engine, DOD Normalization, and Comptime Adaptive Flow-Typing.

use forgen::driver::ForgenCompiler;

fn run_datara(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}",
        name, res.error
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "{} exited with {}", name, code);

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

#[test]
fn test_zig_arithmetic_and_logic() {
    let out = run_datara(
        r#"
fn main() {
    let eval_res = zig_eval_int("1024 * 64")
    let call_res1 = zig_call("zig_kernel_add", 400)
    let call_res2 = zig_call("zig_kernel_mul", 50)
    let total = eval_res + call_res1 + call_res2
    out total
}
"#,
        "test_v130_zig_logic",
    );
    // 65536 + 800 + 100 = 66436
    assert_eq!(out, "66436");
}

#[test]
fn test_csharp_native_aot_multiple_invocations() {
    let out = run_datara(
        r#"
fn main() {
    let res_i64_a = csharp_invoke_i64("MathLib", "SquareAndInc", 12)
    let res_i64_b = csharp_invoke_i64("MathLib", "SquareAndInc", 20)
    out res_i64_a + res_i64_b
}
"#,
        "test_v130_csharp_aot",
    );
    // (12 * 12 + 1) + (20 * 20 + 1) = 145 + 401 = 546
    assert_eq!(out, "546");
}

#[test]
fn test_lua_script_state_and_evaluation() {
    let out = run_datara(
        r#"
fn main() {
    let val1 = lua_eval_int("2^10")
    let val2 = lua_eval_int("100 * 5 + 23")
    let status = lua_exec("local x = 42; return x")
    out val1 + val2 + status
}
"#,
        "test_v130_lua_state",
    );
    // 1024 + 523 + 0 = 1547
    assert_eq!(out, "1547");
}

#[test]
fn test_polyglot_parallel_microsecond_scheduler() {
    let out = run_datara(
        r#"
fn main() {
    let t1 = polyglot_parallel_exec("zig", "500 + 200")
    let t2 = polyglot_parallel_exec("lua", "300 * 3")
    let t3 = polyglot_parallel_exec("csharp", "10")
    out t1 + t2 + t3
}
"#,
        "test_v130_polyglot_parallel",
    );
    // (500+200) + (300*3) + (10*10+1) = 700 + 900 + 101 = 1701
    assert_eq!(out, "1701");
}

#[test]
fn test_comptime_dynamic_flow_typing_multi_transition() {
    let out = run_datara(
        r#"
fn main() {
    mut val register_val = 50
    let step1 = register_val + 25
    register_val = 300
    let step2 = register_val * 2
    out step1 + step2
}
"#,
        "test_v130_flow_typing_multi",
    );
    // (50 + 25) + (300 * 2) = 75 + 600 = 675
    assert_eq!(out, "675");
}

#[test]
fn test_dod_struct_behavior_memory_layout() {
    let out = run_datara(
        r#"
struct Transform3D {
    x: Int
    y: Int
    z: Int
}

behavior Transform3D {
    magnitude_sq() -> Int {
        return this.x * this.x + this.y * this.y + this.z * this.z
    }
}

fn main() {
    let t = Transform3D { x: 3, y: 4, z: 12 }
    out t.magnitude_sq()
}
"#,
        "test_v130_dod_struct",
    );
    // 9 + 16 + 144 = 169
    assert_eq!(out, "169");
}

#[test]
fn test_class_deprecation_warning_diagnostic() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(
        r#"
class LegacyObject {
    val: Int
}

fn main() {
    let obj = LegacyObject { val: 42 }
    out obj.val
}
"#,
        "test_class_deprecated_diag.dtr",
        None,
    );
    assert!(res.success, "Must remain backwards-compatible");
    assert!(
        res.diagnostics.contains("W0100") || res.diagnostics.contains("deprecated"),
        "Compiler must emit W0100 deprecation diagnostic for 'class'"
    );
}

#[test]
fn test_polyglot_combined_pipeline() {
    let out = run_datara(
        r#"
struct PolyglotResult {
    zig_val: Int
    csharp_val: Int
    lua_val: Int
}

behavior PolyglotResult {
    total() -> Int {
        return this.zig_val + this.csharp_val + this.lua_val
    }
}

fn main() {
    let z = zig_eval_int("50 + 50")
    let cs = csharp_invoke_i64("", "Kernel", 5)
    let l = lua_eval_int("20 * 4")
    let res = PolyglotResult { zig_val: z, csharp_val: cs, lua_val: l }
    out res.total()
}
"#,
        "test_v130_polyglot_pipeline",
    );
    // 100 + (5*5+1=26) + 80 = 206
    assert_eq!(out, "206");
}
