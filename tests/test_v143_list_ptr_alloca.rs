use std::process::Command;

#[test]
fn test_v143_list_alloca_emits_ptr_type() {
    let forgen_exe = env!("CARGO_BIN_EXE_forgen");

    let temp_dir = std::env::temp_dir().join("forgen_test_v143_alloca");
    let _ = std::fs::create_dir_all(&temp_dir);
    let test_file = temp_dir.join("test_list_alloca.dtr");

    let source = r#"
fn compute_sum(n: Int) -> Float {
    mut a: List<Float> = []
    mut i = 0
    while i < n {
        a.push(1.5)
        i = i + 1
    }
    let idx = 0
    return a[idx]
}

fn main() {
    let res = compute_sum(10)
    out res
}
"#;
    std::fs::write(&test_file, source).expect("must write test file");

    let output = Command::new(forgen_exe)
        .env("FORGEN_KEEP_LLVM", "1")
        .arg("run")
        .arg("--llvm")
        .arg(&test_file)
        .output()
        .expect("must run forgen run --llvm");

    assert!(output.status.success(), "Command failed: {:?}", String::from_utf8_lossy(&output.stderr));

    // Find the generated .ll file in .forgen_cache/build
    let cache_dir = std::path::Path::new(".forgen_cache/build");
    if cache_dir.exists() {
        let mut found_alloca_ptr = false;
        let mut found_invalid_alloca_i64 = false;
        if let Ok(entries) = std::fs::read_dir(cache_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("test_list_alloca") && name.ends_with(".ll") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        for line in content.lines() {
                            if line.contains("%var_a = alloca ptr") {
                                found_alloca_ptr = true;
                            }
                            if line.contains("%var_a = alloca i64") {
                                found_invalid_alloca_i64 = true;
                            }
                        }
                    }
                }
            }
        }
        assert!(found_alloca_ptr, "Expected %var_a to be alloca ptr, align 8");
        assert!(!found_invalid_alloca_i64, "Found invalid %var_a = alloca i64");
    }
}
