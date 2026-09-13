# Datara Examples Catalog (v1.3.0)

This directory contains executable reference examples for the Datara programming language and the Forgen native compiler. Every example in this directory is strictly verified against the Datara v1.3.0 language specification.

---

## How to Run the Examples

You can verify and run any example using `forgen`:

```bash
# Type, effect, and ownership verification:
forgen check examples/<file>.dtr

# Native execution (Cranelift/LLVM native backend):
forgen run examples/<file>.dtr

# JIT fast-execution mode:
forgen run --jit examples/<file>.dtr
```

---

## Catalog of Examples

### 1. Fundamentals & Flow Control
- `01_hello_world.dtr`: Minimal Datara program demonstrating entry point `fn main()` and string output.
- `01_vertical_slice.dtr`: Minimal vertical slice testing lexical analysis and compiler pipeline.
- `02_math_and_loops.dtr`: Integer arithmetic, while loop counter, and string formatting (`fmt"..."`).
- `04_decide_and_control.dtr`: Pattern-matching control flow using the `decide` expression.
- `04_enum_adt.dtr`: Algebraic Data Types (ADTs) and enums with tagged payload matching.
- `05_pipeline_dataflow.dtr`: Pipeline operator (`|>`) dataflow transformations.

### 2. Data-Oriented Design (DOD) & Post-OOP Modeling
- `02_class_modern_oop.dtr`: Struct declaration and behavior invocation.
- `03_post_oop_class.dtr`: Post-OOP architecture: contiguous flat data structs paired with decoupled `behavior` blocks.
- `03_split_behavior.dtr`: Decoupled behavior declaration across separate definitions.
- `18_data_oriented_structs.dtr`: Physics particle simulation demonstrating contiguous memory layout, zero vtable overhead, and cache-line locality.

### 3. Proof-Carrying Code & Real-World CLI Engines
- `06_phase1_complete_app.dtr`: Full modular application with entity processing and verification contracts.
- `07_entity_process_model.dtr`: Entity-component architecture and state transition pipeline.
- `08_text_analyzer_cli.dtr`: Statistical text analyzer with readability index (Coleman-Liau) and PCC division contracts (`require != 0`).
- `09_error_propagation_question.dtr`: Result and Outcome handling with postfix error propagation (`?`) and fallback (`or`).
- `09_matrix_math_cli.dtr`: Multi-dimensional matrix arithmetic with linear algebra routines.
- `10_database_query_cli.dtr`: In-memory relational query engine with filtering and aggregation.
- `11_crypto_pow_cli.dtr`: Cryptographic proof-of-work generator and block hasher using Knuth LCG.
- `zero_js_dashboard.dtr`: Reactive dashboard application rendered natively.

### 4. Gradual Typing & Adaptive Comptime Optimization
- `12_dynamic_variables_val.dtr`: The Variable Triad (`let`, `mut`, `val`, `mut val`) for gradual typing.
- `19_adaptive_flow_typing.dtr`: Comptime adaptive flow-typing with SSA register specialization, eliminating dynamic boxing overhead.
- `dynamic_guarded_demo.dtr`: Capability-guarded dynamic variable execution.

### 5. Universal Polyglot Zero-Latency Engine (New in v1.3.0)
- `13_polyglot_zig_math.dtr`: Direct Zig SIMD math expressions and foreign kernel dispatch (`zig_eval_int`, `zig_call`).
- `14_polyglot_csharp_nativeaot.dtr`: In-process C# / .NET NativeAOT unmanaged symbol invocation (`csharp_invoke_i64`).
- `15_polyglot_lua_scripting.dtr`: Hot-reloadable Lua / LuaJIT scripting and expression evaluation (`lua_eval_int`, `lua_exec`).
- `16_polyglot_python_zerocopy.dtr`: Python ecosystem bridge (NumPy, SciPy) with zero-copy memory buffers (`py.exec`, `py.eval_int`, `py.eval_float`).
- `17_polyglot_parallel_computing.dtr`: Multi-threaded parallel execution across Zig, Lua, and C# NativeAOT runners (`polyglot_parallel_exec`).

---

## Verification Test Run

All examples can be validated in a single command:
```powershell
Get-ChildItem -Path examples -Filter *.dtr | ForEach-Object { forgen run $_.FullName }
```
