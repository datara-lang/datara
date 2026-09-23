# Datara & Forgen Documentation Portal

Welcome to the official documentation for the **Datara** systems programming language and the **Forgen** native AOT compiler toolchain.

---

## Quick Start Path

If you are learning the language, read the documents in this order:

1. **[Official 10-Step Tutorial](TUTORIAL.md)** — hands-on basics: installation, first program, data types, control flow.
2. **[Language Reference Manual](DATARA_LANGUAGE_REFERENCE_MANUAL.md)** — the complete syntax and type system.
3. **[Project & Package Model](DATARA_PROJECT_MODEL.md)** — organizing code with `datara.toml`, modules, and `dpm`.
4. **[Glossary](GLOSSARY.md)** — terminology reference when any concept is unclear.

---

## Documentation Map

### 1. Tutorials & Guides
* **[Official 10-Step Tutorial](TUTORIAL.md)** — Step-by-step hands-on guide from fresh installation to production CLI application.
* **[Unified Technical Glossary](GLOSSARY.md)** — Core terminology, memory models, and formal concept definitions.
* **[Полное руководство по языку Datara (RU)](DATARA_LANGUAGE_GUIDE_RU.md)** — Исчерпывающий академический справочник по синтаксису, парадигмам и архитектуре на русском языке.

### 2. Normative Language References & Specifications (v1.0+)
* **[Language Reference Manual](DATARA_LANGUAGE_REFERENCE_MANUAL.md)** — Complete syntax, type system, affine ownership, pattern matching, traits, and error handling.
* **[Project & Package Model](DATARA_PROJECT_MODEL.md)** — Working with `datara.toml`, `datara.lock`, workspace modules, and `dpm`.
* **[Core vs Standard Library](CORE_VS_STDLIB.md)** — Architectural boundary between compiler builtins and standard library modules (`stdlib.*`).
* **[Normative Specification (v1.0)](SPEC_V1.md)** — Formal definition of syntax rules, memory model, and 13 Evidence Gates.
* **[Conformance Matrix](CONFORMANCE_MATRIX.md)** — Status and validation matrix across all language features.
* **[Complete Technical Specification (v2.0)](DATARA_FORGEN_COMPLETE_TECHNICAL_SPECIFICATION_V2.md)** — Detailed specification of compiler passes, IR, and lowering.

### 3. Architecture & Compiler Guarantees
* **[Current Architecture](CURRENT_ARCHITECTURE.md)** — High-level overview of the Forgen toolchain (Parser, Semantics, DMIR, Cranelift/LLVM backends).
* **[Optimizer Contract](OPTIMIZER_CONTRACT.md)** — Evidence Gate optimizer, Bounds Check Elimination (BCE), and zero-cost abstractions.
* **[Semantic Contract](SEMANTIC_CONTRACT.md)** — Soundness invariants, affine ownership rules, and borrow checking guarantees.
* **[Semantic Invariants](SEMANTIC_INVARIANTS.md)** — Mathematical proofs of safety and determinism.
* **[Semantic Adaptation Engine](SEMANTIC_ADAPTATION_ENGINE.md)** — Cross-platform ABI alignment and context propagation.
* **[LSP & IDE Architecture](DATARA_IDE_LSP_ARCHITECTURE.md)** — Language Server Protocol engine design, completion, and diagnostics.
* **[Shell & REPL Architecture](DATARA_SHELL_ARCHITECTURE.md)** — Interactive evaluator and JIT execution loop.

### 4. Practical Guides & Interop
* **[Game Development with Datara](gamedev.md)** — Data-oriented design (DOP), zero-pause memory management, cache-efficient layouts, and high-frequency loops.
* **[Rust Interop Bridge Guide](rust_bridge.md)** — Calling Rust crates directly from Datara and embedding Datara in existing native workflows.
* **[Sparks Decentralized Package Protocol](sparks.md)** — Specification for Sparks packages, capabilities, and cryptographic signatures.

### 5. Benchmarks & Performance
* **[Performance Benchmarks & Methodology](PERFORMANCE.md)** — Compilation speed, runtime throughput, memory latency, and determinism benchmarks.
* **[Performance Goals](PERFORMANCE_GOALS.md)** — Target metrics and workload baselines the toolchain is measured against.
* **[Interactive Benchmark Showcase](index.html)** — Interactive HTML dashboard with the verified performance matrix.

### 6. Toolchain & Deployment Guides
* **[Cross-Compilation](CROSS_COMPILE.md)** — Building Datara binaries for other operating systems and CPU architectures.
* **[Embedding](EMBEDDING.md)** — Embedding the Forgen compiler and Datara runtime into host applications.
* **[Enterprise](ENTERPRISE.md)** — Adoption guidance, support model, and organizational deployment notes.
* **[Polyglot Interop Guide](POLYGLOT_INTEROP_GUIDE.md)** — Interoperability with C, Python, Rust, and Node.js ecosystems.
* **[GitHub Release Guide](GITHUB_RELEASE_GUIDE.md)** — Maintainer workflow for cutting tagged releases and publishing artifacts.

### 7. Formal Semantics & Development Specs
* **[Formal Language Semantics](FORMAL_LANGUAGE_SEMANTICS.md)** — Formal operational semantics of the core language.
* **[Expand Specification (v1)](EXPAND_SPEC_V1.md)** — Comptime expansion pipeline specification and execution plan.
* **[Roadmap v1.3.0 (Polyglot)](ROADMAP_v1.3.0_POLYGLOT.md)** — Historical roadmap for the polyglot interop milestone.

### 8. Historical & Archival Specifications (v0.1 Archive)
* **[Datara Language Spec v0.1 (Archival)](canonical/DATARA_LANGUAGE_SPEC_v0.1.md)** — Authoritative archival spec for backwards-compatibility auditing.
* **[Forgen Compiler Architecture v0.1 (Archival)](canonical/FORGEN_COMPILER_ARCHITECTURE_v0.1.md)** — Original compiler architecture document.
* **[Datara & Forgen Core Concept v0.1 (Archival)](canonical/DATARA_FORGEN_CONCEPT_v0.1.md)** — The founding concept document.

---

## Repository Reference

Some project-level documents live outside `docs/` and are not part of the published site. Read them in the repository:

* `ROADMAP.md` — Immediate priorities, feature tracking, and long-term milestones.
* `CHANGELOG.md` — Release history and notable changes.
* `CONTRIBUTING.md` — How to contribute to the compiler, standard library, and documentation.

---

## Site Notes

* Pages on this site are published as `.html` (e.g. `TUTORIAL.html`); links written as `TUTORIAL.md` are automatically resolved. The corresponding `.md` sources live in the [`docs/` directory of the repository](https://github.com/datara-lang/datara/tree/main/docs).
* The documentation is built with Jekyll and shares one layout across all pages. To add a new page, drop a `PAGE_NAME.md` file into `docs/` and link it from the map above.
