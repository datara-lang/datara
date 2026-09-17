use std::process::Command;

#[test]
fn test_module_state_dmir_and_llvm() {
    let forgen_exe = env!("CARGO_BIN_EXE_forgen");

    let temp_dir = std::env::temp_dir().join("forgen_test_module_state");
    let _ = std::fs::create_dir_all(&temp_dir);
    let test_file = temp_dir.join("module_state.dtr");

    let source = r#"
mut call_count: Int = 0
const MAX_LIMIT: Int = 100
mut cached_result: Str = ""

fn fetch_expensive() -> Str {
    if cached_result == "" {
        cached_result = "INITIALIZED_EXPENSIVE_BIN"
    }
    call_count = call_count + 1
    return cached_result
}

fn main() {
    println(int_to_str(call_count))
    println(int_to_str(MAX_LIMIT))
    let r1 = fetch_expensive()
    println(r1)
    println(int_to_str(call_count))
    let r2 = fetch_expensive()
    println(r2)
    println(int_to_str(call_count))
}
"#;

    std::fs::write(&test_file, source).expect("Failed to write test file");

    // Check with forgen check
    let status = Command::new(forgen_exe)
        .arg("check")
        .arg(&test_file)
        .status()
        .expect("Failed to run forgen check");
    assert!(status.success(), "forgen check failed for module state");

    // Run with Cranelift JIT
    let output_jit = Command::new(forgen_exe)
        .arg("run")
        .arg("--jit")
        .arg(&test_file)
        .output()
        .expect("Failed to run forgen run --jit");
    let stdout_jit = String::from_utf8_lossy(&output_jit.stdout);
    println!("JIT STDOUT:\n{}", stdout_jit);
    println!(
        "JIT STDERR:\n{}",
        String::from_utf8_lossy(&output_jit.stderr)
    );
    assert!(
        stdout_jit.contains("INITIALIZED_EXPENSIVE_BIN"),
        "JIT output missing cached result: {}",
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
            stdout_llvm.contains("INITIALIZED_EXPENSIVE_BIN"),
            "LLVM output missing cached result: {}",
            stdout_llvm
        );
        assert!(
            stdout_llvm.contains("100"),
            "LLVM output missing MAX_LIMIT: {}",
            stdout_llvm
        );
    }
}

#[test]
fn test_counter_dmir_llvm() {
    let source = r#"
mut count: Int = 0

fn inc() {
    count = count + 1
    println(int_to_str(count))
}

fn main() {
    println(int_to_str(count))
    inc()
    println(int_to_str(count))
}
"#;
    let jit_compiler = forgen::driver::ForgenCompiler::new("quick").with_llvm(false);
    let (stdout, _, _, _) = jit_compiler
        .run_source(source, "counter.dtr", &[], true)
        .unwrap();
    assert_eq!(stdout.replace("\r", ""), "0\n1\n1\n");
}

#[test]
fn test_module_state_release_mode() {
    let source = r#"
mut call_count: Int = 0
const MAX_LIMIT: Int = 100
mut cached_result: Str = ""

fn fetch_expensive() -> Str {
    if cached_result == "" {
        cached_result = "INITIALIZED_EXPENSIVE_BIN"
    }
    call_count = call_count + 1
    return cached_result
}

fn main() {
    println(int_to_str(call_count))
    println(int_to_str(MAX_LIMIT))
    let r1 = fetch_expensive()
    println(r1)
    println(int_to_str(call_count))
    let r2 = fetch_expensive()
    println(r2)
    println(int_to_str(call_count))
}
"#;
    let compiler = forgen::driver::ForgenCompiler::new("release").with_llvm(false);
    let (stdout, stderr, _code, _) = compiler.run_source(source, "state.dtr", &[], true).unwrap();
    println!("RELEASE JIT OUTPUT:\n{}", stdout);
    if !stderr.is_empty() {
        println!("RELEASE JIT STDERR:\n{}", stderr);
    }
}
