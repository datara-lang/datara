use forgen::codegen::wasm::WasmEmitter;
use forgen::driver::ForgenCompiler;
use std::fs;
use std::process::Command;

#[test]
fn bench_wasm_vs_javascript_empirical() {
    println!(
        "\n+-----------------------------------------------------------------------------------------+"
    );
    println!(
        "| EMPIRICAL BENCHMARK: Datara WebAssembly vs JavaScript (Node.js v24 / V8)                |"
    );
    println!(
        "+-----------------------------------------------------------------------------------------+"
    );

    let temp_dir = std::env::temp_dir().join("datara_wasm_vs_js_bench");
    let _ = fs::create_dir_all(&temp_dir);

    // 1. Recursive Fibonacci Benchmark: fib(35)
    let fib_source = r#"
fn fib(n: Int) -> Int {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn main() -> Int {
    return fib(35)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let dmir_fib = compiler
        .compile_source_to_dmir(fib_source, "bench_fib.dtr")
        .expect("Fib DMIR lowering must succeed");
    let fib_wasm_path = temp_dir.join("fib_bench.wasm");
    WasmEmitter::emit_wasm_binary(&dmir_fib, &fib_wasm_path).expect("Fib WASM emission failed");
    if let Ok(wat) = fs::read_to_string(fib_wasm_path.with_extension("wat")) {
        println!("FIB WAT:\n{}", wat);
    }

    // 2. Heavy Numeric Loop Benchmark: sum of 50,000,000 iterations
    let loop_source = r#"
fn main() -> Int {
    mut sum = 0
    mut i = 1
    while i <= 50000000 {
        sum = sum + (i * 3) - (i / 2)
        i = i + 1
    }
    return sum
}
"#;
    let dmir_loop = compiler
        .compile_source_to_dmir(loop_source, "bench_loop.dtr")
        .expect("Loop DMIR lowering must succeed");
    let loop_wasm_path = temp_dir.join("loop_bench.wasm");
    WasmEmitter::emit_wasm_binary(&dmir_loop, &loop_wasm_path).expect("Loop WASM emission failed");

    let node_script = format!(
        r#"
const fs = require('fs');
const {{ performance }} = require('perf_hooks');

const importObject = {{
    "datara:rt": {{
        alloc: (sz) => 65536n,
        print: (v) => {{}},
        err: (v) => {{}},
        list_create: () => 0n,
        list_get: () => 0n,
        list_append: () => 0n,
        list_len: () => 0n
    }},
    env: {{
        now: () => BigInt(Date.now())
    }}
}};

async function run() {{
    // Workload 1: Recursive Fibonacci(35)
    function jsFib(n) {{
        if (n <= 1) return n;
        return jsFib(n - 1) + jsFib(n - 2);
    }}

    // Warmup JS
    jsFib(20);

    const t0_js = performance.now();
    const res_js_fib = jsFib(35);
    const t1_js = performance.now();
    const js_fib_ms = t1_js - t0_js;

    const fibWasmBytes = fs.readFileSync('{0}');
    const wasmFibMod = await WebAssembly.instantiate(fibWasmBytes, importObject);
    
    // Warmup WASM
    wasmFibMod.instance.exports.main();

    const t0_wasm = performance.now();
    const res_wasm_fib = wasmFibMod.instance.exports.main();
    const t1_wasm = performance.now();
    const wasm_fib_ms = t1_wasm - t0_wasm;

    // Workload 2: Heavy Numeric Loop (50,000,000 iterations)
    function jsLoop() {{
        let sum = 0n;
        let i = 1n;
        while (i <= 50000000n) {{
            sum = sum + (i * 3n) - (i / 2n);
            i = i + 1n;
        }}
        return sum;
    }}

    const t0_js_loop = performance.now();
    const res_js_loop = jsLoop();
    const t1_js_loop = performance.now();
    const js_loop_ms = t1_js_loop - t0_js_loop;

    const loopWasmBytes = fs.readFileSync('{1}');
    const wasmLoopMod = await WebAssembly.instantiate(loopWasmBytes, importObject);
    const t0_wasm_loop = performance.now();
    const res_wasm_loop = wasmLoopMod.instance.exports.main();
    const t1_wasm_loop = performance.now();
    const wasm_loop_ms = t1_wasm_loop - t0_wasm_loop;

    console.log(`REPORT_FIB_VAL_JS:${{res_js_fib}}`);
    console.log(`REPORT_FIB_VAL_WASM:${{res_wasm_fib}}`);
    console.log(`REPORT_FIB_JS:${{js_fib_ms.toFixed(2)}}`);
    console.log(`REPORT_FIB_WASM:${{wasm_fib_ms.toFixed(2)}}`);
    console.log(`REPORT_FIB_RES_EQUAL:${{BigInt(res_js_fib) === res_wasm_fib}}`);
    console.log(`REPORT_LOOP_JS:${{js_loop_ms.toFixed(2)}}`);
    console.log(`REPORT_LOOP_WASM:${{wasm_loop_ms.toFixed(2)}}`);
    console.log(`REPORT_LOOP_RES_EQUAL:${{res_js_loop === res_wasm_loop}}`);
    console.log(`REPORT_WASM_FIB_SIZE:${{fibWasmBytes.length}}`);
    console.log(`REPORT_WASM_LOOP_SIZE:${{loopWasmBytes.length}}`);
}}

run().catch(console.error);
"#,
        fib_wasm_path.to_string_lossy().replace('\\', "/"),
        loop_wasm_path.to_string_lossy().replace('\\', "/")
    );

    let script_path = temp_dir.join("bench_runner.js");
    fs::write(&script_path, node_script).expect("Failed to write runner script");

    let output = Command::new("node")
        .arg(&script_path)
        .output()
        .expect("Failed to execute node benchmark");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("NODE BENCHMARK OUTPUT:\n{}", stdout);
    if !stderr.is_empty() {
        eprintln!("NODE STDERR:\n{}", stderr);
    }

    assert!(stdout.contains("REPORT_FIB_RES_EQUAL:true"));
    assert!(stdout.contains("REPORT_LOOP_RES_EQUAL:true"));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
