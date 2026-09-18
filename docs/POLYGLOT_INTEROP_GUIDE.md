# Universal Polyglot Interoperability Guide — Datara & Forgen v1.4.4

## 1. Overview & Architecture

Datara provides the fastest and most seamless multi-language interoperability layer in modern programming. Instead of forcing developers to rewrite existing libraries or deal with slow RPC / serialization boundaries, Datara connects natively to the C, C++, Rust, Python, Go, C# (.NET NativeAOT), JavaScript/Node.js, Zig, Java/Kotlin (GraalVM), and Lua/Luau ecosystems.

Across all 4 abstraction levels:
- **Zero-Copy Memory**: Arrays and buffers can be passed directly across language boundaries via `SliceView<T>` and Shadow Heap Pinning without allocation.
- **Zero-Trust FFI Enforcement**: Foreign calls are strictly capability-gated (`[Net]`, `[FS]`, `[Proc]`). Wrapping a call in `unsafe(justification: "...")` permits pointer manipulation and foreign calling conventions, but **never bypasses system capability security**.
- **Unified Manifest (`dpm.toml`)**: A single configuration file declares dependencies for all languages, installed and compiled with `forgen install`.
- **Whole-Program Tree Shaking**: Unused functions from foreign libraries and crates are pruned at link time via MSVC `/OPT:REF /OPT:ICF`, Linux `-Wl,--gc-sections`, and Rust Thin-LTO bitcode embedding.

---

## 2. Multi-Language Interop Matrix

| Language | Syntax | Interop Mechanism | Memory Sharing | DCE / Tree Shaking |
|:---|:---|:---|:---|:---|
| **C** | `import c "header.h" with link("lib.lib")` | Direct header parsing & C ABI | Zero-copy raw pointers | `/OPT:REF`, `--gc-sections` |
| **C++** | `import cpp "header.hpp" with link("lib.lib")` | Clang/MSVC C++ ABI (`extern "C"`) | Zero-copy raw pointers | `/LTCG`, `/OPT:REF` |
| **Rust** | `use rust.serde as serde` | Automated `cdylib` wrapper + Cargo | Zero-copy slices & buffers | Thin-LTO `embed-bitcode=yes` |
| **Python** | `use python.numpy as np` | Embedded CPython + Shadow Heap | Shadow Heap Pinning (<500ns) | Dynamic runtime |
| **JS / TS / Node** | `use npm.express as express` | Node.js bridge / V8 IPC | TypedArray buffer views | Tree-shaken via bundler |
| **Go** | `use go.cryptoutil` | `go build -buildmode=c-shared` | C-shared memory pointer | Linker section GC |
| **C# / .NET** | `use csharp.FastMath` | .NET 8+ NativeAOT compilation | Unmanaged memory export | NativeAOT internal trimmer |
| **Zig** | `use zig.algorithms` | Direct `zig build-lib` / C ABI | Direct zero-copy buffers | Linker section GC |
| **Java / Kotlin** | `use java.Enterprise` | GraalVM `native-image --shared` | C-compatible shared object | GraalVM static analysis |
| **Lua / Luau** | `use lua.script` / `use luau.script` | Embedded LuaJIT / Luau VM | Shared state & string views | Embedded VM footprint |

---

## 3. Language-Specific Guides & Examples

### 3.1 C & C++ (`import c` / `import cpp`)
Datara parses C and C++ headers directly, extracts function declarations, layout-compatible structs, and enums, and links against the native `.lib` or `.a` archive:

```datara
import c "sqlite3.h" with link("sqlite3.lib");

fn open_database(path: Str) -> Int {
    mut db: RawPtr = 0 as RawPtr
    mut rc: Int = 0
    unsafe(justification: "Invoke SQLite C API") {
        rc = sqlite3_open(path, db)
    }
    return rc
}
```

For C++, expose APIs with `extern "C"` to disable C++ name mangling:
```cpp
// math_simd.hpp
#ifdef __cplusplus
extern "C" {
#endif

void fast_matrix_multiply(const float* a, const float* b, float* out, int n);

#ifdef __cplusplus
}
#endif
```

### 3.2 Rust Ecosystem (`use rust.<crate>`)
Datara interacts with crates directly. Forgen generates a `cdylib` crate wrapper, passes `RUSTFLAGS="-C embed-bitcode=yes"` for whole-program Thin-LTO, and compiles with release optimizations:

```datara
use rust.serde as serde
use rust.tokio as tokio
```

Declare in `dpm.toml`:
```toml
[rust-dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.35", features = ["full"] }
```

### 3.3 Python & NumPy (`use python.<module>`)
Zero-copy buffer export with **Shadow Heap Pinning**:
```datara
use python.numpy as np

fn analyze_data(values: List<Float>) {
    let py = Py { version: "3.x" }
    py.bind_buffer("data", values) // Zero-copy pinned memory
    py.exec(r#"
import numpy as np
mean_val = float(np.mean(data))
std_val = float(np.std(data))
"#)
    let mean = py.eval_float("mean_val")
    out fmt"Mean: {mean.value}"
}
```

### 3.4 Go Interoperability (`use go.<package>`)
Datara interfaces with Go packages compiled as C-shared libraries (`-buildmode=c-shared`):
```datara
use go.cryptoutil as crypto

fn hash_payload(data: Str) -> Str {
    mut result: Str = ""
    unsafe(justification: "Call Go c-shared crypto archive") {
        result = crypto.Sha256Hex(data)
    }
    return result
}
```

### 3.5 C# / .NET NativeAOT (`use csharp.<assembly>`)
Direct zero-overhead calls into .NET 8+ assemblies compiled with `PublishAot=true`:
```datara
use csharp.FastMath as math

fn calculate(x: Float) -> Float {
    return math.EvaluateSpline(x)
}
```

### 3.6 Zig Interoperability (`use zig.<module>`)
Seamless interop with Zig via standard C ABI exports:
```datara
use zig.simd_math as zmath

fn run_kernel(buf: SliceView<Float>) {
    unsafe(justification: "Invoke Zig SIMD kernel") {
        zmath.vector_add(buf.as_ptr(), buf.len())
    }
}
```

### 3.7 JavaScript / TypeScript / Node.js (`use npm.<package>`)
Fast asynchronous communication and IPC with Node.js modules:
```datara
use npm.express as express

fn start_server() {
    let app = express.create()
    app.get("/health", fn(req, res) {
        res.send("OK")
    })
    app.listen(8080)
}
```

### 3.8 Java / Kotlin via GraalVM Native Image (`use java.<package>`)
Call into enterprise Java or Kotlin libraries compiled into native shared objects:
```datara
use java.EnterpriseRules as rules

fn validate_record(id: Int) -> Bool {
    return rules.check_compliance(id)
}
```

### 3.9 Lua and Luau Scripting (`use lua.<module>` / `use luau.<module>`)
Embedded high-performance scripting:
```datara
use lua.ai_logic as ai

fn tick_ai(entity_id: Int) {
    ai.update_behavior(entity_id)
}
```

---

## 4. Unified Dependency Manifest (`dpm.toml`)

All dependencies across all languages live in one declarative file:

```toml
[package]
name = "enterprise_pipeline"
version = "1.4.4"

[c-dependencies]
sqlite3 = { version = "3.45", windows = "sqlite3.lib", linux = "libsqlite3.a" }

[cpp-dependencies]
fast_simd = { header = "simd.hpp", link = "fast_simd.lib" }

[rust-dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }

[python-dependencies]
numpy = ">=1.24"
torch = ">=2.0"

[npm-dependencies]
express = "^4.18"

[go-dependencies]
cryptoutil = { path = "./go/cryptoutil.go", buildmode = "c-shared" }

[dotnet-dependencies]
fastmath = { path = "./dotnet/FastMath.csproj", aot = true }
```

Run single-command synchronization:
```bash
forgen install
```

---

## 5. Zero-Trust FFI Capability Enforcement

In Datara v1.4.4, `unsafe(justification: "...")` only allows:
1. Dereferencing raw pointers (`RawPtr`).
2. Calling foreign ABI functions.
3. Structured inline assembly (`asm { ... }`).

It **never** grants system capabilities. If an external C/C++ function performs network I/O (`connect`, `send`, `socket`) or filesystem writes (`fwrite`, `unlink`), the caller function **must** declare and hold `Capability<NetworkConnect>` or `Capability<FileWrite>`. Otherwise, compilation halts with a `SecurityViolation` error (`E-CAP-001`).

---

## 6. Standard Output and Printing Policy in Datara v1.4.4

> **Golden Rule**: «Перевод строки — оператор (`out` / `err`), без перевода — функция `print`. Всё.»

| Форма | Куда | Перевод строки (`\n`) | Статус | Назначение |
|:---|:---|:---|:---|:---|
| `out e` | stdout | **Да** | Канон, оператор | Полнострочный вывод с автоматическим переводом строки |
| `err e` | stderr | **Да** | Канон, оператор | Вывод ошибок в stderr с переводом строки |
| `print(x)` | stdout | **Нет** | Канон, функция | Посимвольный вывод, прогресс-бары, streaming без `\n` |
| `println(x)` | stdout | Да | **Deprecated (`W0102`)** | Устаревший синоним; используйте оператор `out` |
| `eprintln(x)` | stderr | Да | **Deprecated (`W0102`)** | Устаревший синоним; используйте оператор `err` |

### Typed Format Streaming Fusion
Datara v1.4.4 compiles `out fmt"..."` directly to unrolled streaming I/O calls without heap string allocation or temporary buffer concatenation:
```datara
let name = "Datara"
let count = 42

// Zero-allocation streamed directly to stdout with newline
out fmt"Processing {name}: {count} items"

// Partial-line progress output without newline
print("Loading: [")
mut i = 0
while i < 10 {
    print("=")
    i = i + 1
}
out "] 100%"
```

