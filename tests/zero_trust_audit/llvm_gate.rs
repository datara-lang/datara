//! Shared gate for LLVM-toolchain-dependent tests: when `llc` (and `opt`)
//! cannot be found — e.g. a CI runner without LLVM in PATH — the test skips
//! instead of failing. Locally LLVM is on PATH and every test runs.

pub fn llvm_toolchain_available() -> bool {
    let exe = if cfg!(windows) { "llc.exe" } else { "llc" };
    if which_llc(exe) {
        return true;
    }
    // rustup toolchains sometimes ship llvm-tools with llc — reuse the
    // compiler's own lookup.
    forgen::codegen::linker::find_llc().is_some()
}

fn which_llc(name: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join(name);
            if p.is_file() {
                return true;
            }
        }
    }
    false
}
