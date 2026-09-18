# Forgen Optimizer Pass Contract

Every optimization pass in the Forgen compiler core must adhere to this formal engineering contract.
Forgen employs a strict **Evidence Gate** (`evidence.rs`): no optimization is recorded as `Applied` without a verified structural IR fingerprint delta.

---

## 1. Optimizer Architecture & Subsystems

The Forgen optimizer operates on Datara Mid-level Intermediate Representation (DMIR) across five core subsystems:

### 1.1 Inlining & Interprocedural Optimization (IPO)
- **Pass 1: Pure Function Inlining (`inline::inline_pure_functions`)**: Inlines single-block, pure (`Effect::Pure`), non-recursive leaf functions within cost budget.
- **Pass 2: Attribute-Driven Inlining (`inline_attr`)**: Strictly honors `#[inline(always)]` while rejecting `#[inline(never)]`.
- **Pass 3: Devirtualization (`ipo::devirtualize`)**: Replaces dynamic behavior/trait dispatch with direct static calls when a single target implementation is proven.
- **Pass 4: Constant Argument Specialization (`ipo::specialize_constant_args`)**: Clones and specializes functions called with invariant constants.
- **Pass 5: Dead Clone & Symbol Stripping (`ipo::dead_clone_elimination`, `dce::dead_symbol_elimination`)**: Prunes unused function specializations and unreachable code from the module call graph.

### 1.2 Scalar & Aggregate Transforms
- **Pass 6: SROA Stack Scalarization (`scalar::scalarize_structures`)**: Deconstructs non-escaping local structures into scalar SSA values, eliminating stack allocations.
- **Pass 7: Scalar Replacement of Aggregates (`sra`)**: Replaces aggregate loads and stores with register-allocated values.
- **Pass 8: Mem2Reg (`mem2reg`)**: Promotes memory-based local variables (`LoadVar`/`AssignVar`) into minimal SSA form.
- **Pass 9: Constant Folding & Propagation (`const_fold`)**: Evaluates constant arithmetic, boolean logic, and pure string operations at compile time.
- **Pass 10: Comptime Evaluation (`comptime_eval`)**: Evaluates `comptime { ... }` blocks and propagates compile-time values.

### 1.3 Loop Optimizations & Polyhedral Engine
- **Pass 11: Loop Invariant Code Motion (LICM, `loops::licm`)**: Hoists loop-invariant computations out of loop headers when proven pure.
- **Pass 12: Loop Folding (`loops::fold`)**: Replaces constant-strided arithmetic loops with closed-form mathematical equations ($O(1)$ evaluation).
- **Pass 13: Bounds Check Elimination (`loops::bce`)**: Statically proves array indices within bounds using dominance analysis, eliding redundant bounds checks.
- **Pass 14: Loop Unrolling (`loops::unroll`)**: Completely or partially unrolls small, countable loops with known trip counts.
- **Pass 15: Polyhedral Loop Nest Analysis (`polyhedral`)**: Performs loop interchange, skewing, and cache tiling for multi-dimensional data access patterns.
- **Pass 16: Induction Variable Overflow Check Elision (`loops::ovf_elide`)**: Elides runtime overflow checks on induction variables with proven safety bounds.

### 1.4 Vectorization & Memory Subsystems
- **Pass 17: Superword-Level Parallelism (`slp`)**: Combines adjacent isomorphic scalar operations into native SIMD vector instructions (AVX2/NEON).
- **Pass 18: Stream & Pipeline Fusion (`pipeline_fusion`)**: Fuses chained iterator/collection transforms (`map |> filter`) to avoid intermediate allocations.
- **Pass 19: Escape Analysis & Stack Promotion (`escape`)**: Promotes heap-bound objects to thread-local or stack storage when pointers do not escape the function scope.
- **Pass 20: Tail Call Optimization (`recursion::tco`)**: Converts self-recursive tail calls into direct jumps, guaranteeing $O(1)$ stack usage.
- **Pass 21: Dead Code Elimination (`dce`)**: Recursively eliminates dead SSA instructions with zero uses and no side effects.

---

## 2. The Optimizer Evidence Gate (`evidence.rs`)

### 2.1 The Principle
A line in a log or optimization trace does NOT prove an optimization took place. Every mutating pass must produce a verifiable structural transformation.

### 2.2 Invariant Verification Protocol
1. **Pre-Pass Fingerprint**: Prior to running any pass, `evidence::ir_fingerprint(&module)` computes a deterministic 64-bit structural hash of all functions, basic blocks, SSA parameters, instructions, and terminators.
2. **Execution**: The pass executes and appends candidate decision records (`Applied`, `Candidate`, `Rejected`).
3. **Post-Pass Fingerprint**: The post-pass hash is computed. If the pre-pass and post-pass fingerprints are identical (`pre_hash == post_hash`):
   - Every `Applied` decision emitted during that pass is automatically downgraded to `Rejected`.
   - The reason is annotated: `"[downgraded: pass reported Applied but IR is unchanged]"`.
   - Optimization counters and speedup assertions are rolled back.
4. **Zero-Trust Audit**: The compiler emits honest metrics in the optimization report, guaranteeing zero false-positive claims.
