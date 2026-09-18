# Datara Formal Language Semantics & Invariants Specification
Version: 1.4.4
Status: Canonical Language Specification

---

## 1. Core Philosophy: Safe, Fast, Lexical — Without Rust's Lifetime Complexity

Datara achieves zero-overhead memory safety and performance parity (or superiority) with C and Rust, but avoids Rust's most painful pain point: **infectious lifetime parameters (`<'a, 'b: 'a>`)**.

Instead of complex lifetime graphs and borrow-checker puzzles, Datara uses:
1. **Strict Ownership with Move Semantics**: Exactly one active owner binding at any program point.
2. **Lexical View Scoping (Aliasing XOR Mutability)**: Borrows (`&x` and `mut &x`) are syntactically and structurally bounded by lexical block scopes. Borrows cannot escape their defining scope or outlive their owner.
3. **No Infectious Lifetime Signatures**: Functions do not carry lifetime parameters. Return values are either owned values (cheaply moved or stack-allocated) or derived slices with statically verified lexical provenance.
4. **Dual Production Compilers**:
   - **Cranelift Backend**: Sub-millisecond JIT, instant iterative development (`forgen run`, hot-reloading).
   - **LLVM Backend**: Tier-1 peak production optimizer (`forgen build --llvm -O3`) with auto-vectorization, loop unrolling, and link-time optimization (LTO) designed to match or beat Rust and C++ in throughput and latency.

---

## 2. Value Semantics: Copy vs Move

### 2.1 Copy Types
The following primitive types implement bitwise `Copy` by default:
- Integers: `Int`, `I8`, `I16`, `I32`, `I64`, `Byte`, `U8`, `U16`, `U32`, `U64`
- Floating-point numbers: `Float`, `F32`, `F64`
- Booleans: `Bool`
- Characters: `Char`
- Raw pointers inside `unsafe`: `RawPtr`

**Invariant**: Passing a `Copy` type to a function, assigning it, or returning it copies bits without invalidating the source binding.

### 2.2 Move Types
All other types are resource-owning and implement `Move` by default:
- Heap strings: `StrBuf`, `String`
- Collections: `List<T>`, `Map<K, V>`, `Set<T>`
- Memory and wire views: `SliceView`
- Classes, structs, and interfaces
- Concurrency handles: `Channel<T>`, `ThreadHandle`
- OS handles: `FileHandle`, `Socket`

**Invariant**: When an owner of a `Move` type is passed to a function by value, assigned to a new binding, or placed into a collection:
1. Ownership is transferred immediately.
2. The source binding transitions to state `ValueState::Moved`.
3. Any subsequent read, write, or borrow of the source binding raises compile-time diagnostic `E-BORROW-001`.

---

## 3. Borrowing & View Semantics: Aliasing XOR Mutability

### 3.1 Immutable Views (`&x`) and Mutable Borrows (`mut &x`)
- An immutable view `&x` allows read-only access to an owner.
- A mutable borrow `mut &x` allows exclusive in-place modification.

### 3.2 The Aliasing XOR Mutability Theorem
For any resource $R$:
$$\text{ActiveMutableBorrows}(R) \le 1 \land (\text{ActiveMutableBorrows}(R) > 0 \implies \text{ActiveViews}(R) == 0)$$

1. **Multiple Readers**: Arbitrary numbers of simultaneous immutable views `&x` are permitted.
2. **Single Writer**: A mutable borrow `mut &x` requires that no other active borrows (mutable or immutable) exist.
3. **No Reassignment While Borrowed**: An owner cannot be reassigned, moved, or destroyed while any view is active (`E-BORROW-003`).

### 3.3 Lexical Scoping & Escape Prevention
- Borrows are strictly bounded by their enclosing lexical scope block (`{ ... }`).
- A borrow of a local stack variable cannot escape its stack frame. Returning a local borrow produces `E-BORROW-005`.
- Returning a view from a function is permitted ONLY when derived directly from an input parameter view, with identical lexical lifetime guaranteed by the caller's frame.

---

## 4. Collections & Ownership Lifecycle

1. **Collection Ownership**: Collections own their elements. Inserting a Move-type into a `List<T>` or `Map<K, V>` transfers ownership into the collection.
2. **Element Access**:
   - `list.get(index)` returns `Outcome<T>` or an unowned view, ensuring out-of-bounds access is checked without panics.
   - Index assignment `list.set(index, val)` moves `val` into the slot, destroying the previous occupant.
3. **Zero-Copy Wire Protocols (`SliceView`)**:
   - `SliceView` wraps a contiguous memory buffer with bounds checking.
   - Slicing `s.subslice(start, len)` creates a new `SliceView` referencing the same underlying memory without allocation.
   - Byte-level reads and writes (`read_u16_be`, `write_u32_be`, etc.) guarantee exact endianness and alignment independence.

---

## 5. Ownership with Errors & Deterministic Early Exit

### 5.1 Outcome Unpacking (`?`) and Fallback (`or`)
Datara strictly rejects C++ and Java style untyped exception unwinding (`try/catch/throw`).
- All fallible operations return `Outcome<T, E>` or `Maybe<T>`.
- The postfix `?` operator unwraps the success value or returns the error immediately.
- The `or` operator provides an in-place fallback expression without branching overhead.

### 5.2 Deterministic Reverse Drop Invariant
When a function returns early (via `return`, `?`, or an unhandled outcome):
1. The compiler emits destructor cleanup for all currently live, initialized local Move bindings.
2. Destructors run in **reverse order of declaration**.
3. Already moved bindings (`ValueState::Moved`) are omitted from cleanup, preventing double-free defects.

---

## 6. Capabilities & Safe FFI Wrapper Layer

### 6.1 Unsafe Isolation
- All direct foreign FFI calls, raw pointer dereferencing, and inline assembly must reside in an `unsafe(justification: "...")` block.
- `unsafe` is an isolation boundary, not an excuse to disable compiler checks. Unsafe blocks cannot:
  - Bypass static capability checks (`[Net]`, `[FS]`, `[Proc]`).
  - Escape raw unchecked pointers into safe Datara public types.

### 6.2 The Four-Layer Bridge Architecture
Direct C ABI calls must never be exposed as public Datara APIs:
$$\text{Foreign Library (C/Rust/Python)} \longrightarrow \text{Unsafe Bridge} \longrightarrow \text{Safe Datara Wrapper} \longrightarrow \text{Application Code}$$

1. **Safe Wrappers**: Encapsulate raw pointers inside classes implementing deterministic cleanup (`free()` on drop).
2. **String Conversions**: Datara length-prefixed UTF-8 strings are safely converted to null-terminated `const char*` across the boundary with validated lifetime.
3. **Error Translations**: Foreign integer return codes and Python exceptions are mapped into idiomatic `Outcome<T, Str>`.
4. **Capability Gating**: External functions requiring network, filesystem, or process spawning must be granted capabilities statically.

---

## 7. Dual-Engine Parity: Cranelift & LLVM

Datara treats both Cranelift and LLVM as first-class, tier-1 compilation engines:

| Feature | Cranelift Backend | LLVM Backend |
| :--- | :--- | :--- |
| Primary Role | Ultra-fast JIT, REPL, dev builds, hot-reload | Production release builds, peak benchmark speed |
| Compilation Time | Sub-millisecond to ~50ms | 100ms - 2s |
| Optimizations | SSA copy prop, LICM, dead code stripping | Polyhedral loop vectorization, unrolling, LTO, PGO |
| ABI & Struct Layout | `@packed` byte-exact, identical offsets | `@packed` byte-exact, identical offsets |
| Systems Primitives | Portable byte-swaps, SIMD 128 | Native AVX-512, Neon, SVE, byte-swaps |
| Verification Gate | Cranelift IR verifier | LLVM Module verifier |

**Dual-Engine Guarantee**: Any valid Datara program compiled under Cranelift produces functionally identical computational results and memory layouts under LLVM.

---

## 8. Solving the Three Classical Borrowing Challenges (Without Rust's Pitfalls)

### 8.1 Structs with References: Generational Arena Handles & Scoped View Structs
- **The Problem**: In Rust, storing a reference `&'a T` in a struct requires lifetime annotations that infect all enclosing types, making graphs, trees, and ECS architectures extremely painful.
- **Datara's Solution**:
  1. **Generational Arena Handles (`Arena<T>`, `Handle<T>`)**: Datara provides first-class arenas where entities are accessed via lightweight 8-byte handles (`Index + Generation`). This allows cyclical graphs, ECS architectures, and parent-linked trees without any lifetime annotations.
  2. **Scoped `view struct`**: When a developer intentionally needs a transient struct borrowing from an input buffer (e.g. HTTP header parser tokens), Datara uses `view struct Token { slice: &Str, line: Int }`. The `view` keyword marks the struct as a lexical view whose lifetime is bound to the owner of `slice`, completely avoiding generic lifetime parameters (`<'a>`).

### 8.2 Honest Zero-Copy Parsers: Provenance Tracking & Arena Promotion
- **The Problem**: Returning parsed slices outside a function boundary often requires allocations if lifetimes cannot be expressed.
- **Datara's Solution**:
  - Functions accepting `&SliceView` or `&Str` can return derived slices whose provenance is statically tied to the input parameter.
  - For long-lived caching across lexical boundaries, Datara provides `slice.promote(arena)`: the slice is moved into an arena without re-parsing or redundant copying.

### 8.3 Non-Lexical Lifetimes: Liveness-Based Borrow Scope (Def-Use Chain Liveness)
- **The Problem**: In early lexical models, a borrow locked a variable until the closing curly brace `}`, preventing subsequent mutations even after the borrow was no longer used.
- **Datara's Solution**:
  - In Datara, a borrow's active span ends at its **Last Use** in the SSA def-use chain, NOT at the closing brace. Once the last instruction reading the borrow executes, the borrow is immediately released, allowing mutation of the owner without artificial sub-scoping.

---

## 9. Engineering Invariant Solutions

### 9.1 Float and Math Determinism Across Cranelift and LLVM
- To prevent divergence between Cranelift and LLVM due to floating-point reordering:
  - Both backends operate in strict IEEE-754 compliant mode by default (`strictfp` / non-associative float math).
  - SIMD reassociations and FMA fusion are restricted to explicit `@fastmath` annotations.
  - Integer arithmetic follows two's complement wrapping or explicit trapping, verified by DMIR overflow intrinsics.

### 9.2 String Null-Sentinel Invariant for Zero-Copy FFI
- Every owned Datara string buffer (`StrBuf`, string literal, dynamically allocated string) is allocated with capacity $N + 1$ and maintains a trailing null byte `\0` at index $N$, while tracking length $N$ in $O(1)$.
- This guarantees zero-allocation, zero-copy passing to C libraries expecting `const char*`.
- Transient arbitrary sub-slices (`&Str`) passed to C functions are buffered in a thread-local stack scratchpad if under 256 bytes, avoiding heap allocation entirely.

### 9.3 Capability Scalability: Hierarchical Bundling & Monotonic Upward Inference
- To prevent the "Java Checked Exceptions" annotation explosion:
  - Capabilities are hierarchical: `[IO]` automatically bundles `[Net]`, `[FS]`, and `[Env]`; `[System]` encompasses all OS capabilities.
  - Capability requirements are **automatically inferred upwards** across internal call graphs. Developers only declare capability boundaries on public API modules and security sandboxes.
  - If a function holds `[IO]`, calling a function requiring `[Net]` is satisfied automatically via capability subsumption.

---

## 10. The 4 Abstraction Levels & Zero-Overhead Hardware Architecture

Datara establishes a strict priority hierarchy for all language design decisions:
1. **Performance & Optimization**: Zero runtime overhead, 1:1 hardware translation, Cranelift sub-millisecond JIT and LLVM -O3 peak throughput.
2. **Safety & Zero-UB**: Guaranteed absence of undefined behavior, memory corruption, data races, and capability leaks.
3. **Developer Ergonomics & Practical Power**: Elimination of cognitive friction — zero lifetime annotations (`<'a, 'b>`), flat composition, and seamless polyglot interoperability.

```
+-------------------------------------------------------------------------+
| Level 1: Scripting & Prototyping                                        |
|   • Inferred types, fmt"...", clean flow, zero boilerplate              |
+-------------------------------------------------------------------------+
                                 | calls freely
+-------------------------------------------------------------------------+
| Level 2: Application & Enterprise Domain Logic                          |
|   • Affine ownership, contracts (require/ensure), effects [Net, FS]     |
+-------------------------------------------------------------------------+
                                 | calls freely
+-------------------------------------------------------------------------+
| Level 3: Systems Programming & Wire Protocols                           |
|   • SliceView zero-copy, RawPtr, @packed struct, endianness (hton/ntoh) |
+-------------------------------------------------------------------------+
                                 | calls freely
+-------------------------------------------------------------------------+
| Level 4: Kernel, MMIO & Direct Hardware Control                         |
|   • VolatilePtr, memory fences (atomic_fence_*), typed_zero_init        |
|   • Optional inline assembly: asm { mov rax, x; mov y, rax }             |
+-------------------------------------------------------------------------+
```

### 10.1 Delineation of the Four Levels

| Level | Target Domain | Core Features & Syntax | Memory & Safety Model |
| :--- | :--- | :--- | :--- |
| **Level 1** | Rapid scripting, tools, high-level automation | Dynamic inference, `let`/`mut`, string interpolation (`fmt"..."`), high-level collections (`List`, `Map`), concise closures (`=>`). | Managed affine ownership, automatic reclamation, Zero-UB. |
| **Level 2** | Enterprise apps, web servers, domain logic | Static typing, class/behavior flat composition, design-by-contract (`require`, `ensure`), error handling with postfix `?` and `or`, security capabilities (`[Net]`, `[FS]`). | Affine ownership, move semantics, zero lifetime parameters, static capability enforcement. |
| **Level 3** | Systems programming, network stacks, serialization | Zero-copy `SliceView`, `RawPtr`, memory arenas (`arena_alloc`), `@packed struct`, endianness primitives (`hton`, `ntoh`, `bswap`), native C/Rust/Python FFI. | Explicit layouts, arena lifecycles, guarded pointer dereferences, zero runtime abstraction cost. |
| **Level 4** | OS kernels, device drivers, embedded MMIO, hypervisors | `VolatilePtr`, hardware memory fences (`atomic_fence_*`), zeroed memory buffers (`typed_zero_init`), native SIMD vectors, optional inline `asm { ... }`. | Direct hardware mapping, 1:1 CPU instruction translation, hardware volatile semantics, Zero-UB. |

### 10.2 Hardware Control Without Mandatory Assembly

A fundamental tenet of Datara is that **assembly is NOT mandatory at Level 4**. Writing ultra-high-performance kernel or driver code does not force developers into unmaintainable, non-portable inline assembly strings:

1. **MMIO & Hardware Registers via `VolatilePtr`**:
   ```datara
   fn configure_uart(base_addr: RawPtr) {
       let uart = volatile_ptr(base_addr)
       let status = uart.read32()
       uart.write32(status or(0x01))
   }
   ```
   Compiles directly to 1:1 hardware volatile load and store instructions without raw assembly.

2. **Hardware Memory Ordering Fences**:
   ```datara
   fn publish_data() {
       atomic_fence_release()
       // data is now visible to other CPU cores before continuing
       atomic_fence_seq_cst()
   }
   ```
   Compiles directly to hardware barrier instructions (`MFENCE` on x86-64, `DMB ISH` on ARM64).

3. **Guaranteed Zeroed Allocations (`typed_zero_init`)**:
   Enforces that OS page tables, descriptor rings, and DMA buffers are initialized to clean zeroes, preventing info leaks and uninitialized memory UB.

### 10.3 Inline Assembly Variable Access & Bidirectional Lowering

When developers require niche or privileged CPU instructions (e.g. `CPUID`, `RDTSC`, or interrupt toggling), Datara provides safe, bidirectional inline assembly:

```datara
fn hardware_compute(input: Int) -> Int {
    mut output: Int = 0
    unsafe(justification: "Direct register execution") {
        asm {
            mov rax, input      // Reads Datara variable directly into RAX
            add rax, 42
            mov output, rax     // Writes RAX directly back into Datara variable
        }
    }
    return output
}
```

#### Ground Truths of Inline Assembly in Datara:
1. **Can Datara variables be read and modified in assembly?**
   **YES**. The compiler matches operand names against local Datara bindings, loads them into registers prior to the assembly instructions, and stores modified registers back into the designated Datara stack slots.
2. **Can variables be invented inside assembly without declaration in Datara?**
   **NO**. Datara enforces Zero-UB and affine ownership invariants. The compiler must know every variable's type, size, alignment, and stack layout. Variables must be declared in Datara (e.g. `mut output: Int = 0`) before being assigned within assembly.
3. **Cross-Level Interoperability**:
   Functions at Level 1, 2, 3, and 4 share an identical unified native ABI. A Level 4 hardware function can directly call a Level 1 helper, or vice versa, with zero marshaling or glue code.

