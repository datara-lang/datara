# bench_vs_rust — honest comparative matrix (PLAN_V153 C1)

Machine: Windows 11 (26200). Date: 2026-11-08.
Datara side: `ForgenCompiler::new("domain")` native AOT (Cranelift), 20 runs
per workload, mean reported. Rust side: release profile (`-O2`), the same
in-process harness (`tests/bench_datara_vs_rust.rs`, `#[ignore]`d from CI).

Command:
```
cargo test --release --test bench_datara_vs_rust -- --ignored --nocapture
```

## Matrix (two consecutive runs, 2026-11-08)

| Workload             | Datara (run1) | Datara (run2) | Rust -O2 | Ratio      |
|----------------------|---------------|---------------|----------|------------|
| Integer Loop 10M     | 3.10 ms       | 3.05 ms       | ~0.10 ms | ~31x       |
| Float Loop 10M       | 4.05 ms       | 3.85 ms       | ~0.10 ms | ~40x       |
| Array Processing 1M  | 1.00 ms       | 0.80 ms       | ~0.10 ms | ~9x        |
| OOP Point SROA 10M   | 3.10 ms       | 3.25 ms       | ~0.10 ms | ~32x       |
| Generic Monomorph 1M | 1.95 ms       | 2.05 ms       | ~0.10 ms | ~20x       |
| String Fmt 200K      | 0.10 ms       | 0.20 ms       | 18-19 ms | **0.01x — Datara wins ~90x** |
| Pipeline Dataflow 5M | 3.10 ms       | 3.05 ms       | ~0.10 ms | ~31x       |

## Honest reading (правило честности)

- **What "Rust ~0.10 ms" means**: the Rust reference implementations are
  auto-vectorized and constant-folded by LLVM (a 10M-iteration sum becomes a
  closed form). The Datara side runs the SAME source as a loop over
  Cranelift-emitted code — this matrix measures the *codegen floor of a
  young compiler against a mature optimizing backend* on arithmetic kernels,
  NOT a like-for-like language gap.
- **Where Datara genuinely wins: the String Formatter** — the workload
  builds 200K strings; Datara's TMM string machinery (scratch-ring +
  amortized in-place concat, no per-string malloc) beats Rust's naive
  `format!` accumulation ~90x. This is a real, structural advantage of the
  dataflow lifetime design for string-heavy workloads.
- **The honest path to parity** on arithmetic kernels is the optimizer
  pipeline (LICM/CSE exist; induction-chain elimination + strength
  reduction would close most of the 10-40x on these exact shapes) —
  tracked as C2 (hotspot profiling).

Verdict for the README claim: Datara is **NOT faster than Rust on arithmetic
loops** (30-40x behind on these micro-kernels due to missing induction
optimization); it IS dramatically faster than naive Rust on string building.
Both statements are honest and measured.

## Repro

```
cargo test --release --test bench_datara_vs_rust -- --ignored --nocapture
```
