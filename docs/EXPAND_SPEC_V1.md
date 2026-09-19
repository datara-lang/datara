# Datara `expand` — Production Specification (v1.4.4) + Master Execution Plan

**Status:** NORMATIVE — approved for implementation in 1.4.4
**Path:** `d:\DATARA\datara + forgen\docs\EXPAND_SPEC_V1.md` — единственный источник правды проекта.
**Execution:** **ЧАСТЬ III (§29–§33) — план доведения проекта до production-ready.** Исполнение идёт строго
по воркспакам WP-0…WP-9, каждый со своим гейтом. Прежде чем писать код — прочитай §29 (что уже
сделано / что нет) и §30 (правила исполнения). Не начинай WP-N+1 пока гейт WP-N зелёный.
**Порядок чтения (важно, план физически разорван по файлу):**
`§29 → §30 → §31 (WP-0…WP-3) → §27–§28 (приложение Части II, контекст) → §31 (продолжение, WP-4…WP-9) → §32 (готовые промпты) → §33 (Definition of Done) → **ЧАСТЬ IV: §34 → §35 → §36 (WP-10: EMB-0…EMB-9) → §37 (нововведения E1–E8) → §38–§43 → §44 (roadmap к ядру)**` → маркер `<!-- END OF SPEC -->` (последняя строка файла).
**Scope:** core language (`src/`), all three backends (Cranelift JIT/AOT, LLVM, WASM), tooling (fmt/lint/LSP/doc)
**Supersedes:** nothing (new). **Migrates:** `@derive(...)` becomes a built-in `expand` library in 1.5
**Not an MVP:** this document specifies the complete, stable, cross-level, cross-backend feature. There is no
"phase 2" that rewrites it. 1.4.4 ships the full surface described here, with the full test matrix in §14.

---

## 0. Executive summary

`expand` is Datara's metaprogramming primitive. It is **not** a Rust-style macro:

| | Rust macro (`macro_rules!` / `proc_macro`) | Datara `expand` |
|---|---|---|
| Input | `TokenStream` (untyped text) | `resolver` symbol table (typed, layout-known) |
| Output | `TokenStream` (unchecked) | **the same `ast::Decl` / `Stmt` / `Expr` types the parser produces** |
| Type errors | surface as whole-program macro-expansion noise | surface as ordinary `E-TYPE-*` at the expand site |
| Hygiene | `SyntaxContext` machine, `$crate`, `concat_idents!` | no call-site scope access at all + deterministic mangling |
| Backends | macro output is opaque in MIR, `dyn Display` survives | generated AST lowers through the standard DMIR path, optimized by SRA/IPO/LoopFold/devirt |
| Proof | none | Evidence Gate pass `expand` with before/after fingerprint |
| Sandbox | `proc_macro` can read files/network | pure-only `EffectSet::Pure`, no `unsafe`, step/emit/instantiation caps |

One sentence: **`expand` does not generate text, it generates type-checked Datara AST from typed Datara facts,
and that AST is then compiled by exactly the same pipeline as handwritten code.**

The decisive architectural choice in this spec: **AST-as-IR**. `expand` does not emit a private IR that later
phases must learn about. It emits `ast::MethodDecl`, `ast::ClassDecl`, `ast::FieldDecl`, `ast::Stmt`,
`ast::Expr` — the very types `src/parser/*` produces. Consequences:

1. Zero new codegen paths. Cranelift, LLVM and WASM need **no changes** to compile expanded code.
2. Zero new optimizer paths. SRA, LoopFold, IPO devirtualization, adaptive SoA and the Evidence Gate apply
   to expanded methods because they are ordinary methods.
3. Zero new checker paths. Generated code is type-checked by `src/types/check/*` unchanged.
4. `forgen doc`, `forgen fmt`, `forgen lint`, LSP completion and go-to-definition work with no special cases.

---

## 1. Why this is the right primitive for Datara

1. **The five entities already carry facts** (`struct` layout, `component` POD-ness, `behavior` purity,
   `role` capabilities, `trait` bounds). `expand` is the *consumer* of those facts: it can read `T.fields`
   with real `TypeNode`s, `T.kind` (`SymbolKind::Class | Component | Role | Trait`), `T.methods`, and
   `T.generic_params` directly from `resolver`. This is typed reflection that Rust does not have.
2. **Fail-closed is preserved.** Everything `expand` produces is checked. Nothing can bypass `E0941`,
   `E0943`, `E0310`, `E-ROLE-001`, `E-CAP-001`, `E-STRUCT-001`, `E-COMP-001`, `E-BEH-001`.
3. **Determinism is preserved.** Expansion is a pure function of the resolver table; iteration order is
   sorted; the result is a stable fingerprint. Two runs of `forgen build` produce byte-identical artifacts.
4. **The JIT/AOT gap must be closed as part of this work** (§10.4) because expand-generated methods are the
   first real users of the full runtime surface. That gap is a release blocker for 1.4.4, not a follow-up.

---

## 2. Inviolable invariants (numbered, testable)

| # | Invariant | Enforcement | Test |
|---|---|---|---|
| I1 | `expand` input is typed: only `resolver` facts, never raw tokens or source text | expand evaluator has no lexer/parser handles | §14.1 |
| I2 | `expand` output is ordinary AST; no expand-specific IR type exists | emitted values are `ast::*` structs | §14.2 |
| I3 | Emitted code passes the standard checker; expand cannot emit anything unchecked | no `unchecked` flag exists in AST | §14.3 |
| I4 | `expand` body is `Pure`; no IO/Net/FS/Proc/FFI effects | `EffectSet` check → `E-EXPAND-006` | §14.4 |
| I5 | `expand` cannot access the call site's local scope | evaluator receives only its own `Scope` | §14.5 |
| I6 | Generated bindings are collision-free | deterministic mangling + resolver uniqueness check → `E-EXPAND-008` | §14.5 |
| I7 | Expansion is deterministic and order-independent | sorted iteration, no `now/random/env`; fingerprint in `ExpansionReport` | §14.6 |
| I8 | Resource bounded: recursion ≤ 32, steps ≤ 200_000, emitted nodes ≤ 8_192, instantiations ≤ 4_096 | counters → `E-EXPAND-004`, `E-EXPAND-007` | §14.7 |
| I9 | Cannot redefine an existing symbol; cannot emit `main`/`extern`/`unsafe` | resolver collision check → `E-EXPAND-003`; parser-level forbidden emit → `E-EXPAND-002` | §14.3 |
| I10 | Cross-backend parity: identical program semantics on Cranelift/LLVM/WASM | differential golden tests | §14.8 |
| I11 | Capability audit includes generated code | `collect_transitive_calls` over final DMIR → `E-CAP-001` | §14.9 |
| I12 | Every `datara_rt_*` symbol reachable from expanded code is JIT-registered or AOT-only with a warning | symbol manifest test → `E-EXPAND-009` | §14.10 |

---

## 3. Surface syntax

### 3.1 Level 2 — usage (no knowledge of `expand` required)

A Level-2 programmer never writes `expand`. Methods simply exist:

```datara
struct User {
    id: Int
    name: Str
}

// The method below is produced by the built-in `expand ToJson` library.
let u = User { id: 7, name: "ada" }
out u.to_json()              // {"id":7,"name":"ada"}
out fmt"hash={u.hash()}"     // hash=...
```

Cognitive load: zero. There is no `#[derive(...)]` string, no attribute to remember, no import to add.
If the method does not exist and no `expand` provides it, the error is the ordinary
`E-TYPE-001 unknown method 'to_json' on 'User'` **plus a machine-readable suggestion**:

```json
{"code":"E-TYPE-001","message":"unknown method 'to_json' on 'User'",
 "help":"add '@expand(ToJson)' to struct User, or call expand ToJson<User>() explicitly"}
```

### 3.2 Level 3 — authoring

```datara
expand ToJson<T: Serializable> {
    emit method {
        name: "to_json"
        params: []
        ret: Str
        body: {
            mut sb = StrBuf { }
            sb.push("{")
            for f in T.fields {
                if f.index > 0 { sb.push(",") }
                sb.push(fmt"\"{f.name}\":")
                sb.push(json_encode(this.{f.name}))
            }
            sb.push("}")
            return sb.join()
        }
    }
}
```

Reading the syntax against `src/`:

* `expand ToJson<T: Serializable>` — declaration; `<...>` bounds are checked against `resolver` traits
  exactly like `FunctionDecl.generic_constraints` (`ast/mod.rs:489`).
* `emit method { ... }` — an **AST literal**. Field names inside are the field names of `ast::MethodDecl`
  (`ast/mod.rs:581`): `name`, `attributes`, `generic_params`, `params`, `return_type`, `requires`,
  `ensures`, `decreases`, `body`, `is_expression_body`, `is_replaces`, `replaces_target`.
  `ret:` is accepted as sugar for `return_type:`; `params: []` is the empty `Vec<Param>`.
* `body: { ... }` — a normal `Stmt::Block`. Every statement inside is an ordinary Datara statement, parsed
  by the **same** `parse_stmt` (`src/parser/stmt.rs`). There is no second grammar.
* `T.fields` — typed reflection (§8). Each `f` has `name: Str`, `index: Int`, `type_name: Str`,
  `kind: FieldKind`, and `get(self)`/`set(self, v)` accessors used as `this.{f.name}`.
* `this.{f.name}` — dynamic member access resolved at expansion time to a static
  `Expr::MemberAccess { object: this, member: <literal field name> }`. No runtime reflection.
* `json_encode(x)` — ordinary function; must be resolvable or `E-EXPAND-002`.

### 3.3 Attribute form (eager instantiation)

```datara
@expand(ToJson, Hash)
struct User {
    id: Int
    name: Str
}
```

`@expand(...)` on a `struct`/`component`/`enum` requests eager instantiation for that type. Equivalent to
calling each named expand with that type. `@derive(...)` remains accepted in 1.4.4 (see §16).

### 3.4 Explicit instantiation expression

```datara
expand ToJson<User>()        // Value of type ExpandSet; valid as a statement or as a program-level decl
```

Used in `datara.toml`-less projects and in tests. Result type `ExpandSet` is a compiler-internal type that
exists only during expansion; using it at runtime is `E-EXPAND-010`.

### 3.5 Grammar (normative, EBNF in repo style)

```
expand_decl      ::= attrs? "expand" IDENT generic_params? ("for" type)? expand_body
generic_params   ::= "<" generic_param ("," generic_param)* ">"
generic_param    ::= IDENT (":" IDENT ("+" IDENT)*)?
expand_body      ::= block | "=>" expr
block            ::= "{" expand_stmt* "}"
expand_stmt      ::= "emit" expand_node
                   | "let" IDENT ("=" expr)?          // immutable, expand-local
                   | "mut" IDENT "=" expr             // mutable, expand-local
                   | IDENT "=" expr
                   | "for" IDENT "in" expr block
                   | "if" expr block ("else" block)?
                   | "return" expr?
                   | "out" expr | "err" expr          // compiler-log channel only (§8.6)
expand_node      ::= "method" node_lit | "field" node_lit | "struct" node_lit
                   | "component" node_lit | "behavior" node_lit
                   | "enum" node_lit | "trait" node_lit | "impl" node_lit
                   | "const" node_lit | "type" node_lit
node_lit         ::= "{" (IDENT ":" node_value (","|NL)*)* "}"
node_value       ::= expr | block | "[" (node_value (","|NL)*)? "]"
```

Notes binding the grammar to the parser:

* `node_lit` is parsed by a dedicated `parse_node_literal` that accepts an ordered field list whose keys
  are validated against the target `ast::*` struct. Unknown key → `E-EXPAND-002` with a did-you-mean list
  built from the struct's field names (the checker already has that metadata).
* `block` inside a `node_value` position is parsed as `Stmt::Block`; `[]` as an empty `Vec`.
* `expand` is a **contextual keyword**: it is added to `TokenType` (`src/lexer/tokens.rs:14-79` block) and
  to the keyword table (`src/lexer/mod.rs:851-914`). It is reserved only at top level, so existing code
  using `expand` as an identifier in expression position still parses — the parser only matches
  `TokenType::Expand` in `parse_declaration` (`src/parser/decl.rs:166-179` dispatch block).
* Trailing commas and newlines are insignificant inside `node_lit`, matching map-literal conventions.

---

## 4. AST structures (normative Rust — `src/ast/mod.rs`)

Everything below is added to `src/ast/mod.rs`. Existing types are **not** changed except for two new `Decl`
variants (and their `span()` arms at `ast/mod.rs:317-338`).

### 4.1 New `Decl` variants

```rust
pub enum Decl {
    Use(UseDecl),
    CImport(CImportDecl),
    Class(ClassDecl),
    Enum(EnumDecl),
    Behavior(BehaviorDecl),
    Component(ComponentDecl),
    Role(RoleDecl),
    Function(FunctionDecl),
    Flow(FunctionDecl),
    Task(FunctionDecl),
    Packet(PacketDecl),
    ExternFn(ExternFnDecl),
    Type(TypeDecl),
    Register(RegisterDecl),
    Trait(TraitDef),
    Impl(ImplBlock),
    Bridge(BridgeDecl),
    Global(GlobalDecl),
    Expand(ExpandDecl),                       // NEW §4.2
    ExpandInstantiate(ExpandInstantiateDecl), // NEW §4.5
}

impl Decl {
    pub fn span(&self) -> &SourceSpan {
        match self {
            // ... existing arms unchanged ...
            Decl::Expand(d) => &d.span,
            Decl::ExpandInstantiate(d) => &d.span,
        }
    }
}
```

`Expand` is appended, not inserted: `Decl` is `#[derive(Serialize, Deserialize)]` and the check cache
(`src/driver/check_cache.rs`) round-trips these payloads, so inserting mid-enum would invalidate every
cached artifact. Appending keeps 1.4.4 compatible with 1.4.3 cache payloads.

### 4.2 `ExpandDecl`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandDecl {
    pub name: String,
    pub attributes: Vec<Attribute>,
    /// `T: Bound + Bound` — same shape as `FunctionDecl.generic_constraints` (ast/mod.rs:489).
    pub generic_params: Vec<(String, Vec<String>)>,
    /// Optional `for <Type>` target restriction (§5.4).
    pub target: Option<TypeNode>,
    pub body: ExpandBody,
    pub is_export: bool,
    pub span: SourceSpan,
    /// Span of the body only: used for `E-EXPAND-*` attribution and `forgen why`.
    pub body_span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpandBody {
    Block(Vec<ExpandStmt>, SourceSpan),
    /// `expand X<T> => emit method { ... }`
    Expr(Box<ExpandStmt>, SourceSpan),
}
```

`ExpandBody::Expr` mirrors `FunctionDecl.is_expression_body` (`ast/mod.rs:496`) so one-line expansions are
idiomatic, not a special case.

### 4.3 `ExpandStmt`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpandStmt {
    /// `emit <node>` — contributes a declaration to the program.
    Emit(ExpandNode, SourceSpan),
    /// `let x = expr`
    Let { name: String, init: Expr, span: SourceSpan },
    /// `mut x = expr`
    Mut { name: String, init: Expr, span: SourceSpan },
    /// `x = expr`
    Assign { target: Expr, value: Expr, span: SourceSpan },
    /// `for x in expr { ... }`
    For { var_name: String, iterable: Expr, body: Vec<ExpandStmt>, span: SourceSpan },
    /// `if c { } else { }`
    If {
        condition: Expr,
        then_body: Vec<ExpandStmt>,
        else_body: Option<Vec<ExpandStmt>>,
        span: SourceSpan,
    },
    Return(Option<Expr>, SourceSpan),
    /// `out expr` — expansion log channel (compiler stdout), §8.6
    Out(Expr, SourceSpan),
    /// `err expr` — expansion log channel, error severity, §8.6
    Err(Expr, SourceSpan),
    Block(Vec<ExpandStmt>, SourceSpan),
}
```

**Reflection is deliberately not a new expression kind.** `T.fields` parses as the existing
`Expr::MemberAccess { object: Identifier("T"), member: "fields" }`. The evaluator intercepts
`<bound-generic>.<reflection-member>` before ordinary evaluation (§8.2). Consequences:

* no new `Expr` variant → no serde churn, no parser edits in `src/parser/expr.rs`;
* the same syntax is reusable later by `comptime` without a second design;
* if the receiver is not a bound generic, `.fields` stays an ordinary member access and the normal
  `E-TYPE-001` path applies — no silent special casing.

<!-- CHUNK-C1-END -->

---

## 5. Expansion semantics

### 5.1 What "expansion" is, formally

Let `P` be a program after `resolve` and let `E = {E₁…Eₙ}` be its `Decl::Expand` items.
Expansion is a **pure function**:

```
Φ : (Program, Resolver) → (Program, ExpandReport)
```

`Φ` is applied once per instantiation request. A *request* is one of:

| Request source | Syntax | Priority |
| --- | --- | --- |
| Attribute | `@expand(ToJson, Hash)` on `struct/component/enum` | eager, source order |
| Program declaration | `expand ToJson<User>()` at top level | eager, source order |
| Method miss fallback | `u.to_json()` with no such method | lazy, first-use order |

Eager requests are processed in **source order**; lazy requests are processed after all eager requests,
in **first-use order** determined by a deterministic walk (`Decl` order → `Stmt` order → `Expr` post-order).
Ordering is fully specified because it is observable in the generated method set and therefore in
`fingerprint` (§11.2). See invariant I6.

### 5.2 Instantiation

Instantiation of `ExpandDecl X` with type `T` runs the evaluator (§6) with a frame:

```
Frame {
    expand:  X,
    binding: { X.generic_params[0] -> T },     // single generic for 1.4.4
    sink:    Vec<Decl>,                        // emitted nodes
    scope:   Vec<HashMap<SymbolId, Value>>,    // expand-local let/mut
    steps:   0, emits: 0,
}
```

While the frame is active, `T` is a **bound generic**: any `<bound-generic>.<reflection-member>` access
resolves against the reflection table (§8) instead of ordinary member lookup.

On success, the sink is:
1. type-checked as ordinary AST (§9.4),
2. mangled (§11.3),
3. appended to the program.

On failure, **nothing** is appended and the diagnostic points at the `expand` body span, never at the
call site (contrast: Rust points at the call site with expansion noise). Invariant I2.

### 5.3 Generic bounds

`expand ToJson<T: Serializable>` requires, at instantiation:

* `T` is a nominal type known to `resolver` (`Class | Component | Enum | Role | Trait` — `SymbolKind`),
* for each bound `B`: `resolver` records `T` as implementing `B`, **or** `B` is one of the structural
  built-ins (§8.5) which are checked directly against layout/entity kind.

Failure → `E-EXPAND-002` with the exact missing bound and the list of implemented traits.
No bound is ever silently dropped. Invariant I7.

### 5.4 Target restriction — `expand X for Type`

`expand Deref for Vec3 { ... }` restricts an expand to one nominal type. This is the mechanism that keeps
the global method-miss index tight: `for`-restricted expands are only considered for their target type and
therefore cost nothing for every other type in the program. Unrestricted expands are considered only when
their bound names a trait the type implements, i.e. there is always a syntactic reason to consider them.
Invariant I7 (no unbounded search).
### 5.5 Duplicate and coherence rules

Generated methods live in a **separate namespace tier** until lowered. Priority (identical to §6 of the
five-entity design):

```
handwritten impl  >  handwritten behavior  >  handwritten struct method  >  expand-generated
```

Rules:

1. An expand-generated method that collides with a *handwritten* method of the same name on the same type
   is **discarded silently** — handwritten always wins. This makes `@expand(ToJson)` and a manual
   `to_json` composable without error. The `ExpandReport` records `shadowed: true` so `forgen why` can
   explain it. Invariant I5.
2. Two expands generating the same method for the same type → `E-EXPAND-006` naming both expand decls
   (not an opaque conflict). Deterministic: whichever sorts first is reported as "first".
3. `replaces` inside an expand body follows the existing `MethodDecl.is_replaces` rules
   (`src/resolver`), including recomputation of `replaces_target`. An expand may therefore intentionally
   override a handwritten method only by saying so — never by accident.

### 5.6 Recursion, self-reference and data-driven expansion

An expand body may `emit` a type that is itself annotated `@expand`, and that inner request is processed
after the current frame completes (breadth-first over the request queue). Depth is capped by
`EXPAND_MAX_DEPTH = 8`; exceeding it is `E-EXPAND-005`. Rationale: a cycle in expansion requests is a
programming error, not a feature, and must be reported with the cycle path, not by hanging.

### 5.7 Determinism obligations

For a fixed input program, `Φ` must be **byte-identical** across runs, machines and backends. Therefore:

* all reflection iteration is over **sorted** collections (`Vec` sorted by `SymbolId`, not `HashMap` order);
* mangle names derive from `SymbolId` and a stable counter, never from pointer or hash iteration order;
* generated `span`s are the `expand` body spans (stable), and generated code carries
  `Attribute { name: "origin", args: ["expand", "<expand name>", "<type name>"] }` so tooling can
  attribute every generated line without relying on spans that do not exist in source;
* `ExpandReport` is serialized with `BTreeMap`, matching `check_cache` conventions.

Invariant I6, tested by §14.11 (`expand determinism`: 50 runs, one artifact hash).

### 5.8 Interaction with the five entities

| Host entity | Expand may add | Expand may never add |
| --- | --- | --- |
| `struct` | `Method`, `Field` (via `emit field`), `Invariant` | `Using`, non-POD field on a `component` |
| `component` | `Field` (POD only), `Invariant` | `Method` — `E-COMP-001` applies to generated code too |
| `behavior`-targeted | `Method` (must be `Pure`) | `Field`, `Using` — `E-BEH-001` |
| `enum` | `Method`, `Impl` | variants |
| `trait` | `Impl` for a type, `default_body` only via `emit impl` | trait method signatures of another trait |
| `role` | nothing generated (roles carry capabilities, not bodies) | any body — `E-ROLE-001` |

Generated code is subject to the **same** entity rules as handwritten code. There is no "macro escape
hatch": if `expand` tries to put a method into a `component`, it fails exactly like a human would.
---

## 6. Evaluation model (the expand interpreter)

### 6.1 Where it lives

`src/expand/eval.rs`. It reuses the *value* and *budget* machinery of `src/optimizer/comptime_eval.rs`
(`ComptimeValue`, step accounting, `comptime_error` diagnostics) and adds an **emit sink**. It is
deliberately a separate module from `comptime_eval` because:

* `comptime` folds expressions; `expand` *builds declarations*;
* `comptime` may fall back to runtime silently; `expand` may never fall back — a failed expand is a
  hard error (§5.2), because a missing generated method would otherwise appear as a confusing
  `E-TYPE-001` far from the cause.

Shared code is extracted into `src/expand/budget.rs` (`Budget { steps, emits, depth, instantiation }`)
which `comptime_eval` also adopts, so both share one definition of "too expensive".

### 6.2 Values

```rust
pub enum ExpandValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<ExpandValue>),
    /// A nominal type token: `T`, `Int`, `User`, `List<Int>`.
    Type(TypeNode),
    /// A reflection record (field, method, variant, generic).
    Reflect(ReflectValue),
    /// A node literal not yet emitted (`emit` argument, `let n = method { ... }`).
    Node(ExpandNode),
    Unit,
}
```

`ExpandValue::Node` is what makes `let m = method { ... }` / `emit m` work and what allows an expand to
build a list of nodes and emit them in a loop:

```datara
expand Ops<T> {
    let methods = [method { name: "add", ... }, method { name: "sub", ... }]
    for m in methods { emit m }
}
```

### 6.3 Statements

| `ExpandStmt` | Semantics |
| --- | --- |
| `Emit(node)` | validates `node` (§7.3), appends to sink, `emits += 1`, checks `EXPAND_MAX_EMITS` |
| `Let/Mut` | evaluates `init` in a new scope entry; `Mut` allows `Assign` later |
| `Assign` | only on `Mut` bindings, else `E-EXPAND-003` |
| `For` | iterates `List` values; **iteration count is bounded** by `EXPAND_MAX_ITER = 4096`; each iteration adds to `steps` |
| `If` | condition must be `Bool`; `Int` conditions are rejected here exactly as in the main checker (Gate 5) |
| `Return` | ends the frame; `Return(Some(Node))` implicitly emits that node |
| `Out/Err` | append to `ExpandReport.log` — **never** to program stdout. Expansion is compile-time; `forgen build` prints these only with `--expand-log` |
| `Block` | new scope |

### 6.4 Expressions available inside expand

1. Everything `comptime_eval` supports: literals, arithmetic (checked — overflow is `E-EXPAND-004`,
   not a silent wrap), comparisons, `Bool` logic, string concat, `len`, `fmt`, `if`-expressions.
2. Reflection expressions (§8) — the only genuinely new expression family.
3. Node literals (§7) — parsed by the expand parser, evaluated to `ExpandValue::Node`.
4. Calls to functions declared `const`-capable (§6.5).

**Not available:** IO, file, env, network, `unsafe`, `spawn`, channels, `Val`, Python/Rust/C bridges.
Any such call is `E-EXPAND-007` (effect violation) with the offending callee named. Invariant I4.

### 6.5 Calling user functions at expansion time

An ordinary `fn` may be called from an expand body **iff** all of:

* it is `Pure` per `src/effects/mod.rs` (`EffectSet` contains only `Pure`), and
* it contains no `unsafe`, no `extern`, no bridge call, and
* it is not `async`, and
* its body is already resolvable at expansion time (declared anywhere in the program; ordering is
  resolved by the resolver, not by expansion).

If all four hold, the evaluator interprets its body. Otherwise `E-EXPAND-007`. This is what lets library
authors factor expansion logic into helper functions — the single most requested capability from
`proc_macro` users — without inventing a second language.

### 6.6 Budgets (normative constants)

```rust
pub const EXPAND_MAX_STEPS: u64 = 1_000_000;   // evaluator steps per instantiation
pub const EXPAND_MAX_EMITS: usize = 4_096;     // emitted nodes per instantiation
pub const EXPAND_MAX_ITER: usize = 4_096;      // loop iterations per `for`
pub const EXPAND_MAX_DEPTH: usize = 8;         // nested instantiation depth
pub const EXPAND_MAX_INSTANTIATIONS: usize = 1_024; // total frames per compilation
pub const EXPAND_MAX_NODES_PER_TYPE: usize = 512;   // emitted decls attached to one type
```

Exceeding any of them is a *hard* error (`E-EXPAND-004`/`005`), never a silent truncation, and the
diagnostic reports the counter, the limit and the source span of the loop or emit. Rationale: a
compile-time interpreter without budgets is a DoS vector in CI; and a truncated expansion would produce
code that compiles but is wrong.

### 6.7 Error propagation

`?` inside an expand body is **not** allowed (there is no `Outcome` at expansion time; failures are
diagnostics, not values). `or` is allowed for `Maybe`-style expand-local helpers. If an expand must signal
a user-facing error, it uses `err fmt"..."` which records a *warning/error* in the report and, when
`@expand` declares `@strict`, aborts the instantiation.

<!-- CHUNK-C3-END -->
---

## 7. Node literals — AST-as-IR, exhaustively

### 7.1 Principle

A node literal is **not** a template string and **not** a `quote!{}` blob. It is a struct literal whose
type is one of the `ast::*` declaration types, parsed by `src/expand/node_parse.rs`, validated against
that struct's field list, and converted by a total function `ExpandNode::into_decl()`.

Consequence: **the set of legal keys is exactly the set of public fields of the target `ast` struct.**
Adding a field to `ast::MethodDecl` automatically extends the literal syntax, and the compiler refuses
unknown keys with a did-you-mean list generated from the struct definition. There is no third grammar to
keep in sync. This is the property that makes `expand` maintainable for a decade: it cannot drift from
the AST because it *is* the AST.

### 7.2 Key tables (normative — mirrors `src/ast/mod.rs`)

**`method` → `MethodDecl` (`ast/mod.rs:581`)**

| key | type in `MethodDecl` | required | notes |
| --- | --- | --- | --- |
| `name` | `String` | yes | must be a valid identifier; `E-EXPAND-002` otherwise |
| `attributes` | `Vec<Attribute>` | no | `[inline(always), pure]` shorthand accepted |
| `generic_params` | `Vec<String>` | no | defaults `[]` |
| `params` | `Vec<Param>` | no | see `param` sub-form |
| `return_type` | `Option<TypeNode>` | no | `ret:` is accepted as a synonym |
| `requires` / `ensures` | `Vec<ContractClause>` | no | `[{ condition: x > 0, message: "..." }]` |
| `decreases` | `Option<Expr>` | no | |
| `body` | `Option<Box<Stmt>>` | no | a `{ ... }` block, parsed by `parse_stmt` |
| `is_expression_body` | `bool` | no | set automatically when `body` is `=> expr` |
| `is_replaces` | `bool` | no | |
| `replaces_target` | `Option<String>` | no | must be present when `is_replaces` |
| `span` | `SourceSpan` | — | **filled by the evaluator** with `body_span`; not writable |

**`field` → `FieldDecl` (`ast/mod.rs:571`)**: `name`, `type_node` (`type:` synonym), `bit_field`,
`default_value`, `is_mut`, `span` (filled).

**`param` → `Param` (`ast/mod.rs:604`)**: `name`, `type_node` (`type:` synonym), `ownership_mode`
(default `"view"`), `span` (filled).

**`struct` → `ClassDecl` (`ast/mod.rs:376`)**: `name`, `attributes`, `generic_params`, `base_type`,
`compositions`, `body_items`, `invariants`, `is_export`, `span` (filled).

**`component` → `ComponentDecl` (`ast/mod.rs:412`)**: `name`, `body_items`, `is_export`, `span`.
Generated `body_items` are re-validated: `ClassItem::Method` inside a component → `E-COMP-001`.

**`behavior` → `BehaviorDecl` (`ast/mod.rs:405`)**: `target_type`, `body_items`, `span`.
Generated `ClassItem::Field` → `E-BEH-001`.

**`enum` → `EnumDecl` (`ast/mod.rs:389`)**: `name`, `generic_params`, `variants`, `is_export`, `span`.
`variants` entries are `{ name: "...", fields: [TypeNode, ...] }`.

**`trait` → `TraitDef` (`ast/mod.rs:438`)**: `name`, `generic_params`, `super_traits`, `methods`,
`is_export`, `span`. `methods` entries follow `TraitMethodSignature` (`ast/mod.rs:428`):
`name`, `generic_params`, `params`, `return_type`, `default_body`.

**`impl` → `ImplBlock` (`ast/mod.rs:448`)**: `trait_name`, `target_type`, `target_type_args`, `methods`
(`Vec<FunctionDecl>` — note: functions, not methods), `span`.

**`const` → `GlobalDecl` (`ast/mod.rs:341`)**: `name`, `type_node`, `init`, `is_export`, `is_mut`,
`is_const`.

**`type` → `TypeDecl` (`ast/mod.rs:477`)**: `name`, `base_type`, `is_export`.

### 7.3 Validation rules

1. Every key must exist on the target struct → else `E-EXPAND-002` with the did-you-mean list.
2. Every value's static type must match the field type → else `E-EXPAND-002` naming key, expected and
   actual. `Option<T>` keys accept a bare value or `None`.
3. Missing required keys → `E-EXPAND-002` listing **all** missing keys at once, not one per run.
4. A `name` that is not a valid Datara identifier, or that collides with a reserved builtin → `E-EXPAND-002`.
5. `body` blocks are parsed by `parse_stmt`, so **everything illegal in normal code is illegal here**
   (`:=`, `try/catch`, `Int && Int` under Gate 5, unproven division under `E0941`). The expand author
   cannot smuggle invalid code past the checker by generating it.
6. `emit` of a declaration whose entity rules are violated (§5.8) fails **at emit time** with the ordinary
   entity code, attributed to the `emit` span. Fail-closed at the earliest possible point.
7. `span` is never writable; attempting to set it is `E-EXPAND-002` with the explanation that spans are
   compiler-owned (this keeps determinism, §5.7).

### 7.4 What is deliberately NOT a node literal

* No `emit expr` — expansion produces declarations, not statements. To generate code inside a method,
  put the statements in the method's `body`.
* No `quote`/`unquote`/`$` — no string-level composition exists anywhere in the design.
* No `import`/`use`/`cimport` emission — imports are resolved before expansion; generating an import
  would invalidate the resolver table mid-phase. A library that needs another module uses it directly.
* No `extern`/`asm` emission from `expand` in 1.4.4 — both are surface for L4 authors and must be written
  explicitly. (Reason: `asm` bodies and `extern` ABI shapes are the two places where generated code could
  plausibly surprise a security reviewer; excluding them keeps the audited surface identical to
  handwritten code.)

<!-- CHUNK-C4-END -->

## 8. Phase placement in the pipeline (normative)

### 8.1 Why `expand` cannot live where `derive` lives

`crate::derive::expand_derives_and_comptime(&mut program)` runs at **step 2a**, *before* the resolver
(`driver/pipeline.rs:319` in the check tail, `:535` in the analyze tail). That is correct for `derive`,
because derives are pure structural rewrites: `@derive(Json)` needs only the field list of one struct, which
it reads syntactically from `ClassDecl.body_items`.

`expand` needs **more**: `T.fields` reflection, bound satisfaction (`<T: Serialize>`), symbol identity for
hygiene (§11), and it must be able to call *user functions at expansion time* (§6.5). None of that exists
before the resolver runs. Therefore `expand` runs **after** the resolver, and its output must be re-resolved
incrementally.

### 8.2 The phase, exactly

```
 1. lex + parse                       (unchanged)
 2a. derive + comptime rewrite        (unchanged, driver/pipeline.rs:319 / :535)
 2b. attribute validation             (unchanged: inline / alloc / asm)
 3. resolver.resolve_program          (unchanged)
 3b. ★ expand phase                   <-- NEW
     ├─ collect: scan resolved program for instantiation sites
     ├─ interpret: run each ExpandDecl body in the comptime sandbox
     ├─ emit: ExpandNode -> ast::Decl, appended to program.declarations
     └─ re-resolve: resolver.resolve_emitted(&emitted, program, diag)
 4. type checker                      (unchanged, now sees generated decls)
 5. effects                           (unchanged)
 6. ownership                         (unchanged)
 7. security / capabilities           (unchanged)
 8. DMIR lowering                     (unchanged)
 9. optimizer + Evidence Gate         (unchanged, plus expand_report from 3b)
```

Only **one** new stage is inserted, and **no existing stage changes its contract**. Every later phase
receives a `Program` that looks exactly like a handwritten one. This is invariant **I-1**.

### 8.3 Module layout (normative file list)

```
src/expand/
  mod.rs            — public entry: expand_program(...)
  collect.rs        — instantiation-site discovery (from resolver tables)
  interp.rs         — the expand evaluator (§6): statements, expressions, budgets
  value.rs          — ExpandValue + TotalOrd + deterministic canonical hashing
  node_parse.rs     — node-literal parsing (ast struct literals, §7)
  node_emit.rs      — ExpandNode::into_decl(), span filling, entity re-validation
  reflect.rs        — T.fields / T.methods / T.variants reflection over resolver::Symbol
  bounds.rs         — `T: Bound` satisfaction against resolver traits/roles
  report.rs         — ExpandReport: per-instantiation site, fingerprint, budget used
  diag.rs           — E-EXPAND-* construction helpers (span attribution)
```

### 8.4 Entry points

```rust
// src/expand/mod.rs

/// Runs after `Resolver::resolve_program`, before `TypeChecker::check_program`.
/// Mutates `program` by appending generated declarations; re-resolves them.
/// Returns a report that is threaded into the optimizer so its evidence gate
/// can attribute optimization deltas to expansion (see §14.3).
pub fn expand_program(
    program: &mut Program,
    resolver: &mut Resolver,
    diag: &mut DiagnosticEngine,
    limits: ExpandLimits,
) -> ExpandReport;

/// Same, but never mutates the resolver — used by `check` in "expansion dry-run"
/// mode, where generated code is validated but not fed to codegen. Guarantees
/// that `forgen check` and `forgen build` agree bit-for-bit on diagnostics.
pub fn expand_program_readonly(
    program: &Program,
    resolver: &Resolver,
    diag: &mut DiagnosticEngine,
    limits: ExpandLimits,
) -> ExpandReport;
```

`expand_program` is total on the *diagnostic* axis: on any failure it emits diagnostics and returns a report
with `failed: true`. It never panics and never leaves `program` half-mutated — it builds a local
`Vec<Decl>` and only commits it if at least one declaration passed validation, mirroring the incremental
commit style used by `derive`.

### 8.5 Incremental re-resolution

```rust
impl Resolver {
    /// Register declarations produced by expansion without re-running the
    /// whole-program pass. Duplicate detection is identical to `resolve_program`
    /// so a generated name colliding with a handwritten one is E-RESOLVE-002
    /// with both spans, exactly as if both were written by hand.
    pub fn resolve_emitted(
        &mut self,
        emitted: &[Decl],
        program: &Program,
        diag: &mut DiagnosticEngine,
    );
}
```

Rules:

1. `resolve_emitted` must not resurrect a symbol removed by a previous phase. Tables are append-only here.
2. Ordering is the emission order of §10.3. Re-resolving is therefore reproducible.
3. A generated declaration may reference handwritten declarations (always). A handwritten declaration may
   **not** reference a generated one unless it used an explicit instantiation expression (§3.4) — this keeps
   the "no action at a distance" property: you can always see where generated names come from by reading the
   instantiation site.
4. `module_aliases` is not consulted for generated names: generated declarations live in the module of their
   expand definition, which is a fixed, documented choice (§5.7).

<!-- CHUNK-C5-END -->

## 9. Determinism and reproducibility (normative algorithms)

Determinism is a first-class product guarantee for Datara (see `docs/SEMANTIC_INVARIANTS.md`) and expansion
is the single easiest place to lose it. These rules are **normative**, and each is covered by a test in §17.

### 9.1 D-1: iteration order

Every iteration over a mapping inside `src/expand/` uses an ordered container:

| data | container | why |
| --- | --- | --- |
| fields of a reflected type | `Vec<FieldDecl>` in **declaration order** | source order is already deterministic |
| methods of a reflected trait/role | `BTreeMap<SymbolId, …>` | hash order is not stable across runs |
| instantiation sites | `BTreeMap<SiteKey, Vec<Site>>`, `SiteKey = (file: String, offset: u32)` | canonical total order |
| bound candidates | `BTreeMap<String, SymbolId>` | lexical order, not registration order |
| emitted declarations | `Vec<Decl>` in emission order | derived from the above |

`HashMap` iteration is forbidden in `src/expand/`. This is enforced by a repo lint
(`scripts/lint_expand_determinism.py`, added with the feature) that greps the module and fails CI — the same
discipline `src/optimizer/evidence.rs` already applies to fingerprinting.

### 9.2 D-2: canonical value hashing

`ExpandValue` implements a canonical byte encoding:

```
Int(i)     -> 0x01 || i.to_le_bytes()
Float(f)   -> 0x02 || (if f.is_nan() { 0u64 } else { f.to_bits() }).to_le_bytes()
Bool(b)    -> 0x03 || [b as u8]
Str(s)     -> 0x04 || u32_le(s.len()) || s.as_bytes()          (UTF-8, NFC-normalised on entry)
List(v)    -> 0x05 || u32_le(v.len()) || concat(canonical(each))
Map(kv)    -> 0x06 || u32_le(n) || concat(sorted by canonical(key))
Type(t)    -> 0x07 || u32_le(name.len()) || name.bytes()       (SymbolId, not rendered text)
Fields(fs) -> 0x08 || u32_le(n) || concat(name.len() || name || canonical(type))
```

Two expansions are **identical** iff their emitted declaration sequences have equal canonical hashes. This
hash is (a) the fingerprint contribution used by §14.3, (b) what `forgen inspect --expand` prints, and (c)
what the reproducibility test in §17.4 compares across machines, backends and thread counts.

`NaN` collapses to one canonical representation so platform payload bits cannot leak into the build
fingerprint. `-0.0` and `0.0` are **distinct** (IEEE semantics; consistent with the language's `0.0`
division trap).

### 9.3 D-3: prohibition on ambient nondeterminism

Inside expand there is **no** access to:

* wall-clock or entropy (`now()`, `now_ns()`, RNG) → `E-EXPAND-006` at *type-check of the expand body*, so it
  cannot even be written, not merely rejected at run time;
* the filesystem, environment, network, processes → `E-EXPAND-006` (same mechanism);
* pointer addresses / allocation addresses → no `RawPtr` in the expand type universe (§6.2);
* the *order* of independent instantiations → expansion is a pure function of `(resolved program, limits)`;
  there is no shared mutable state between instantiations, and no instantiation can observe another's
  progress.

### 9.4 D-4: reproducible budgets

Budget exhaustion (§6.6) must be a deterministic predicate on the program, not on machine speed. Budgets are
counted in **abstract units** — steps, emitted nodes, emitted bytes, recursion depth — never in milliseconds.
A `--expand-timeout` wall-clock guard *does* exist in the CLI, but when it fires it produces `E-EXPAND-007`
(non-reproducible by construction) and `forgen check` in CI refuses `--expand-timeout` entirely; CI runs with
abstract limits only. Same philosophy as `comptime_eval`'s step limit.

### 9.5 D-5: stable diagnostics

Diagnostic *order* is `(file, offset, code)` sorted, then rendered. Two runs on the same input produce
byte-identical `forgen check --format=json` output. This is already true of the rest of the compiler
(`diagnostics/engine.rs`) and expansion must not violate it: `src/expand/diag.rs` never emits from inside a
loop over an unordered container.

<!-- CHUNK-C6-END -->

## 10. Backend compatibility — Cranelift, LLVM, WASM (normative)

This section is the architectural heart of the design: **expansion introduces no new codegen surface at all.**

### 10.1 C-1: emission is AST-level, not IR-level

`expand` emits `ast::Decl` values. It does **not** emit `dmir::Inst`. Consequences:

* The three backends (`codegen/cranelift`, `codegen/llvm`, `codegen/wasm`) never learn that a declaration was
  generated. They consume a `dmir::Module` identical in shape to one lowered from handwritten code.
* Any construct a backend supports for handwritten code works for generated code with **zero** backend
  changes. Any construct a backend does not support produces its **existing** backend diagnostic
  (`E1405 AsmUnsupportedBackend`, `E0955 AsyncBackendUnsupported`, …) carrying the generated declaration's
  span, which points into the expand body via the mapping rule of §10.5.
* This is why the feature is shippable in one release: there is no "three backends × N emittable constructs"
  matrix to write, test and maintain. The matrix is 3 × 1.

### 10.2 C-2: the emittable node set is exactly the DMIR-reachable node set

A node literal is legal only if `dmir/lowering/` has a lowering for it **and** all three backends have a case
for the resulting instruction. In 1.4.4 that set is, per §7.2:

| emitted | DMIR path | Cranelift | LLVM | WASM |
| --- | --- | --- | --- | --- |
| `method` with `body` | `lower_function` | yes | yes | yes |
| `field` | `StructInit` / layout | yes | yes | yes (`datara:rt` alloc) |
| `param` | func signature | yes | yes | yes |
| `struct` | `lower_class` | yes | yes | yes |
| `component` | layout only (POD) | yes | yes | yes |
| `behavior` | method merge → `lower_function` | yes | yes | yes |
| `enum` | tag + payload, `match` | yes | yes | yes |
| `trait` + default method | `lower_function` | yes | yes | yes |
| `impl` | `lower_function` (not `lower_method` — impl methods are functions) | yes | yes | yes |
| `const` | `GlobalDecl` | yes | yes | yes |
| `type` alias | resolver-only | yes | yes | yes |

Explicitly **not emittable** in 1.4.4, because reachability differs per backend: `spawn`/channels (the
Cranelift-JIT symbol table in `codegen/cranelift/jit.rs` has no `spawn`, while `forgen build` links it),
`asm`, `extern`, `import c`. Emitting a thread-spawning method from an expand body would make `forgen run`
fail where `forgen build` succeeds — a difference in *observable behaviour between backends*, forbidden by
**I-2**. Rejection happens at emit time with `E-EXPAND-008`, naming the construct and the backend that would
break, plus the message "write it explicitly in a `build`-only module".

**I-2 (single-behaviour invariant):** for any program `P`, `forgen check P`, `forgen run P`,
`forgen build --release P`, `forgen build --llvm P` and `forgen build --wasm P` must either all succeed or all
fail with the same `E-EXPAND-*`/entity diagnostics. Only *performance* may differ between backends.

<!-- CHUNK-C7-END -->

### 10.3 C-3: cross-backend differential obligation

The test suite (§17.2) contains a differential harness that, for every expand fixture:

1. lowers with Cranelift (default) and captures the program's stdout;
2. lowers with LLVM (`--llvm`) and captures stdout;
3. lowers with WASM and runs it under the bundled runner, capturing stdout;
4. asserts `stdout_1 == stdout_2 == stdout_3` **byte for byte**, and that the canonical hash of the emitted
   declarations (§9.2) is identical in all three runs.

Fixtures must include: `ToJson` (§18), a generated `SquareMatrix<N>`, a generated `enum` + exhaustive
`match`, a generated `impl` whose body uses `StrBuf`, and a generated `component` array iterated in a
`parallel for` (this one asserts `E0943` is *not* raised for POD components and *is* raised for a `struct`
with a move field — proving entity semantics survive expansion unchanged).

### 10.4 C-4: WASM capabilities path

`codegen/wasm/capabilities.rs:222 collect_transitive_calls` walks the DMIR module to build the `datara:rt`
import list. Generated methods appear as ordinary `Inst::Call` / `Inst::MethodCall` / `Inst::Out` /
`Inst::StructInit` nodes, so `collect_transitive_calls` needs **no change**. Two obligations:

1. `Inst::Out { .. } => "datara_rt_print"` and `Inst::Err { .. } => "datara_rt_err"` (lines 264–269) already
   cover generated `out`/`err`, because those arrive through the normal `Stmt::Out`/`Stmt::Err` lowering
   (`dmir/lowering/stmt.rs:296,304`).
2. A generated declaration that reaches a runtime function *not* in the wasm allow-list must fail at **check**
   time, not at wasm-instantiation time. Implementation: add a sibling
   `validate_wasm_reachable(module) -> Vec<(String, Span)>` next to `collect_transitive_calls`, called from
   `codegen/wasm` whenever the module contains expand-generated functions (a compiler-known flag, not a
   heuristic). Returned names are reported with `E-EXPAND-008` and the expand-author span, so the author
   learns immediately.

### 10.5 C-5: span mapping (the reason backends stay honest)

Generated declarations carry **synthetic spans** that are never `0:0`. A generated node's span is a
`SourceSpan` pointing at the `emit` statement inside the expand body that produced it. This gives:

* backend diagnostics (`E1405`, `E0955`, `E-CODEGEN-*`) that point to the expand author's line, not to a
  mysterious generated region;
* `forgen why` / `forgen explain` traces that read "this instruction comes from `emit method { … }` at
  `json.dtr:14:9`, instantiated by `user.to_json()` at `main.dtr:31:12`";
* for codegen, a deterministic symbol name (§13.4) so DWARF/PDB line tables can attribute generated frames.

### 10.6 C-6: evidence-gate neutrality

The Evidence Gate (`optimizer/evidence.rs`) fingerprints the IR before and after each pass and downgrades
`Applied → Rejected` when the IR changed but module verification failed. Generated code enters this loop as
ordinary IR; therefore the gate protects expansion output **for free**. §14.3 adds attribution on top; it does
not change the gate's contract.

<!-- CHUNK-C8-END -->

## 11. Hygiene — why `SymbolId` makes it free

### 11.1 The problem Rust pays for

`macro_rules!` hygiene needs a separate namespace-tracking machine (`SyntaxContext`, marks, `$crate`) because
macro bodies are token streams that are *not yet* name-resolved. Rust's `proc_macro` has the opposite problem:
it is fully unhygienic (it can emit `let x` that captures a caller's `x`), which is why proc-macro crates
routinely document "do not name your variable `__t`".

### 11.2 What Datara gets for free

`resolve_program` runs **before** `expand`. By the time an expand body executes, every identifier in it is
already bound to a symbol in `Resolver` (`classes`, `roles`, `traits`, `functions`, `Symbol.fields`,
`Symbol.methods`, per `resolver/symbols.rs`). Therefore:

| concern | mechanism |
| --- | --- |
| a name emitted by expand can't capture a caller's local | locals exist only in `TypeChecker` scopes, which start *empty* for generated declarations — there is nothing to capture |
| an expand body can't see caller locals | expand has its own scope stack (§6.3); `resolver` locals are not consulted |
| two expand instantiations can't collide | generated names are qualified by the instantiation key (§13.4), not by bare text |
| an expand definition can't be shadowed by a user declaration | `expand` names live in a separate table (`Resolver::expands`), and the resolution order in §13.1 is fixed |
| `this` inside a generated method refers to the right type | `self`/`this` is bound by the *declaring* entity of the generated node (`behavior`/`impl`), which node emission sets explicitly (§7.2), never inferred |

**Normative rule H-1:** expand bodies may only name symbols that the resolver knows. A symbol that does not
exist is `E-EXPAND-002` (`no such symbol in expand scope`) — never a silently-emitted string that happens to
resolve later. This is what makes hygiene *structural* rather than *best-effort*.

**Normative rule H-2:** generated declarations are resolved by `resolve_emitted` (§8.5) with the same
duplicate detection as handwritten code. There is no "generated code is allowed to shadow" escape hatch.

**Normative rule H-3:** expand cannot read or write a declaration's *name* as text to influence resolution
(no `concat_idents`-style composition). Names are constructed only as string literals in `name:` keys of node
literals, and they are validated as identifiers (`E-EXPAND-002`). Composition of names is a deliberate
non-feature for 1.4.4: it is the single most common source of unhygienic macro hackery, and the reflection +
node-literal design removes the need for it.

### 11.3 Reflection and the `HashMap` hazard

`Symbol.fields` and `Symbol.methods` are `HashMap<String, Symbol>`. Reflection (`T.fields`) **must not**
iterate them, because that would make emission order depend on hash seeding and break §9.1. Instead:

```rust
// src/expand/reflect.rs
pub struct ReflectedType {
    pub name: String,          // canonical type name, not a rendered string
    pub fields: Vec<FieldView>, // declaration order, from ClassDecl.body_items
    pub methods: Vec<MethodView>, // name-sorted (BTreeMap over symbol name)
    pub variants: Vec<VariantView>, // enum declaration order
    pub is_component: bool,
    pub is_export: bool,
}
```

`ReflectedType::fields` is built by walking the declaring `ClassDecl.body_items` in source order and looking up
each name in `Symbol.fields` — lookup is a map access, iteration is not. This is the one place where an
implementation detail of the resolver could silently break determinism, so it gets a dedicated test
(`test_expand_reflection_order`) that expands the same type 200 times in one process and asserts one hash.

<!-- CHUNK-C9-END -->

## 12. Diagnostics — `E-EXPAND-*` (normative)

All codes are added to `src/diagnostics/codes.rs` as variants of `ErrorCode` with an `as_str` arm and both
locale arms in `description()`. Naming follows the existing entity convention (`E-COMP-001`, `E-BEH-001`,
`E-ROLE-001`, `E-IMPL-001`).

| variant | code | severity | trigger | `help:` line |
| --- | --- | --- | --- | --- |
| `ExpandUnknownDefinition` | `E-EXPAND-001` | error | instantiation names an `expand` that does not exist | did-you-mean list over `Resolver::expands` |
| `ExpandInvalidNodeLiteral` | `E-EXPAND-002` | error | unknown/missing/mistyped key in a node literal; non-identifier `name`; attempt to set `span` | prints the expected key set of the target `ast` struct |
| `ExpandBoundUnsatisfied` | `E-EXPAND-003` | error | `T` does not satisfy `<T: Bound>` | lists required vs available methods |
| `ExpandEmitRejected` | `E-EXPAND-004` | error | emitted declaration violates an entity rule (reported with the **entity's own code**, this is the wrapper that adds the expand-site note) | "the emitted declaration is the problem, at this `emit`" |
| `ExpandBudgetExceeded` | `E-EXPAND-005` | error | steps / nodes / bytes / depth budget exhausted | prints which budget, used vs limit, and the recursion path |
| `ExpandForbiddenOperation` | `E-EXPAND-006` | error | `now()`, RNG, FS, env, net, process, `RawPtr` inside expand | "expansion must be a pure function of the program" |
| `ExpandTimeout` | `E-EXPAND-007` | error | `--expand-timeout` wall-clock guard fired (CLI only) | "not reproducible; CI refuses this flag" |
| `ExpandBackendUnreachable` | `E-EXPAND-008` | error | emitted code reaches a construct/function not available on all three backends | names the construct and the affected backend |
| `ExpandRecursiveExpansion` | `E-EXPAND-009` | error | an expand instantiation (transitively) instantiates itself | prints the instantiation chain |
| `ExpandReportInvalid` | `E-EXPAND-010` | internal | `ExpandReport` failed its own invariant check (should be unreachable) | "please report: expansion report inconsistent" |

### 12.1 Attribution rules

1. Every `E-EXPAND-*` diagnostic carries **two** spans where both exist: the *instantiation site* (primary,
   what the user must look at) and the *expand definition* (secondary note, "defined here"). This mirrors the
   existing two-span style used by borrow errors.
2. `E-EXPAND-004` never replaces the entity code. The emitted declaration's own diagnostic
   (`E-COMP-001`, `E-BEH-001`, `E-STRUCT-001`, `E-ROLE-001`, `E-IMPL-001`, `E0941`, `E0310`, …) is emitted
   with the `emit`-statement span, plus a note "generated by `expand ToJson` at json.dtr:14:9". This keeps a
   single source of truth for each rule: the entity checker.
3. Diagnostics from expand are emitted **after** all resolver diagnostics and **before** type-checker
   diagnostics, in `(file, offset, code)` order (§9.5). This is enforced by running expand as its own stage
   (§8.2) rather than inside the resolver.
4. `forgen explain E-EXPAND-00N` must work for every code above: `codes.rs` gains a description plus a
   `why_it_happened` / `how_to_fix` pair in the existing explain table, in **both** `ru` and `en`.
5. `--format=json` output for these codes includes the extra fields `expand_name`, `instantiation_span`,
   `definition_span`, `budget_used`, `budget_limit` — machine-readable so an agent (or `forgen lsp`) can
   auto-fix without parsing prose. This is the "one mechanism, two audiences" rule from the language design.

### 12.2 Localisation

Both locales are required in the same commit as the code. The existing pattern is
`if locale == "ru" { match self { … } } else { match self { … } }` inside `description()`; expand codes follow
it exactly. Missing a locale arm is a compile error (non-exhaustive match), which is the enforcement.

## 13. Resolution, naming and instantiation keys (normative)

### 13.1 Where an `expand` participates in name resolution

`Resolver` gains exactly one new table, alongside the existing `classes`, `roles`, `traits`, `functions`,
`enums`:

```rust
// src/resolver/mod.rs
pub struct Resolver {
    // ... existing tables unchanged ...
    pub expands: HashMap<String, ExpandSymbol>,   // NEW (v1.4.4)
}

pub struct ExpandSymbol {
    pub name: String,               // canonical, as written: "ToJson"
    pub decl_span: SourceSpan,
    pub generics: Vec<GenericParam>,      // ["T: Serialize", "N: Int"]
    pub target_kind: ExpandTargetKind,    // Any | Type(String)
    pub report: ExpandReport,             // §8.4 / §12 — what it can emit
    pub file: String,                     // declaring module, for diagnostics
}
```

`ExpandSymbol` is populated by `resolve_program` (§8.5) in the same pass that fills `classes`/`traits`, so an
`expand` declaration is a first-class named entity, visible to `forgen doc`, `forgen tree`, and the LSP symbol
index — exactly like `behavior` and `role`.

### 13.2 Method-lookup order (fixed, not configurable)

When the resolver/type-checker sees `recv.name(args)` where `recv : T`, the lookup order is:

| # | source | kind of symbol produced | searchable by |
| --- | --- | --- | --- |
| 1 | `impl` methods for `T` | `I_<Trait>_<T>_<name>` | exact name match |
| 2 | `behavior` methods for `T` | `B_<T>_<name>` | exact name match |
| 3 | **`expand` candidates** for `T` | `X_<Expand>__<Key>_<name>` | §13.3 |
| 4 | prelude / stdlib free functions | `datara_rt_*` | existing |

**Normative rule R-1:** earlier tiers win unconditionally. A user-written `behavior User { to_json() }`
silently shadows `expand ToJson` for `User` — no ambiguity error, because "handwritten beats generated" is the
same rule the language already uses for `impl` vs `behavior` and for `replaces`.

**Normative rule R-2:** within tier 3, if **two** `expand` definitions can both produce `name` for `T` and
neither declares `replaces`, that is `E-EXPAND-011` (ambiguous expansion), listing both definition sites. This
is the only new ambiguity rule; it exists so that tier 3 can never be a coin flip.

**Normative rule R-3:** tier 3 is only consulted when tiers 1–2 miss. Consequence: adding an `expand` can
never break existing code that already compiles. This is the compatibility guarantee that lets `expand` ship in
a minor version (1.4.4) without a migration gate.

### 13.3 Instantiation key and symbol mangling

An instantiation is identified by a **key**, not by a string built at the call site:

```rust
pub struct InstantiationKey {
    pub expand: String,        // "ToJson"
    pub args: Vec<TypeKey>,    // canonical type keys, in declaration order of generics
}

pub struct TypeKey {
    pub name: String,          // "User", "List", "Int"
    pub args: Vec<TypeKey>,    // nested generics
}
```

Mangling is total and injective over `TypeKey`:

```
mangle(Int)            = "Int"
mangle(Str)            = "Str"
mangle(List<Int>)      = "List_L_Int_R"
mangle(Map<Str,Int>)   = "Map_L_Str_C_Int_R"
mangle(Foo)            = "Foo"
mangle(ToJson<User>)   = "ToJson__User"
mangle(SquareMatrix<Float,3>) = "SquareMatrix__Float__3"
```

**Normative rule N-1:** `_L_`, `_C_`, `_R_` are the escapes for `<`, `,`, `>`. Any literal `_` inside an
identifier is preserved as-is. Because identifiers cannot contain `<`, `,`, `>`, the encoding is injective: two
different keys can never mangle to the same string. A dedicated test (`test_expand_mangling_injective`) asserts
this over a generated corpus.

**Normative rule N-2:** generated top-level symbols are named

```
X_<Expand>__<Key>_<item>     for declarations produced by expand
```

and generated methods inside an existing type keep the entity prefix (§8.2, `B_`/`I_`), with the key appended:

```
B_User__X_ToJson__User_to_json
I_Serialize_User__X_ToJson__User_to_json
```

**Normative rule N-3:** the mangled name is derived **only** from the key and the declared item name. It never
depends on file order, `HashMap` iteration, spans, timestamps, or the compiler's own process state. Two
independent compilations of the same sources produce byte-identical symbol tables (§9).

### 13.4 Instantiation is memoised, and the memo is part of the build cache

```rust
// src/expand/cache.rs
pub struct ExpansionCache {
    entries: BTreeMap<InstantiationKey, Expanded>,
}
pub struct Expanded {
    pub key_hash: u64,          // §9.2 canonical hash of (key, definition_hash, body AST)
    pub decls: Vec<Decl>,
    pub report: ExpandReport,
}
```

`definition_hash` is the canonical hash of the `expand` declaration's AST **after** `comptime` folding. If a
build cache entry exists for `key_hash`, expansion is skipped entirely. This is what keeps a 200-instantiation
codebase fast: expansion cost is paid once per `(key, definition)` pair, not once per use site.

**Normative rule N-4:** the cache key includes the definition hash, so editing the `expand` body invalidates
every instantiation of it — no stale generated code is possible. `forgen check --clean` drops the cache.

### 13.5 Why this beats Rust's model, precisely

| property | Rust `macro_rules!` | Rust `proc_macro` | Datara `expand` |
| --- | --- | --- | --- |
| input is type-checked before expansion | no (token trees) | no (token stream) | **yes** (§5.2) |
| output is type-checked as normal code | after expansion | after expansion | **yes, same checker path** (§8.2) |
| hygiene | separate machine | none | **free via `SymbolId`** (§11) |
| name resolution participates | no | no | **yes** (§13.1, tier 3) |
| instantiation identity | textual | textual | **structural `InstantiationKey`** (§13.3) |
| incremental caching of expansions | no | no | **yes, keyed by `key_hash`** (§13.4) |
| can shadow/break existing code | yes | yes | **no** (rule R-3) |

## 14. Security model — expand is sandboxed by construction (normative)

### 14.1 Threat model

An `expand` may come from a third-party package (`dpm` dependency, or a `.dtr` file the user was told to
`use`). Treating it as trusted would reintroduce, in a language whose entire selling point is fail-closed
compilation, the exact hole `proc_macro` has today: a dependency that reads your filesystem at build time or
silently rewrites your code. Therefore `expand` is treated as **adversarial input that happens to run inside
the compiler**.

The four attacks to defeat, and the mechanism that defeats each:

| attack | mechanism |
| --- | --- |
| read files / network / env at build time (exfiltration, non-reproducibility) | purity whitelist §14.2, `E-EXPAND-006` |
| infinite loop / expansion bomb (DoS the CI) | step, node, depth and byte budgets §6.6, `E-EXPAND-005` |
| unhygienic capture of caller names | structural hygiene via `SymbolId` §11, rules H-1..H-3 |
| emit code that escapes its authority (an `expand` granting FS/net to generated code) | emitted code re-enters the *same* effect/ownership/capability checkers §14.4; `E-CAP-*` fires on generated code exactly as on handwritten code |

### 14.2 The purity whitelist (normative, exhaustive for 1.4.4)

Inside an `expand` body, only the following are callable. Anything else — including any user function that
transitively reaches a non-whitelisted function — is `E-EXPAND-006`.

| category | allowed calls |
| --- | --- |
| reflection | `T.fields`, `T.methods`, `T.variants`, `T.name`, `T.is_component`, `T.is_enum`, `field.name`, `field.ty`, `method.name`, `method.params`, `method.return_ty`, `variant.name`, `variant.payload` |
| expansion control | `emit …`, `instantiate …`, budget query `budget_remaining()` |
| pure string/int work | `len`, pure subset of `str_*`, `int_to_str`, pure subset of `math_*`, `ctz/clz/popcnt`, list/map operations on expansion-time values |
| program constants | `const NAME` values, `comptime` results, enum variant metadata |

Explicitly **forbidden** (each is `E-EXPAND-006` with the offending call named in the message):

```
now() / time_*        file_* / dir_* / path_*        env_get_checked / env_*
net_* / socket_*      spawn / join / channel_*       py_* / rust_* / js_* / zig_* / lua_* / csharp_*
input*                mem_alloc / arena_*            ptr_* / volatile_* / atomic_fence_*
RawPtr (any construction)          unsafe (unless the declaration is `unsafe expand`, §14.3)
panic / assert  (a failing expansion must be a diagnostic, not a crash — see §6.7)
```

**Normative rule S-1:** the whitelist is enforced by *symbol classification*, not by textual scanning. The
expand evaluator consults `Effects` (`src/effects/mod.rs`), which already classifies every builtin into
`Pure`/`IO`/`Foreign`; a call whose symbol carries any effect other than `Pure` or the dedicated
`Effect::Comptime` is rejected. This reuses the existing effect table, so a newly added IO builtin is
automatically forbidden inside expand without touching `src/expand/`.

**Normative rule S-2:** the whitelist is **closed for 1.4.4**. New entries require a versioned spec change plus
an entry in §14.5's test corpus. There is no `@allow_build_effects` escape hatch; a legitimately build-time-only
capability (e.g. embedding a file's bytes) must instead be requested in `datara.toml` and performed by a
generated `comptime` constant, not by the expand body.

### 14.3 `unsafe expand` — the narrow, auditable exception

Some real metaprogramming must emit `unsafe` blocks (e.g. generating a `C`-ABI shim). The declaration form:

```datara
unsafe expand CSymbol<T> {
    require T.is_component
    unsafe(justification: "generated C ABI shim; fields are POD by E-COMP-002") {
        // ...
    }
    return method { name: "size_of", params: [], ret: Int, body: node_int(8) }
}
```

Rules:

1. `unsafe expand` is itself an `unsafe` construct: **every instantiation site** must be inside an
   `unsafe(justification: …)` block or the enclosing function must be `unsafe`. Otherwise `E0942`, unchanged.
2. The `justification` string is a **compile-time constant** and is copied verbatim into the generated `unsafe`
   block. It appears in `forgen audit` output alongside the expand name (§16.4), so the audit trail names both
   the expander and the justification.
3. `unsafe expand` does **not** relax the purity whitelist. It only allows emitting `unsafe` nodes; the expand
   body still cannot call `now()`, touch the filesystem, or read the environment.

**Rationale:** "generates unsafe code" and "runs unsafe code" are different privileges. Conflating them is what
makes Rust proc-macros unauditable. Datara keeps them separate: the expand *body* stays pure, only its *output*
may be `unsafe`, and that output is visible and checked.

### 14.4 Emitted code has no extra authority

Generated declarations re-enter the pipeline at §8.2's insertion point — *before* the type checker, the effect
checker, the ownership analysis and the security verifier. Therefore:

- A generated method calling `file_read` produces exactly the same `E-CAP-001`/`IO` diagnostics as if the user
  had typed it. `expand` cannot launder a capability.
- A generated method moving a value twice produces `E-BORROW-001`.
- A generated `component` with a `Str` field produces `E-COMP-002`.
- For WASM, generated code participates in `collect_transitive_calls`
  (`src/codegen/wasm/capabilities.rs`), so an emitted `Inst::Out` normalises to the `print` capability op and
  an emitted `list_push` to `list_append`. There is no "generated code bypasses the capability manifest" path,
  because the manifest is derived from the **final** DMIR, after expansion.

**Normative rule S-3:** the capability manifest, `forgen audit`, SBOM generation and the attested-WASM hash are
all computed on the post-expansion program. Two builds that differ only in expansion output differ in their
attestation hash — which is the desired property: generated code is real code.

### 14.5 The `expand` security test corpus (required before 1.4.4 ships)

Integration suite `tests/test_v144_expand_security.rs`, one test per row:

| # | test | expected |
| --- | --- | --- |
| 1 | expand body calls `now()` | `E-EXPAND-006`, names `now`, points at the call |
| 2 | expand body calls `file_read("/etc/passwd")` | `E-EXPAND-006` |
| 3 | expand body calls user `fn helper()` that calls `now()` | `E-EXPAND-006` with the transitive chain |
| 4 | expand body constructs a `RawPtr` | `E-EXPAND-006` |
| 5 | expand emits a method calling `file_read` | `E-CAP-001` on the *generated* method, note "generated by expand X" |
| 6 | `unsafe expand` instantiated outside `unsafe` | `E0942` at the instantiation site |
| 7 | recursive `expand A` → `expand A` | `E-EXPAND-009` with the chain |
| 8 | `for i in 0..1_000_000_000 { emit … }` | `E-EXPAND-005`, budget named, identical output every run (§9.4) |
| 9 | expand emits a name colliding with a user symbol | duplicate-definition diagnostic from `resolve_emitted`, no silent shadow |
| 10 | two expands producing the same method for the same type | `E-EXPAND-011`, both definitions listed |
| 11 | expand from a `dpm` dependency fails verification | primary span names the **dependency** file, not the user's |
| 12 | expand references an undeclared symbol | `E-EXPAND-002` (rule H-1), never a late failure |

## 15. Migration from `derive` — no breakage, one direction

`@derive(Display, Json, Hash, Clone, Deserialize)` stays **exactly as it is** in 1.4.4. Internally the five
built-in derives become `expand` instantiations in `src/derive/mod.rs`, reimplemented as the reference users of
the expand API (this doubles as the best integration test). The user-visible surface is unchanged.

1. **Built-in derives never change semantics.** Their output is now produced by expand-generated code, which
   participates in SRA/IPO/LoopFold — a strict improvement. Golden tests pin the generated shape.
2. **User-defined proc-macro-style derives do not exist and will not be added.** A custom derive is
   `expand ToMyFormat<T>`, used as `user.to_my_format()` or via the `@use-expand(MyThing)` attribute form
   (§3.3). Migration is mechanical.
3. Derive-generated methods get the `B_<T>_*` prefix exactly like handwritten `behavior` methods (§13.2
   tier 2), so derive-generated and expand-generated methods with the same name conflict with the same
   duplicate diagnostic — one rule for both.

<!-- CHUNK-C12B-END -->

## 16. Backend compatibility — execution details extending §10 (JIT discipline, differential corpus)

**Normative rule BE-1:** `expand` is a **frontend** phase. Its output enters the shared DMIR module before
backend selection. No backend ever sees the `expand` construct itself; there is no backend-specific
expansion path, no backend-specific naming, no backend-specific lowering.

**Rule BE-2 (differential corpus):** for every program in the expand test suite, the three commands

```bash
forgen run prog.dtr            # Cranelift JIT (dev)
forgen build --llvm prog.dtr   # AOT LLVM -O3 (prod)
forgen build --wasm prog.dtr   # WASM + capabilities (edge)
```

must produce **byte-identical stdout** and identical exit codes. CI enforces this per commit
(`tests/test_v144_expand_backends.rs`). A divergence between backends is a P0 blocker, same severity as an
Evidence Gate failure.

**Rule BE-3 (JIT symbol discipline):** everything the expand phase emits must be resolvable by the JIT
symbol table (`src/codegen/cranelift/jit.rs` registry). Emitted methods lower to `datara_rt_*` calls that
are already registered, or to intra-module functions declared before first call. If a future emitted
construct needs a runtime function, the phase must add it to `jit.rs::reg!` in the same PR — the 1.4.4
lesson (`spawn` runs only via `build`) must not repeat for `expand`. `check` warns `E-EXPAND-012` if a
generated method references an unregistered runtime symbol.

**Rule BE-4 (WASM):** expand-generated methods flow through the capability analyzer
(`codegen/wasm/capabilities.rs::collect_transitive_calls`) like handwritten code. Generated calls to
`datara_rt_*` are already normalized there (`out|println|print -> print`); expand adds no new normalization
cases. Generated method names carry the `B_`/`I_` prefixes (§13.2) internally but **do not leak into
`main`-adjacent wasm exports**: only the original `fn main` and explicitly `pub` user symbols are exported.

**Rule BE-5 (LLVM):** generated code participates in Thin-LTO whole-program analysis. Because generated
bodies are typed and effect-annotated **before** lowering, LLVM receives the same `noalias`/`nonnull`
attributes it would receive for handwritten equivalents — no per-backend special casing is needed to get
them (§17).

## 17. Zero-cost performance contract

The promise "expand-generated code optimizes identically to handwritten code" is a **tested contract**, not
a slogan:

1. **No runtime metadata.** Expansion is fully erased at compile time. A binary containing an expanded
   `to_json` must be indistinguishable in size from the handwritten one, up to name strings.
2. **Pass equivalence proof.** `optimizer/evidence.rs` fingerprints run on the post-expansion module. The
   report labels generated methods (`B_Toj_<T>_to_json`) so `forgen why` can attribute each pass
   (SRA/inline/LoopFold/devirtualize) to generated and handwritten code alike.
3. **Benchmark gate (CI-blocking):** `expand ToJson` over the 6-field `UserProfile` benchmark must land
   within **5%** of the handwritten equivalent on `forgen build --llvm`. Regression beyond 5% fails CI.
4. **Budget honesty.** Expansion work is charged to the compile-time budget (§9.4) and reported in
   `forgen inspect` (`expand: 3 instantiations, 41 steps, 0.2ms`) — so large expand usage cannot silently
   inflate build times.

## 18. Determinism — build-level rules extending §9 (reproducible outputs)

**Rule D-1:** expansion is a pure function of `(expand definition, monomorphization types, comptime
arguments)`. Same inputs → byte-identical emitted module → same attestation hash (rule S-3). Concretely:

- symbol interning and iteration order inside `src/expand/` use only sorted/BTreeMap containers;
- field iteration (`T.fields`) uses declaration order, which is already fixed by the parser;
- no wall-clock, no filesystem, no environment reads inside the expansion engine (enforced by rule E-6);
- generated spans carry deterministic synthetic file names (`<expand ToJson for UserProfile>:12`), so
  diagnostics and golden tests are stable.

**Rule D-2:** two compilations of the same program on the same toolchain produce bit-identical outputs
across all three backends. Verified by the differential corpus (§16) run twice and hashed.

<!-- CHUNK-C13A-END -->

## 19. Diagnostics UX — errors that fix themselves

Every `E-EXPAND-*` code ships with an `explain` entry and a **machine-applicable suggestion**:

| code | human text (RU/EN) | suggested fix (auto-applicable) |
| --- | --- | --- |
| `E-EXPAND-001` | «expand `X` не найден / not found» | offers the closest matching expand name (Levenshtein ≤ 2) |
| `E-EXPAND-002` | «symbol `y` не объявлен в expand-теле» | points at the missing `let` or wrong bound member |
| `E-EXPAND-003` | «emit собрал невалидный IR: <fact>» | shows the exact emitted node span inside the body |
| `E-EXPAND-004` | «bound `<T: Serialize>` не выполнен для `Foo`» | lists the missing methods + offers `impl` stub |
| `E-EXPAND-005` | «бюджет расширения исчерпан» | names the limit; `--expand-budget` escape hatch for maintainers |
| `E-EXPAND-006` | «недетерминизм/эффект в expand-теле» | points at the offending call, links sandbox doc |
| `E-EXPAND-009` | «циклическое раскрытие A → B → A» | prints the chain, suggests depth guard |
| `E-EXPAND-011` | «двойная генерация метода» | lists both generating expands, suggests `replaces` |

**Rule UX-1 (one error, one cause):** an expansion failure produces **exactly one** primary diagnostic.
Author-body errors point into the `expand` body with a secondary note at the instantiation site;
instantiation/bound errors point at the instantiation with a note at the definition. Never both as
primary. This is the rule that killed Rust macro diagnostics for 10 years — it is a hard invariant here.

**Rule UX-2 (`--format=json`):** all expand diagnostics emit the structured form already used by the LSP,
including the `suggestion` field with a complete replacement range, so both humans and LLM agents fix on
first attempt.

## 20. LSP / IDE integration

`src/lsp/mod.rs` additions (same PR as the phase):

1. **Go-to-definition** on `user.to_json()` jumps into `expand ToJson` body, with hover
   «generated by expand ToJson<UserProfile>, instantiated at file:line».
2. **Completion inside expand bodies** offers `T.fields[*].name/type`, bound methods of `T`, and `emit`
   IR-literal fields, typed against `ExpandTargetKind`.
3. **Semantic tokens:** `expand`, `emit`, `method{}`/`struct{}` IR-literal keywords get dedicated token
   kinds; generated methods get italic decoration at reference sites.
4. **Document outline** shows `expand` declarations between `behavior` and `impl`.

## 21. Acceptance criteria for 1.4.4 (CI-blocking, complete list)

1. All §14.5 security corpus tests pass on all three backends.
2. Differential backend corpus (§16 BE-2) — byte-identical outputs, two runs, hashed equal (§18 D-2).
3. Benchmark gate §17.3 — generated `to_json` ≤ 1.05× handwritten time, binary delta ≤ 16 bytes of code.
4. Five built-in derives reimplemented over `expand` (§15) with golden-file tests pinning emitted DMIR.
5. `forgen check` of the full `stdlib/` + `examples/` + new `tests/expand_fixtures/*.dtr` — zero new
   diagnostics, compile budget total < 50 ms expansion work.
6. `forgen explain E-EXPAND-001..012` exist with RU+EN text and one worked example each.
7. Fuzz target `fuzz/expand_engine.rs`: 1M randomized expand bodies (mutated fixtures) — no panic, no
   non-determinism (same seed → same module hash), budget/diagnostic codes only.
8. Documentation: this spec + README section «expand — typed compile-time generation» + one tutorial.

## 22. Implementation order (dependency-true, one pass, no rework)

| step | files | depends on | test gate |
| --- | --- | --- | --- |
| 1 | `lexer/classify.rs`, `lexer/mod.rs`, `lexer/tokens.rs`: keyword `expand`, `emit`, `unsafe expand` | — | lexer unit tests |
| 2 | `parser/decl.rs: parse_expand_decl` (by analogy with `parse_behavior_decl`), IR-literals in `parser/expr.rs` | 1 | parse fixtures round-trip via `forgen fmt` |
| 3 | `ast/mod.rs`: `Decl::Expand`, `ExpandStmt`, `ExpandTargetKind`, `IrLiteral` | 2 | AST snapshot tests |
| 4 | `resolver/mod.rs`: `expands` table, `SymbolKind::Expand`, call-site binding in method-call resolution | 3 | `E-EXPAND-001/011` cases |
| 5 | **`src/expand/mod.rs` + `src/expand/sandbox.rs` + `src/expand/emit.rs`** (new phase) | 4 | §14.5 rows 1–8 |
| 6 | `driver/pipeline.rs`: call `expand_derives → expand::instantiate` between derive and typecheck | 5 | pipeline integration test |
| 7 | `types/check/*`: type the emitted module as ordinary code; `role/behavior/impl` interaction | 6 | §14.5 rows 9–12 |
| 8 | `dmir/lowering`: accept pre-lowered emitted bodies; `B_/I_` naming (§13.2) | 7 | golden DMIR |
| 9 | `optimizer`: **no changes** — verification only that Evidence Gate fingerprints cover generated code | 8 | `forgen why` report test |
| 10 | `security/`: capability + effect audit of expand bodies (§14) | 8 | §14.5 rows 1–6 |
| 11 | diagnostics: codes + `explain` + suggestions (§19) | 7 | explain snapshot tests |
| 12 | `lsp/mod.rs` (§20) | 7 | LSP integration test |
| 13 | backends (§16), CI differential corpus, fuzz target (§21.7) | 5–10 | full corpus green |

Steps 5–10 are parallelizable after step 4; everything else is strictly ordered. The whole plan is sized
to land inside the 1.4.4 release train with the derive re-implementation (§15) as the living integration
test — **we do not ship an MVP; we ship the production feature with its security corpus, benchmark gate
and backend differential tests included.**

<!-- CHUNK-C14A-END -->

---

# ЧАСТЬ II. Концепция языка нового поколения — до конца, без «MVP-мышления»

Часть I закрыла `expand`. Эта часть — обязательные решения уровня языка, которые входят в ту же версию
1.4.4 (или прямо ограничивают 1.5). Принцип один: **мы не останавливаемся на MVP никогда**. Каждая идея
ниже доводится до критерия проверки, файла реализации и теста — как в §21.

## 23. Идеальная диагностика ошибок — лучше, чем у всех, включая Go

Go силён простотой ошибок. Мы делаем не «простоту», а **полноту с нулём шума**: каждая ошибка имеет одну
причину, одно место, один способ исправления — и это исправление можно применить автоматически.

### 23.1 Контракт диагностики (правила языка, не пожелания)

| Правило | Суть | Проверка |
| --- | --- | --- |
| **DX-1: один код навсегда** | единая нумерация `E-XXX` (ликвидировать двойную систему `E-SYNTAX-001` vs `E0941` из аудита); код, попавший в релиз, не переиспользуется никогда | `forgen codes --check-consistency` в CI |
| **DX-2: одна ошибка — одна причина** | ровно один primary span; остальные — `note:` вторичные | snapshot-тесты на 200fixture'ах |
| **DX-3: всегда есть `help:`** | ни одного диагноста без машиночитаемой подсказки | lint в `diagnostics/engine.rs` |
| **DX-4: подсказка применяема** | `forgen check --fix` применяет все `suggestion` автоматически; `--diff` показывает патч | golden-тесты fix-применения |
| **DX-5: точка минимальной правки** | для контрактных ошибок (`E0941/E0943/E-TYPE`) span указывает не на место краха, а на **минимальное изменение**, которое всё чинит (например, добавить `require b != 0`, а не переписать функцию) | fixture на каждый код |
| **DX-6: язык сообщений — настройка** | `datara.toml [diagnostics] lang = "ru|en"`, все тексты в таблицах, не строках в коде | audit: нет сырых `format!` в диагностах |
| **DX-7: режим тишины и режим JSON** | `--quiet` (только `error: 3`), `--format=json` (для LLM/LSP), `NO_COLOR` — уже есть, закрепить контрактом | CI-матрица режимов |
| **DX-8: ошибка не топит файл** | `synchronize()` уже работает — расширить контрактом «одна компиляция отчёты по максимуму независимых ошибок», как `rustc`, но без каскада | multi-error fixtures |

### 23.2 Трассировка через FFI — то, чего нет ни у кого

Сейчас паника в Rust/Python-бридже теряет контекст. Вводится **сквозной трейс**:

```
datara:main:12 -> rust:serde::from_str:441 -> datara:main:12 [parse_outcome]
```

- граница `use rust.* / use python.*` перехватывает панику/исключение, превращает в `Outcome.err` с
  конкатенированным трейсом (правило из «хаба»: один `?`, один трейс через все языки);
- `forgen run` печатает трейс на trap (`MAX+1`) с номерами строк обоих языков;
- в WASM трейс — это capability-лог (`E-CAP` указывает на точку запроса права).

### 23.3 Компилятор, который объясняет себя

- `forgen why <fn|call>` — почему инлайн/не инлайн, почему девиртуализация, откуда факт (Evidence Gate).
- `forgen context <sym>` — что компилятор знает о символе: тип, эффекты, владение, капы, источники.
- Для LLM: `--format=json` этих же команд = «самопроверяющийся язык»: агент не гадает, а читает факты.

**Критерий «идеальности» (принимается, если все true):** по 200fixture-корпусу — 100% ошибок имеют `help`,
≥ 70% имеют авто-`--fix`, 0 каскадных дублей, медиана 1 диагностик на ошибку (у `rustc`/`clang` — 3–7).

<!-- CHUNK-C15A-END -->

## 24. Полное решение проблем C, C++ и Rust — таблица закрытия

Каждая историческая проблема трёх языков получает **конкретный механизм Datara** и проверку. «Решено» =
есть правило языка + тест, а не намерение.

### 24.1 Проблемы C → решения Datara

| Проблема C | Механизм Datara | Проверка |
| --- | --- | --- |
| UB (неопределённое поведение) | fail-closed: trap вместо UB на переполнение; деление только под `require`; индексация — bounds-checked, unchecked только в `unsafe` L3/L4 | trap-тесты, fuzz: 0 UB-моделей |
| Ручная память: утечки/double-free | аффинное владение (`Uninit/Owned/Moved/Borrowed`), `E-BORROW-001..006`, арены/scratch | test_v144_audit_fixes, ownership-корпус |
| Строки: `char*` без длины | `Str { ptr, len }`, UTF-8 валидация на входе, `null → ""` в `datara_fast_strlen` | str-корпус, fuzz |
| Макросы `#define` | нет текстовых макросов; `expand` (Часть I) — типизированный, гигиеничный | §21 |
| Заголовки/ABI-ад | `import c "x.h" with link(...)` — свой парсер заголовков, без Clang; единый ABI L1–L4 | test_cimport*.rs |
| Нет типов у данных | `struct` + refinement (`Int in 0..len`, `where`), `packet`-битовые поля | refine-корпус |
| Гонки данных по умолчанию | `parallel for` анализ, `E0943`; разделяемое — только `component`/каналы | test_v144_concurrency |

### 24.2 Проблемы C++ → решения Datara

| Проблема C++ | Механизм Datara | Проверка |
| --- | --- | --- |
| 3 системы мета (templates/constexpr/#define) | одна: `comptime` + `expand` | Часть I |
| SFINAE / ошибки на 500 строк | bounds `T: Trait` + `E-EXPAND-004` с одной причиной | §19 DX-2 |
| Наследование-лапша, хрупкий `virtual` | нет наследования: `struct + behavior + using`; полиморфизм — `trait/impl`, девиртуализация | ipo-тесты |
| ODR/линковщик-ад | модульная модель Level 2/3 (`namespace.rs`), coherence `impl` 1:1 | resolver-тесты |
| Время сборки | Cranelift dev 117мс; инкрементальный кэш починить для `use`-проектов (P2 из аудита) | bench compile |
| UB в `std::move`-после-use | moved-from — типовое состояние, `E-BORROW-001` на повторном использовании | ownership-корпус |

### 24.3 Проблемы Rust → решения Datara (главное)

| Проблема Rust | Механизм Datara | Статус |
| --- | --- | --- |
| Lifetimes `<'a>` — барьер входа | лексические `view/mut-view`, без аннотаций; borrow checker на фигурных скопах | **уже работает** (E-BORROW-*) |
| `unsafe` — открытая дверь | `unsafe(justification: "...")` обязателен,auditable, `E0942`; цель: доля `unsafe` в stdlib < 1% | есть, добавить метрику в `forgen audit` |
| Переполнение — wrap в release | **trap всегда** (checked arithmetic); `wrapping()/saturating()` — явные узлы | уже работает |
| `unwrap()/panic!` культура | `Outcome/Maybe` + `?/or`; `panic` — только явный, с трейсом (§23.2) | закрепить линтом L1xxx: `unwrap` без `require` → warning |
| Макросы: syn/quote/гиена | `expand`: типизированный вход/выход, гигиена по `SymbolId`, песочница | Часть I, §1–§22 |
| Медленная сборка | Cranelift-first dev-путь, Evidence Gate не мешает dev, бюджет честности §17.4 | есть |
| Нет доказательств оптимизаций | Evidence Gate: fingerprint → apply → verify → статус | уже работает |
| Async-фрагментация | один `await` поверх PCS-wavefront; честный `sync-only WASM` или полный бэк — без третьего состояния | план 1.5 |
| `Int+Float`/`Int&&Int` (наши P0) | запрет неявных коэрций, строго `Bool` в `&&/||` | **фикс 1.4.4, приоритет P0** (аудит §3) |
| Null/исключения/ошибки вперемешку | `Maybe<T>`/`Outcome<T,Str>` + `?/or` — единственные каналы ошибок; `try/catch` удалён | уже работает |

**Итог §24:** ни одна проблема C/C++/Rust не остаётся без механизма и теста. Список закрыт — это и есть
определение «язык решает проблемы предшественников целиком».

<!-- CHUNK-C16A-END -->

## 25. Размеры бинарников — минимальные по построению

Принцип: бинарь Datara содержит **только то, что программа вызывает**. Ничего «на всякий случай».

### 25.1 Механизмы (все проверяемы)

| Механизм | Как работает | Файл/инструмент |
| --- | --- | --- |
| **RT-tree DCE** | рантайм линкуется выборочно: `collect_transitive_calls` (уже есть в WASM) обобщить на AOT — в бинарь попадают только вызванные `datara_rt_*` | `codegen/wasm/capabilities.rs` → вынести в `driver/link.rs` |
| **Section GC** | `--gc-sections` + `/OPT:REF,ICF` на MSVC по умолчанию в release | `codegen/llvm/emitter` flags |
| **ThinLTO + opt-level=z опция** | `forgen build --size` = `-Oz + LTO + panic=abort` | `codegen/llvm` |
| **panic=abort в release** | trap без раскрутки: минус таблицы unwind | backend flag |
| **Стрип по умолчанию** | debug-инфо — отдельный файл `.dSYM/.pdb` по флагу `--debug-info` | driver |
| **Zero metadata** | нет строк-метаданных `expand`/владения в рантайме (§17.1) | §17 |
| **Статический CRT-выбор** | `--crt=static|dynamic`, по умолчанию static для CLI/edge | driver/link |
| **Comptime-фолдинг строк** | `fmt"..."` с константами → `ConstStr` (§ ConstFold), минус рантайм-конкатенации | `optimizer/const_fold.rs` |

### 25.2 Целевые бюджет размера (CI-гейт, как §17.3)

| Программа | Бюджет 1.4.4 | Ориентир конкурентов |
| --- | --- | --- |
| `hello` (release, static CRT) | **≤ 64 КБ** | Rust ~300+ КБ, Go ~1.2 МБ, C ~30 КБ (но без безопасности) |
| CLI 500 строк (http-less) | ≤ 150 КБ | Go-CLI ~2 МБ |
| WASM-модуль `to_json` | ≤ 12 КБ | Rust+wasm-bindgen ~40+ КБ |

Правило: **регрессия размера > 5% против предыдущего релиза на фиксированных fixtures ломает CI** —
размер стал частью контракта, как время сборки и скорость.

## 26. Безопасность лучше Rust — измеримо, не декларативно

Rust задал планку. Мы её превышаем по **пяти измеримым осям**:

1. **Fail-closed полнее.** У Rust: переполнение — wrap в release, `unwrap` — паника, деление — panic.
   У Datara: переполнение — trap, деление — доказано компилятором (`E0941`), гонка — ошибка компиляции
   (`E0943`), исчерпаемость `match` — ошибка (`E0310`). Класс ошибок «ошибка есть, но в рантайме»
   сокращён до минимума.
2. **`unsafe` — подотчётный.** Каждый `unsafe` несёт `justification` (строку-обоснование), попадает в
   `forgen audit`-отчёт с автором и причиной; цель: **< 1% unsafe-строк в stdlib**. У Rust unsafe
   анонимен и его сотни тысяч строк.
3. **Capabilities — модель прав, а не конвенция.** `fs-read/net-connect/proc` — на уровне манифеста
   `datara.toml`, `E-CAP-001/002` — ошибки компиляции; в WASM — enforced изолированным модулем.
   В Rust это лишь трейты и дисциплина.
4. **Доказуемые оптимизации.** Evidence Gate: ни один пас не применяется без верификации модуля после.
   В Rust оптимизатор полагается на UB-контракты (`noalias`), нарушение которых = UB в проде.
5. **Метапрограммирование в песочнице.** `expand` не может читать файлы, сеть, время; лимит шагов;
   детерминизм по построению (§14, §18). Proc-макросы Rust исполняются с полными правами процесса.

**Метрика превосходства (публикуется в каждом релизе):** таблица «класс ошибок → пойман на этапе:
компиляция / runtime-trap / недостижим» по корпусу 200 fixtures — Datara против эквивалентного Rust-кода.
Цель: **100% классов закрыты компиляцией или trap, 0 классов UB**.

<!-- CHUNK-C17A-END -->

<!-- ================================================================== -->
<!-- ЧАСТЬ III. MASTER EXECUTION PLAN — до production-ready             -->
<!-- ================================================================== -->

# ЧАСТЬ III. MASTER EXECUTION PLAN — проект, готовый к продакшну

## 29. Текущее состояние (фактический аудит, зафиксирован по живым проверкам)

Этот раздел — **истина, а не реклама**. Исполнитель обязан обновлять статусы по мере работы.

### 29.1 Реализовано и проверено тестами (НЕ переделывать)

| Что | Где | Проверка |
| --- | --- | --- |
| E-OUT-001: запрет печати непечатаемых типов (Map/struct/List) | `types/check/stmt.rs`, `types/check/expr.rs`, `check/call.rs` | 12/12 evidence + живые пробы |
| Typed Format Fusion: `out fmt"..."` zero-alloc, без `concat_N` | `dmir/lowering/out.rs` | 3/3 fusion-тесты |
| TTY-flush: `\n` флашит только в терминале, пайпы через 64КБ буфер, `atexit` | `runtime/datara_runtime.c` | ручные пробы |
| W0102: deprecated `print/println/eprintln` → канон `out/err` | `types/check/call.rs` | тесты диагностики |
| Entity-Guided Evidence Optimization (стабильна) | `optimizer/`, `tests/test_entity_evidence_optimization.rs` | 12/12 |
| Evidence Gate, PCS-детерминизм, 3 бэкенда, `import c`, Python zero-copy | `optimizer/evidence.rs`, `schedule/`, `codegen/` | 268+ тестов |

### 29.2 НЕ реализовано (обязано быть сделано этим планом)

| Что | Состояние | Воркспак |
| --- | --- | --- |
| **`expand` целиком** | **0%**: нет `src/expand/`, нет токена, нет `Decl::Expand` | WP-1 |
| Stale-кэш `forgen check`: смена семантики чекера не инвалидирует кэш (ложный `OK`) | подтверждён баг | WP-0 |
| `Int + Float → Float` молча (`types/check/binary.rs:452`) | дыра аудита | WP-0 |
| `Int` как truthy в `&&/||` (`types/check/binary.rs:429`) | дыра аудита | WP-0 |
| `err fmt"..."` fusion — нет теста | отсутствует | WP-0 |
| `datara_rt_print_str(NULL)` тихо печатает `"None"` | противоречит fail-closed | WP-0 |
| Разделение 5 сущностей на 5 контрактов | спроектировано (§10/§16), не реализовано | WP-2 |
| JIT/AOT паритет: `spawn/channel/slice` падают в `run`, ок в `build` | разрыв | WP-3 |
| `forgen check --fix`, explain-таблицы, FFI-трейс (§23) | частично | WP-4 |
| Размерные гейты бинарей (§25) | не в CI | WP-5 |
| dpm registry + lock + SBOM | нет | WP-6 |
| Прод-либы: http/json/sqlite/log | заглушки | WP-7 |
| `forgen debug` (DAP) | нет | WP-8 |
| Единые доки, README, CONTRIBUTING, CI, релиз | дрейф 4 спек | WP-9 |

## 30. Правила исполнения (обязательны для исполнителя — человека или LLM)

1. **Порядок строгий:** WP-0 → WP-1 → … → WP-9. Внутри WP — шаги по номерам. Пропуск шага = откат.
2. **Один воркспак = серия коммитов** с осмысленными сообщениями; перед коммитом полный
   `cargo test --release` зелёный.
3. **«Сделано» = есть исполняемый тест или команда с ожидаемым выводом.** Пункт без гейта из §31
   не считается сделанным (§28.3 распространяется на всё).
4. **Источник правды — этот файл, а не память диалога.** После любого сброса контекста: перечитай
   §29 и §31, продолжи с первого незакрытого шага.
5. **Лимиты кода:** файлы `src/` < 61 440 байт; `lowering/stmt.rs` уже у лимита — новый код
   (например, expand) только в новых модулях.
6. **Мультибэкенд:** любая фича Части I проверяется на Cranelift-JIT, LLVM и WASM
   (дифференциальный тест вывода), если не помечено иначе.
7. **Обратная совместимость:** новое всегда добавляется как ошибка/предупреждение с миграционным
   `help:` (по образцу W0102), ломающие изменения — только через флаг `-Z compat` и changelog.
8. **Обновляй §29:** закрыл пункт — измени строку таблицы и сделай это частью коммита.

<!-- CHUNK-C18B-END -->

## 31. Воркспаки WP-0…WP-9 — шаги и гейты *(WP-0…WP-3 здесь; WP-4…WP-9 — в продолжении §31 после §27–§28)*

Формат каждого шага: `действие → файлы → гейт (команда + ожидаемый результат)`.

### WP-0 — Доверие: язык не должен врать (≈1-2 дня)

| # | Шаг | Гейт |
| --- | --- | --- |
| 0.1 | Semantic-version ключа кэша: `build.rs` считает хеш `src/types/check/** + src/diagnostics/** + src/dmir/**`, кладёт в `env!("FORGEN_SEMANTIC_HASH")`; `driver/check_cache.rs::compute_key` включает его | тест `test_cache_semantic_invalidation.rs`: запись, собранная «старой семантикой», новым чекером отбрасывается |
| 0.2 | Запрет неявного `Int+Float` в арифметике: `types/check/binary.rs:452` — требовать явный `as Float`/`* 1.0`, ошибка + `help:` | негативный тест `a: Int + b: Float` → ошибка; позитивный `a * 1.0 + b` → OK |
| 0.3 | Запрет `Int` в `&&/||` (`binary.rs:429`): только `Bool` | тест `a: Int && b: Int` → ошибка; `a > 0 && b > 0` → OK |
| 0.4 | Тест `err fmt"..."` fusion: нет аллокации, вывод в stderr | `tests/test_typed_format_fusion.rs` +1 кейс |
| 0.5 | `datara_rt_print_str(NULL)` → trap с сообщением о нарушении инварианта (вместо тихого `"None"`) | прогон: NULL → panic-сообщение, не тихий вывод |
| 0.6 | Полный `cargo test --release`, коммит `fix: language must never lie` | все тесты зелёные |

### WP-1 — `expand` целиком по Части I (≈1-2 недели; THE feature)

Шаги — это §22 в виде гейтов. Реализация строго по §4 (AST), §5-§7 (семантика/интерпретатор), §14 (безопасность).

| # | Шаг | Гейт |
| --- | --- | --- |
| 1.1 | Токен `expand` (`lexer/`), `Decl::Expand`+`ExpandStmt`+IR-литералы (`ast/`, §4), `parse_expand_decl` (`parser/decl.rs`) | `forgen check` парсит `expand ToJson { ... }`; тест `test_expand_parse.rs` |
| 1.2 | Resolver: таблица `expands`, вызов `Type.method()` ищет expand; порядок `impl > behavior > expand`; коды `E-EXPAND-001..004` | несуществующий expand → `E-EXPAND-001` с точным span |
| 1.3 | Модуль `src/expand/mod.rs`: интерпретатор тела поверх `comptime_eval` (бюджет), рефлексия `T.fields` из `resolver.classes`, гигиена по `SymbolId`; выход — `ast::Decl` (AST-as-IR, §0) | acceptance: `expand ToJson<T>` генерит `to_json()` для тестовой struct |
| 1.4 | Вставка сгенерированных decl'ов в `Program` после `expand_derives_and_comptime`, до typecheck (`driver/pipeline.rs:319`) | `user.to_json()` типизируется штатным чекером |
| 1.5 | Кодоген без спец-путей: сгенерённый метод проходит SRA/IPO/Gate | Cranelift `run` печатает JSON; тест |
| 1.6 | Дифференциал трёх бэкендов: Cranelift-JIT = LLVM = WASM побайтно на 5 expand-программах | `tests/test_expand_differential.rs` |
| 1.7 | Security-корпус §14 (12 тестов): без файлов/сети/времени, лимиты, `unsafe expand` | все зелёные |
| 1.8 | `fmt`/`lint`/LSP: сниппеты, go-to-def, completion `T.fields` | LSP-тест; fmt не ломает expand |
| 1.9 | Acceptance §21: 8 программ, `check ≤ 50 мс` каждая | CI-джоба `expand-acceptance` |

### WP-2 — 5 сущностей = 5 контрактов (§10/§16, ≈1 неделя)

| # | Шаг | Гейт |
| --- | --- | --- |
| 2.1 | Парсер: `component` = только поля (метод → `E-COMP-001`); `behavior` = только методы; `struct` = без методов | негативные тесты каждого запрета |
| 2.2 | Resolver: раздельные таблицы; мердж полей только из `using` | тест композиции |
| 2.3 | Чекер: `role` ≠ `TraitDef` (убрать конверсию `check/decl.rs:66`), `role_bounds: (EffectSet, Cap)`; POD-проверка component | тесты `E-ROLE-001/002`, `E-COMP-001` |
| 2.4 | Optimizer: SoA только для `List<Component>`; inline x2 для Pure-behavior; devirt только trait/impl | bench: SoA-кейс ≥ +30% |
| 2.5 | Security: component-Copy разрешён в `parallel for` без `E0943` | тест гонки позитив/негатив |

### WP-3 — JIT/AOT паритет (≈3-5 дней)

| # | Шаг | Гейт |
| --- | --- | --- |
| 3.1 | Либо `spawn/join/channel/slice` в Cranelift-JIT (`jit.rs` reg!-таблица + линковка runtime), либо явный check-warning «requires build» | `run` печатает результат ИЛИ `check` даёт warning — молчания нет |
| 3.2 | Тест-матрица всех прелюдий на run и build | матрица в CI |

## 27. Скорость выше Rust — за счёт большего числа статических фактов *(Часть II — приложение; исполнение продолжается в продолжении §31 ниже)*

Компилятор Rust обязан **доказывать** aliasing, эффекты, чистоту — дорого и не всегда возможно. Datara
получает эти факты **бесплатно из синтаксиса сущностей** (Entity-Guided Optimization):

| Факт из синтаксиса | Оптимизация, которую он разблокирует | Почему Rust не может |
| --- | --- | --- |
| `component` = Copy+POD | автоматический SoA-layout `List<Component>` → unit-stride → векторизация | нет гарантии POD |
| `behavior` = Pure по умолчанию | агрессивный inline + LoopFold без анализа эффектов | чистота не в языке |
| `role` = эффекты+капы в сигнатуре | **effect erasure**: снятие рантайм-чеков прав в hot path | права — рантайм |
| `trait/impl` coherence 1:1 | девиртуализация + `fn__spec_*` специализация (`ipo.rs`) | есть, но без Evidence Gate |
| `require`-контракты | устранение bounds-чеков после доказательства, деление без panic-ветки | контрактов нет |
| `expand` до оптимизатора | генерённый код оптимизируется как рукописный | derive-код за макро-стеной |
| детерминированный PCS | lockstep-симуляции без случайных клоков | нет такой модели |
| Evidence Gate | каждый пас верифицирован → агрессивность без риска UB | риск UB сдерживает |

**Целевые забеги (CI, `docs/data/*.json`, три платформы):** fib, sum 1e8, matmul, sort, hashmap-chain,
to_json — Datara ≥ parity с `rustc -O3`, а на SoA/Pure-кейсах — **+30–80%** (unit-stride векторизация).
Плюс производительность компилятора: dev-цикл ≤ 120 мс против секунд у `rustc`.

## 28. Финальная формула — концепция, доведённая до конца *(Часть II — приложение)*

Всё, что выше, сходится в одно предложение, которое и есть язык нового поколения:

> **Datara — единственный язык, где архитектура сущностей = оптимизация = безопасность = диагностике.**
> Пишешь как на Python, получаешь как в Rust, контролируешь железо как в C, подключаешь чужие библиотеки
> одной строкой, генерируешь код типизированным `expand` в песочнице — и компилятор **доказывает**, что
> это быстро, мало и не взорвётся: `forgen check` за 5 мс, бинарь ≤ 64 КБ, ноль UB, трейс через все
> языки, права на каждую зависимость.

### 28.1 Что уже есть (не обещание, а код в `src/`)

аффинное владение без `<'a`; fail-closed арифметика/гонки/деление/исчерпаемость; Evidence Gate;
PCS-детерминизм; три бэкенда (Cranelift/LLVM/WASM) с дифференциальными тестами; `import c` без Clang;
zero-copy Python-бридж; L1–L4 с единым ABI; контракты `require/ensure/invariant`; 268 тестов.

### 28.2 Что добавляет этот документ (план, приоритизированный)

| Приоритет | Идея | Где |
| --- | --- | --- |
| **P0 (1.4.4)** | `expand` целиком, Часть I §1–§22 | `src/expand/`, пайплайн |
| **P0 (1.4.4)** | P0-фиксы аудита: запрет `Int+Float→Float` молча, `Int` в `&&/||`; единая нумерация `E-XXX` (DX-1) | `types/check/binary.rs`, `diagnostics/codes.rs` |
| **P1 (1.4.4/1.5)** | `--fix`, explain-таблицы, FFI-трейс (§23), размерные гейты (§25) | driver, diagnostics |
| **P1** | разделение 5 сущностей на 5 контрактов (`component`=POD, `behavior`=Pure, `role`=права, `trait`=тип-контракт, `impl`=доказательство) | parser/resolver/checker |
| **P2** | SoA-трансформ, effect erasure, RT-tree DCE на AOT, unsafe-метрика, async-бэкенд | optimizer, security, codegen |

### 28.3 Критерий «готово» (применяется к каждому пункту 28.2)

Правило языка сформулировано → файл реализации указан → тест/CI-гейт существует → бенч/размер/диагностика
проверены на трёх бэкендах. **Пункт без гейта не считается сделанным.** Так работает весь этот документ —
и так работает язык: ничего не принято без доказательства.

**Версия 1.4.4 — это не набор фич. Это утверждение: системный язык может быть одновременно самым простым
для входа, самым быстрым в сборке и самым доказуемым в проде. Остальное — исполнение по плану выше.**

> ⚠️ **ДОКУМЕНТ НЕ ЗАКАНЧИВАЕТСЯ ЗДЕСЬ.** Ниже — продолжение Части III (исполнение):
> **§31 (продолжение): WP-4…WP-9**, **§32: готовые промпты-исполнители**, **§33: Definition of Done**.
> Читатель/исполнитель обязан дойти до `<!-- END OF SPEC -->`. Порядок чтения: §29 → §30 → §31 (WP-0…WP-3,
> выше) → §27–§28 (приложение, только что прочитаны) → §31 (продолжение, WP-4…WP-9) → §32 → §33 →
> **§34–§44 (ЧАСТЬ IV — Embedded / Freestanding, WP-10)**.

## 31 (продолжение). Воркспаки WP-4…WP-9

<!-- §27–§28 выше — приложение Части II. Часть III (исполнение) продолжается здесь. -->

### WP-4 — Диагностика: единая нумерация, `--fix`, FFI-трейс (§23, ≈1 неделя)

| # | Шаг | Гейт |
| --- | --- | --- |
| 4.1 | Единый реестр кодов: слить `E-SYNTAX-001…E-CODEGEN-003` и `E09xx` в один namespace `E-XXX`, alias-таблица старых имён | `forgen explain <code>` работает для 100% кодов; тест полноты реестра |
| 4.2 | `forgen check --fix`: автофиксы E-OUT-001 (подсказка `to_str`/`@derive(Display)`), W0102 (`print→out`), E0941 (вставить `require`), E0310 (добавить `_`), E-TYPE-008 (вставить `* 1.0`) | корпус 20 fixtures: ≥70% чинятся автоматически, diff минимальный |
| 4.3 | FFI-трейс: panic-хук рантайма печатает цепочку `datara:main:12 → rust:serde_json:441 → c:sqlite3:210` | тест: паника в Rust-cdylib даёт 3-строчный трейс |
| 4.4 | DX-2…DX-8 из §23: одна ошибка = одна причина, точка минимальной правки, `--format=json` для LSP/LLM | snapshot-тесты JSON для 5 ключевых ошибок |

### WP-5 — Размер бинарей (§25, ≈3-4 дня)

| # | Шаг | Гейт |
| --- | --- | --- |
| 5.1 | `--size` профиль: `panic=abort`, strip, `/OPT:REF,ICF`, section GC | hello AOT ≤ 64 КБ (Windows + Linux) |
| 5.2 | RT-tree DCE для AOT: недостижимые runtime-функции не линкуются | бинарь с/без List отличается ≥ 20% |
| 5.3 | WASM: вырезка неиспользуемых imports/exports | `to_json.wasm` ≤ 12 КБ |
| 5.4 | CI-гейт размеров: JSON-отчёт `docs/data/size.json`, fail при росте > 5% без обоснования в PR | джоба size-gate зелёная |

### WP-6 — dpm: lock, SBOM, capabilities на зависимость (≈2 недели)

| # | Шаг | Гейт |
| --- | --- | --- |
| 6.1 | `dpm.lock`: пины sha256, resolved-граф, совместим с `datara.toml` | `dpm add git+…` → lock воспроизводим |
| 6.2 | SBOM (CycloneDX 1.5) на `forgen build --release` | SBOM содержит все зависимости + хеши |
| 6.3 | capabilities на зависимость: `[capabilities] fs-read=["assets/*"]` → `E-CAP` на импорт вне прав | тест: зависимость читает вне прав → `E-CAP-002` |
| 6.4 | Локальный registry: `dpm publish` в git-репо + path-registry | второй проект тянет пакет по lock |
| 6.5 | Reproducible build: одна lock → один бинарь-хеш на чистой машине | два CI-раннера: хеши равны |

### WP-7 — Прод-четвёрка библиотек (≈3-4 недели; dogfood `expand`)

| # | Шаг | Гейт |
| --- | --- | --- |
| 7.1 | `std.json`: парсер + сериализатор; `expand ToJson/FromJson` — основной API (живое доказательство Части I) | roundtrip 10k объектов; ≥ parity с serde_json на бенче |
| 7.2 | `std.log`: уровни, structured, zero-alloc форматы через `out fmt` | sink-тест; бенч 1M строк |
| 7.3 | `std.sqlite` через `import c` (без Clang) | CRUD-тест, результат — `Outcome<T>` |
| 7.4 | `std.http`: роутер + epoll/WSAPoll, keep-alive, capability net-listen | echo ≥ 10k req/s на ноутбуке; гонок нет (корпус E0943) |
| 7.5 | Демо-сервис: TODO API на 4 файлах, `forgen build --release`, деплой одним бинарем | fresh-clone → запущенный сервис ≤ 10 мин |

### WP-8 — `forgen debug`: DAP-минимум (≈1-2 недели)

| # | Шаг | Гейт |
| --- | --- | --- |
| 8.1 | DWARF/PDB: имена переменных и line-table уже эмится (`codegen/`) — добить ДА-эмит для locals | gdb/lldb видит `x`, `total`, `sb` |
| 8.2 | DAP-адаптер `forgen debug --dap`: launch, breakpoints, step-over/into, scopes | сессия в VS Code: breakpoint на `main:12`, значение `x` видно |
| 8.3 | `forgen inspect --var x` как fallback без DAP (текстовый watchpoint) | вывод значения на каждой строке, где `x` жив |
| 8.4 | Тест детерминизма под дебагом: тот же вывод, что без дебага | lockstep-прогон бит-в-бит |

### WP-9 — Единые доки + релиз 1.4.4 (≈1 неделя)

| # | Шаг | Гейт |
| --- | --- | --- |
| 9.1 | Одна правда: `LANGUAGE_REFERENCE.md` (канон) + `TUTORIAL.md`; SPEC_V1 → archived, AGENTS.md → правка под реальность (`struct`-канон, `&|`-битовые, `Outcome`-бридж, `import c`) | grep-скрипт: 0 упоминаний `from`-наследования и `extends` в актуальных доках |
| 9.2 | README ≤ 40 строк: quickstart 3 команды, 4 демо (trap, E0941, E0943, E-OUT-001), бенч-таблица, honest status | новичок: fresh clone → hello за ≤ 10 мин |
| 9.3 | CONTRIBUTING.md + 5 good-first-issue (алиасы, explain-тексты, differential-тест, перевод, bench-json) | 2 issues размечены, шаблон PR есть |
| 9.4 | CI: build+test (Win/Linux), expand-acceptance, дифференциал бэкендов, size-gate, bench-smoke | все джобы зелёные на main |
| 9.5 | Трейдмарк-защита: LICENSE MIT OR Apache-2.0, NOTICE с именем Datara/Forgen, git-лог чист | автор 90%+ коммитов виден в `git shortlog` |
| 9.6 | Релиз: тег `v1.4.4`, notes = §29.1 таблица + changelog, артефакты: forgen-{win,linux}, SBOM, size.json | тег поставлен, артефакты приложены |

## 32. Готовые промпты-исполнители (вставляй в новую сессию с доступом к репо)

**Промпт A — WP-0 (язык не врёт):**
> Открой `docs/EXPAND_SPEC_V1.md` (путь: `d:\DATARA\datara + forgen\docs\EXPAND_SPEC_V1.md`). Прочитай §29, §30, §31. Исполни WP-0 шаг за шагом (0.1–0.6), строго по гейтам. Источник правды — файл, не память. После каждого шага: `cargo test --release`. В конце — коммит `fix: language must never lie` + отчёт по гейтам.

**Промпт B — WP-1 (expand):**
> Открой `docs/EXPAND_SPEC_V1.md`. Прочитай Часть I (§0–§22) и §31 WP-1. Исполни шаги 1.1–1.9 по порядку. Каждый шаг = коммит + гейт. Acceptance: `expand ToJson<T>` генерит `to_json()` для struct, `user.to_json()` печатает JSON через `forgen run` на Cranelift, дифференциал трёх бэкендов зелёный (`tests/test_expand_differential.rs`). Не MVP: полный корпус §14 (12 security-тестов) обязателен.

**Промпт C — WP-2…WP-9:**
> Открой `docs/EXPAND_SPEC_V1.md`. Прочитай §29–§31. Продолжи с первого незакрытого WP (сверь с §29.2 и git-логом). Исполняй воркспаки по порядку, гейты обязательны, «пункт без гейта не сделан». Закончи WP — обнови статусы в §29.2 и закоммить.

## 33. Definition of Done — проект готов к продакшну, когда:

| Блок | Критерий | Проверка |
| --- | --- | --- |
| Доверие | WP-0 закрыт: кэш не врёт, `Int+Float`/`Int&&` закрыты, trap вместо тихого NULL | тест-корпус WP-0 зелёный |
| Язык | `expand` (WP-1) + 5 контрактов (WP-2) + JIT/AOT-паритет (WP-3) | acceptance §21 + дифференциал бэкендов |
| DX | единые коды, `--fix` ≥70%, FFI-трейс (WP-4) | snapshot-тесты |
| Продукт | размер-гейты (WP-5), lock+SBOM+caps (WP-6), прод-четвёрка (WP-7) | CI-джобы |
| Тулинг | `forgen debug` DAP (WP-8) | сессия в VS Code |
| Релиз | единые доки, README, CI, тег 1.4.4 (WP-9) | fresh-clone → сервис ≤ 10 мин |

**Правило последней инстанции:** любой пункт, у которого нет исполняемого гейта, не существует — независимо от того, что написано в диалоге. Этот документ — единственный источник правды после сброса контекста: открой §29 → найди первый незакрытый WP → исполняй.

---

<!-- ================================================================== -->
<!-- ЧАСТЬ IV. EMBEDDED / FREESTANDING MASTER PLAN (WP-10)              -->
<!-- ================================================================== -->

# ЧАСТЬ IV. EMBEDDED / FREESTANDING — Master Plan (WP-10)

Связь с kernel-лестницей (§16 Части I): это **ступень 1** — freestanding-таргет. После неё открываются
ступени 3 (eBPF) и 4 (userspace drivers / DPDK), а Linux mainline остаётся дальней — но уже заслуженной.

## 34. Позиция и конкурентный ландшафт

Факт по коду: `@bare_metal` и `register Timer2 at 0x4000_0000 { field: UInt16 at 0x00 }` **уже парсятся**
(`tests/test_bare_metal_embedded.rs`), но только в check-mode. LLVM-бэкенд знает только x86_64/AArch64.
Freestanding-исполнения нет. Задача Части IV: от «типизируется» до «мигает светодиодом» — с нововведениями,
которых нет у конкурентов.

| Боль в Rust embedded / C / Zephyr | Решение Datara |
| --- | --- |
| Rust: HAL привязан к таргету, драйвер не протестируешь на ноутбуке | **E1: Host-First Hardware TDD** — shadow-MMIO, один тест host↔QEMU бит-в-бит |
| Rust/C: переполнение стека = порча памяти в рантайме | **E2: статическое доказательство стека** (call graph × frame ≤ бюджет) — никто не имеет |
| C: ISR-дисциплина «на честном слове» (MISRA глазами) | **E3: ISR-контракты, проверяемые статически** (@no_alloc/@no_panic уже есть) |
| svd2rust сложен, генерит нечитаемый код | **E4: `forgen svd-gen`** — детерминированные typed register-блоки в .dtr |
| Два драйвера пишут в один регистр → race на железе | **E5: MMIO provenance** — один владелец региона, конфликт на этапе сборки |
| Одно устройство = отдельный тулчейн/сборка | **E6: один исходник — host и MCU** |
| Переполнение flash узнаётся на линковке | **E7: Flash/RAM бюджет из chip-db до линковки** (E-EMB-006) |
| Драйвер-конфликты и «кто владеет UART» | **E8: пакет-драйвер + отдельный железный контракт `hw.contract.toml`, проверяемый компилятором** (§34.1) |

### 34.1 Нормативно: Sparks — только библиотеки и фреймворки (железо в реестр не попадает)

**Правило.** Sparks (протокол, реестр, `CAPABILITIES_SPEC`) описывает **только программные пакеты**:
библиотеки и фреймворки. Ни чипы, ни платы, ни драйверные железные ресурсы через Sparks не
распространяются, и hardware-поля в `CAPABILITIES_SPEC` не добавляются — никогда.

Обоснование по фактам реестра (`D:\DATARA\sparks`):

- Словарь capabilities Sparks — чисто userspace: `network:inbound/outbound`, `filesystem:read/write`,
  `ffi:native`, `process:spawn`. На MCU нет ни файлов, ни сокетов, ни процессов — «пины» и «регистры»
  в этом словаре сломали бы протокол ради одной платформы.
- `determinism_receipt.target` в `PROTOCOL.md` — host-centric (`x86_64-pc-windows-msvc`); реестр не
  является моделью железа.
- `dpm` не знает слов `hardware` / `board` / `chip` / `target_triple`, и это правильно.

**Три слоя, где реально живёт железо (ни один из них — не Sparks):**

| Слой | Артефакт | Владелец | Что описывает |
| --- | --- | --- | --- |
| Устройство | `datara-chips.toml` (chip-db) | toolchain (`forgen chip add` / импорт SVD/`.dts`) | `flash`, `ram`, `stack`, `clock`, `uart`, `timer`, `gpio[]`, `irq[]`, `hw_rev` конкретного MCU |
| Проект | `datara.board.toml` — рядом с `datara.toml` | проект пользователя | выбранный чип + ревизия, бюджеты памяти и **закрепление ресурсов за владельцами** (`[assign.*]`). Источник истины для E5/E7 |
| Драйвер | `hw.contract.toml` внутри пакета | автор пакета | что драйверу нужно: `[ports]`, `[irq]`, `[regions]`, `[hw_rev]` |

**Разделение обязанностей:**

- `dpm` / Sparks валидируют **код**: версии, зависимости, userspace-capabilities, determinism-receipt.
- `forgen` валидирует **железо**: chip-db ↔ `datara.board.toml` ↔ `hw.contract.toml` установленных
  драйверов. Все `E-EMB-*` — диагностика forgen.
- `dpm` при установке копирует `hw.contract.toml` локально, но **не валидирует его и не включает в
  Sparks-индекс**. Sparks о железе не знает — как crates.io не знает про `#[repr]`.

**Почему так лучше, чем «hardware в sparks-манифесте»:** реестр остаётся универсальным (одна схема для
всех платформ, серверные и embedded-пакеты не расходятся); железо проверяется там, где есть компилятор
и chip-db; MCU-семантика не протекает в контракт, от которого зависят серверные пакеты.

## 35. Freestanding target — технические требования (норматив)

### 35.1 Цели

| MCU | Таргет LLVM | Приоритет |
| --- | --- | --- |
| Cortex-M4F (STM32F4, RP2040-ядра… нет, RP2040=M0+) | `thumbv7em` | 1 (FPU, шире всего) |
| Cortex-M0/M0+ (STM32F0, RP2040) | `thumbv6m` | 2 |
| Cortex-M3 | `thumbv7m` | 3 |
| RISC-V RV32IMC (ESP32-C3, CH32) | `riscv32imc-unknown-none-elf` | 4 |

Cranelift остаётся host-only (dev-цикл). MCU — через LLVM backend (`codegen/llvm`): добавить
`Arch::Thumbv6m/Thumbv7m/Thumbv7em/Riscv32` в `codegen/target.rs` и triple-маппинг в `llvm/mod.rs`.

### 35.2 Runtime-модель (норматив)

1. **Ноль `datara_rt_*`** в freestanding-бинаре. Всё, что тянет runtime, — ошибка `E-EMB-008`.
2. Entry — не `main()`, а `@reset fn` (reset handler): вызывается из vector table.
3. **Нет TLS, нет heap**: память только `@arena`/`scratch_*` (существуют) или статические буферы.
4. **Нет unwinding**: trap = запись в `_FAULT`-регистр + сброс (или `abort` по выбору в datara.toml).
5. `out`/`fmt` в freestanding → затыкаются на `@uart`-символ из chip-db (лог через ring buffer), в ISR — запрещены (E-EMB-008).
6. Стек фиксированный: `@stack(4096)` в `datara.toml`, размер доказывается (§E2).

### 35.3 Constraint-система (компилятор)

* **FP-гейтинг:** Float-операции разрешены только в функциях `@float`-доступных; в `@isr` — ошибка.
* **ISR-контракты:** `@isr fn` требует транзитивно `@no_alloc` (E0950) и `@no_panic` (E0951) — они
  **уже реализованы** в `security/verify_expr.rs`, Часть IV только расширяет область на freestanding.
* **Стек-доказательство:** Evidence Gate pass `stack-proof`: call graph × frame sizes (из codegen) против
  бюджета; max(host) + max(каждого @isr) ≤ бюджет, иначе `E-EMB-004`.

<!-- CHUNK-C19-END -->

## 36. WP-10 — воркспаки EMB-0…EMB-9 *(Часть IV; правила гейтов — §30)*

WP-10 закрывает freestanding/embedded — **ступень 1** kernel-лестницы (§44). Порядок обязателен:
EMB-N+1 не начинается, пока гейт EMB-N не зелёный и не закоммичен.

### EMB-0 — Target layer: 4 новые архитектуры + `--target` (≈3 дня)

| # | Шаг | Файл | Гейт |
| --- | --- | --- | --- |
| 1 | `Arch::{Thumbv6m,Thumbv7m,Thumbv7em,Riscv32imc}` | `src/codegen/target.rs` | unit-тесты target-слоя |
| 2 | Triple-маппинг + FPU/ABI (`thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf`) | `src/codegen/llvm/mod.rs` | `forgen build --target thumbv7em --emit obj x.dtr` → ARM ELF |
| 3 | Секция `[target]` в `datara.toml` (`arch`, `fpu`, `abi`, `entry`, `linker`, `stack`) | `src/project/manifest.rs` | `forgen inspect --target` печатает резолв |
| 4 | Явный отказ Cranelift на MCU-таргете | `src/codegen/cranelift/mod.rs` | `E-EMB-001` на `--jit --target thumbv7em` |

**E6 (одна программа — host и MCU):** тот же `.dtr` собирается и как host-бинарь для теста, и как MCU-объект.
Разница только в `datara.toml`. Ноль `#ifdef`, ноль второй копии драйвера.

### EMB-1 — Freestanding-профиль (≈1.5 недели)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | `--freestanding` запрещает любой вызов `datara_rt_*` | ELF линкуется без runtime-объекта, иначе `E-EMB-008` |
| 2 | `@reset fn` — точка входа; vector table генерируется из набора `@isr` | `forgen build --freestanding --emit elf`, `nm` видит таблицу |
| 3 | Linker script из chip-db (flash/ram/stack, heap = none) | `.ld` в `target/`, `forgen size` читает секции |
| 4 | Trap = запись в `_FAULT` + reset, без unwind | `E-EMB-009` при unwind-пути в freestanding |

### EMB-2 — Chip-db: `datara-chips.toml` (≈4 дня)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | Формат: `flash`, `ram`, `stack`, `clock`, `uart`, `timer`, `gpio[]`, `irq[]`, `hw_rev` | парсится, `datara chips lint` чист |
| 2 | `forgen chip add stm32f407` и импорт из `.dts` | `forgen chip info` печатает карту памяти |
| 3 | Проект привязывает чип: `[board] chip = "stm32f407@rev.C"` в `datara.board.toml` | несовпадение `hw_rev` → `E-EMB-010` |

### EMB-3 — Память без GC и без heap (≈1 неделя)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | `@static_buf { }` — именованные статические буферы с размером из comptime | `forgen size --sections` |
| 2 | `@arena(capacity: N)` — единственный динамический путь; `List`/`Map` вне arena запрещены | `E-EMB-008` на `List` без arena/static |
| 3 | Транзитивная проверка «нет скрытой аллокации» от `@reset` | Evidence Gate pass `no-alloc` |

### EMB-4 — Host-First Hardware TDD — E1 (≈1.5 недели)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | `forgen test --mock-hw` — shadow-MMIO на host-таргете | драйвер тестируется на ноутбуке |
| 2 | Один тест-файл, два рантайма: `--target host --mock-hw` и `--target qemu` | идентичный вывод, бит-в-бит |
| 3 | Матрица MMIO-эффектов (read/write/rmw/irq) в отчёте | `forgen test --report=json` |

**E1 — главное нововведение Части IV.** В Rust embedded идиома «HAL + feature-флаги + `#[cfg(test)]`»:
драйвер тестируется только на целевой железке или на эмуляторе HAL, но не на настоящем регистре.
В Datara регистр — типизированный объект, поэтому подменяется **одним флагом** без правки кода, а тест
обязан дать идентичный результат на host и в QEMU. Такого TDD для драйверов нет ни у Zephyr, ни у Embassy.

<!-- CHUNK-C20-END -->

### EMB-5 — Статическое доказательство стека — E2 (≈1 неделя)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | Codegen отдаёт frame size каждой функции (`stack_frame_bytes` в DMIR/LLVM) | отчёт по всем `fn` |
| 2 | Evidence Gate pass `stack-proof`: обход call graph, `sum(frames)` на пути | `E-EMB-004` при переполнении бюджета |
| 3 | `forgen stack-report --format=json` — цепочки вызовов + запас | отчёт воспроизводим |
| 4 | Проверка ISR-контекста отдельно: `max(@reset-путь) + max(каждый @isr) ≤ @stack` | тест на вложенный `@isr` |

**E2 (нововведение):** ни Rust, ни C не знают размер стека статически — переполнение ловится рантаймом
(если вообще ловится). Datara **доказывает** его на этапе сборки, как типы. Рекурсия в этой модели
допускается только с `@bounded(n)` (шаг на цикл), иначе `E-EMB-004`.

### EMB-6 — `forgen svd-gen`: SVD → типизированные регистры — E4 (≈1 неделя)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | Парсер SVD XML (`peripherals/registers/fields/enumeratedValues`) | round-trip на 3 реальных SVD |
| 2 | Генерация `.dtr` с `register`-блоками + typed enum-полями | компилируется без правок |
| 3 | Детерминизм: два прогона → идентичный файл (сортировка, ноль timestamp) | CI diff-гейт |
| 4 | Опционально `--cpp=false` (без C-заголовков вообще) | заголовков нет |

**E4 (нововведение):** `svd2rust` генерирует Rust-код, читаемый только после `cargo expand`, и требует
всю свою HAL-экосистему. У Datara вывод — обычный `.dtr`, который человек читает и правит, а `expand`
(§0–§22) позволяет при желании заменить генератор своим.

### EMB-7 — MMIO provenance: один владелец региона — E5 (≈1 неделя)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | `register X at ADDR { }` регистрирует регион в таблице владения | `forgen inspect --mmio-map` |
| 2 | Пересечение регионов двух модулей/пакетов → ошибка | `E-EMB-005` с указанием обоих владельцев |
| 3 | Hardware-ресурсы попадают в capabilities пакета (§34, E8) | `forgen verify --hardware` |

**E5 (нововведение):** в C два драйвера пишут в один регистр — это UB без диагностики. Здесь конфликт
регионов — **ошибка сборки**, а не баг прошивки в поле.

### EMB-8 — Бюджет Flash/RAM до линковки — E7 (≈4 дня)

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | Размеры секций считаются из DMIR + layout до линковки | `forgen size --budget` |
| 2 | Сравнение с chip-db: `flash`, `ram`, `stack` | `E-EMB-006` при превышении |
| 3 | Отчёт: топ-10 крупнейших символов, что ужимать | человекочитаемый вывод + json |

**E7 (нововведение):** обычно «не влезло» узнают от линкера в конце, на непонятном языке. Datara
сравнивает бюджет с chip-db **до** линковки и показывает, *чем* перебор.

### EMB-9 — Драйвер + железный контракт — E8 (≈1 неделя)

Драйвер ставится как обычный sparks-пакет (**код**), а его требования к железу лежат в отдельном
`hw.contract.toml` и проверяются **forgen**, а не реестром (§34.1).

| # | Шаг | Гейт |
| --- | --- | --- |
| 1 | Формат `hw.contract.toml`: `[ports]`, `[irq]`, `[regions]`, `[hw_rev]` | парсится; `forgen inspect --hw-contract` печатает требования драйвера |
| 2 | `datara.board.toml` в проекте: `[board] chip` + `[assign.*]` (ресурс → владелец) | `forgen verify --hardware` |
| 3 | `dpm add spark:drv-bmi270` — код библиотеки; `hw.contract.toml` копируется локально | `dpm install` + `forgen build`; **Sparks-индекс побайтово не изменён** |
| 4 | Конфликт: два драйвера на один ресурс → ошибка с обоими владельцами | `E-EMB-005` + тест на конфликт |
| 5 | Версионирование по `hw_rev`: `supports` в контракте vs ревизия в board | несовпадение → `E-EMB-010` |

**E8 (нововведение):** модель железа живёт в проекте и проверяется компилятором, а реестр остаётся
чистым — только библиотеки и фреймворки. `cargo` не знает, что у тебя один UART; `dpm` тоже не знает —
знает `forgen` по chip-db и `datara.board.toml`.

<!-- CHUNK-C21-END -->

## 37. Нововведения E1–E8 — нормативная детализация

Это то, чего нет ни у Rust embedded, ни у Zephyr, ни у C. Каждый пункт: синтаксис → что проверяет
компилятор → чем лучше существующего → гейт.

### 37.1 E1 — Host-First Hardware TDD

```datara
register Timer2 at 0x4000_0000 {
    cr1:  UInt32 at 0x00
    sr:   UInt32 at 0x10  // битовые поля через packet/`in bits`
    arr:  UInt32 at 0x2C
}

@isr fn on_timer() {
    Timer2.sr.clear_irq()
    ticks = ticks + 1
}

fn setup(period: UInt32) / IO {
    Timer2.arr.write(period)
    Timer2.cr1.write(CEN)
}
```

```bash
forgen test --mock-hw drv_timer_test.dtr        # host, shadow-MMIO, ~40мс
forgen test --target qemu --machine stm32f4     # тот же файл, реальный MMIO
```

**Что проверяет компилятор:** одинаковый тип доступа на обоих таргетах; `@isr` без `@no_alloc`/`@no_panic`
запрещён; `rmw`-последовательности не оптимизируются в одну запись (MMIO — `volatile` по построению).
**Чем лучше:** в Rust/Embassy тест драйвера требует `embedded-hal-mock` + второй набор типов; здесь —
один флаг `--mock-hw`, ноль правок кода. **Гейт:** идентичный вывод host vs QEMU (diff-гейт в CI).

### 37.2 E2 — статическое доказательство стека

```datara
@stack(4096)

fn parse(view buf: List<UInt8>) -> Outcome<Frame> { ... }

@isr fn on_uart() {          // E-EMB-004, если путь превышает бюджет
    handle_byte()
}
```
**Проверяет:** `frame(fn)` × путь вызовов ≤ `@stack`; рекурсия требует `@bounded(n)`.
**Чем лучше:** Rust/C ловят переполнение в рантайме (если есть MPU-сторож) или не ловят вовсе — тут
**доказательство на сборке**, как borrow-checker, только для стека. **Гейт:** `forgen stack-report` + тест
на намеренный перебор.

### 37.3 E3 — ISR-контракты (расширение уже реализованного)

```datara
@isr fn on_adc() / IRQ(priority: 2) { ... }   // транзитивно @no_alloc + @no_panic
```
**Проверяет:** `E0950`/`E0951` (уже в `security/verify_expr.rs`) + запрет `Float` в ISR без `@float`,
запрет блокирующих `await`, запрет `out` (лог только в ring buffer).
**Чем лучше:** MISRA-правила для ISR проверяются людьми и линтерами; здесь — компилятором, и нарушение
не собирается. **Гейт:** security-корпус §39, тесты 1–4.

### 37.4 E4 — `forgen svd-gen` (детерминированный)

```bash
forgen svd-gen STM32F407.svd --out src/chip/stm32f407.dtr --rev C
```
**Проверяет:** round-trip против SVD, детерминизм вывода (два прогона → байт-идентичный файл).
**Чем лучше:** `svd2rust` даёт нечитаемый код + требует PAC-экосистему; здесь — человекочитаемый `.dtr`,
который можно править и расширять через `expand`. **Гейт:** CI diff-гейт + компиляция сгенерированного файла.

### 37.5 E5 — MMIO provenance (один владелец региона)

```datara
register Uart1 at 0x4001_1000 { ... }   // владелец региона зафиксирован
```
**Проверяет:** пересечение регионов двух пакетов → `E-EMB-005` с обоими владельцами; доступ вне
объявленного региона → `E-EMB-002`.
**Чем лучше:** в C это UB без диагностики, в Rust — только через `unsafe` и договорённости.
**Гейт:** тест «два драйвера на один UART» падает сборкой.

### 37.6 E6 — одна программа, host и MCU

Один `.dtr` → `--target host` (тест, JIT 40мс) и `--target thumbv7em` (прошивка). Различие только в
`datara.toml`. **Чем лучше:** Rust embedded требует `#[cfg]`-ветки и отдельные крейты; C — `#ifdef`-леса.
**Гейт:** один source, две сборки в CI, один тест-файл.

### 37.7 E7 — бюджет Flash/RAM до линковки

```toml
# datara.board.toml (проект) — железная привязка; в реестр не идёт (§34.1)
[board]
chip = "stm32f407@rev.C"
flash_budget = "512K"
ram_budget   = "128K"
```
**Проверяет:** `E-EMB-006` при превышении + отчёт «топ символов». **Чем лучше:** ошибка приходит до
линковки и объясняет причину, а не `region 'FLASH' overflowed by 4120 bytes`. **Гейт:** тест на перебор.

### 37.8 E8 — драйверы: код из реестра, железный контракт в проекте

**Граница (см. §34.1):** Sparks — только библиотеки и фреймворки. Драйвер — библиотека, поэтому его
**код** ставится из Sparks как обычный пакет. Но требования к железу в реестр не идут: они лежат в
отдельном файле `hw.contract.toml` **внутри пакета**, который после установки проверяет `forgen`, а не `dpm`.

Манифест в реестре — это **JSON** (`packages/drv-bmi270.json`, схема Sparks v1, как у
`D:\DATARA\sparks\packages\lockstep_engine.json`), и он не содержит ни одного железяного поля:

```json
{
  "schema": 1,
  "name": "sparks/drv-bmi270",
  "version": "1.2.0",
  "tarball_url": "tarballs/drv-bmi270-1.2.0.tar",
  "sha256": "…",
  "public_key": "…",
  "signature": "…",
  "key_id": "datara-core-2026-v2",
  "capabilities": [],
  "dependencies": {},
  "tags": ["driver", "embedded", "i2c"]
}
```

Железные требования драйвера лежат **внутри tarball'а** отдельным файлом `hw.contract.toml`, который
`forgen` читает после установки, а `dpm` — только копирует. В реестр и в индекс этот файл не попадает:

```toml
# hw.contract.toml — внутри пакета; для компилятора, в реестр НЕ отправляется
[ports]
i2c = ["i2c1@0x68"]
[irq]
lines = ["EXTI4"]
[hw_rev]
supports = ["A", "B", "C"]
```

Соответствие ресурса и его владельца держит `datara.board.toml` в проекте:

```toml
# datara.board.toml (проект, не пакет)
[board]
chip = "stm32f407@rev.C"
[assign.i2c1]
owner = "drv-bmi270"
```

**Проверяет:** `forgen verify --hardware` — два драйвера на один ресурс → `E-EMB-005`; `hw_rev` чипа не
поддерживается контрактом → `E-EMB-010`.
**Чем лучше:** `cargo` не знает про железо вообще, а `crates.io` не смог бы его описывать универсально.
У нас код универсален (одна схема реестра для серверных и MCU-пакетов), а железо — локальный контракт,
который проверяет компилятор. **Гейт:** тест на конфликт ресурсов между двумя пакетами + тест
«Sparks-индекс побайтово не изменён после `dpm add`».

### 37.9 Сводка: где Datara строго лучше

| Ось | Rust embedded | C + MISRA | Datara |
| --- | --- | --- | --- |
| Тест драйвера на host | через mock-HAL, второй набор типов | вручную, редко | `--mock-hw`, один флаг |
| Стек | рантайм-сторож / не ловится | не ловится | **доказательство на сборке** |
| ISR-дисциплина | `unsafe`, соглашения | линтер + ревью | контракты компилятора |
| Регистры | PAC (нечитаемо) | `#define` + макросы | typed `.dtr` + SVD-ген |
| Конфликт регионов | договорённость | не ловится | **ошибка сборки** |
| Размер | линкер в конце | линкер в конце | **бюджет до линковки** |
| Драйверы | crates без модели железа | копипаста | код-пакет из реестра + hw-контракт, проверяет компилятор |
| Одна программа host+MCU | `#[cfg]`-леса | `#ifdef`-леса | один source, один toml |

<!-- CHUNK-C22-END -->

## 38. Диагностика embedded — коды `E-EMB-*` (норматив)

Единая нумерация по §23 (DX-1). Каждый код обязан иметь `help:` и запись в `forgen explain`.

| Код | Условие | `help:` (кратко) |
| --- | --- | --- |
| `E-EMB-001` | MCU-таргет на неподдерживаемом бэкенде (Cranelift JIT) | собери с `--target <arch>` (LLVM) |
| `E-EMB-002` | Доступ к адресу вне объявленного `register`-региона | объяви регион или поправь смещение |
| `E-EMB-003` | `@isr` использует `Float` в ядре без `@float` | вынеси FPU-часть в `@float fn` |
| `E-EMB-004` | Стек-бюджет превышен (E2) | сократи путь вызовов или подними `@stack(n)` |
| `E-EMB-005` | Пересечение MMIO-регионов у двух владельцев | слей регионы или смени адрес |
| `E-EMB-006` | Flash/RAM бюджет превышен (до линковки) | см. топ символов в `forgen size --budget` |
| `E-EMB-007` | Обращение к hardware-ресурсу без контракта | добавь ресурс в `hw.contract.toml` пакета (§34.1) |
| `E-EMB-008` | Runtime/аллокация запрещены в freestanding | замени на `@static_buf`/`@arena` |
| `E-EMB-009` | Путь с unwinding в freestanding | убери `Outcome`-пропагацию через ISR или включи `abort` |
| `E-EMB-010` | `hw_rev` чипа не поддерживается драйвером | обнови драйвер или укажи `supports` в `hw.contract.toml` |

**Требование DX-2 (§23):** одна ошибка = одна причина. Если регион не объявлен и адрес вне диапазона —
это `E-EMB-002`, а не «две ошибки типов».

## 39. Security-корпус WP-10 (обязательные негативные тесты)

Каждый тест должен **падать сборкой** с указанным кодом. Без этих тестов воркспак не закрыт (§30).

| # | Тест | Ожидание |
| --- | --- | --- |
| 1 | `@isr` вызывает функцию с heap-аллокацией | `E-EMB-008` + `E0950` |
| 2 | `@isr` использует `Float` без `@float` | `E-EMB-003` |
| 3 | `@isr` вызывает `panic`-путь | `E0951` |
| 4 | `@isr` печатает через `out` | `E-EMB-008` (лог только ring buffer) |
| 5 | Путь от `@reset` превышает `@stack` | `E-EMB-004` |
| 6 | unbounded-рекурсия в freestanding без `@bounded` | `E-EMB-004` |
| 7 | Второй пакет объявляет тот же `register`-регион | `E-EMB-005` |
| 8 | Запись по адресу вне региона | `E-EMB-002` |
| 9 | Драйвер без `hw.contract.toml` обращается к UART | `E-EMB-007` |
| 10 | Прошивка 600K при `flash_budget = 512K` | `E-EMB-006` |
| 11 | `hw_rev = "C"`, драйвер поддерживает только `"A"` | `E-EMB-010` |
| 12 | `List` без `@arena`/`@static_buf` в freestanding | `E-EMB-008` |
| 13 | `--jit --target thumbv7em` | `E-EMB-001` |
| 14 | Freestanding-путь с unwinding | `E-EMB-009` |

**Плюс позитивные тесты:**
П1 `--mock-hw` проходит и даёт тот же результат, что QEMU (diff-гейт).
П2 `svd-gen` дважды → байт-идентичный файл.
П3 `forgen size --budget` для сборки в бюджет — зелёный, отчёт воспроизводим.
П4 `forgen stack-report` воспроизводим и сходится с реальным пиком стека в QEMU (проверка канарейкой).

<!-- CHUNK-C23-END -->

## 40. Критерии приёмки WP-10 (CI-blocking, полный список)

WP-10 не «в основном готов» — он готов, когда **все** пункты зелёные. Каждый пункт проверяется командой из колонки «Гейт».

| # | Критерий | Гейт (исполняемая команда / тест) |
| --- | --- | --- |
| 1 | 4 новых таргета собирают объектник | `forgen build --target {thumbv6m,thumbv7m,thumbv7em,riscv32imc} --emit obj` |
| 2 | Freestanding ELF без runtime-объекта | `nm out.elf \| grep datara_rt` → пусто |
| 3 | JIT на MCU-таргете запрещён | `--jit --target thumbv7em` → `E-EMB-001` |
| 4 | Mock-MMIO тест идентичен QEMU | diff-гейт `test --mock-hw` vs `--target qemu` (бит-в-бит) |
| 5 | Стек доказан статически | `forgen stack-report --format=json` сходится с канарейкой QEMU (±5%) |
| 6 | Flash/RAM бюджет проверяется до линковки | `forgen size --budget` → `E-EMB-006` на переполнении |
| 7 | Один владелец MMIO-региона | негативный тест §39 #7 → `E-EMB-005` |
| 8 | Capabilities на железо | негативный тест §39 #9 → `E-EMB-007` |
| 9 | `svd-gen` детерминирован | два прогона → `sha256` совпадает |
| 10 | `@isr`-контракты держат 4 негативных теста | §39 #1–#4 падают с указанными кодами |
| 11 | Одна программа — host и MCU (E6) | один `.dtr`, два манифеста, оба собираются |
| 12 | Драйвер: код-пакет из реестра + hw-контракт (E8) | `dpm add spark:drv-bmi270` → `forgen verify --hardware` + `forgen build`; Sparks-индекс не изменён |
| 13 | Диагностика полная | все `E-EMB-001…010` имеют `help:` и `forgen explain` |
| 14 | Размер freestanding hello | ≤ 4 КБ flash (см. §43) |
| 15 | Никаких new warnings | `cargo clippy -- -D warnings` + `forgen lint` чист |

**Правило §30 действует целиком:** пункт без исполняемого гейта не существует. Пункт с гейтом «проверено вручную» — не существует. Пункт, который «пройдёт после WP-N» — не существует сегодня.

### 40.1 Что НЕ входит в WP-10 (явные границы, чтобы не расползалось)

| Не делаем | Почему | Куда отложено |
| --- | --- | --- |
| Гарантии реального времени (WCET-анализ worst-case) | отдельная научная задача, требует абстрактной интерпретации по циклам | §44, ступень 2.5 |
| Multicore/AMP на MCU | нет модели памяти per-core | §44, ступень 2–3 |
| Поддержка Cortex-A / MMU / кэшей | это уже не MCU-профиль | §44, ступень 4 |
| Загрузчик/OTA (двойной слот, подписи) | инфраструктура, не язык | отдельный воркспак WP-11 (не в 1.4.4) |
| Логгирование через RTT/ITM | платформенная деталь; ring buffer из E3 закрывает 80% | §44 |
| Полная экосистема драйверов | экосистема растёт сама: код публикуется в Sparks как библиотеки (E8) | после релиза |

## 41. Порядок реализации WP-10 (dependency-true, один проход, без переделок)

Порядок не косметический: каждый шаг **разблокирует** следующий. Нарушение порядка = переделка.

```
EMB-0 (targets) ──► EMB-1 (freestanding) ──► EMB-3 (память)
        │                     │                     │
        │                     ├──► EMB-2 (chip-db)──┤
        │                     │                     │
        └─────────────────────┴─────────────────────┴──► EMB-4 (E1 mock-hw)
                                                              │
                            ┌─────────────────────────────────┤
                            │                                 │
                       EMB-5 (E2 stack)                  EMB-6 (E4 svd-gen)
                            │                                 │
                            └──────────► EMB-7 (E5 MMIO) ◄──────┘
                                              │
                                         EMB-8 (E7 budget)
                                              │
                                         EMB-9 (E8 drivers)
```

| Шаг | Зависит от | Разблокирует | Оценка | Гейт-коммит |
| --- | --- | --- | --- | --- |
| EMB-0 | — | всё остальное | 3 дня | `feat(emb): target layer thumb/riscv` |
| EMB-1 | EMB-0 | EMB-3, EMB-4 | 1.5 нед | `feat(emb): freestanding profile` |
| EMB-2 | EMB-0 | EMB-8 | 4 дня | `feat(emb): chip-db` |
| EMB-3 | EMB-1 | EMB-4, EMB-6 | 1 нед | `feat(emb): static_buf + arena` |
| EMB-4 | EMB-3, EMB-1 | EMB-5, EMB-6 | 1.5 нед | `feat(emb): E1 host-first TDD` |
| EMB-5 | EMB-4 | EMB-7 | 1 нед | `feat(emb): E2 stack proof` |
| EMB-6 | EMB-4 | EMB-7 | 1 нед | `feat(emb): E4 svd-gen` |
| EMB-7 | EMB-5, EMB-6 | EMB-8 | 1 нед | `feat(emb): E5 MMIO provenance` |
| EMB-8 | EMB-7, EMB-2 | EMB-9 | 4 дня | `feat(emb): E7 flash/ram budget` |
| EMB-9 | EMB-8, EMB-2 | релиз WP-10 | 1 нед | `feat(emb): E8 driver hw-contract` |

**Итого: ≈ 10–11 недель** одного исполнителя. EMB-5 и EMB-6 можно вести параллельно (не пересекаются по файлам), если есть второй человек — тогда ≈ 8 недель.

### 41.1 Точки конфликта с Частью I–III (проверить перед началом)

| Область | Пересечение | Что сделать заранее |
| --- | --- | --- |
| `src/codegen/target.rs` | используется `expand` (§8) для рефлексии таргета | EMB-0 после WP-1, иначе конфликт правок |
| `security/verify_expr.rs` | `@no_alloc` расширяется до `E-EMB-008` | EMB-3 после WP-0 (fix кэша) |
| `project/manifest.rs` | `[target]` в проекте + чтение `datara.board.toml` рядом с `[capabilities]`; **в sparks-манифест железа не добавляется** (§34.1) | EMB-0/EMB-2 синхронно с WP-6 (dpm) |
| `optimizer/evidence.rs` | новые пассы `stack-proof`, `no-alloc` | EMB-5/EMB-3 — те же гейты, что WP-1 |
| `diagnostics/codes.rs` | 10 новых кодов `E-EMB-*` | единая нумерация §23, без дублей |

**Вывод по планированию:** WP-10 запускается **после** WP-0 (доверие) и **параллельно** WP-6/WP-7 (dpm/прод-либы), но **не раньше** WP-1 (`expand`) — иначе `svd-gen` (EMB-6) придётся переделывать под expand-модель.

### 41.2 Готовый промпт-исполнитель для WP-10 (вставляй в новую сессию с доступом к репо)

```
Ты — инженер-исполнитель компилятора Datara/Forgen v1.4.4.
Файл-контракт: docs\EXPAND_SPEC_V1.md

ПОРЯДОК:
1. Прочитай §30 (правила исполнения) и §29 (текущее состояние).
2. Прочитай §34 → §35 → §36 → §37 → §38 → §39 → §40 → §41 → §42 → §43.
3. Убедись, что WP-0 (§31) и WP-1 (§31 прод., expand) закрыты. Если нет — СТОП,
   сообщи какие гейты красные, не начинай WP-10.
4. Возьми первый незакрытый EMB-N из §36 и реализуй его целиком:
   - шаги строго по таблице воркспака;
   - каждый шаг под свой гейт;
   - негативные тесты из §39, относящиеся к этому EMB-N;
   - коммит формата 'feat(emb): ...' из таблицы §41.
5. Прогони гейт. Красный → чини. Зелёный → коммит → следующий EMB-N.

ЗАПРЕЩЕНО:
- пометить воркспак сделанным без зелёного гейта;
- заменить гейт на «проверено вручную»;
- начать EMB-N+1 при красном EMB-N;
- реализовать только часть воркспака и объявить его готовым.

ОТЧЁТ в конце: таблица EMB-N / статус / команда-гейт / результат.
Если дошёл до конца §40 (все 15 критериев зелёные) — WP-10 закрыт, сообщи явно.
```

<!-- CHUNK-C25B-END -->

## 42. Бэкенды для embedded — что компилирует что (норматив)

Ключевое правило: **MCU-таргеты — только LLVM.** Cranelift не имеет ни thumb/RISC-V, ни freestanding-модели, ни control over section layout. Это не «временно», это архитектурно.

| Бэкенд | Host (x86_64/aarch64) | MCU (thumb/riscv) | Роль в WP-10 |
| --- | --- | --- | --- |
| **LLVM** | да | **да** | основной для embedded: объектник, секции, `--emit elf` |
| **Cranelift JIT** | да | **нет → `E-EMB-001`** | только `forgen test --mock-hw` на host |
| **Cranelift AOT** | да | нет | не участвует |
| **WASM** | — | — | не участвует (другой профиль) |

### 42.1 Три уровня тестирования (пирамида)

```
уровень 3: QEMU (реальная архитектура, реальный MMIO-стенд)   — медленно, CI nightly
уровень 2: --mock-hw на host (shadow-MMIO, JIT)               — быстро, каждый коммит
уровень 1: check + stack-report + size --budget (без запуска)  — мгновенно, pre-commit
```

**Правило:** негативные тесты §39 живут на уровне 1 (падают сборкой). Функциональные — на уровне 2 (обязательно) и 3 (обязательно перед релизом). Тест, который существует только на уровне 3, не блокирует CI — значит его нет.

### 42.2 Дифференциальный корпус (обязателен, §16 расширяется)

| # | Ось | Инвариант |
| --- | --- | --- |
| 1 | `--mock-hw` vs QEMU | вывод и MMIO-трасса идентичны бит-в-бит |
| 2 | LLVM host vs LLVM MCU | арифметика/логика/структуры дают одинаковый результат |
| 3 | `-O0` vs `-O2` (LLVM MCU) | семантика неизменна, включая trap'ы |
| 4 | Два прогона `svd-gen` | `sha256` совпадает |
| 5 | Два прогона сборки | байт-идентичный объектник (детерминизм §18) |

### 42.3 Детерминизм сборки для MCU (ужесточение §18)

| Источник недетерминизма | Правило |
| --- | --- |
| Пути к файлам | только относительные в debug-info, `--remap-path-prefix` |
| Timestamp в заголовках | запрещён (нулевой) |
| Порядок символов/секций | сортировка по имени, стабильная |
| Порядок `@isr` в vector table | по номеру IRQ, не по порядку объявления |
| Хеш chip-db | попадает в `datara.lock`, а не в артефакт |

**Гейт:** `forgen build --freestanding --reproducible` дважды → `sha256` объектников совпадают. Это не «nice to have» — это требование сертифицируемого embedded.

## 43. Размер и скорость в embedded — контракт, а не пожелание

MCU-мир считает байты: 32 КБ flash — это норма, 256 КБ — уже «жирный» чип. Здесь Datara обязана быть строже, чем на host (§25 расширяется).

### 43.1 Бюджеты (CI-blocking, по профилям)

| Профиль | Что входит | Flash | RAM | Стек |
| --- | --- | --- | --- | --- |
| `emb-blink` | GPIO + timer + loop | ≤ 1.5 КБ | ≤ 128 Б | ≤ 512 Б |
| `emb-uart` | UART driver + ring buffer + лог | ≤ 4 КБ | ≤ 512 Б | ≤ 1 КБ |
| `emb-driver` | I2C/SPI драйвер + E1-тесты на host | ≤ 8 КБ | ≤ 1 КБ | ≤ 2 КБ |
| `emb-rtos-min` | scheduler + 4 задачи + IPC | ≤ 24 КБ | ≤ 4 КБ | ≤ 4 КБ/задача |

**Сравнение:** аналог `emb-uart` на C + HAL от вендора — 12–20 КБ. На Rust + `embedded-hal` + `panic-halt` — 6–10 КБ (плюс 3–5 КБ на core-форматирование, если тронул `fmt`). Datara обязана держать 4 КБ, потому что у неё **нет** core-fmt, **нет** unwind-таблиц, **нет** panic-runtime (trap = запись в `_FAULT`).

### 43.2 Что даёт минимальность (конкретные механизмы)

| Механизм | Эффект | Как проверяется |
| --- | --- | --- |
| RT-tree DCE (`dead_symbol_eliminate`) | в бинарь попадает только достижимое от `@reset` | `forgen size --symbols` |
| Section GC (`--gc-sections` интеграция) | мёртвые секции не линкуются | размер объектника |
| Нет core-fmt в freestanding | `out` → ring buffer, а не formatting machinery | `nm` не содержит formatter |
| `@bounded`, trap вместо unwind | нет `.eh_frame`, `.gcc_except_table` | `readelf -S` |
| Zero-init BSS + `@static_buf` | нет `memcpy` от startup-кода | секции `bss`/`data` |
| Monomorphization only-on-use | дженерик-код не инстанцируется, если не вызван | tree-shake отчёт |
| No VTables (behavior = static) | нет vtable-секций | `nm` не содержит `vtable` |
| Deterministic layout | нет выравнивающих паддингов «на всякий случай» | `sizeof` из отчёта |

### 43.3 Скорость в embedded (не только размер)

| Метрика | Цель | Почему достижимо |
| --- | --- | --- |
| ISR-latency (entry → тело) | ≤ 12 циклов на Cortex-M4 | нет prologue-сохранения FPU при `@no_float` |
| Zero-cost абстракций регистров | RMW = `ldr/modify/str` без вызова | `register`-блок типизирован и инлайнится |
| Нет heap-фрагментации | по построению (arena/static) | E-EMB-008 |
| Цикл `for` по массиву | тот же код, что hand-written C | LoopFold/SRA из §27 работают в MCU-профиле |

**Гейт скорости:** `emb-blink`-цикл: Datara ≤ C-версия (LLVM `-O2`) + 0%. Не «в пределах 5%» — для GPIO-тоггла накладных быть не должно вообще.

## 44. Дорожная карта к ядру — ступени 2–6 (после WP-10)

WP-10 = ступень 1. Дальше — по одной ступени, каждая со своим гейтом. **Никакой ступени без предыдущей.**

| Ступень | Что | Что нужно технически | Гейт «ступень пройдена» |
| --- | --- | --- | --- |
| **1. Embedded/freestanding** | WP-10 (§34–§43) | targets, freestanding, chip-db, E1–E8 | §40 целиком зелёный |
| **2. RTOS / hobby OS** | работа под Zephyr / NuttX / собственный планировщик | ступень 1 + task-модель, IPC, приоритеты, no-heap режим | демо-ОС: 4 задачи, IPC, ISR, 30 дней аптайма в QEMU |
| **2.5 WCET** *(опционально)* | статическая оценка worst-case времени | абстрактная интерпретация по циклам + `@wcet(n)` | `forgen wcet-report` сходится с QEMU-замером ±15% |
| **3. eBPF** | Datara → eBPF-байткод (XDP, tracepoints, cgroup) | новый бэкенд + анализатор ограничений верификатора | `forgen build --target bpf` проходит `bpftool prog load` |
| **4. Userspace-драйверы** | DPDK / VFIO / UIO / io_uring | ступень 1 + нулевые аллокации в hot-path + pinned-память | бенч: line-rate без потерь против C-референса |
| **5. Out-of-tree `.ko`** | Linux-модуль (эксперимент) | `-mcmodel=kernel`, `-fno-PIE`, kbuild-интеграция, GPL-граница | `insmod` на QEMU-ядре, `rmmod` без утечек |
| **6. Mainline RFC** | включение в дерево Linux | ступени 1–5 + сообщество + мейнтейнеры + годы | принятый RFC, первый драйвер в mainline |

### 44.1 Приоритет (что реально ценно сейчас)

| Ступень | Коммерческая ценность | Политический барьер | Оценка работы |
| --- | --- | --- | --- |
| 1 (WP-10) | **высокая** (IoT/промышленность/инди) | нет | 10–11 недель |
| 2 (RTOS) | средняя | нет | 2–4 месяца |
| 3 (eBPF) | **высокая** (сети, observability) | нет (верификатор вместо политики) | 2–3 месяца |
| 4 (userspace-drivers) | **высокая** (HFT, telecom, медиа) | нет | 3–5 месяцев |
| 5 (`.ko`) | низкая (доказательство) | средний (GPL) | 2–3 месяца |
| 6 (mainline) | долгосрочная | **очень высокий** | годы |

**Рекомендация:** 1 → 3 → 4 → 2 → 5 → 6. Ступени 3 и 4 дают коммерческий результат **без** входа в mainline — а именно они прямо продолжают нишу «один язык от скрипта до железа» (§ниша): сетевые карты, NVMe, HFT-фиды из одного `.dtr`.

### 44.2 Честная оценка ступени 6

В mainline новый язык попадает не «за качество», а за комбинацию:

| Условие | У Datara сейчас |
| --- | --- |
| Работающий toolchain | **есть** |
| Экосистема драйверов | нет (появится через E8/EMB-9) |
| Мейнтейнеры, знающие язык | нет (нужно 3+ человека) |
| Годы упорной работы + корпоративная поддержка | нет |
| Ясная выгода для ядра | **есть**: fail-closed безопасность + статическое доказательство стека (E2) + MMIO provenance (E5) — три вещи, которых нет у Rust |

**Вывод:** ступень 6 не планируется как цель 1.4.4 или 1.5 — она планируется как **следствие** того, что ступени 1–5 дали реальные драйверы, сообщество и мейнтейнеров. Обещать mainline сегодня — маркетинг; заработать его ступенями 1–5 — стратегия.

### 44.3 Что записать в ROADMAP.md (публичный, короткий)

```
v1.4.4  — WP-0..WP-9 (Часть III) + WP-10 embedded (Часть IV)
v1.4.5  — COMPTIME + SIMD DSL + BARE-METAL PREP (Часть V)
v1.5    — eBPF-бэкенд, WCET-отчёт, экосистема драйверов (код в Sparks + hw-контракты)
v1.6    — userspace-драйверы (DPDK/VFIO/UIO), RTOS-профиль
v2.0    — multicore/AMP, OTA-инфраструктура, kernel-эксперименты
```

Каждая строка релиза обязана иметь исполняемый гейт. Нет гейта — нет строки.

<!-- CHUNK-C28-END -->

---

# ЧАСТЬ V. РЕЛИЗ v1.4.5 «COMPTIME + SIMD DSL + BARE-METAL PREP»

## 45. Архитектурный манифест v1.4.5

Релиз v1.4.5 объединяет три фундаментальных направления языка Datara:
1. **Compile-Time Function Execution (CTFE / Zig-стиль comptime)**: вычисление чистых функций во время компиляции со встраиванием результата в DMIR как константных литералов.
2. **Explicit & Safe SIMD DSL**: эргономичные векторные блоки `simd { ... }` с гарантией типобезопасности и строгим контролем границ (`E0947`).
3. **Bare-Metal & Embedded Preparation**: кодогенерация MMIO-регистров с контролем доступа через `unsafe` / `[devices]`, статический профиль `@arena` и скаффолдинг Cortex-M4.
4. **Compiler Speedup**: оптимизация фаз разрешения символов и lowering'а для достижения суммарного времени компиляции всех примеров (`examples/`) < 500 мс.

---

## 46. ЗАДАЧА 1: Comptime (Compile-time вычисления)

### 46.1 Синтаксис и грамматика
* Объявление функции:
  ```datara
  comptime fn make_crc_table() -> List<Int> {
      mut table: List<Int> = []
      mut i = 0
      while i < 256 {
          mut c = i
          mut j = 0
          while j < 8 {
              if (c & 1) != 0 {
                  c = 0xEDB88320 ^ (c >> 1)
              } else {
                  c = c >> 1
              }
              j = j + 1
          }
          table.push(c)
          i = i + 1
      }
      return table
  }
  ```
* Вызов из обычного кода:
  ```datara
  let CRC_TABLE = make_crc_table()
  ```
* Грамматика EBNF:
  ```ebnf
  comptime_fn_decl ::= "comptime" "fn" IDENT "(" param_list? ")" ("->" type)? block
  comptime_expr    ::= "comptime" (block | expr)
  ```

### 46.2 Семантика интерпретатора (`src/comptime/`)
Интерпретатор времени компиляции изолирован в модуле `src/comptime/mod.rs` (выделен и расширен из прототипа в `src/optimizer/comptime_eval.rs`):
* **Окружение исполнения (`ComptimeScope`)**:
  * Таблица локальных переменных `HashMap<String, ComptimeValue>`.
  * Реестр функций `comptime fn` текущей программы.
  * Счётчик шагов (`step_count: usize`).
  * Счётчик глубины вызовов (`call_depth: usize`).
* **Поддерживаемое подмножество типов и операций**:
  * `Int`: 64-битные знаковые целые, побитовые (`&`, `|`, `^`, `<<`, `>>`), арифметические (`+`, `-`, `*`, `/`, `%`).
  * `Float`: 64-битные числа с плавающей точкой.
  * `Bool`: логические операции (`&&`, `||`, `!`, `==`, `!=`, `<`, `<=`, `>`, `>=`).
  * `Str`: неизменяемые строки, конкатенация `+`, длина `.len()`, срезы.
  * `List<T>`: списочные литералы `[a, b, c]`, методы `.push(x)`, `.len()`, индексация `xs[i]`.
  * `Map<K, V>`: ассоциативные массивы, вставка и поиск по ключу.
  * `Struct`: инициализация POD-структур `Point { x: 1, y: 2 }`, чтение полей `p.x`.
  * Поток управления: `if/else`, `while`, `return`, рекурсивные вызовы `fib(n - 1) + fib(n - 2)`.

### 46.3 Инварианты безопасности и лимиты
* **Лимит рекурсии (I-CT-1)**: Максимальная глубина рекурсивных вызовов ограничена `MAX_RECURSION_DEPTH = 64`. Превышение -> ошибка `E-CT-001: Recursion depth limit exceeded in comptime execution`.
* **Лимит шагов (I-CT-2)**: Максимальное количество инструкций ограничено `MAX_COMPTIME_STEPS = 1_000_000`. Защита от бесконечных циклов -> ошибка `E-CT-001: Step limit exceeded in comptime execution`.
* **Запрет сайд-эффектов (I-CT-3)**: Любые попытки I/O (`println`, `read_file`, сетевые вызовы), FFI-вызовов, `unsafe` блоков или обращений к рантайм-специфичным встроенным функциям компилятора прерываются на этапе проверки эффектов -> ошибка `E-CT-002: Effect not allowed in comptime execution (I/O, runtime builtins, and unsafe are forbidden)`.
* **Константность аргументов (I-CT-4)**: Аргументы, передаваемые в `comptime fn`, обязаны быть известны во время компиляции. Если передан неконстантный идентификатор рантайма -> ошибка `E-CT-003: Arguments to comptime function must be compile-time constants`.

### 46.4 Хук в Lowering (AST -> DMIR)
В модуле `src/dmir/lowering/expr_call.rs`:
1. При понижении вызова функции проверяется флаг `is_comptime` у вызываемой функции либо контекст `Expr::Comptime`.
2. Аргументы вычисляются через `ComptimeEvaluator`.
3. Тело функции интерпретируется в изолированном контексте.
4. Результат трансформируется в DMIR константу:
   * `ComptimeValue::Int(v)` -> `Instruction::ConstInt(v)`
   * `ComptimeValue::Float(v)` -> `Instruction::ConstFloat(v)`
   * `ComptimeValue::Str(s)` -> `Instruction::ConstStr(s)`
   * `ComptimeValue::List(items)` -> генерация массива в секции констант `.rodata` с прямым указателем.
5. Вызов функции полностью удаляется из рантайм-кода: нулевые накладные расходы.

### 46.5 Тест-матрица (`tests/test_comptime.rs`, >= 10 тестов)
1. `test_comptime_arithmetic`: базовые арифметические и побитовые операции.
2. `test_comptime_strings`: конкатенация строк и вычисление длины.
3. `test_comptime_list_literals`: создание и индексация списков.
4. `test_comptime_control_flow`: ветвления `if/else` и циклы `while`.
5. `test_comptime_recursion_fib`: рекурсивное вычисление чисел Фибоначчи.
6. `test_comptime_mutual_calls`: взаимный вызов двух `comptime fn`.
7. `test_comptime_dmir_inspect`: проверка, что в DMIR сгенерирован `ConstInt` без инструкции `Call`.
8. `test_comptime_recursion_limit_err`: превышение лимита вызовов -> ловит `E-CT-001`.
9. `test_comptime_forbidden_io_err`: попытка вызова I/O -> ловит `E-CT-002`.
10. `test_comptime_crc32_table`: вычисление CRC32-таблицы (256 элементов) в `comptime` и побайтовое равенство с рантайм-результатом.

---

## 47. ЗАДАЧА 2: SIMD-DSL (Явный и безопасный)

### 47.1 Синтаксис и типы
Явный векторный синтаксис внутри блоков `simd { ... }`:
```datara
simd {
    mut i = 0
    let n = xs.len()
    while i + 4 <= n {
        let va: simd_f32x4 = simd.load(xs, i)
        let vb: simd_f32x4 = simd.load(ys, i)
        let vres = va * vb + va
        simd.store(vres, out_arr, i)
        i = i + 4
    }
    // Скалярный хвост для n % 4 элементов
    while i < n {
        out_arr[i] = xs[i] * ys[i] + xs[i]
        i = i + 1
    }
}
```
* Поддерживаемые векторные типы:
  * `simd_f32x4` (алиас: `F32x4`, `Float4`): 4x 32-bit float (128 бит).
  * `simd_i32x4` (алиас: `I32x4`, `Int4`): 4x 32-bit int (128 бит).
* Встроенные операции модуля `simd`:
  * `simd.load(array, offset) -> V`
  * `simd.store(vector, array, offset)`
  * `simd.splat(scalar) -> V`
  * Арифметические операторы: `+`, `-`, `*`, `/`.

### 47.2 Безопасность памяти и типов
* **Контроль границ (`E0947`)**:
  Функция `simd.load(xs, i)` требует, чтобы срез `i .. i + 4` находился строго в пределах длины массива `xs.len()`. Если смещение выходит за пределы -> компилятор/рантайм генерирует `E0947: SIMD load out of bounds (array length <len>, requested index <offset>..+4)`.
* **Строгая изоляция типов (`E-TYPE-008`)**:
  Смешивание векторных типов разной разрядности или природы (например, `simd_f32x4 + simd_i32x4`) строго запрещено в типизаторе -> `E-TYPE-008: Cannot mix vector types 'simd_f32x4' and 'simd_i32x4' in binary operation '+'`.
* **Выравнивание (Zero-Unaligned-Fault)**:
  Все `simd.load` и `simd.store` генерируют unaligned векторные инструкции (`movups`/`movdqu` в x86, `vld1`/`vst1` в ARM), исключая GP-fault при произвольном выравнивании в куче или на стеке.

### 47.3 Кодогенерация (Cranelift + LLVM)
* **Cranelift Backend**:
  * Прямой маппинг на XMM-инструкции в `src/codegen/cranelift/backend/simd.rs`.
  * `simd.load` -> `ins().load(clif_types::F32X4, flags, ptr, offset)`.
  * `simd.store` -> `ins().store(flags, val, ptr, offset)`.
  * `+`, `-`, `*`, `/` -> нативные Cranelift `fadd`, `fsub`, `fmul`, `fdiv` над векторными регистрами.
* **LLVM Backend**:
  * Векторные типы `<4 x float>` и `<4 x i32>` в `src/codegen/llvm/simd.rs`.
  * Векторные операции `fadd <4 x float>`, `fmul <4 x float>`, `add <4 x i32>`.
  * `insertelement` / `extractelement` / `shufflevector` при необходимости скалярного доступа.

### 47.4 Тест-матрица (`tests/test_simd_dsl.rs`, >= 7 тестов)
1. `test_simd_load_store_roundtrip`: загрузка, сохранение и проверка идентичности данных.
2. `test_simd_arithmetic_f32x4`: векторные операции `+`, `-`, `*`, `/`.
3. `test_simd_arithmetic_i32x4`: векторные операции над целыми числами.
4. `test_simd_tail_handling`: массив произвольной длины (не кратной 4) с корректной обработкой хвоста.
5. `test_simd_bounds_check_err`: выход за границы массива при `simd.load` -> ловит `E0947`.
6. `test_simd_type_mixing_err`: попытка сложения `F32x4` и `I32x4` -> ловит `E-TYPE-008`.
7. `test_simd_speedup_benchmark`: замер скорости векторного умножения массивов на N=10000 элементов (SIMD быстрее скалярного кода).
8. `test_simd_llvm_codegen`: проверка генерации LLVM IR с вектором `<4 x float>`.

---

## 48. ЗАДАЧА 3: Подготовка Bare-Metal (MMIO, @arena, Cortex-M4)

### 48.1 MMIO классы и регистры
* Синтаксис:
  ```datara
  @mmio(0x40021000)
  struct GpioB {
      moder: Int at 0x00,
      odr:   Int at 0x14,
      bsrr:  Int at 0x18,
  }
  ```
* Или объявление через `register` в AST:
  ```datara
  register GPIOB at 0x40021000 {
      moder: Int at 0x00,
      odr:   Int at 0x14,
      bsrr:  Int at 0x18,
  }
  ```
* **Семантика доступа**:
  * Чтение поля -> `volatile load` по адресу `base_address + offset`.
  * Запись поля -> `volatile store` по адресу `base_address + offset`.
  * В LLVM IR: `load volatile i32, ptr inttoptr (i64 0x40021014 to ptr)`. Оптимизатор LLVM не имеет права кешировать или выбрасывать такие операции.
* **Provenance Gate (I-MMIO-1)**:
  Доступ к MMIO-регистрам разрешён только:
  1. Внутри блока `unsafe(justification: "...") { ... }`.
  2. ИЛИ если устройство объявлено в `datara.toml` в секции `[devices]` (например, `devices = ["GPIOB"]`).
  Неавторизованный доступ -> ошибка `E-MMIO-001: MMIO access to 'GPIOB' requires an 'unsafe' block or declaration in datara.toml [devices]`.

### 48.2 Память: `@arena` в bare-профиле
* При сборке с `--profile bare` динамический кучевой аллокатор отключается.
* Аннотация `@arena(size: 65536)` объявляет статический пул в секции `.bss`:
  * Нулевые накладные расходы на инициализацию.
  * Указатель текущей позиции аллокации смещается линейно; освобождение памяти происходит сбросом арены.

### 48.3 Скелет `forgen new --bare cortex-m4`
Команда CLI создает готовый проект для встраиваемых систем:
* `datara.toml`:
  ```toml
  [package]
  name = "firmware"
  version = "0.1.0"
  profile = "bare"

  [target]
  arch = "thumbv7em-none-eabihf"
  cpu = "cortex-m4"

  [devices]
  allowed = ["GPIOC", "RCC"]
  ```
* `src/main.dtr`:
  ```datara
  @mmio(0x40021000)
  struct GpioC {
      moder: Int at 0x00,
      odr:   Int at 0x14,
  }

  fn main() {
      // Инициализация GPIO и мигание светодиодом
      let gpio = GpioC {}
      unsafe(justification: "Toggle LED pin via MMIO") {
          gpio.odr = gpio.odr ^ (1 << 13)
      }
      while true {}
  }
  ```
* `memory.ld`: Linker script с описанием карты памяти Cortex-M4:
  `FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 512K`
  `RAM (xrw)  : ORIGIN = 0x20000000, LENGTH = 128K`
* **Ограничение бэкенда**:
  Cranelift не поддерживает архитектуру ARM/Thumb (`thumbv7em-none-eabihf`). Сборка для Cortex-M4 осуществляется исключительно через LLVM:
  `forgen build --llvm --target thumbv7em-none-eabihf`
  Попытка сборки через Cranelift выдает диагностику `E-TARGET-001: Target 'thumbv7em-none-eabihf' is only supported via LLVM backend. Use '--llvm'`.

### 48.4 Тест-матрица (`tests/test_baremetal.rs`)
1. `test_mmio_volatile_load_store`: генерация `volatile load` и `volatile store` в DMIR.
2. `test_mmio_provenance_gate`: проверка запрета доступа без `unsafe` и без `[devices]` -> `E-MMIO-001`.
3. `test_mmio_manifest_allowed`: доступ разрешен при наличии устройства в `datara.toml`.
4. `test_bare_skeleton_generation`: создание проекта через `forgen new --bare cortex-m4`.
5. `test_bare_llvm_compilation`: успешная компиляция скелета в объектный файл ELF для `thumbv7em-none-eabihf` через LLVM.

---

## 49. ЗАДАЧА 4: Ускорение компилятора (Compiler Speedup)

### 49.1 Профилирование компилятора
В компиляторе задействован встроенный механизм замера фаз `CompilationTimings`:
`discovery_ms`, `parse_ms`, `resolve_ms`, `typecheck_ms`, `lower_ms`, `opt_ms`, `codegen_ms`, `link_ms`, `total_ms`.

### 49.2 Топ-3 горячих фазы и их оптимизация
1. **`resolver` (устранение избыточных реаллокаций)**:
   * Замена повторяющегося форматирования составных строк `format!("{}.{}", ns, name)` на срезы и интернированные строки (`Arc<str>` / `SymbolId`).
   * Предварительное резервирование емкостей хеш-таблиц (`with_capacity(64)`).
2. **`lowering` (сокращение клонирования AST-деревьев)**:
   * Переход на перемещение выражений (`std::mem::take` / `std::mem::replace`) вместо `.clone()` в `src/dmir/lowering/expr.rs` и `expr_call.rs`.
   * Использование ссылок на неизменяемые таблицы типов.
3. **`derive & comptime folding` (ранний отсев)**:
   * Быстрый пропуск файлов и блоков, не содержащих атрибутов `@derive` или выражений `comptime`, без рекурсивного обхода всего AST.

### 49.3 Целевой показатель (Speed Gate)
* **Контракт скорости**: Суммарное время компиляции всех примеров (`examples/*.dtr`, 30+ файлов) в режиме `--check`:
  * До оптимизации: замеряется baseline (T_base).
  * После оптимизации: T_opt < 500 мс (суммарно для всех 30+ примеров).

---

## 50. Подводные камни (Pitfalls) и Архитектурные Решения v1.4.5

| # | Подводный камень | Опасность | Архитектурное решение в v1.4.5 |
|---|---|---|---|
| **P1** | Бесконечный цикл или глубокая рекурсия в `comptime fn` | Зависание компилятора, исчерпание памяти хоста | Жесткий лимит шагов `1_000_000` и лимит рекурсии `64` с ошибкой `E-CT-001` |
| **P2** | Попытка выполнить I/O, доступ к ФС или вызов runtime builtins в `comptime` | Недетерминизм сборки, уязвимости хост-системы | Строгий fail-closed фильтр эффектов в `ComptimeEvaluator` -> `E-CT-002` |
| **P3** | Неконстантные аргументы в `comptime fn` из рантайм-кода | Невозможность вычислить результат на этапе компиляции | Проверка константности на этапе lowering'а -> ошибка `E-CT-003` |
| **P4** | Выход за границы массива в `simd.load` при некратной длине | Segfault / чтение чужой памяти в рантайме | Автоматическая генерация bounds-check (`E0947`) и паттерн безопасного скалярного хвоста |
| **P5** | Неявное приведение типов в SIMD (`F32x4 + I32x4`) | Порча данных в регистрах XMM | Строгий отказ типизатора с кодом `E-TYPE-008` (без неявных кастов) |
| **P6** | Оптимизатор LLVM удаляет чтение/запись MMIO | Аппаратные регистры не обновляются, зависание MCU | Обязательный квалификатор `volatile` на всех операциях чтения/записи MMIO |
| **P7** | Несанкционированный доступ к адресам оборудования | Нарушение песочницы, утечки в bare-metal | Provenance Gate: MMIO доступен только в `unsafe` или через `datara.toml [devices]` (`E-MMIO-001`) |
| **P8** | Попытка использовать Cranelift для ARM Cortex-M4 | Паника компилятора, отсутствие бэкенда | Понятная ошибка `E-TARGET-001` с указанием использовать `--llvm` |
| **P9** | Раздувание бинарника от повторного инлайнинга comptime-литералов | Увеличение `.rodata` секции | Дедупликация идентичных константных таблиц в DMIR/LLVM |
| **P10** | Невыровненные обращения к памяти в SIMD (`movaps` vs `movups`) | General Protection Fault (#GP segfault) на x86 | Все `simd.load`/`simd.store` генерируют unaligned инструкции (`movups`/`movdqu`, `vld1`/`vst1`) |
| **P11** | Несоответствие разрядности целых чисел при записи в 32-битные MMIO регистры | Искажение соседних регистров периферии, BusFault на MCU | Приведение и валидация ширины регистра (32 бита на Cortex-M4) с маскированием или ошибкой типизатора |
| **P12** | Переупорядочивание инструкций вокруг MMIO регистров компилятором | Нарушение протокола периферии (например, включение тактирования до настройки пинов) | Использование барьеров памяти `mmio.fence()` / `llvm.arm.dmb` и сохранение строгой последовательности volatile-доступов |
| **P13** | Хостовые аллокации в `comptime` и утечка указателей хоста в таргет | Падения рантайма из-за некорректных адресов памяти | Полноценная сериализация структур данных из памяти хоста в целевой DMIR-формат (`datara_rt_list_create` или `.rodata` таблицы) |
| **P14** | Отсутствие стартап-кода и таблицы векторов прерываний для bare-metal | MCU не может стартовать после сброса (зависание в BootROM) | Скелет `forgen new --bare cortex-m4` включает минимальный векторный файл `src/vectors.dtr` с `Reset_Handler` |
| **P15** | Попытка сборки bare-metal без кросс-компилятора LLVM | Ошибки линковки `lld` или отсутствие CRT-стабов | Информативная диагностика с рекомендацией `forgen toolchain install arm-none-eabi` |

---

## 51. ПРИЁМОЧНАЯ МАТРИЦА И DEFINITION OF DONE v1.4.5

Для завершения релиза v1.4.5 должны быть выполнены все 6 критериев:

1. **Полный гейт 0 failed**:
   * Все существующие тесты компилятора (`cargo test`) проходят зелёными.
2. **Comptime**:
   * Тест-сьют `tests/test_comptime.rs` (>= 10 тестов) проходит успешно.
   * Демонстрация вычисления таблицы CRC32 на этапе компиляции с побайтовой проверкой.
   * Полноценная изоляция эффектов (`E-CT-001`, `E-CT-002`, `E-CT-003`).
3. **SIMD-DSL**:
   * Тест-сьют `tests/test_simd_dsl.rs` (>= 7 тестов) проходит успешно.
   * Замер производительности: векторизованный код быстрее скалярного на N=10000.
   * Проверка контроля границ (`E0947`) и изоляции типов (`E-TYPE-008`).
4. **Bare-Metal**:
   * Unit-тесты генерации `volatile` инструкций для MMIO в DMIR и LLVM.
   * Проверка Provenance Gate (`E-MMIO-001`) при доступе без `unsafe` и без `[devices]`.
   * Проект `forgen new --bare cortex-m4` успешно генерирует скелет со стартап-кодом, linker script и манифестом.
   * Ограничение бэкенда: выдача `E-TARGET-001` при попытке сборки ARM через Cranelift.
   * Скелет компилируется для `thumbv7em-none-eabihf` через LLVM.
5. **Ускорение компилятора**:
   * Проведены замеры времени фаз до и после оптимизаций.
   * Суммарное время компиляции всех примеров `examples/*.dtr` < 500 мс.
6. **Качество кода**:
   * `cargo fmt --check` и `cargo clippy` проходят без предупреждений.
   * MSRV совместимость сохранена.

<!-- END OF SPEC -->

