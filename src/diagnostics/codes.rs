use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCode {
    // Syntax Errors (E-SYNTAX-*)
    SyntaxUnexpectedToken,
    SyntaxUnterminatedString,
    SyntaxUnterminatedComment,
    SyntaxInvalidNumber,
    SyntaxInvalidChar,
    SyntaxExpectedExpression,
    SyntaxExpectedIdentifier,
    SyntaxExpectedType,
    SyntaxInvalidEscape,
    RecursionLimitExceeded,

    // Resolution Errors (E-RESOLVE-*)
    ResolveUndefinedSymbol,
    ResolveDuplicateSymbol,
    ResolveUnknownType,
    ResolveCircularDependency,
    ResolveUnreachableModule,
    PrivateItemAccess,

    // Type Errors (E-TYPE-*)
    TypeMismatch,
    TypeCannotInfer,
    TypeMissingReturn,
    TypeInvalidBinaryOp,
    TypeInvalidUnaryOp,
    TypeInvalidMemberAccess,
    TypeGenericMismatch,
    TypeIncomparableOperands,
    TypeUnknownMethod,
    // v1.4.1: `?` used inside a function whose return type cannot receive
    // the propagated error (not Outcome/Result/Maybe-shaped).
    QuestionPropagation,

    // Borrow & Ownership Errors (E-BORROW-*)
    BorrowUseAfterMove,
    BorrowCannotMutateImmutable,
    BorrowConflictActiveView,
    BorrowMultipleMutableViews,
    BorrowEscapingView,
    BorrowConflict,

    // Effect Errors (E-EFFECT-*)
    EffectImpureInPureContext,
    EffectUnsafeOperation,
    EffectUnhandledIO,

    // Codegen Errors (E-CODEGEN-*)
    CodegenBackendFailed,
    CodegenLinkerFailed,
    CodegenIOError,

    // Security & Zero-Trust Gate Errors (E0940-E0943)
    SecurityViolation,
    ProofCarryingCodeViolation,
    UncheckedFFIViolation,
    DataRaceViolation,

    // Real-Time & Safety Verification Gate Errors (E0950-E0951)
    AllocationViolation,
    PanicViolation,

    // Polyglot & C Import Errors (E0960-E0962)
    CImportHeaderNotFound,
    CImportReadFailed,
    CImportUnsupportedConstruct,

    // Language Safety & Formal Verification Errors (E0310-E0947)
    NonExhaustiveMatch,
    UnreachablePattern,
    LoopControlOutsideLoop,
    DimensionMismatch,
    InvariantViolation,
    TerminationViolation,
    RangeViolation,
    ContractViolation,

    // Machine-readable Codegen & Internal Errors (E0901-E0904)
    InternalVerification,
    InlineAsmUnsupported,
    UnknownBinop,
    CApiArgLimitExceeded,
    AsyncBackendUnsupported,

    // Cross-compilation Errors (E0980)
    CrossCompilationMissingToolchain,

    // Inline Attribute Errors (E0956-E0957, v1.3.4)
    InlineAlwaysExtern,
    InlineAttributeInvalid,

    // Allocator Tier Errors (E1401, E1406, v1.4.0)
    ArenaEscape,
    PoolCapacityExceeded,

    // Structured Inline Assembly Errors (E1402-E1405, v1.4.0)
    AsmUnsupportedInst,
    AsmOperandNotInt,
    AsmRequiresUnsafe,
    AsmUnsupportedBackend,

    // Optimizer Correctness Warnings (E-OPT-*)
    OptimizationUnproven,

    // Deprecation & Modernization Warnings (W0100)
    DeprecatedFeature,

    // Canonical-Form Warnings (W-SYN-*): the language accepts a legacy or
    // synonymous spelling, but the canonical keyword is something else.
    // These are warnings (not errors) so existing code keeps compiling
    // while nudging codebases toward one spelling per construct.
    CanonicalKeyword,
    CanonicalTypeSpelling,
    BoolIntComparison,

    // Bridge Block Errors (E-BRIDGE-*, v1.4.2)
    BridgeTypeMismatch,
    BridgeUnsupportedType,
    BridgeUnknownLanguage,

    // Capability 2.0 Errors (E-CAP-*, v1.4.2)
    CapMissingPermission,
    CapGlobViolation,

    // Entity-Guided Evidence Optimization Errors (Datara v1.5.0)
    ClassKeywordForbidden,
    StructMethodForbidden,
    ComponentMethodForbidden,
    ComponentNonPodField,
    BehaviorFieldForbidden,
    BehaviorImpure,
    RoleCapabilityMissing,
    ImplCoherenceViolation,
    ImplMissingSuperTrait,
    UnprintableType,
    DeprecatedPrintFunction,

    // Comptime Errors (E-CT-*, v1.4.5)
    ComptimeRecursionLimit,
    ComptimeForbiddenEffect,
    ComptimeNonConstArg,

    // Bare-Metal & Target Errors (v1.4.5)
    MmioRequiresUnsafe,
    TargetRequiresLlvm,

    // Link-Phase Errors (E-LINK-*, v1.5.0 M4)
    LinkDuplicateSymbol,

    // TMM Errors (E-TMM-*, v1.5.0 M5)
    TmmArcCycle,
    TmmBorrowEscapesCall,
    AnyTypeLeak,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::SyntaxUnexpectedToken => "E-SYNTAX-001",
            ErrorCode::SyntaxUnterminatedString => "E-SYNTAX-002",
            ErrorCode::SyntaxUnterminatedComment => "E-SYNTAX-003",
            ErrorCode::SyntaxInvalidNumber => "E-SYNTAX-004",
            ErrorCode::SyntaxInvalidChar => "E-SYNTAX-005",
            ErrorCode::SyntaxExpectedExpression => "E-SYNTAX-006",
            ErrorCode::SyntaxExpectedIdentifier => "E-SYNTAX-007",
            ErrorCode::SyntaxExpectedType => "E-SYNTAX-008",
            ErrorCode::SyntaxInvalidEscape => "E-SYNTAX-009",
            ErrorCode::RecursionLimitExceeded => "E0105",

            ErrorCode::ResolveUndefinedSymbol => "E-RESOLVE-001",
            ErrorCode::ResolveDuplicateSymbol => "E-RESOLVE-002",
            ErrorCode::ResolveUnknownType => "E-RESOLVE-003",
            ErrorCode::ResolveCircularDependency => "E-RESOLVE-004",
            ErrorCode::ResolveUnreachableModule => "E-RESOLVE-005",
            ErrorCode::PrivateItemAccess => "E0042",

            ErrorCode::TypeMismatch => "E-TYPE-001",
            ErrorCode::TypeCannotInfer => "E-TYPE-002",
            ErrorCode::TypeMissingReturn => "E-TYPE-003",
            ErrorCode::TypeInvalidBinaryOp => "E-TYPE-004",
            ErrorCode::TypeInvalidUnaryOp => "E-TYPE-005",
            ErrorCode::TypeInvalidMemberAccess => "E-TYPE-006",
            ErrorCode::TypeGenericMismatch => "E-TYPE-007",
            ErrorCode::TypeIncomparableOperands => "E-TYPE-008",
            ErrorCode::TypeUnknownMethod => "E-TYPE-009",
            ErrorCode::QuestionPropagation => "E-TYPE-010",

            ErrorCode::BorrowUseAfterMove => "E-BORROW-001",
            ErrorCode::BorrowCannotMutateImmutable => "E-BORROW-002",
            ErrorCode::BorrowConflictActiveView => "E-BORROW-003",
            ErrorCode::BorrowMultipleMutableViews => "E-BORROW-004",
            ErrorCode::BorrowEscapingView => "E-BORROW-005",
            ErrorCode::BorrowConflict => "E-BORROW-006",

            ErrorCode::EffectImpureInPureContext => "E-EFFECT-001",
            ErrorCode::EffectUnsafeOperation => "E-EFFECT-002",
            ErrorCode::EffectUnhandledIO => "E-EFFECT-003",

            ErrorCode::CodegenBackendFailed => "E-CODEGEN-001",
            ErrorCode::CodegenLinkerFailed => "E-CODEGEN-002",
            ErrorCode::CodegenIOError => "E-CODEGEN-003",

            ErrorCode::SecurityViolation => "E0940",
            ErrorCode::ProofCarryingCodeViolation => "E0941",
            ErrorCode::UncheckedFFIViolation => "E0942",
            ErrorCode::DataRaceViolation => "E0943",
            ErrorCode::AllocationViolation => "E0950",
            ErrorCode::PanicViolation => "E0951",
            ErrorCode::CImportHeaderNotFound => "E0960",
            ErrorCode::CImportReadFailed => "E0961",
            ErrorCode::CImportUnsupportedConstruct => "E0962",
            ErrorCode::NonExhaustiveMatch => "E0310",
            ErrorCode::UnreachablePattern => "E0311",
            ErrorCode::LoopControlOutsideLoop => "E0312",
            ErrorCode::DimensionMismatch => "E0420",
            ErrorCode::InvariantViolation => "E0945",
            ErrorCode::TerminationViolation => "E0946",
            ErrorCode::RangeViolation => "E0947",
            ErrorCode::ContractViolation => "E0948",
            ErrorCode::InternalVerification => "E0901",
            ErrorCode::InlineAsmUnsupported => "E0902",
            ErrorCode::UnknownBinop => "E0903",
            ErrorCode::CApiArgLimitExceeded => "E0904",
            ErrorCode::AsyncBackendUnsupported => "E0955",
            ErrorCode::CrossCompilationMissingToolchain => "E0980",
            ErrorCode::InlineAlwaysExtern => "E0956",
            ErrorCode::InlineAttributeInvalid => "E0957",
            ErrorCode::ArenaEscape => "E1401",
            ErrorCode::AsmUnsupportedInst => "E1402",
            ErrorCode::AsmOperandNotInt => "E1403",
            ErrorCode::AsmRequiresUnsafe => "E1404",
            ErrorCode::AsmUnsupportedBackend => "E1405",
            ErrorCode::PoolCapacityExceeded => "E1406",
            ErrorCode::OptimizationUnproven => "E-OPT-001",
            ErrorCode::DeprecatedFeature => "W0100",
            ErrorCode::CanonicalKeyword => "W-SYN-001",
            ErrorCode::CanonicalTypeSpelling => "W-SYN-002",
            ErrorCode::BoolIntComparison => "W-TYPE-002",
            ErrorCode::BridgeTypeMismatch => "E-BRIDGE-001",
            ErrorCode::BridgeUnsupportedType => "E-BRIDGE-002",
            ErrorCode::BridgeUnknownLanguage => "E-BRIDGE-003",
            ErrorCode::CapMissingPermission => "E-CAP-001",
            ErrorCode::CapGlobViolation => "E-CAP-002",
            ErrorCode::ClassKeywordForbidden => "E0100",
            ErrorCode::StructMethodForbidden => "E-STRUCT-001",
            ErrorCode::ComponentMethodForbidden => "E-COMP-001",
            ErrorCode::ComponentNonPodField => "E-COMP-002",
            ErrorCode::BehaviorFieldForbidden => "E-BEH-001",
            ErrorCode::BehaviorImpure => "E-BEH-002",
            ErrorCode::RoleCapabilityMissing => "E-ROLE-001",
            ErrorCode::ImplCoherenceViolation => "E-IMPL-001",
            ErrorCode::ImplMissingSuperTrait => "E-IMPL-002",
            ErrorCode::UnprintableType => "E-OUT-001",
            ErrorCode::DeprecatedPrintFunction => "W0102",

            ErrorCode::ComptimeRecursionLimit => "E-CT-001",
            ErrorCode::ComptimeForbiddenEffect => "E-CT-002",
            ErrorCode::ComptimeNonConstArg => "E-CT-003",
            ErrorCode::MmioRequiresUnsafe => "E-MMIO-001",
            ErrorCode::TargetRequiresLlvm => "E-TARGET-001",

            ErrorCode::LinkDuplicateSymbol => "E-LINK-001",
            ErrorCode::AnyTypeLeak => "E-ANY-001",
            ErrorCode::TmmArcCycle => "E-TMM-001",
            ErrorCode::TmmBorrowEscapesCall => "E-TMM-002",
        }
    }

    /// Diagnostics are ENGLISH-ONLY by owner decision (2026-09-25):
    /// the locale parameter is kept for API compatibility but ignored.
    pub fn description(&self, _locale: &str) -> &'static str {
        match self {
            ErrorCode::SyntaxUnexpectedToken => "Unexpected token in source",
            ErrorCode::SyntaxUnterminatedString => "Unterminated string literal",
            ErrorCode::SyntaxUnterminatedComment => "Unterminated multi-line comment",
            ErrorCode::SyntaxInvalidNumber => "Invalid numeric literal format",
            ErrorCode::SyntaxInvalidChar => "Invalid character in input stream",
            ErrorCode::SyntaxInvalidEscape => "Invalid escape sequence in string literal",
            ErrorCode::SyntaxExpectedExpression => "Expected an expression",
            ErrorCode::SyntaxExpectedIdentifier => "Expected an identifier",
            ErrorCode::SyntaxExpectedType => "Expected a type annotation",
            ErrorCode::RecursionLimitExceeded => {
                "Expression nesting exceeds maximum allowed depth (64)"
            }

            ErrorCode::ResolveUndefinedSymbol => "Undefined symbol reference",
            ErrorCode::ResolveDuplicateSymbol => "Duplicate symbol declaration",
            ErrorCode::ResolveUnknownType => "Unknown type identifier",
            ErrorCode::AnyTypeLeak => {
                "A dynamic (val) value leaked into a statically-typed context"
            }
            ErrorCode::ResolveCircularDependency => "Circular module dependency detected",
            ErrorCode::ResolveUnreachableModule => "Unreachable module detected",
            ErrorCode::PrivateItemAccess => {
                "Cannot access private item outside of its declaring module"
            }

            ErrorCode::TypeMismatch => "Static type mismatch",
            ErrorCode::TypeCannotInfer => "Unable to infer expression type",
            ErrorCode::TypeMissingReturn => "Non-unit function missing return value",
            ErrorCode::TypeInvalidBinaryOp => "Binary operator not defined for operand types",
            ErrorCode::TypeInvalidUnaryOp => "Unary operator not defined for operand type",
            ErrorCode::TypeInvalidMemberAccess => "Field or method does not exist on type",
            ErrorCode::TypeGenericMismatch => "Generic type argument mismatch",
            ErrorCode::TypeIncomparableOperands => {
                "Ordering comparison over incompatible types: implicit conversions are forbidden"
            }
            ErrorCode::TypeUnknownMethod => "No method with this name exists for the receiver type",
            ErrorCode::QuestionPropagation => {
                "'?' requires the enclosing function to return a Result/Option-like type"
            }

            ErrorCode::BorrowUseAfterMove => "Use of moved value (use-after-move)",
            ErrorCode::BorrowCannotMutateImmutable => "Cannot mutate or reassign immutable binding",
            ErrorCode::BorrowConflictActiveView => {
                "Cannot mutate value while active immutable view exists"
            }
            ErrorCode::BorrowMultipleMutableViews => {
                "Cannot create multiple concurrent mutable views"
            }
            ErrorCode::BorrowEscapingView => {
                "Escaping view: reference cannot outlive local binding"
            }
            ErrorCode::BorrowConflict => {
                "Borrow conflict: mutable reference exclusivity violation or reading during active &mut"
            }

            ErrorCode::EffectImpureInPureContext => "Side effect occurred in pure context",
            ErrorCode::EffectUnsafeOperation => "Unsafe operation without explicit unsafe block",
            ErrorCode::EffectUnhandledIO => "Unhandled IO operation",

            ErrorCode::CodegenBackendFailed => "Native codegen backend failed",
            ErrorCode::CodegenLinkerFailed => "Linker execution failed",
            ErrorCode::CodegenIOError => "IO failure during artifact generation",

            ErrorCode::SecurityViolation => {
                "Security Violation: Operation requires capability token"
            }
            ErrorCode::ProofCarryingCodeViolation => {
                "Proof-Carrying Code Violation: Unproven operation"
            }
            ErrorCode::UncheckedFFIViolation => {
                "Security Violation: Foreign call requires unsafe justification"
            }
            ErrorCode::DataRaceViolation => {
                "Concurrency Violation: Potential data race across threads"
            }
            ErrorCode::AllocationViolation => {
                "Real-Time Violation: Dynamic memory allocation in '@no_alloc' context"
            }
            ErrorCode::PanicViolation => {
                "Real-Time Violation: Unproven panic path in '@no_panic' context"
            }
            ErrorCode::CImportHeaderNotFound => "C header file not found",
            ErrorCode::CImportReadFailed => "Failed to read C header file",
            ErrorCode::CImportUnsupportedConstruct => "Unsupported C construct in header file",
            ErrorCode::NonExhaustiveMatch => {
                "Non-exhaustive patterns in match expression: missing patterns"
            }
            ErrorCode::UnreachablePattern => "Unreachable pattern in match expression",
            ErrorCode::LoopControlOutsideLoop => {
                "'break' or 'continue' outside of a loop body (while/for/loop)"
            }
            ErrorCode::DimensionMismatch => {
                "Dimension mismatch: incompatible units of measure in arithmetic expression"
            }
            ErrorCode::InvariantViolation => {
                "Class invariant violation: class invariant cannot be proven upon method exit"
            }
            ErrorCode::TerminationViolation => {
                "Termination violation: cannot prove function termination in pure context"
            }
            ErrorCode::RangeViolation => {
                "Range violation: value exceeds static type interval or array bounds"
            }
            ErrorCode::ContractViolation => {
                "Contract violation: precondition (requires) or postcondition (ensures) not satisfied"
            }
            ErrorCode::InternalVerification => {
                "Internal DMIR intermediate representation verification failure"
            }
            ErrorCode::InlineAsmUnsupported => {
                "Inline assembly is unsupported on the selected target backend"
            }
            ErrorCode::UnknownBinop => "Unknown binary operator encountered during code generation",
            ErrorCode::CApiArgLimitExceeded => {
                "C API argument limit exceeded (maximum 8 supported arguments)"
            }
            ErrorCode::AsyncBackendUnsupported => {
                "Async execution (async/await) is unsupported on the selected target backend: requires PCS runtime"
            }
            ErrorCode::CrossCompilationMissingToolchain => {
                "Cross-compilation toolchain not found for target triple (lld or cross-linker required)"
            }
            ErrorCode::InlineAlwaysExtern => {
                "'@inline(always)' is rejected on extern declarations: the compiler cannot inline a body it does not have"
            }
            ErrorCode::InlineAttributeInvalid => {
                "Malformed '@inline' attribute: expected '@inline', '@inline(always)' or '@inline(never)'"
            }
            ErrorCode::ArenaEscape => {
                "A value allocated inside an @arena function escapes the arena region: only Copy types (Int, Float, Bool, Str) may be returned"
            }
            ErrorCode::AsmUnsupportedInst => {
                "Instruction outside the supported asm safe subset: only mov/add/sub between GP registers, Int variables and integer immediates are accepted"
            }
            ErrorCode::AsmOperandNotInt => "Asm block operands must be Int variables",
            ErrorCode::AsmRequiresUnsafe => {
                "Asm blocks must be wrapped in an unsafe(justification: \"...\") block"
            }
            ErrorCode::AsmUnsupportedBackend => {
                "Structured asm blocks are not supported on this backend: use the Cranelift backend"
            }
            ErrorCode::PoolCapacityExceeded => {
                "The @pool capacity is exceeded: the function provably allocates more values than the pool holds"
            }
            ErrorCode::OptimizationUnproven => {
                "Optimizer layout transformation skipped: semantic equivalence could not be proven"
            }

            ErrorCode::DeprecatedFeature => {
                "Deprecated language construct: use the modern equivalent"
            }
            ErrorCode::CanonicalKeyword => "Synonymous keyword: use the canonical spelling",
            ErrorCode::CanonicalTypeSpelling => {
                "Synonymous type spelling: use the canonical type name"
            }
            ErrorCode::BoolIntComparison => {
                "Comparison between Bool and an integer: operand types must match"
            }
            ErrorCode::BridgeTypeMismatch => "Bridge argument type mismatch",
            ErrorCode::BridgeUnsupportedType => {
                "Unsupported type in declarative bridge function signature"
            }
            ErrorCode::BridgeUnknownLanguage => "Unknown foreign language in bridge declaration",
            ErrorCode::CapMissingPermission => {
                "Operation requires declared permission in datara.toml [capabilities]"
            }
            ErrorCode::CapGlobViolation => {
                "Path or target address violates allowed glob in datara.toml [capabilities]"
            }
            ErrorCode::ClassKeywordForbidden => {
                "The 'class' keyword does not exist in Datara: use 'struct' for data and 'behavior' or 'impl' for methods"
            }
            ErrorCode::StructMethodForbidden => {
                "Methods cannot be defined inside 'struct': move methods into 'behavior <Type>' or 'impl <Trait> for <Type>'"
            }
            ErrorCode::ComponentMethodForbidden => {
                "Components are pure POD data containers and cannot declare methods: move logic to a 'behavior'"
            }
            ErrorCode::ComponentNonPodField => {
                "Component fields must be POD types (Int, Float, Bool, Char, or nested component): heap types are forbidden"
            }
            ErrorCode::BehaviorFieldForbidden => {
                "Behaviors cannot declare state or fields: move fields into 'struct' or 'component'"
            }
            ErrorCode::BehaviorImpure => {
                "Methods in a behavior are pure by default: impure operations (IO, Net) require explicit '#[effect(IO)]'"
            }
            ErrorCode::RoleCapabilityMissing => {
                "Role capability requirement is not satisfied by the target type or environment"
            }
            ErrorCode::ImplCoherenceViolation => {
                "Coherence violation: duplicate implementation of trait for this type"
            }
            ErrorCode::ImplMissingSuperTrait => {
                "Missing super-trait implementation required by trait definition"
            }
            ErrorCode::UnprintableType => {
                "Type is not printable: primitive type (Int, Float, Bool, Char, Str) or @derive(Display) is required"
            }
            ErrorCode::DeprecatedPrintFunction => {
                "Print function is deprecated: use 'out' or 'err' statement instead"
            }
            ErrorCode::ComptimeRecursionLimit => {
                "Recursion depth or step limit exceeded in comptime execution (E-CT-001)"
            }
            ErrorCode::ComptimeForbiddenEffect => {
                "Effect not allowed in comptime execution: I/O, runtime builtins, and unsafe are forbidden (E-CT-002)"
            }
            ErrorCode::ComptimeNonConstArg => {
                "Arguments to comptime function must be compile-time constants (E-CT-003)"
            }
            ErrorCode::MmioRequiresUnsafe => {
                "MMIO access requires an 'unsafe' block or declaration in datara.toml [devices] (E-MMIO-001)"
            }
            ErrorCode::TargetRequiresLlvm => {
                "Target is only supported via LLVM backend (E-TARGET-001)"
            }
            ErrorCode::LinkDuplicateSymbol => {
                "Duplicate symbol definition in link phase: two declarations mangle to the same link name (E-LINK-001)"
            }
            ErrorCode::TmmArcCycle => {
                "Strong reference cycle in @arc ownership graph: the refcount can never reach zero (E-TMM-001)"
            }
            ErrorCode::TmmBorrowEscapesCall => {
                "A borrowed value must not outlive the call that produced or accepted it (E-TMM-002)"
            }
        }
    }
}

/// M2 Diagnostics Gold: exhaustive variant list, generated from the
/// ErrorCode enum so the coverage test cannot drift from reality.
pub fn all_codes() -> Vec<ErrorCode> {
    use ErrorCode::*;
    vec![
        SyntaxUnexpectedToken,
        SyntaxUnterminatedString,
        SyntaxUnterminatedComment,
        SyntaxInvalidNumber,
        SyntaxInvalidChar,
        SyntaxExpectedExpression,
        SyntaxExpectedIdentifier,
        SyntaxExpectedType,
        SyntaxInvalidEscape,
        RecursionLimitExceeded,
        ResolveUndefinedSymbol,
        ResolveDuplicateSymbol,
        ResolveUnknownType,
        ResolveCircularDependency,
        ResolveUnreachableModule,
        PrivateItemAccess,
        TypeMismatch,
        TypeCannotInfer,
        TypeMissingReturn,
        TypeInvalidBinaryOp,
        TypeInvalidUnaryOp,
        TypeInvalidMemberAccess,
        TypeGenericMismatch,
        TypeIncomparableOperands,
        TypeUnknownMethod,
        QuestionPropagation,
        BorrowUseAfterMove,
        BorrowCannotMutateImmutable,
        BorrowConflictActiveView,
        BorrowMultipleMutableViews,
        BorrowEscapingView,
        BorrowConflict,
        EffectImpureInPureContext,
        EffectUnsafeOperation,
        EffectUnhandledIO,
        CodegenBackendFailed,
        CodegenLinkerFailed,
        CodegenIOError,
        SecurityViolation,
        ProofCarryingCodeViolation,
        UncheckedFFIViolation,
        DataRaceViolation,
        AllocationViolation,
        PanicViolation,
        CImportHeaderNotFound,
        CImportReadFailed,
        CImportUnsupportedConstruct,
        NonExhaustiveMatch,
        UnreachablePattern,
        LoopControlOutsideLoop,
        DimensionMismatch,
        InvariantViolation,
        TerminationViolation,
        RangeViolation,
        ContractViolation,
        InternalVerification,
        InlineAsmUnsupported,
        UnknownBinop,
        CApiArgLimitExceeded,
        AsyncBackendUnsupported,
        CrossCompilationMissingToolchain,
        InlineAlwaysExtern,
        InlineAttributeInvalid,
        ArenaEscape,
        PoolCapacityExceeded,
        AsmUnsupportedInst,
        AsmOperandNotInt,
        AsmRequiresUnsafe,
        AsmUnsupportedBackend,
        OptimizationUnproven,
        DeprecatedFeature,
        CanonicalKeyword,
        CanonicalTypeSpelling,
        BoolIntComparison,
        BridgeTypeMismatch,
        BridgeUnsupportedType,
        BridgeUnknownLanguage,
        CapMissingPermission,
        CapGlobViolation,
        ClassKeywordForbidden,
        StructMethodForbidden,
        ComponentMethodForbidden,
        ComponentNonPodField,
        BehaviorFieldForbidden,
        BehaviorImpure,
        RoleCapabilityMissing,
        ImplCoherenceViolation,
        ImplMissingSuperTrait,
        UnprintableType,
        DeprecatedPrintFunction,
        ComptimeRecursionLimit,
        ComptimeForbiddenEffect,
        ComptimeNonConstArg,
        MmioRequiresUnsafe,
        TargetRequiresLlvm,
        LinkDuplicateSymbol,
        TmmArcCycle,
        TmmBorrowEscapesCall,
        AnyTypeLeak,
    ]
}
