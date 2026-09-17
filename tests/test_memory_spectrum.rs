use std::process::Command;

#[test]
fn test_4_tier_memory_spectrum() {
    let forgen_exe = env!("CARGO_BIN_EXE_forgen");

    let temp_dir = std::env::temp_dir().join("forgen_test_memory_spectrum");
    let _ = std::fs::create_dir_all(&temp_dir);
    let test_file = temp_dir.join("memory_spectrum.dtr");

    let source = r#"
// Tier 1: Struct with high-level value semantics
struct Vec3 {
    x: Float
    y: Float
    z: Float
}

fn tier1_affine() -> Float {
    let v = Vec3 { x: 1.5, y: 2.5, z: 3.5 }
    return v.x + v.y + v.z
}

// Tier 2: Arena allocator slab
fn tier2_arena() -> Int {
    let p = arena_alloc(64)
    let used_before = arena_used()
    arena_reset(0)
    let used_after = arena_used()
    return used_before - used_after
}

// Tier 3: Raw pointer allocation and byte offsets
fn tier3_raw_ptr() -> Int {
    let p = mem_alloc(128)
    ptr_write_i64(p, 0, 42)
    ptr_write_i64(p, 8, 100)
    ptr_write_f64(p, 16, 3.14159)
    ptr_write_u8(p, 24, 255)

    let val1 = ptr_read_i64(p, 0)
    let val2 = ptr_read_i64(p, 8)
    let val3 = ptr_read_u8(p, 24)

    mem_free(p)
    return val1 + val2 + val3
}

// Tier 4: Hardware CPU instructions and fences
fn tier4_hardware() -> Int {
    let buf = mem_alloc(64)
    cpu_prefetch(buf)
    ptr_write_i64(buf, 0, 999)
    cpu_fence()
    let res = ptr_read_i64(buf, 0)
    mem_free(buf)
    return res
}

fn main() {
    let t1 = tier1_affine()
    println(fmt"Tier 1 Affine: {t1}")

    let t2 = tier2_arena()
    println(fmt"Tier 2 Arena Diff: {t2}")

    let t3 = tier3_raw_ptr()
    println(fmt"Tier 3 Raw Ptr: {t3}")

    let t4 = tier4_hardware()
    println(fmt"Tier 4 Hardware: {t4}")
}
"#;

    std::fs::write(&test_file, source).expect("Failed to write test file");

    // Check with forgen check
    let status = Command::new(forgen_exe)
        .arg("check")
        .arg(&test_file)
        .status()
        .expect("Failed to run forgen check");
    assert!(
        status.success(),
        "forgen check failed for 4-tier memory spectrum"
    );

    // Run with Cranelift JIT
    let output_jit = Command::new(forgen_exe)
        .arg("run")
        .arg("--jit")
        .arg(&test_file)
        .output()
        .expect("Failed to run forgen run --jit");
    let stdout_jit = String::from_utf8_lossy(&output_jit.stdout);
    let stderr_jit = String::from_utf8_lossy(&output_jit.stderr);
    assert!(
        stdout_jit.contains("Tier 1 Affine: 7.5"),
        "JIT output missing Tier 1: {}\nSTDERR: {}",
        stdout_jit,
        stderr_jit
    );
    assert!(
        stdout_jit.contains("Tier 2 Arena Diff: 64"),
        "JIT output missing Tier 2: {}",
        stdout_jit
    );
    assert!(
        stdout_jit.contains("Tier 3 Raw Ptr: 397"),
        "JIT output missing Tier 3: {}",
        stdout_jit
    );
    assert!(
        stdout_jit.contains("Tier 4 Hardware: 999"),
        "JIT output missing Tier 4: {}",
        stdout_jit
    );

    // Run with LLVM backend if clang is present
    if forgen::codegen::linker::find_clang().is_some() {
        let output_llvm = Command::new(forgen_exe)
            .arg("run")
            .arg("--llvm")
            .arg(&test_file)
            .output()
            .expect("Failed to run forgen run --llvm");
        let stdout_llvm = String::from_utf8_lossy(&output_llvm.stdout);
        assert!(
            stdout_llvm.contains("Tier 1 Affine: 7.5"),
            "LLVM output missing Tier 1: {}",
            stdout_llvm
        );
        assert!(
            stdout_llvm.contains("Tier 2 Arena Diff: 64"),
            "LLVM output missing Tier 2: {}",
            stdout_llvm
        );
        assert!(
            stdout_llvm.contains("Tier 3 Raw Ptr: 397"),
            "LLVM output missing Tier 3: {}",
            stdout_llvm
        );
        assert!(
            stdout_llvm.contains("Tier 4 Hardware: 999"),
            "LLVM output missing Tier 4: {}",
            stdout_llvm
        );
    }
}
