//! v1.4.4: Zero-Trust FFI Capability Enforcement Tests.

use forgen::driver::ForgenCompiler;

#[test]
fn test_v144_ffi_capability_enforcement_rejects_unsafe_bypass() {
    let compiler = ForgenCompiler::new("check");

    // Foreign C networking call inside unsafe WITHOUT NetworkConnect capability must fail compile-time
    let code_connect = r#"
extern fn connect(sock: Int, addr: Str, port: Int) -> Int

fn main() {
    unsafe(justification: "try to bypass network capability via unsafe FFI") {
        let fd = connect(0, "127.0.0.1", 8080)
    }
}
"#;

    let res = compiler.check_source(code_connect, "ffi_sandbox.dtr");
    assert!(
        !res.success,
        "Calling foreign FFI 'connect' inside unsafe without Capability<NetworkConnect> must be rejected by Zero-Trust"
    );
    assert!(
        res.diagnostics.contains("Capability<NetworkConnect>"),
        "Diagnostics must mention Capability<NetworkConnect>: {}",
        res.diagnostics
    );
    assert!(
        res.diagnostics.contains("Zero-Trust"),
        "Diagnostics must mention Zero-Trust: {}",
        res.diagnostics
    );
}

#[test]
fn test_v144_ffi_capability_enforcement_file_write_rejected() {
    let compiler = ForgenCompiler::new("check");

    // Foreign C file write inside unsafe WITHOUT FileWrite capability must fail
    let code_write = r#"
extern fn fwrite(ptr: Str, size: Int, count: Int, stream: Int) -> Int

fn main() {
    unsafe(justification: "raw foreign write without permission") {
        let n = fwrite("payload", 1, 7, 0)
    }
}
"#;

    let res = compiler.check_source(code_write, "ffi_write.dtr");
    assert!(
        !res.success,
        "Calling foreign FFI 'fwrite' inside unsafe without Capability<FileWrite> must be rejected"
    );
    assert!(
        res.diagnostics.contains("Capability<FileWrite>"),
        "Diagnostics must mention Capability<FileWrite>: {}",
        res.diagnostics
    );
}
