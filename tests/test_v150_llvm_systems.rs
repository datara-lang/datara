//! v1.5.0: LLVM-backend regression for the systems primitives.
//!
//! History: the LLVM IR emitted short symbol names (`slice_alloc`,
//! `SliceView_set_byte`, `@destroy`, ...) while the runtime archive only
//! exports the `datara_sys_*` / `datara_hw_*` spellings. Every
//! systems-primitives build on `--llvm` failed at the link step (LNK2019)
//! and silently fell back to Cranelift. These tests compile through the
//! real LLVM pipeline and assert both the exit code and the stdout.

use forgen::driver::ForgenCompiler;

fn run_datara_llvm(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release").with_llvm(true);
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}\n{}",
        name, res.error, res.diagnostics
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    if code != 0 {
        eprintln!("STDERR: {}", stderr);
    }
    (stdout.trim().replace("\r\n", "\n"), code)
}

#[test]
fn test_llvm_slice_view_roundtrip() {
    let src = r#"
fn main() -> Int {
    val s = slice_alloc(32)
    s.set_byte(0, 65)
    s.write_u16_be(2, 4000)
    s.write_u32_be(4, 987654321)

    val b0 = s.get_byte(0)
    val u16_val = s.read_u16_be(2)
    val u32_val = s.read_u32_be(4)

    if b0 == 65 && u16_val == 4000 && u32_val == 987654321 {
        print("llvm slice rw ok")
        s.free()
        return 0
    }
    s.free()
    return 1
}
"#;
    let (stdout, code) = run_datara_llvm(src, "test_llvm_slice_rw.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "llvm slice rw ok");
}

#[test]
fn test_llvm_destroy_noop() {
    let src = r#"
fn main() -> Int {
    val a = slice_alloc(16)
    a.set_byte(1, 66)
    val b = slice_alloc(16)
    b.set_byte(2, 67)
    destroy(a)
    drop(b)
    print("llvm destroy ok")
    return 0
}
"#;
    let (stdout, code) = run_datara_llvm(src, "test_llvm_destroy.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "llvm destroy ok");
}

#[test]
fn test_llvm_systems_primitives() {
    let src = r#"
fn main() -> Int {
    val s = slice_alloc(64)
    s.write_u32_be(0, 0xCAFEBABE)
    val magic = s.read_u32_be(0)
    val swapped = bswap16(0xAABB)
    val h = hton16(0x1234)
    val sub = s.subslice(0, 8)
    val n = sub.len()
    atomic_fence_seq_cst()
    val z = typed_zero_init(8)
    if magic == 0xCAFEBABE && swapped == 0xBBAA && n == 8 && z != 0 {
        print("llvm systems ok")
        s.free()
        return 0
    }
    s.free()
    return 1
}
"#;
    let (stdout, code) = run_datara_llvm(src, "test_llvm_systems.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "llvm systems ok");
}
