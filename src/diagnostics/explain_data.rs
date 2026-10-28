//! M2 Diagnostics Gold: extended per-code explanations (PLAN_V150).
//!
//! Every ErrorCode resolves to a category example with [INVALID]/[VALID]
//! code snippets. Summaries come from ErrorCode::description("en") so
//! wording cannot drift between diagnostics and explanations.

use crate::diagnostics::codes::all_codes;

pub struct CategoryExample {
    pub explanation: &'static str,
    pub bad: &'static str,
    pub good: &'static str,
    pub rationale: &'static str,
}

pub static CATEGORIES: &[(&str, CategoryExample)] = &[
    (
        "syntax",
        CategoryExample {
            explanation: "Syntax errors are detected by the lexer/parser before any analysis. Read the caret span: it points at the exact token the parser could not accept.",
            bad: "val x = (1 +\nval s = \"unterminated",
            good: "val x = (1 + 2)\nval s = \"closed\"",
            rationale: "Fail-fast parsing keeps later analysis stages trustworthy.",
        },
    ),
    (
        "resolve",
        CategoryExample {
            explanation: "Name resolution failed: the symbol is not declared in scope, not imported, or not visible from this module. Declare it, import it with 'use', or fix the spelling (the compiler suggests near matches).",
            bad: "fn main() {\n    out unknown_symbol\n}",
            good: "fn main() {\n    val known_symbol = 1\n    out known_symbol\n}",
            rationale: "Resolution must be total before type checking can be trusted.",
        },
    ),
    (
        "type",
        CategoryExample {
            explanation: "Datara is strictly typed (Gate 7): no silent or lossy implicit conversions. Make the conversion explicit with 'as', '.to_int()', 'parse_int(...)', or fix the literal form.",
            bad: "val n: Int = \"text\"\nval f: Float = 10",
            good: "val n: Int = parse_int(\"text\") or -1\nval f: Float = 10.0",
            rationale: "Explicit conversions keep every narrowing visible at the call site.",
        },
    ),
    (
        "borrow",
        CategoryExample {
            explanation: "Affine ownership: a value has exactly one owner, and mutation requires exclusive 'mut' access with no active immutable views. Reorder the uses or declare the binding mutable.",
            bad: "val x = 1\nx = 2",
            good: "mut x = 1\nx = 2\nout x",
            rationale: "Affine ownership guarantees deterministic deallocation without a GC.",
        },
    ),
    (
        "effects",
        CategoryExample {
            explanation: "Effects and capabilities are part of the function contract. IO/Net/FS operations require the matching capability annotation ([Net], [FS], [Env]) or an explicit unsafe(justification) block.",
            bad: "fn fetch() -> Str {\n    return exec_utf8(\"curl\")\n}",
            good: "fn fetch() -> Str [Net] {\n    return exec_utf8(\"curl\")\n}",
            rationale: "Capability contracts make side effects reviewable at a glance.",
        },
    ),
    (
        "toolchain",
        CategoryExample {
            explanation: "The backend could not complete: check the toolchain with 'forgen doctor', install the LLVM/linker components ('forgen install llvm'), or pick a supported target. This class of error is environmental, not a source error.",
            bad: "forgen build --llvm app.dtr   (llvm toolchain missing)",
            good: "forgen install llvm\nforgen build --llvm app.dtr",
            rationale: "Backend problems are environmental; source changes cannot fix them.",
        },
    ),
    (
        "cimport",
        CategoryExample {
            explanation: "C header interop failed: the header path is wrong, unreadable, or uses constructs outside the declarative subset. Use plain C declarations (functions, structs, enums, typedefs) and pass the correct relative path.",
            bad: "import c \"missing_header.h\" with link(\"m\");",
            good: "import c \"vendor/math.h\" with link(\"libm\");",
            rationale: "Declarative C interop trades exotic-header support for safety and speed.",
        },
    ),
    (
        "match",
        CategoryExample {
            explanation: "Match dispatch must be exhaustive and reachable. Cover every enum variant (or add a wildcard arm), and remove arms that can never fire.",
            bad: "match c {\n    Color.Red => 1\n}",
            good: "match c {\n    Color.Red => 1\n    Color.Green => 2\n    Color.Blue => 3\n}",
            rationale: "Exhaustiveness turns forgotten cases into compile-time errors.",
        },
    ),
    (
        "control",
        CategoryExample {
            explanation: "'break' and 'continue' only exist inside loop bodies (while/for/loop). Move the statement inside a loop, or use 'return' to exit a function.",
            bad: "fn f() -> Int {\n    break\n    return 1\n}",
            good: "fn f() -> Int {\n    while true {\n        break\n    }\n    return 1\n}",
            rationale: "Structured control flow keeps jump targets statically provable.",
        },
    ),
    (
        "contracts",
        CategoryExample {
            explanation: "Design-by-contract proof failed: a class invariant, requires/ensures clause, static range, unit dimension, or termination argument could not be proven statically. Narrow the value domain or adjust the contract honestly.",
            bad: "fn div(a: Int, b: Int) -> Int {\n    require b != 0\n    return a / b\n}",
            good: "fn div(a: Int, b: NonZeroInt) -> Int {\n    return a / b\n}",
            rationale: "Proof-carrying contracts hold because unprovable ones fail loudly.",
        },
    ),
    (
        "oop",
        CategoryExample {
            explanation: "Datara splits data and behavior: 'struct' holds fields, 'behavior'/'impl' hold methods, 'component' is pure POD, traits need coherent implementations. Move the code to the right construct.",
            bad: "struct S {\n    id: Int\n    get_id() -> Int { return this.id }\n}",
            good: "struct S {\n    id: Int\n}\nbehavior S {\n    get_id() -> Int {\n        return this.id\n    }\n}",
            rationale: "Data/behavior separation enables SoA transforms and flat composition.",
        },
    ),
    (
        "output",
        CategoryExample {
            explanation: "Printability is static: only primitives, @derive(Display) structs, and types with to_string/to_str may be printed. Derive Display or format collections element-wise.",
            bad: "struct Naked { id: Int }\nout n",
            good: "@derive(Display)\nstruct Naked { id: Int }\nout n",
            rationale: "Static printability eliminates accidental memory dumps in output.",
        },
    ),
    (
        "comptime",
        CategoryExample {
            explanation: "comptime execution is pure and bounded: arguments must be compile-time constants, no IO/unsafe, and recursion must terminate within the step limit. Precompute with literals and const expressions.",
            bad: "val n: Int = comptime { runtime_call() }",
            good: "val n: Int = comptime { 1024 * 64 }",
            rationale: "Compile-time execution must be pure, bounded and deterministic.",
        },
    ),
    (
        "asm",
        CategoryExample {
            explanation: "Hardware-level constructs are gated: structured asm must stay in the safe subset (mov/add/sub over GP registers and Int variables) and be wrapped in unsafe(justification: \"...\"); MMIO requires a [devices] declaration; some targets only support these via LLVM.",
            bad: "asm {\n    mov rax, x\n    vshuffle ymm0, ymm1, 5\n}",
            good: "unsafe(justification: \"GP-register data move\") {\n    asm {\n        mov rax, x\n        mov y, rax\n    }\n}",
            rationale: "Hardware access is first-class but always explicit and justified.",
        },
    ),
    (
        "arena",
        CategoryExample {
            explanation: "Region allocations cannot escape: an @arena/@pool function may only return Copy types (Int, Float, Bool, Char) or prove the value is not region-resident. Copy the value out or restructure ownership.",
            bad: "fn make() -> Str @arena {\n    return \"kept\"   // region-resident escape\n}",
            good: "fn make() -> Int @arena {\n    return compute()   // Copy type escapes by value\n}",
            rationale: "Region reclamation is only sound when nothing escapes the region.",
        },
    ),
    (
        "canonical",
        CategoryExample {
            explanation: "A legacy synonym was used. The language defines one canonical spelling per construct; the synonym still compiles but keeps warning through the grace period.",
            bad: "let x = 5\nval s: String = \"a\"",
            good: "val x = 5\nval s: Str = \"a\"",
            rationale: "One canonical spelling per construct keeps the language searchable.",
        },
    ),
    (
        "link",
        CategoryExample {
            explanation: "Link-phase failure: two declarations in the program mangle to the same link symbol, so the emitted module would contain two bodies under one name. The first definition is named in the message; rename one declaration or qualify the name so every symbol is unique.",
            bad: "fn Foo_bar(x: Int) -> Int { return x }\nstruct Foo { }\nbehavior Foo {\n    fn bar(x: Int) -> Int { return x + 1 }   // mangles to Foo_bar too\n}",
            good: "fn foo_bar_free(x: Int) -> Int { return x }\nstruct Foo { }\nbehavior Foo {\n    fn bar(x: Int) -> Int { return x + 1 }   // Foo_bar is now unique\n}",
            rationale: "One definition per link symbol is a precondition for deterministic codegen and for backend verification.",
        },
    ),
    (
        "tmm",
        CategoryExample {
            explanation: "Temporal Memory Management violation. @arc objects are freed deterministically when the refcount reaches zero, so a strong-reference cycle leaks. Break cycles with @weak fields and upgrade() -> Outcome<T> before use. Borrowed FFI values must not outlive the call: do not store or return them.",
            bad: "@arc\nstruct Node {\n    next: Node   // strong back-edge leaks\n}",
            good: "@arc\nstruct Node {\n    next: Weak<Node>\n}\n\nfn walk(n: Weak<Node>) {\n    val strong = n.upgrade() or return\n    out strong.value\n}",
            rationale: "Deterministic destruction without a GC requires the ownership graph to stay acyclic or explicitly weak on back-edges.",
        },
    ),
(
        "any",
        CategoryExample {
            explanation: "A dynamic value (val/any) leaked into a context that expects a static type. Dynamic values carry a runtime tag; a static context would reinterpret the bits. Narrow with `is` (the value is unboxed in the branch) or convert explicitly with .as_int()/.as_float() (Outcome), .to_int()/.to_float() (fail-loud), .to_str().",
            bad: "val xs = [42, \"hi\"]\nfn takes_int(v: Int) -> Int { return v }\ntakes_int(xs[0])   // which tag does xs[0] carry?",
            good: "val xs = [42, \"hi\"]
fn takes_int(v: Int) -> Int { return v }\nval x = xs[0]\nif x is Int {\n    takes_int(x)                   // narrowed: a real Int in this branch\n}\ntakes_int(xs[0].as_int() or 0)     // or convert explicitly",
            rationale: "The static/dynamic boundary must be explicit — that is what keeps the static path zero-checked and the dynamic path zero-boxed.",
        },
    ),
];

pub static CODE_CATEGORY: &[(&str, &str)] = &[
    ("E-SYNTAX-001", "syntax"),
    ("E-SYNTAX-002", "syntax"),
    ("E-SYNTAX-003", "syntax"),
    ("E-SYNTAX-004", "syntax"),
    ("E-SYNTAX-005", "syntax"),
    ("E-SYNTAX-006", "syntax"),
    ("E-SYNTAX-007", "syntax"),
    ("E-SYNTAX-008", "syntax"),
    ("E-SYNTAX-009", "syntax"),
    ("E0105", "syntax"),
    ("E-RESOLVE-001", "resolve"),
    ("E-RESOLVE-002", "resolve"),
    ("E-RESOLVE-003", "resolve"),
    ("E-RESOLVE-004", "resolve"),
    ("E-RESOLVE-005", "resolve"),
    ("E0042", "resolve"),
    ("E-TYPE-001", "type"),
    ("E-TYPE-002", "type"),
    ("E-TYPE-003", "type"),
    ("E-TYPE-004", "type"),
    ("E-TYPE-005", "type"),
    ("E-TYPE-006", "type"),
    ("E-TYPE-007", "type"),
    ("E-TYPE-008", "type"),
    ("E-TYPE-009", "type"),
    ("E-TYPE-010", "type"),
    ("W-TYPE-002", "type"),
    ("E-BORROW-001", "borrow"),
    ("E-BORROW-002", "borrow"),
    ("E-BORROW-003", "borrow"),
    ("E-BORROW-004", "borrow"),
    ("E-BORROW-005", "borrow"),
    ("E-BORROW-006", "borrow"),
    ("E-EFFECT-001", "effects"),
    ("E-EFFECT-002", "effects"),
    ("E-EFFECT-003", "effects"),
    ("E0940", "effects"),
    ("E0942", "effects"),
    ("E0943", "effects"),
    ("E0950", "effects"),
    ("E0951", "effects"),
    ("E-CAP-001", "effects"),
    ("E-CAP-002", "effects"),
    ("E-CODEGEN-001", "toolchain"),
    ("E-CODEGEN-002", "toolchain"),
    ("E-CODEGEN-003", "toolchain"),
    ("E0901", "toolchain"),
    ("E0903", "toolchain"),
    ("E0904", "toolchain"),
    ("E0980", "toolchain"),
    ("E-TARGET-001", "toolchain"),
    ("E1405", "toolchain"),
    ("E0955", "toolchain"),
    ("E0956", "toolchain"),
    ("E0957", "toolchain"),
    ("E0960", "toolchain"),
    ("E0961", "toolchain"),
    ("E0962", "cimport"),
    ("E-BRIDGE-001", "cimport"),
    ("E-BRIDGE-002", "cimport"),
    ("E-BRIDGE-003", "cimport"),
    ("E0310", "match"),
    ("E0311", "match"),
    ("E0312", "control"),
    ("E0941", "contracts"),
    ("E0945", "contracts"),
    ("E0946", "contracts"),
    ("E0947", "contracts"),
    ("E0948", "contracts"),
    ("E0420", "contracts"),
    ("E-OPT-001", "contracts"),
    ("E0100", "oop"),
    ("W0100", "oop"),
    ("E-STRUCT-001", "oop"),
    ("E-COMP-001", "oop"),
    ("E-COMP-002", "oop"),
    ("E-BEH-001", "oop"),
    ("E-BEH-002", "oop"),
    ("E-ROLE-001", "oop"),
    ("E-IMPL-001", "oop"),
    ("E-IMPL-002", "oop"),
    ("E-OUT-001", "output"),
    ("W0102", "output"),
    ("E-CT-001", "comptime"),
    ("E-CT-002", "comptime"),
    ("E-CT-003", "comptime"),
    ("E1402", "asm"),
    ("E1403", "asm"),
    ("E1404", "asm"),
    ("E-MMIO-001", "asm"),
    ("E0902", "asm"),
    ("E1401", "arena"),
    ("E1406", "arena"),
    ("W-SYN-001", "canonical"),
    ("W-SYN-002", "canonical"),
    ("E-LINK-001", "link"),
    ("E-TMM-001", "tmm"),
    ("E-TMM-002", "tmm"),
    ("E-ANY-001", "any"),
];

/// M2 audit: up to 2 actionable hints per category, surfaced in every
/// explanation of a code in that category (PLAN_V150 M2: "каждому коду —
/// код, сообщение, span, до 2 подсказок"). The test gate in
/// tests/test_explain_coverage.rs enforces 1..=2 non-empty hints per
/// category and full category coverage of the ErrorCode set.
pub static CATEGORY_HINTS: &[(&str, &[&str])] = &[
    ("syntax", &["Read the caret span: it points at the exact token the parser rejected.", "Check brackets/quotes are balanced before the reported line."]),
    ("resolve", &["Declare the symbol or import it with 'use'.", "Check the spelling — the compiler suggests near matches."]),
    ("type", &["Make the conversion explicit ('as', .to_int(), parse_int(...)).", "Match the literal form to the expected type (10.0 for Float)."]),
    ("borrow", &["Reorder uses so the move happens after the last read.", "Declare the binding with 'mut' if in-place mutation is intended."]),
    ("effects", &["Add the matching capability annotation ([Net]/[FS]/[Env]) to the signature.", "Wrap the call in unsafe(justification: \"...\") only for genuine escalations."]),
    ("toolchain", &["Run 'forgen doctor' to check the backend toolchain.", "Install the missing backend ('forgen install llvm') or pick another target."]),
    ("cimport", &["Use plain C declarations (functions, structs, enums, typedefs) in the header.", "Pass the header path relative to the source file and the library via link(...)."]),
    ("match", &["Add arms for every missing variant (the error lists them) or a wildcard arm.", "Remove duplicate/unreachable arms — they are reported too."]),
    ("control", &["Move 'break'/'continue' inside a while/for/loop body.", "Use 'return' to leave a function instead of break."]),
    ("contracts", &["Narrow the value domain (require b != 0, NonZeroInt) so the proof can succeed.", "Adjust the contract honestly if the property genuinely does not hold."]),
    ("oop", &["Move methods into a 'behavior'/impl block for the struct.", "Use 'using Component' for flat composition instead of inheritance."]),
    ("output", &["Derive Display: @derive(Display) on the struct.", "Format collections element-wise instead of printing the whole container."]),
    ("comptime", &["Use only compile-time constants and const expressions as arguments.", "Replace runtime calls with literal/const computation inside the block."]),
    ("asm", &["Wrap the block in unsafe(justification: \"...\") with a concrete reason.", "Keep structured asm in the safe subset (mov/add/sub over GP registers and Int variables)."]),
    ("arena", &["Return a Copy type (Int/Float/Bool/Char) from the region function.", "Copy the needed value out of the region before returning."]),
    ("canonical", &["Switch to the canonical spelling (val instead of let, Str instead of String).", "Run 'forgen fix --suggestions' to apply mechanical renames."]),
    ("link", &["Rename one of the colliding symbols (the error names both definitions).", "Qualify top-level helpers into modules so generated mangled names stay unique."]),
    ("tmm", &["Break the ownership cycle with @weak on the back-edge field.", "Mark FFI parameters owned/borrowed explicitly so lifetime checks can prove the boundary."]),
    ("any", &["Narrow with `if x is Int { ... }` — the value is unboxed in the branch.", "Convert explicitly: .as_int()/.as_float() (Outcome) or .to_int()/.to_float() (fail-loud)."]),
];

/// Full explanation text for an ErrorCode: summary from description("en")
/// plus the category's [INVALID]/[VALID] examples and its audit hints.
/// Returns None for codes outside the enum.
pub fn explain_for_code(code: &str) -> Option<String> {
    let variant = all_codes().into_iter().find(|c| c.as_str() == code)?;
    let key = CODE_CATEGORY.iter().find(|(c, _)| *c == code)?.1;
    let cat = &CATEGORIES.iter().find(|(k, _)| *k == key)?.1;
    let mut s = String::new();
    s.push_str(&format!(
        "An expression was rejected with code {} ({}).\n\n",
        code,
        variant.description("en")
    ));
    s.push_str(cat.explanation);
    s.push_str("\n\n[INVALID] Bad Code:\n");
    s.push_str(cat.bad);
    s.push_str("\n\n[VALID] Good Code:\n");
    s.push_str(cat.good);
    s.push_str("\n\nRationale: ");
    s.push_str(cat.rationale);
    if let Some((_, hints)) = CATEGORY_HINTS.iter().find(|(k, _)| *k == key) {
        s.push_str("\n\nHints:");
        for h in *hints {
            s.push_str("\n  - ");
            s.push_str(h);
        }
    }
    s.push('\n');
    Some(s)
}
