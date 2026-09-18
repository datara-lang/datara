//! Integration tests for Datara v1.3.0 Universal Polyglot Zero-Latency Engine
//! and Comptime Adaptive SSA Flow-Typing.

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
fn test_comptime_flow_typing_unboxed_dynamics() {
    let out = run_datara(
        r#"
fn main() {
    mut val x = 42
    let y = x + 18
    x = 100
    let z = x + y
    out z
}
"#,
        "test_comptime_flow_typing",
    );
    assert_eq!(out, "160");
}

#[test]
fn test_dod_struct_and_behavior() {
    let out = run_datara(
        r#"
struct Vector2D {
    x: Int
    y: Int
}

behavior Vector2D {
    sum() -> Int {
        return this.x + this.y
    }
}

fn main() {
    let vec = Vector2D { x: 15, y: 25 }
    out vec.sum()
}
"#,
        "test_dod_struct_behavior",
    );
    assert_eq!(out, "40");
}

#[test]
fn test_class_rejection_error_emitted() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(
        r#"
class LegacyEntity {
    id: Int
}

fn main() {
    let e = LegacyEntity { id: 7 }
    out e.id
}
"#,
        "test_class_deprecated.dtr",
        None,
    );
    assert!(
        !res.success,
        "Class keyword must be rejected with E0100 in Datara: {:?}",
        res.error
    );
    assert!(
        res.diagnostics.contains("E0100") || res.diagnostics.contains("Use 'struct'"),
        "Compiler must emit E0100 error for 'class': {}",
        res.diagnostics
    );
}

#[test]
fn test_zig_zero_latency_interop() {
    let out = run_datara(
        r#"
fn main() {
    let zig_res = zig_eval_int("100 + 42")
    let call_res = zig_call("test_sym", 25)
    out zig_res + call_res
}
"#,
        "test_zig_interop",
    );
    // 142 + 50 = 192
    assert_eq!(out, "192");
}

#[test]
fn test_csharp_native_aot_interop() {
    let out = run_datara(
        r#"
fn main() {
    let res_i64 = csharp_invoke_i64("", "ComputeKernel", 10)
    out res_i64
}
"#,
        "test_csharp_interop",
    );
    // 10 * 10 + 1 = 101
    assert_eq!(out, "101");
}

#[test]
fn test_lua_in_process_zero_delay() {
    let out = run_datara(
        r#"
fn main() {
    let i_val = lua_eval_int("50 * 4")
    let status = lua_exec("print('lua ok')")
    out i_val + status
}
"#,
        "test_lua_interop",
    );
    // 200 + 0 = 200
    assert_eq!(out, "200");
}

#[test]
fn test_polyglot_parallel_microsecond_runner() {
    let out = run_datara(
        r#"
fn main() {
    let zig_task = polyglot_parallel_exec("zig", "300 + 120")
    let lua_task = polyglot_parallel_exec("lua", "80 - 20")
    out zig_task + lua_task
}
"#,
        "test_polyglot_parallel",
    );
    // 420 + 60 = 480
    assert_eq!(out, "480");
}
