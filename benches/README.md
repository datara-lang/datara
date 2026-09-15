# Datara / Forgen Benchmarks

The `.dtr` programs here are run by `forgen bench` (auto-discovered); the
single Criterion harness (`compiler_phases.rs`, `cargo bench`) profiles the
compiler pipeline itself.

## Compiler pipeline (cargo bench)

- `compiler_phases.rs` - per-phase timings (lexer, parser, typecheck,
  ownership, Cranelift codegen, WASM emission) over a fixed sample program.

## Micro (b-series)

- `b01_arith_int.dtr` - 50M-iteration integer accumulate loop.
- `b02_arith_f64.dtr` - 50M-iteration f64 accumulate loop.
- `b03_mem_copy.dtr` - 1M-element list copy.
- `b04_str_build.dtr` - 20k-iteration string concatenation.
- `b05_branch_mix.dtr` - hot/cold branch mix in a loop.
- `b06_fn_calls.dtr` - small-function call overhead (`add(a, b)`).
- `b07_class_fields.dtr` - `Point { x, y }` field access cost.
- `b08_map_ops.dtr` - 200k map insert/lookup operations.
- `b09_sort_small.dtr` - 2000 rounds of small-array sorting.
- `b10_loop_dep_chain.dtr` - 100M-iteration loop-carried dependency chain.

## Macro (m-series)

- `m01_matmul_96.dtr` - 96x96 dense matrix multiply.
- `m02_primes_100k.dtr` - sieve for primes up to 100k.
- `m03_string_parse.dtr` - 50k string builds plus `.len()`.
- `m04_json_roundtrip.dtr` - 20k map/JSON round trips.
