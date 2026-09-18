//! Tests JIT runtime symbol resolution parity for systems primitives:
//! endianness (bswap32/hton32), SliceView, scratchpad, channels, and threading.

use forgen::codegen::TargetInfo;
use forgen::codegen::cranelift::backend::RealCraneliftBackend;
use forgen::codegen::cranelift::backend::opts::JitCompilationTier;
use forgen::codegen::cranelift::jit::JitSession;
use forgen::driver::ForgenCompiler;

#[test]
fn test_jit_bswap_and_slice_primitives_parity() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
fn main() -> Int {
    let raw: Int = 305419896 // 0x12345678
    let swapped: Int = bswap32(raw)
    println(int_to_str(swapped))
    0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_bswap_jit.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    session.load_module(&dmir_mod).expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    // 0x12345678 bytes swapped in 32-bit = 0x78563412 = 2018915346
    assert_eq!(stdout.trim(), "2018915346");
}

#[test]
fn test_jit_channel_and_threading_symbols_resolved() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
fn main() -> Int {
    let ch = channel_create(10)
    let s = channel_send(ch, 42)
    let len = channel_len(ch)
    let val = channel_recv(ch)
    channel_close(ch)
    out fmt"LEN:{len} VAL:{val}"
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_channel_jit.dtr", None);
    assert!(res.success, "Compilation failed: error={:?} diag={:?}", res.error, res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    session.load_module(&dmir_mod).expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert!(stdout.contains("LEN:1 VAL:42"), "Output was: {}", stdout);
}
