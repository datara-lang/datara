# stdlib modules written IN Datara (M1 proof)

> PLAN_V150 M1 checkbox: «Часть модулей пишется НА ДАТАРЕ (файл-доказательство,
> что язык тянет реальный код)». This file is that proof. It is enforced by a
> gate — `cargo test --release --test test_stdlib_on_datara` — which asserts
> both the purity of the listed modules and an end-to-end run of a program
> that uses all of them together.

## What "written on Datara" means here

A module is **written on Datara** when its `.dtr` source is ordinary Datara
code — free functions, `struct`/`behavior`, generics, `Outcome`/`or` — that
only calls compiler builtins and other stdlib modules. No `import c`,
no `unsafe`, no hand-written C for the module itself. (The C runtime it
stands on is the substrate every language has; the module layer above it is
pure Datara.)

## The modules (as of 2026-10-01)

| Module | File | Lines | Contents |
|---|---|---|---|
| `stdlib.text.text` | `stdlib/text/text.dtr` | ~130 | checked `text_parse_int/float` → `Outcome`, UTF-8 codepoint utils (`text_char_len`, `text_slice_cp`, `text_char_at_cp`), `text_validate/sanitize`, `text_count_char`, pad utils |
| `stdlib.collections.sort` | `stdlib/collections/sort.dtr` | ~85 | `sort_stable` (merge sort, generic<T>), `sort_unstable`, `is_sorted`, `binary_search` — all Datara-level algorithms |
| `stdlib.collections.iter` | `stdlib/collections/iter.dtr` | ~120 | `Iterator` behavior, `iter_from`, `iter_zip`, `iter_fold<T,A,F>` (left fold with A/F generics) |
| `stdlib.io.path` | `stdlib/io/path.dtr` | ~139 | `path_basename/parent/extension/stem/join`, `path_is_absolute` — pure Datara over `stdlib.text` |
| `stdlib.diagnostics` | `stdlib/diagnostics/diagnostic.dtr` | ~56 | `Diagnostic` struct + behavior: `warning`, `error_at`, `is_error`, `render()` in the compiler's own format |
| `stdlib.io.fs` (guarded layer) | `stdlib/io/fs.dtr` | ~81 | `read/write/append/exists_file_secured(path, cap: SystemCapabilities)` wrappers |
| `stdlib.sys` (wrapper layer) | `stdlib/sys/sys.dtr` | ~78 | `sys_args/env/env_get/env_set/exec` — the `sys_exec` wrapper carries the one `unsafe(justification:)` required by the capability model |

Total: **~690 lines** of shipping stdlib code written in Datara, exercised by
the self-hosted Datara test suite (`stdlib_tests/`, 60+ `@test`s run by
`forgen test stdlib_tests`) and the M1 acceptance demo
(`examples/showcase/ini_parser` — an INI parser built entirely on these
modules, no FFI).

## The gate

```bash
cargo test --release --test test_stdlib_on_datara
```

The gate has two halves:

1. **Purity**: every module listed above must (a) exist and be non-trivial
   (>50 lines), and (b) contain **zero** `import c "`, `unsafe(`, and (for
   the pure core: text/sort/iter/path/diagnostics) zero `extern fn`
   declarations. `stdlib/sys/sys.dtr` is allowed exactly its documented
   `unsafe(justification:)` wrapper for `sys_exec`.
2. **End-to-end**: a probe program that `use`s
   `stdlib.text.text` + `stdlib.collections.sort` + `stdlib.io.path` +
   `stdlib.diagnostics.diagnostic` together is compiled and executed; the
   runtime results (checked parse `or`-fallback, codepoint slice, stable
   sort order, path decomposition, diagnostic render) are asserted. This is
   the "the language pulls real code" claim, verified, not narrated.

Datara-level proofs live in `stdlib_tests/tests/test_*.dtr`
(`forgen test stdlib_tests`).
