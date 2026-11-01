# Datara for C / Rust / Python developers

One page, verified claims only (every number comes from a committed
benchmark artifact — see tests/ and evidence/).

## The one-paragraph pitch

Datara compiles to native machine code (Cranelift JIT/AOT, LLVM, WASM —
all three produce byte-identical program output). It has Python-grade
ergonomics at the top tier, C-grade hardware control at the bottom tier,
and a GC-free ownership model in between that needs no lifetime
annotations.

## Coming from C

| C | Datara |
|---|---|
| manual malloc/free | ownership + dataflow lifetime inference (frees inserted by the compiler) |
| header parsing by hand | `import c "sqlite3.h" with link("sqlite3.lib");` |
| `volatile uint32_t*` MMIO | `VolatilePtr` + `atomic_fence_seq_cst()` (Level 4) |
| makefiles | `forgen build` (Level 1: single file, no manifest) |

## Coming from Rust

| Rust | Datara |
|---|---|
| `Result<T, E>` + `?` | `Outcome<T>` + `?` / `or fallback` — same shape, no trait machinery |
| lifetime annotations | none: affine ownership is inferred |
| trait objects for dynamic dispatch | `is`-narrowing on NaN-boxed dynamic values (tag test, not vtable) |
| cargo | dpm (merkle-verified packages, Ed25519 signatures) |

## Coming from Python

| Python | Datara |
|---|---|
| interpreter startup, GIL | native binary, real threads + stackful fibers |
| numpy zero-copy via buffer protocol | `py.bind_buffer("arr", list)` — the same zero-copy contract, verified by pointer-identity assert |
| `list` of mixed types | dynamic values with `x is Int` narrowing (tag test) |
| pip | dpm |

## Fibers in one breath

```datara
fn worker(id: Int) { out int_to_str(id * 2) }
fn main() {
    val h = fiber_spawn(worker, 21, 64)
    out int_to_str(fiber_run_until_done())
    out int_to_str(fiber_join(h))
}
```

Multi-threaded scheduler: `fiber_mt_start(n)` — 100k fibers in ~76 ms.
