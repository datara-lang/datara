# bench_natvar — NatVar fibers vs tokio (PLAN_V152 N1.4)

Machine: Windows 11 (26200), 64-bit. Date: 2026-11-03.
Datara side: `forgen run --jit` (Cranelift JIT host) + C runtime
(`datara_natvar.c`, slice-1 scheduler + select() IO slice).
tokio side: tokio 1.45 (rt-multi-thread, 4 workers), rustc release,
`bench_natvar/tokio_echo/`.

Both sides talk to the **same** tokio echo server binary
(`tokio_echo.exe server 45150`), so the client comparison isolates the
concurrency machinery. Every number below is a real run from 2026-11-03.

## 1. Echo: 1 connection × 10 000 messages × 128 bytes

| Client                | Run 1 | Run 2 | Run 3 | msgs/s (median) |
|-----------------------|-------|-------|-------|-----------------|
| Datara fibers (JIT)   | 364 ms| 367 ms| 368 ms| **27 247**      |
| tokio client (4 thr.) | 391 ms| 390 ms| 387 ms| 25 641          |

Datara wins this shape by ~6 %: each message parks the fiber in the
scheduler's select() slice and wakes it on the echo; the wake path is a
single Windows fiber switch + `select()` return. tokio's `write_all/
read` round trip goes through waker registration and its ready queue on
every operation.

Fairness note: both clients are separate processes against the same
server; the scheduler is single-threaded while tokio uses 4 workers —
on a pure client-side ping-pong the extra workers only add contention,
which is exactly the shape this benchmark measures.

**p99 latency: not reported.** `now_ms()` has millisecond resolution;
per-message loopback RTT is below 0.1 ms, so a p99 would be fake
precision. Throughput and total wall time are the honest metrics here.

## 2. 100 000 sleeping fibers/tasks (20 ms sleep each)

| Implementation        | Spawn   | Drain (total)      | Peak memory |
|-----------------------|---------|--------------------|-------------|
| Datara fibers (JIT)   | 324 ms  | 5 257–5 538 ms     | 547,8 MB    |
| tokio tasks           | —       | **59–65 ms**       | **44 MB**   |

**tokio wins by ~80× time and ~12× memory. ЧТО и ПОЧЕМУ (правило
честности):**

1. **OS-backed stacks.** A Datara fiber is a real Windows fiber
   (`CreateFiberEx`, 8 KB reserve + 16 KB commit) / POSIX ucontext.
   100 000 spawns allocate 100 000 committed stacks up front — that is
   the 324 ms spawn and most of the 548 MB. A tokio task is a lazy
   heap state machine with no stack until it runs.
2. **Round-robin timer drain.** The slice-1 scheduler pushes every
   ready fiber through a FIFO queue and re-checks sleeping fibers on
   every pass; with 100 000 sleepers the drain is O(sleepers) per
   scheduler pass over the whole 20 ms window. tokio uses a timer
   wheel: O(1) insert, the task is not touched until its deadline
   fires.

**When Datara fibers are the right tool:** hundreds of fibers, deep
call stacks, blocking-style code with `fiber_net_recv` parks (echo
shape above), full dataflow lifetime tracking per fiber. **When tokio
wins:** hundred-thousands of trivial waiters — use MT scheduler slice
(datara_natvar_mt) workloads or keep per-connection fibers below
~10⁴. This is the documented trade-off, not a bug.

## 3. Known limitations (honesty list)

- **Concurrent parked sockets cap: 256** (`NATVAR_IO_MAX`), and on
  Windows `select()`'s `fd_set` is FD_SETSIZE = 64 — so the "10 000
  concurrent echo connections" variant of the benchmark is **not
  supported** by the current IO slice; it would corrupt the fd_set.
  Fix path (1.5.3+): chunked select() over ≤64-fd groups, or
  WSAEventSelect/IOCP slice. The honest benchmark therefore measures
  1 connection × 10 000 messages.
- **Clock resolution:** ms (see p99 note above).
- **Single-threaded scheduler** on the JIT host; the MT slice
  (datara_natvar_mt) is exercised by tests/test_atomics but not by
  this benchmark.

## Reproduce

```powershell
cd bench_natvar/tokio_echo; cargo build --release --offline
Start-Process .\target\release\tokio_echo.exe -ArgumentList 'server','45150'
# from repo root:
.\target\release\forgen.exe run bench_natvar/echo_fiber.dtr       # Datara client
.\bench_natvar\tokio_echo\target\release\tokio_echo.exe client 45150 10000
.\target\release\forgen.exe run bench_natvar/sleep_fibers.dtr     # 100k fibers
.\bench_natvar\tokio_echo\target\release\tokio_echo.exe sleep 100000 20
```

Peak memory for the Datara run was sampled with PowerShell
`PeakWorkingSet64` polling (100 ms interval over a 5.3 s run); the
tokio run self-reports the same counter via `K32GetProcessMemoryInfo`
(its 59 ms lifetime is shorter than a poll interval).
