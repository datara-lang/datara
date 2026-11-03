# Datara Language Tour — v1.5 Features

A quick tour of the features added in 1.5.0–1.5.2, with verified examples.

## TSU: Dynamic Values & Type Narrowing

Mixed lists produce dynamically-typed values. The `is` operator narrows:

```datara
fn describe(v: Int) -> Str {
    return fmt"int {v}"
}

fn main() {
    val xs = [42, "hi"]
    val x = xs[0]
    if x is Int {
        out describe(x)  // "int 42"
    }
    val s = xs[1]
    if s is Str {
        out s  // "hi"
    }
}
```

Without narrowing, passing a dynamic value to a static parameter gives
**E-ANY-001** with a fix-it hint.

## Fibers: Stackful Concurrency

Fibers are cooperatively scheduled, microsecond-cheap, and integrate
with a timer wheel and channels.

```datara
fn worker(id: Int) {
    out int_to_str(id * 2)
}

fn main() {
    val h = fiber_spawn(worker, 21, 64)
    val done = fiber_run_until_done()
    out int_to_str(fiber_join(h))  // exit code
}
```

### Channels

```datara
fn producer(ch: Int) {
    fiber_chan_send(ch, 5)
}

fn main() {
    val ch = fiber_chan_create()
    fiber_spawn(producer, ch)
    val v = fiber_chan_recv(ch)
    fiber_run_until_done()
    out int_to_str(v)  // 5
}
```

### Multi-threaded Scheduler

```datara
fn main() {
    fiber_mt_start(8)       // 8 OS workers with work stealing
    // spawn tasks via fiber_mt_spawn(fn, arg)
    fiber_mt_wait_idle()
    out int_to_str(fiber_mt_done_count())
}
```

### Non-blocking IO

`fiber_net_recv` parks the fiber until the socket is readable:

```datara
val res = fiber_net_recv(sock, 4096, 5000)
if res.is_success {
    out fmt"got {res.value}"
}
```

## Atomics: Shared Counters

```datara
val cell = atomic_new(0)
atomic_fetch_add(cell, 1)
val v = atomic_load(cell)
```

8 workers × 1250 fetch_add = exactly 10000 (verified on all 3 backends).

## Outcome & `?`: Deterministic Error Handling

```datara
fn checked(n: Int) -> Outcome<Int8> {
    return n as? Int8
}

fn rethrow(n: Int) -> Outcome<Int8> {
    let ok = checked(n)?
    return Outcome.ok(ok)
}

fn main() {
    out rethrow(100) or -1   // 100
    out rethrow(300) or -1   // -1 (overflow → err → or)
}
```

## Generic Collections: Deque, Set, PriorityQueue

```datara
use stdlib.collections.deque.deque_new

fn main() {
    val dq = deque_new()
    dq.push_back(42)
    val v = dq.pop_back()
    out int_to_str(v)  // 42
}
```

## Testing Framework

```datara
use stdlib.testing.testing_new
use stdlib.testing.test_report

fn main() {
    val t = testing_new()
    t.assert_eq_int(2 + 2, 4, "arith")
    t.assert_true(1 < 2, "lt")
    val fails = test_report(t)  // prints summary, returns failure count
}
```

## Statistics & Random

```datara
use stdlib.math.stats.stats_mean
use stdlib.math.random.random_seed
use stdlib.math.random.random_range

fn main() {
    val d = [1.0, 2.0, 3.0, 4.0, 5.0]
    out float_to_str(stats_mean(d))  // 3.0
    random_seed(42)
    out int_to_str(random_range(1, 100))  // deterministic
}
```

## Toolchain Commands

```bash
forgen migrate 1.4 1.5 .          # modernize 1.4 sources
forgen inspect dmir file.dtr      # dump IR
forgen profile target             # PGO profiling
forgen dpm publish                # publish with Ed25519 signing
forgen bridge check file.dtr      # static bridge audit
forgen check file.dtr             # fast type check
forgen doctor --bridges           # interop diagnostics
```
