# Python Bridge Hot-Path Audit — py.bind_buffer (PLAN_V151 N3)

Date: 2026-10-31 · Machine: Windows 11 x64 (Ryzen znver4 host) ·
Runner: `forgen run evidence/bench_python/bind_bench.dtr` (Cranelift tier)

## What was audited

`py.bind_buffer(name, list)` → runtime `datara_py_export_memview`
(src/runtime/datara_py.c). The bind path creates a **zero-copy
`PyMemoryView_FromMemory`** directly over the Datara `List<Float>`
allocation — there is **no memcpy branch anywhere in the bind path**, so a
copy counter would count nothing; the runtime proof of zero-copy is the
pointer-identity assert (`datara_py_assert_same_ptr`) plus bidirectional
mutation visibility (a copied buffer cannot propagate writes in-place).

## Results (10 MB List<Float> = 1,250,000 × f64)

| Check | Result |
|---|---|
| bind_buffer status | OK |
| pointer identity (`datara_py_assert_same_ptr`) | **1 — same allocation** |
| numpy `np.frombuffer(memoryview(...))` shape | (1250000,) — no copy |
| numpy sum over the aliased buffer | 390,624,687,500 (exact f64 sum) |
| Datara→Python mutation (`buf[0]=4242.0`) | Python sees 4242 — **FWD_OK** |
| Python→Datara mutation (`np_arr[7]=777.0`) | Datara sees 777 — **REV_OK** |
| copies observed | **0** (by construction + pointer proof) |

## Timings (now_precise_ms — NOTE: returns microseconds despite the name)

| Phase | Time |
|---|---|
| First (cold) bind — includes first `__main__` dict access | ~83 ms |
| Cached rebind ×100 → per-rebind | ~215 µs |
| Python-side numpy sum over 10 MB (frombuffer + sum) | ~0.96 ms (~10.4 GB/s) |

Honest notes (правило честности):

1. **The "sub-500 ns repeat bind" claim in the shadow-pin comments is NOT
   reproduced by this audit** — the measured rebind through the public
   `py.bind_buffer` path is ~215 µs, i.e. the full
   GIL-ensure → FromMemory → setattr path runs every time. The shadow-pin
   cache (datara_py.c:1222) either is not engaged by this path or its hit
   condition differs. Follow-up: profile `datara_py_export_memview` and
   make the cached handle the fast path, or document the real cost.
2. `datara_rt_now_precise_ms` returns **microseconds**, not milliseconds
   (QPC × 1e6 / freq) — the bench labels carry this caveat; renaming is
   deferred to the ABI-freeze window (docs/abi.md, N5).
3. First-bind cost is dominated by Python-side first-touch work (module
   dict access), not by data movement — no 10 MB memcpy appears anywhere.

## Bonus fix surfaced by the audit (same session)

- A no-op `Float→Float` cast (e.g. `(np_ms as Float) * 1000.0`) crashed
  Cranelift with an ISLE x64_lea panic: the identity path in the Cast arm
  emitted `iadd` on an f64 carrier. The identity paths (same-width i→i and
  the fallback family) now preserve the operand's actual carrier
  (fadd on f64/f32, iadd on i64) — cranelift/backend/compile_func.rs;
  the LLVM emitter already keyed off value_types.
