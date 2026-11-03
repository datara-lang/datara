# Contributing to Datara & Forgen

## Quick Gate Checklist (every PR must pass)

```bash
cargo fmt --all -- --check     # 0 diffs
cargo clippy --release --all-targets 2>&1 | grep -c "warning: .*generated"
# → 0
cargo test --release           # 0 failed (313+ suites, 1530+ tests)
```

Plus: no .rs file in src/ may exceed 60 KB (size-gate test enforces this).
If you add a stdlib module, put it in `stdlib/<name>/<name>.dtr` with a
doc header, add a @test suite in `stdlib_tests/tests/`, and document it
in `docs/STDLIB.md`.

## How to add a stdlib module

1. Create `stdlib/<name>/<name>.dtr` — doc-comment header, then fns.
2. Every escalated fn needs `/ IO` (or other effects) in the signature.
3. Add test file `stdlib_tests/tests/test_<name>.dtr` with a main() that
   exercises each fn and prints OK/FAIL lines.
4. Register in the resolver (src/resolver/mod.rs) if new builtins are used.
5. Update docs/STDLIB.md with a usage example.

## How to add a test suite

1. Integration: `tests/test_<topic>.rs` using `ForgenCompiler::new("release")`
   + `compile_source_native` + `run_executable`.
2. Datara-level: `stdlib_tests/tests/test_<name>.dtr` (auto-discovered).
3. Both: the Rust test compiles and runs the .dtr; the .dtr tests the stdlib.

## Architecture guide

- `src/parser/` → AST (src/ast/mod.rs)
- `src/resolver/` → symbol tables, name resolution
- `src/types/check/` → type checker (expr.rs = expressions, stmt.rs = statements, call.rs = calls)
- `src/dmir/lowering/` → AST → DMIR (IR)
- `src/optimizer/` → passes (mod.rs = pipeline, const_fold.rs, dce.rs, inline.rs, ...)
- `src/codegen/cranelift/` → CLIF + machine code (backend/compile_func.rs is the main)
- `src/codegen/llvm/` → LLVM IR emission (emit_inst.rs, emit_call.rs)
- `src/codegen/wasm/` → WASM
- `src/runtime/` → C runtime (datara_*.c)
- `src/driver/` → compilation pipeline orchestration

## Key invariants

- **3-backend parity**: Cranelift/JIT/LLVM must produce byte-identical stdout
  (verify: `python tools/parity_matrix.py`).
- **Field order**: struct literal fields are stored ALPHABETICALLY by the
  lowering (expr_composite.rs); Outcome/Maybe are exempt (declared order,
  matching the C ABI).
- **Built-in Outcome ABI**: [0]=is_success, [1]=value, [2]=error_msg
  (datara_runtime.h:290) — field offset resolvers know this.
- **60 KB size gate**: no .rs in src/ may exceed 61,440 bytes.

---

Thank you for your interest in contributing to Datara! We welcome contributions to the compiler, standard library, documentation, and tooling.

---

## Code of Conduct

All contributors and maintainers are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Development Environment Setup

### Prerequisites
- **Rust Toolchain**: 1.85+ stable (`rustup default stable`).
- **C/C++ Build Tools**:
  - Windows: Visual Studio Build Tools (C++ Desktop development)
  - Linux: `build-essential` (Ubuntu/Debian) or `base-devel` (Arch)
  - macOS: `xcode-select --install`

### Getting the Code
```bash
git clone https://github.com/waters1ze/datara.git
cd datara
cargo check
cargo test
```

### Windows: source `scripts/msvc-env.sh` before running the tests

On Windows under Git-Bash, any test that AOT-links a native binary fails with a
message that looks like a compiler bug:

```
Linking failed: status=ExitStatus(1)
linker: ...\PortableGit\...\usr\bin\link.exe
stderr: /usr/bin/link: extra operand '/DEBUG:NONE'
```

It is not a compiler bug. Git-Bash ships a GNU `link` (a coreutils hard-link
utility) that shadows MSVC's `link.exe` on `PATH`, so the linker driver hands
MSVC flags to the wrong program. Prepend the real toolchain:

```bash
source scripts/msvc-env.sh
cargo test --release
```

The script discovers the MSVC and Windows SDK versions via `vswhere`, so it
survives BuildTools updates. It must be **sourced**, not executed. A bare
`cargo test` is not a valid signal on Windows for the suites that link — if you
see the message above, you have not sourced it.

---

## Architecture Guidelines

1. **Compiler Pipeline**:
   - `src/lexer`: Tokenization and span tracking.
   - `src/parser`: Recursive-descent AST construction with error recovery.
   - `src/resolver`: Scope resolution and name binding.
   - `src/types`: Hindley-Milner type inference and effect checking.
   - `src/dmir`: Conversion from AST to SSA intermediate representation.
   - `src/optimizer`: Proof-carrying passes governed by the Evidence Gate.
   - `src/codegen`: Cranelift native machine code lowering and linking.
   - `src/runtime`: Minimal C runtime for I/O, memory, and OS interop.

2. **The Evidence Gate Principle**:
   - Every mutating optimization pass MUST be verifiably provable. Do not add heuristic passes that report success without an intermediate representation structural delta.

3. **Strict Invariants**:
   - Always write regression tests in `tests/` for any bug fix.
   - Code must pass `cargo fmt --all -- --check` and `cargo clippy -- -D warnings`.

---

## Submitting Pull Requests

1. Fork the repository and create a feature branch (`git checkout -b feature/my-feature`).
2. Implement your changes following standard Rust conventions.
3. Verify that all tests pass (`cargo test`).
4. Commit with clear, descriptive commit messages.
5. Push to your fork and submit a Pull Request.
