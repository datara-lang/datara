//! Tests for Datara v1.2.5 3D Linear Algebra, Geometry, and GPU Compute Pipeline

use forgen::driver::ForgenCompiler;
use std::fs;

fn run_datara(code: &str, tag: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(code, tag, None);
    assert!(
        res.success,
        "Compilation failed for {}: error={:?} diag=\n{}",
        tag, res.error, res.diagnostics
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "{} exited with {}", tag, code);

    let _ = fs::remove_file(&exe);
    let _ = fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

#[test]
fn test_v125_math3d_vector_algebra() {
    let code = r#"
use math.math3d

fn main() {
    let a = Vec3 { x: 1.0, y: 0.0, z: 0.0 }
    let b = Vec3 { x: 0.0, y: 1.0, z: 0.0 }
    let c = a.cross(b)
    println(float_to_str(c.z))
}
"#;
    let out = run_datara(code, "test_v125_math3d_vector_algebra");
    assert!(out.starts_with("1"));
}

#[test]
fn test_v125_math3d_matrix_transform() {
    let code = r#"
use math.math3d

fn main() {
    let mat = mat4_translate(10.0, 20.0, 30.0)
    let p = Vec3 { x: 1.0, y: 2.0, z: 3.0 }
    let res = mat.transform_point(p)
    println(float_to_str(res.x))
    println(float_to_str(res.y))
    println(float_to_str(res.z))
}
"#;
    let out = run_datara(code, "test_v125_math3d_matrix_transform");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("11"));
    assert!(lines[1].starts_with("22"));
    assert!(lines[2].starts_with("33"));
}

#[test]
fn test_v125_gpu_buffer_and_pipeline() {
    let code = r#"
use gpu.compute

fn main() {
    // 100 floats (4 bytes each) = 400 bytes, rounded up to 448 (multiple of 64)
    let buf = gpu_buffer_create(100, 4)
    println(int_to_str(buf.alignment))
    println(int_to_str(buf.size_bytes))

    let pipe = gpu_pipeline_new("main", 64, 1, 1)
    let groups = pipe.dispatch_1d(1000)
    // ceil(1000 / 64) = 16 groups
    println(int_to_str(groups))
}
"#;
    let out = run_datara(code, "test_v125_gpu_buffer_and_pipeline");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, vec!["64", "448", "16"]);
}

#[test]
fn test_v125_gpu_shader_builder() {
    let code = r#"
use gpu.compute

fn main() {
    let sb = shader_builder_new()
    let wgsl = sb.generate_vector_add_wgsl(0, 1, 2)
    println(int_to_str(str_len(wgsl)))
}
"#;
    let out = run_datara(code, "test_v125_gpu_shader_builder");
    assert!(out.parse::<i64>().unwrap() > 50);
}
