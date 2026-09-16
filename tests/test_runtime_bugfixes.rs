//! Runtime bugfix tests: exit() code propagation, binary-safe file I/O
//! builtins, and Windows socket double-bind prevention.

use forgen::codegen::llvm::LlvmEmitter;
use forgen::codegen::target::TargetInfo;
use forgen::driver::ForgenCompiler;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

fn run_source(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, name, None);
    assert!(
        res.success,
        "Compilation should succeed: {:?}\n{}",
        res.error, res.diagnostics
    );
    let exe = res.exe_path.unwrap();
    let (stdout, stderr, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    if code != 0 {
        eprintln!("stderr: {}", stderr);
    }
    (stdout, code)
}

/// exit(code) must terminate the process with exactly that code.
#[test]
fn test_exit_code_propagates() {
    let source = r#"
fn main() -> Int {
    println("before-exit")
    exit(7)
    return 0
}
"#;
    let (stdout, code) = run_source(source, "exit_propagation.dtr");
    assert_eq!(code, 7, "exit(7) must yield process exit code 7");
    assert!(
        stdout.contains("before-exit"),
        "output before exit: {}",
        stdout
    );
}

/// exit(0) is a success exit.
#[test]
fn test_exit_zero_is_success() {
    let source = r#"
fn main() -> Int {
    exit(0)
    return 1
}
"#;
    let (_stdout, code) = run_source(source, "exit_zero.dtr");
    assert_eq!(code, 0, "exit(0) must yield exit code 0");
}

/// The LLVM pipeline must resolve exit() to datara_rt_exit (previously
/// the name mapping and the IR declaration were missing, so --llvm
/// builds failed to link with an undefined symbol).
#[test]
fn test_exit_resolved_on_llvm_path() {
    let source = r#"
fn main() -> Int {
    exit(7)
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, "exit_llvm.dtr", None);
    assert!(res.success, "compilation: {:?}", res.error);
    let dmir = res.dmir_module.expect("DMIR module");
    let program = res.program.expect("AST program");

    let resolver = Resolver::new();
    let types = TypeChecker::new(&resolver);
    let target = TargetInfo::native();
    let emitter = LlvmEmitter::new(&target);
    let ir = emitter
        .emit_module(&dmir, &program, &types)
        .expect("LLVM emission must succeed");
    assert!(
        ir.contains("declare void @datara_rt_exit(i32)"),
        "IR must declare datara_rt_exit"
    );
    assert!(
        ir.contains("call void @datara_rt_exit("),
        "IR must call datara_rt_exit, got:\n{}",
        ir
    );
}

/// file_read_bytes / file_write_bytes round-trip binary content that
/// contains NUL bytes: the text path cannot carry length through the
/// NUL-terminated C string ABI, so these builtins use the list ABI
/// (one Int element per byte value 0..=255).
#[test]
fn test_file_bytes_round_trip_with_nul_bytes() {
    // Write a binary blob: all 256 byte values, NUL first and last.
    let input_path = std::path::Path::new("target").join("bugfix_bytes_in.bin");
    std::fs::create_dir_all("target").unwrap();
    let blob: Vec<u8> = (0..=255u8).collect();
    std::fs::write(&input_path, &blob).unwrap();
    let output_path = std::path::Path::new("target").join("bugfix_bytes_out.bin");
    let _ = std::fs::remove_file(&output_path);

    let input_str = input_path.to_string_lossy().replace('\\', "/");
    let output_str = output_path.to_string_lossy().replace('\\', "/");
    let source = format!(
        r#"
fn main(sys_caps: SystemCapabilities) -> Int {{
    let data = file_read_bytes("{input_str}")
    if data.len() != 256 {{ return 1 }}
    if data[0] != 0 {{ return 2 }}
    if data[10] != 10 {{ return 3 }}
    if data[128] != 128 {{ return 4 }}
    if data[255] != 255 {{ return 5 }}
    if file_write_bytes("{output_str}", data) != 1 {{ return 6 }}
    let back = file_read_bytes("{output_str}")
    if back.len() != 256 {{ return 7 }}
    if back[0] != 0 {{ return 8 }}
    if back[255] != 255 {{ return 9 }}
    return 0
}}
"#
    );

    let (_stdout, code) = run_source(&source, "file_bytes_round_trip.dtr");
    assert_eq!(code, 0, "binary round-trip must be lossless, exit={}", code);

    let written = std::fs::read(&output_path).unwrap();
    assert_eq!(written, blob, "written file must equal the original bytes");
}

/// On Windows, SO_REUSEADDR must NOT be set by socket_bind: it would
/// let a second socket bind the same addr:port while the first is still
/// bound (double-bind hijack). The second bind must fail with -1.
#[test]
#[cfg(windows)]
fn test_double_bind_same_port_fails_on_windows() {
    let source = r#"
fn main(net_caps: SystemCapabilities) -> Int {
    mut port = 47210
    mut verdict = 2
    while port < 47230 {
        let a = socket_create(1)
        if socket_bind(a, "127.0.0.1", port) == 0 {
            let b = socket_create(1)
            if socket_bind(b, "127.0.0.1", port) == 0 {
                verdict = 1
            } else {
                verdict = 0
            }
        }
        port = port + 1
    }
    if verdict == 2 {
        return 2
    }
    if verdict == 0 {
        return 0
    }
    return 1
}
"#;
    let (_stdout, code) = run_source(source, "double_bind.dtr");
    assert_ne!(
        code, 2,
        "no bindable port found in range; test environment issue"
    );
    assert_eq!(
        code, 0,
        "second bind of the same port must fail on Windows (code 1 means double-bind succeeded)"
    );
}
