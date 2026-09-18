use forgen::driver::ForgenCompiler;
use forgen::resolver::Resolver;

#[test]
fn test_resolver_has_atomic_fence() {
    let r = Resolver::new();
    assert!(
        r.resolve_symbol("atomic_fence_seq_cst").is_some(),
        "atomic_fence_seq_cst must be in resolver"
    );
    assert!(
        r.resolve_symbol("volatile_ptr").is_some(),
        "volatile_ptr must be in resolver"
    );
}

#[test]
fn test_is_stdlib_security_bypass_prevented() {
    // User file inside a folder named `my_stdlib` must NOT bypass capability enforcement!
    let source = r#"
extern fn fopen(path: Str, mode: Str) -> Int

fn main() {
    unsafe(justification: "C stdlib call") {
        let f = fopen("data.bin", "rb")
    }
}
"#;
    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_source(source, "my_stdlib/exploit.dtr");
    assert!(
        !res.success,
        "Zero-Trust: foreign FFI call in user file inside 'my_stdlib' must fail without File capability"
    );
    let has_sec_error = res.diagnostics.contains("Capability<")
        || res.diagnostics.contains("requires '")
        || res.diagnostics.contains("Security Violation");
    assert!(
        has_sec_error,
        "Expected SecurityViolation diagnostic, got: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_node_import_routing_recognized() {
    let source = r#"
use node.express as express

fn main() {
    println("Node routing test")
}
"#;
    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_source(source, "test_node.dtr");
    let handled = res.success
        || res.diagnostics.contains("express")
        || res.diagnostics.contains("node")
        || res.diagnostics.contains("npm");
    assert!(
        handled,
        "Node import must be routed to JS/NPM handler: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_level4_hardware_primitives_typecheck() {
    let source = r#"
fn hardware_test(reg_addr: RawPtr) {
    // 1. MMIO Volatile access
    let vptr: VolatilePtr = volatile_ptr(reg_addr)
    let val32: Int = vptr.read32()
    vptr.write32(val32)

    // 2. Direct volatile intrinsics
    let val64: Int = volatile_read64(reg_addr)
    volatile_write64(reg_addr, val64)

    // 3. Hardware memory fences
    atomic_fence_acquire()
    atomic_fence_release()
    atomic_fence_acq_rel()
    atomic_fence_seq_cst()
    atomic_fence("SeqCst")

    // 4. Safe typed zero init
    let zeroed_page: RawPtr = typed_zero_init(4096)
}

fn main() {
    println("Hardware primitives verified")
}
"#;
    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_source(source, "test_hw.dtr");
    assert!(
        res.success,
        "Level 4 hardware intrinsics must compile cleanly: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_asm_bidirectional_variable_access() {
    let source = r#"
fn asm_variable_bridge() -> Int {
    let input: Int = 100
    mut output: Int = 0

    unsafe(justification: "Verified hardware register arithmetic") {
        asm {
            mov rax, input
            add rax, 42
            mov output, rax
        }
    }

    return output
}

fn main() {
    let res = asm_variable_bridge()
    println(int_to_str(res))
}
"#;
    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_source(source, "test_asm_bridge.dtr");
    assert!(
        res.success,
        "Bidirectional variable access in asm must compile cleanly: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_cross_level_function_calling_compatibility() {
    let source = r#"
// Level 1: Simple high-level helper
fn level1_greeting(name: Str) -> Str {
    return fmt"Hello, {name}!"
}

fn level1_add(a: Int, b: Int) -> Int {
    return a + b
}

// Level 2: Domain logic with contracts
fn level2_safe_div(a: Int, b: Int) -> Int {
    require b != 0
    return a / b
}

// Level 3: Systems level with raw pointers
fn level3_systems_task(ptr: RawPtr) -> Int {
    let slice = slice_from_buffer(ptr, 64)
    let b = slice.get_byte(0)
    slice.free()
    return b
}

// Level 4: Hardware / Asm level calling Level 1, 2, and 3
fn level4_orchestrator() -> Int {
    // Level 4 calling Level 1
    let sum = level1_add(10, 20)
    let msg = level1_greeting("Datara")

    // Level 4 calling Level 2
    let div = level2_safe_div(sum, 2)

    // Level 4 internal assembly mutating variable
    mut result: Int = 0
    unsafe(justification: "Hardware inline compute") {
        asm {
            mov rax, div
            add rax, 5
            mov result, rax
        }
    }

    atomic_fence_seq_cst()
    return result
}

fn main() {
    let res = level4_orchestrator()
    println(int_to_str(res))
}
"#;
    let compiler = ForgenCompiler::new("check");
    let res = compiler.check_source(source, "test_cross_level.dtr");
    assert!(
        res.success,
        "Cross-level function calling across all 4 levels must compile seamlessly: {:?}",
        res.diagnostics
    );
}
