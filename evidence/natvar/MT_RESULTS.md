# NatVar slice 2: MT scheduler (PLAN_V151 N2)

Windows 11 x64, Ryzen 5 7600, n=11, cmd /c wall-time.

| Workload | Result |
|---|---|
| 10k fibers / 8 workers | 10000/10000 done, all slots correct (Cranelift/JIT/LLVM identical) |
| 100k fibers / 8 workers | median 76 ms, 100000/100000 correct (~755 ns/fiber: spawn + run + verify) |

Correctness pattern: each task writes its own slot — single-writer per cell,
race-free by construction. Cross-worker shared-counter increments are data
races by design (same semantics as Go/Java without synchronization) — atomic
ops and mutexes are the slice-3 surface. Honest caveat: tokio reference NOT
run (no tokio in this environment); echo-10k benchmark requires the IO poller
(slice 3).
