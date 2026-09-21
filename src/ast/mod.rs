pub use crate::diagnostics::SourceSpan;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<(String, String)>,
    pub span: SourceSpan,
}

/// v1.4.0: the four x86-64 general-purpose scratch registers the structured
/// `asm { ... }` safe subset supports. The 32-bit aliases (`eax`..`edx`)
/// lex to the same 64-bit register: the whole subset is Int (i64) typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsmReg {
    Ax,
    Bx,
    Cx,
    Dx,
}

impl AsmReg {
    /// Maps a register name (Intel, 32- or 64-bit spelling) to its slot.
    pub fn from_name(name: &str) -> Option<AsmReg> {
        match name {
            "rax" | "eax" | "ax" => Some(AsmReg::Ax),
            "rbx" | "ebx" | "bx" => Some(AsmReg::Bx),
            "rcx" | "ecx" | "cx" => Some(AsmReg::Cx),
            "rdx" | "edx" | "dx" => Some(AsmReg::Dx),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AsmReg::Ax => "rax",
            AsmReg::Bx => "rbx",
            AsmReg::Cx => "rcx",
            AsmReg::Dx => "rdx",
        }
    }

    pub fn slot(&self) -> usize {
        match self {
            AsmReg::Ax => 0,
            AsmReg::Bx => 1,
            AsmReg::Cx => 2,
            AsmReg::Dx => 3,
        }
    }
}

/// v1.4.0: one operand of a structured asm line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsmOperand {
    Reg(AsmReg),
    /// A Datara variable in scope. Must be Int (E1403 otherwise).
    Var(String, SourceSpan),
    /// Integer immediate.
    Imm(i64),
}

/// v1.4.0: mnemonics of the structured asm safe subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsmOp {
    Mov,
    Add,
    Sub,
}

/// v1.4.0: a single line of a structured `asm { ... }` block.
///
/// Well-formed lines inside the safe subset parse to `Inst`; anything else
/// (unknown mnemonic, memory operands, floating-point, unparsable text) is
/// kept as `Unsupported` so the driver-level validation can reject it with
/// E1402 instead of a generic syntax error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsmLine {
    Inst {
        op: AsmOp,
        dst: AsmOperand,
        src: AsmOperand,
        span: SourceSpan,
    },
    Unsupported {
        raw: String,
        span: SourceSpan,
    },
}

impl AsmLine {
    pub fn span(&self) -> &SourceSpan {
        match self {
            AsmLine::Inst { span, .. } | AsmLine::Unsupported { span, .. } => span,
        }
    }
}

/// True when the statement tree contains any `asm`/`asm!` block (structured
/// or legacy template).
pub fn stmt_contains_asm(stmt: &Stmt) -> bool {
    stmt_has_asm_kind(stmt, false)
}

/// True when the statement tree contains a structured `asm { ... }` block.
/// Used to set the DMIR `Function::has_inline_asm` marker that non-Cranelift
/// backends reject with E1405. Legacy `asm!` template blocks keep their
/// existing LLVM-only contract and do NOT set the marker.
pub fn stmt_contains_structured_asm(stmt: &Stmt) -> bool {
    stmt_has_asm_kind(stmt, true)
}

fn stmt_has_asm_kind(stmt: &Stmt, structured_only: bool) -> bool {
    match stmt {
        Stmt::Asm { structured, .. } => !structured_only || !structured.is_empty(),
        Stmt::Block(stmts, _) => stmts.iter().any(|s| stmt_has_asm_kind(s, structured_only)),
        Stmt::Let { init, .. }
        | Stmt::Mut { init, .. }
        | Stmt::Const { init, .. }
        | Stmt::Val { init, .. }
        | Stmt::CompactBind { init, .. } => expr_has_asm_kind(init, structured_only),
        Stmt::Assign { target, value, .. } => {
            expr_has_asm_kind(target, structured_only) || expr_has_asm_kind(value, structured_only)
        }
        Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => {
            expr_has_asm_kind(e, structured_only)
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            expr_has_asm_kind(condition, structured_only)
                || stmt_has_asm_kind(then_branch, structured_only)
                || else_branch
                    .as_ref()
                    .is_some_and(|b| stmt_has_asm_kind(b, structured_only))
        }
        Stmt::For { iterable, body, .. } | Stmt::ParallelFor { iterable, body, .. } => {
            expr_has_asm_kind(iterable, structured_only) || stmt_has_asm_kind(body, structured_only)
        }
        Stmt::While {
            condition, body, ..
        } => {
            expr_has_asm_kind(condition, structured_only)
                || stmt_has_asm_kind(body, structured_only)
        }
        Stmt::Loop { body, .. }
        | Stmt::Parallel(body, _)
        | Stmt::Simd(body, _)
        | Stmt::Unsafe { body, .. } => stmt_has_asm_kind(body, structured_only),
        Stmt::TryCatch {
            try_block,
            catch_block,
            ..
        } => {
            stmt_has_asm_kind(try_block, structured_only)
                || stmt_has_asm_kind(catch_block, structured_only)
        }
        Stmt::With { init, body, .. } => {
            expr_has_asm_kind(init, structured_only) || stmt_has_asm_kind(body, structured_only)
        }
        Stmt::Return(Some(e), _) => expr_has_asm_kind(e, structured_only),
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Return(None, _) => false,
    }
}

fn expr_has_asm_kind(expr: &Expr, structured_only: bool) -> bool {
    match expr {
        Expr::Block(stmts, trailing, _) => {
            stmts.iter().any(|s| stmt_has_asm_kind(s, structured_only))
                || trailing
                    .as_ref()
                    .is_some_and(|t| expr_has_asm_kind(t, structured_only))
        }
        Expr::Binary { left, right, .. } => {
            expr_has_asm_kind(left, structured_only) || expr_has_asm_kind(right, structured_only)
        }
        Expr::Unary { expr, .. }
        | Expr::ErrorPropagate(expr, _)
        | Expr::Wrapping(expr, _)
        | Expr::Saturating(expr, _)
        | Expr::Comptime { expr, .. }
        | Expr::Cast { expr, .. } => expr_has_asm_kind(expr, structured_only),
        Expr::Call { callee, args, .. } => {
            expr_has_asm_kind(callee, structured_only)
                || args.iter().any(|a| expr_has_asm_kind(a, structured_only))
        }
        Expr::MemberAccess { object, .. } => expr_has_asm_kind(object, structured_only),
        Expr::IndexAccess { object, index, .. } => {
            expr_has_asm_kind(object, structured_only) || expr_has_asm_kind(index, structured_only)
        }
        Expr::Range { start, end, .. } => {
            expr_has_asm_kind(start, structured_only) || expr_has_asm_kind(end, structured_only)
        }
        Expr::Tuple(exprs, _) | Expr::ListLiteral(exprs, _) => {
            exprs.iter().any(|e| expr_has_asm_kind(e, structured_only))
        }
        Expr::MapLiteral(entries, _) => entries.iter().any(|(k, v)| {
            expr_has_asm_kind(k, structured_only) || expr_has_asm_kind(v, structured_only)
        }),
        Expr::ObjectInit { fields, .. } => fields
            .iter()
            .any(|(_, e)| expr_has_asm_kind(e, structured_only)),
        Expr::Pipeline { stages, .. } => {
            stages.iter().any(|e| expr_has_asm_kind(e, structured_only))
        }
        Expr::InterpolatedString { expressions, .. } => expressions
            .iter()
            .any(|e| expr_has_asm_kind(e, structured_only)),
        Expr::Decide { arms, else_arm, .. } => {
            arms.iter().any(|a| {
                expr_has_asm_kind(&a.condition, structured_only)
                    || expr_has_asm_kind(&a.body, structured_only)
            }) || else_arm
                .as_ref()
                .is_some_and(|e| expr_has_asm_kind(e, structured_only))
        }
        Expr::Match { value, arms, .. } => {
            expr_has_asm_kind(value, structured_only)
                || arms.iter().any(|a| {
                    a.guard
                        .as_ref()
                        .is_some_and(|g| expr_has_asm_kind(g, structured_only))
                        || expr_has_asm_kind(&a.body, structured_only)
                })
        }
        Expr::Select { arms, else_arm, .. } => {
            arms.iter().any(|a| {
                expr_has_asm_kind(&a.condition, structured_only)
                    || expr_has_asm_kind(&a.body, structured_only)
            }) || else_arm
                .as_ref()
                .is_some_and(|e| expr_has_asm_kind(e, structured_only))
        }
        Expr::Lambda { body, .. } => expr_has_asm_kind(body, structured_only),
        Expr::OrRecovery { expr, arms, .. } => {
            expr_has_asm_kind(expr, structured_only)
                || arms.iter().any(|a| {
                    a.guard
                        .as_ref()
                        .is_some_and(|g| expr_has_asm_kind(g, structured_only))
                        || expr_has_asm_kind(&a.body, structured_only)
                })
        }
        Expr::ArrayRepeatLiteral { elem, .. } => expr_has_asm_kind(elem, structured_only),
        Expr::Literal(..) | Expr::Identifier(..) => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitFieldRange {
    Single(usize),
    Range { start: usize, end: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterField {
    pub name: String,
    pub type_node: TypeNode,
    pub offset: u64,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDecl {
    pub name: String,
    pub base_address: u64,
    pub fields: Vec<RegisterField>,
    pub attributes: Vec<Attribute>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CImportDecl {
    pub header_path: String,
    pub link_libs: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Decl>,
    pub attributes: Vec<Attribute>,
    pub file: String,
    #[serde(default)]
    pub link_libraries: Vec<String>,
    /// v1.3.3 namespace support: `use path as alias` (or the last path
    /// segment) maps to the module's exported top-level function names, so
    /// `alias.func(...)` resolves to the module's `func`. Flat unqualified
    /// names remain available for compatibility.
    #[serde(default)]
    pub module_aliases: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Decl {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Decl::Use(d) => &d.span,
            Decl::CImport(d) => &d.span,
            Decl::Class(d) => &d.span,
            Decl::Enum(d) => &d.span,
            Decl::Behavior(d) => &d.span,
            Decl::Component(d) => &d.span,
            Decl::Role(d) => &d.span,
            Decl::Function(d) | Decl::Flow(d) | Decl::Task(d) => &d.span,
            Decl::Packet(d) => &d.span,
            Decl::ExternFn(d) => &d.span,
            Decl::Type(d) => &d.span,
            Decl::Register(d) => &d.span,
            Decl::Trait(d) => &d.span,
            Decl::Impl(d) => &d.span,
            Decl::Bridge(d) => &d.span,
            Decl::Global(d) => &d.span,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalDecl {
    pub is_export: bool,
    pub is_mut: bool,
    pub is_const: bool,
    pub name: String,
    pub type_node: Option<TypeNode>,
    pub init: Expr,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeFn {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeNode>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDecl {
    pub lang: String,
    pub module: String,
    pub functions: Vec<BridgeFn>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub group: Vec<String>,
    pub alias: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDecl {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub generic_params: Vec<String>,
    pub base_type: Option<String>,
    pub compositions: Vec<String>,
    pub body_items: Vec<ClassItem>,
    pub invariants: Vec<Expr>,
    pub is_export: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDecl {
    pub name: String,
    pub generic_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub is_export: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<TypeNode>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorDecl {
    pub target_type: String,
    pub body_items: Vec<ClassItem>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDecl {
    pub name: String,
    pub body_items: Vec<ClassItem>,
    pub is_export: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDecl {
    pub name: String,
    pub methods: Vec<MethodDecl>,
    pub is_export: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitMethodSignature {
    pub name: String,
    pub generic_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeNode>,
    pub default_body: Option<Box<Stmt>>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitDef {
    pub name: String,
    pub generic_params: Vec<String>,
    pub super_traits: Vec<String>,
    pub methods: Vec<TraitMethodSignature>,
    pub is_export: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplBlock {
    pub trait_name: Option<String>,
    pub target_type: String,
    pub target_type_args: Vec<String>,
    pub methods: Vec<FunctionDecl>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractClause {
    pub condition: Expr,
    pub message: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Refinement {
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    Predicate {
        var_name: String,
        predicate: Box<Expr>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDecl {
    pub name: String,
    pub base_type: TypeNode,
    pub is_export: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDecl {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub generic_params: Vec<String>,
    pub generic_constraints: Vec<(String, String)>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeNode>,
    pub requires: Vec<ContractClause>,
    pub ensures: Vec<ContractClause>,
    pub decreases: Option<Expr>,
    pub body: Box<Stmt>,
    pub is_expression_body: bool,
    pub is_export: bool,
    #[serde(default)]
    pub is_comptime: bool,
    pub span: SourceSpan,
}

impl FunctionDecl {
    pub fn is_async(&self) -> bool {
        self.attributes.iter().any(|a| a.name == "async")
    }

    pub fn is_comptime(&self) -> bool {
        self.is_comptime || self.attributes.iter().any(|a| a.name == "comptime")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClassItem {
    Field(FieldDecl),
    Method(MethodDecl),
    Using(String, SourceSpan),
    Invariant(Expr, SourceSpan),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketField {
    pub name: String,
    pub bits: usize,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketDecl {
    pub name: String,
    pub fields: Vec<PacketField>,
    pub span: SourceSpan,
}

/// Register class of one SysV AMD64 return eightbyte (v1.3.3).
///
/// The System V AMD64 ABI returns a two-eightbyte aggregate in register
/// pairs: INTEGER eightbytes take RAX then RDX, SSE eightbytes take XMM0
/// then XMM1 (independent per-class sequences). One eightbyte of a
/// layout-compatible C struct is INTEGER for an 8-byte integer/pointer
/// field and SSE for an 8-byte float field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SysVClass {
    Integer,
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternFnDecl {
    pub abi: String,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeNode>,
    /// Attributes written before the `extern` declaration (v1.3.4). They are
    /// kept so the frontend can validate them — e.g. `@inline(always)` on
    /// an extern "C" declaration is rejected with E0956 instead of being
    /// silently dropped.
    #[serde(default)]
    pub attributes: Vec<Attribute>,
    /// Set by `cimport` when the C function returns a by-value struct that
    /// the native backend lowers through the hidden sret return-slot ABI:
    /// the value is the struct size in bytes. `None` for every other shape
    /// (scalar returns, pointer returns, non-sret-capable aggregates).
    #[serde(default)]
    pub sret_size: Option<usize>,
    /// Set by `cimport` when the C function returns a 16-byte
    /// layout-compatible struct that the native backend lowers through two
    /// SysV AMD64 return registers: `classes[0]` (offset 0) rides RAX/XMM0
    /// and `classes[1]` (offset 8) rides RDX/XMM1. Mutually exclusive with
    /// `sret_size`. `None` for every other shape.
    #[serde(default)]
    pub sysv_classes: Option<[SysVClass; 2]>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDecl {
    pub name: String,
    pub type_node: Option<TypeNode>,
    pub bit_field: Option<BitFieldRange>,
    #[serde(default)]
    pub offset: Option<u64>,
    pub default_value: Option<Expr>,
    pub is_mut: bool,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodDecl {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub generic_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeNode>,
    pub requires: Vec<ContractClause>,
    pub ensures: Vec<ContractClause>,
    pub decreases: Option<Expr>,
    pub body: Option<Box<Stmt>>,
    pub is_expression_body: bool,
    pub is_replaces: bool,
    pub replaces_target: Option<String>,
    pub span: SourceSpan,
}

impl MethodDecl {
    pub fn is_async(&self) -> bool {
        self.attributes.iter().any(|a| a.name == "async")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub type_node: Option<TypeNode>,
    pub ownership_mode: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeNode {
    pub name: String,
    pub generic_args: Vec<TypeNode>,
    pub is_option: bool,
    pub error_type: Option<Box<TypeNode>>,
    pub refinement: Option<Refinement>,
    pub span: SourceSpan,
}

impl TypeNode {
    pub fn new(name: &str, span: SourceSpan) -> Self {
        Self {
            name: name.to_string(),
            generic_args: Vec::new(),
            is_option: false,
            error_type: None,
            refinement: None,
            span,
        }
    }

    pub fn full_type_name(&self) -> String {
        if self.generic_args.is_empty() {
            self.name.clone()
        } else {
            let args = self
                .generic_args
                .iter()
                .map(|a| a.full_type_name())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{}>", self.name, args)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Stmt {
    Block(Vec<Stmt>, SourceSpan),
    Let {
        name: String,
        type_node: Option<TypeNode>,
        init: Expr,
        span: SourceSpan,
    },
    Mut {
        name: String,
        type_node: Option<TypeNode>,
        init: Expr,
        span: SourceSpan,
    },
    Const {
        name: String,
        type_node: Option<TypeNode>,
        init: Expr,
        span: SourceSpan,
    },
    Val {
        name: String,
        type_node: Option<TypeNode>,
        init: Expr,
        is_mut: bool,
        span: SourceSpan,
    },
    CompactBind {
        name: String,
        init: Expr,
        span: SourceSpan,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: SourceSpan,
    },
    Expr(Expr, SourceSpan),
    Out(Expr, SourceSpan),
    Err(Expr, SourceSpan),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
        span: SourceSpan,
    },
    For {
        var_name: String,
        iterable: Expr,
        body: Box<Stmt>,
        span: SourceSpan,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
        span: SourceSpan,
    },
    Loop {
        body: Box<Stmt>,
        span: SourceSpan,
    },
    Break(SourceSpan),
    Continue(SourceSpan),
    TryCatch {
        try_block: Box<Stmt>,
        err_var: String,
        catch_block: Box<Stmt>,
        span: SourceSpan,
    },
    Parallel(Box<Stmt>, SourceSpan),
    ParallelFor {
        var_name: String,
        iterable: Expr,
        body: Box<Stmt>,
        span: SourceSpan,
    },
    Simd(Box<Stmt>, SourceSpan),
    With {
        resource_name: String,
        init: Expr,
        body: Box<Stmt>,
        span: SourceSpan,
    },
    Unsafe {
        justification: Option<String>,
        body: Box<Stmt>,
        span: SourceSpan,
    },
    Asm {
        instructions: Vec<String>,
        options: Vec<String>,
        /// v1.4.0: structured safe-subset lines parsed from an `asm { ... }`
        /// block. Legacy `asm!` template blocks keep this empty and stay on
        /// the raw-template path (LLVM only).
        #[serde(default)]
        structured: Vec<AsmLine>,
        span: SourceSpan,
    },
    Return(Option<Expr>, SourceSpan),
}

impl Stmt {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Stmt::Block(_, s)
            | Stmt::Let { span: s, .. }
            | Stmt::Mut { span: s, .. }
            | Stmt::Const { span: s, .. }
            | Stmt::Val { span: s, .. }
            | Stmt::CompactBind { span: s, .. }
            | Stmt::Assign { span: s, .. }
            | Stmt::Expr(_, s)
            | Stmt::Out(_, s)
            | Stmt::Err(_, s)
            | Stmt::If { span: s, .. }
            | Stmt::For { span: s, .. }
            | Stmt::While { span: s, .. }
            | Stmt::Loop { span: s, .. }
            | Stmt::Break(s)
            | Stmt::Continue(s)
            | Stmt::TryCatch { span: s, .. }
            | Stmt::Parallel(_, s)
            | Stmt::ParallelFor { span: s, .. }
            | Stmt::Simd(_, s)
            | Stmt::With { span: s, .. }
            | Stmt::Unsafe { span: s, .. }
            | Stmt::Asm { span: s, .. }
            | Stmt::Return(_, s) => s,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    Literal(LiteralValue, SourceSpan),
    Identifier(String, SourceSpan),
    InterpolatedString {
        parts: Vec<String>,
        expressions: Vec<Expr>,
        // v1.4.5 W4: per-expression format specifiers (".2", "x", "X", "o",
        // "b"). Same length as `expressions` (empty string = no spec); serde
        // default keeps older serialized ASTs valid.
        #[serde(default)]
        specs: Vec<String>,
        span: SourceSpan,
    },
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
        span: SourceSpan,
    },
    Unary {
        op: String,
        expr: Box<Expr>,
        span: SourceSpan,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: SourceSpan,
    },
    MemberAccess {
        object: Box<Expr>,
        member: String,
        span: SourceSpan,
    },
    IndexAccess {
        object: Box<Expr>,
        index: Box<Expr>,
        span: SourceSpan,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
        span: SourceSpan,
    },
    Tuple(Vec<Expr>, SourceSpan),
    ObjectInit {
        class_name: String,
        generic_args: Vec<TypeNode>,
        fields: Vec<(String, Expr)>,
        span: SourceSpan,
    },
    Pipeline {
        stages: Vec<Expr>,
        span: SourceSpan,
    },
    Decide {
        arms: Vec<DecideArm>,
        else_arm: Option<Box<Expr>>,
        span: SourceSpan,
    },
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
        span: SourceSpan,
    },
    Select {
        arms: Vec<SelectArm>,
        else_arm: Option<Box<Expr>>,
        span: SourceSpan,
    },
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
        span: SourceSpan,
    },
    ListLiteral(Vec<Expr>, SourceSpan),
    MapLiteral(Vec<(Expr, Expr)>, SourceSpan),
    ErrorPropagate(Box<Expr>, SourceSpan),
    OrRecovery {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
        span: SourceSpan,
    },
    ArrayRepeatLiteral {
        elem: Box<Expr>,
        count: usize,
        span: SourceSpan,
    },
    Comptime {
        expr: Box<Expr>,
        span: SourceSpan,
    },
    Wrapping(Box<Expr>, SourceSpan),
    Saturating(Box<Expr>, SourceSpan),
    /// Explicit type cast: `expr as Type` (Gate 7 numeric conversions plus
    /// narrowing reinterpretation for the small integer families).
    Cast {
        expr: Box<Expr>,
        target_ty: String,
        span: SourceSpan,
    },
    /// A braced block used in expression position, e.g. a match/decide/select
    /// arm body: `{ let y = 2; y }`. The trailing expression (if any) is the
    /// block's value; `None` means the block evaluates to Unit.
    Block(Vec<Stmt>, Option<Box<Expr>>, SourceSpan),
}

impl Expr {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Expr::Literal(_, s)
            | Expr::Identifier(_, s)
            | Expr::InterpolatedString { span: s, .. }
            | Expr::Binary { span: s, .. }
            | Expr::Unary { span: s, .. }
            | Expr::Call { span: s, .. }
            | Expr::MemberAccess { span: s, .. }
            | Expr::IndexAccess { span: s, .. }
            | Expr::Range { span: s, .. }
            | Expr::Tuple(_, s)
            | Expr::ObjectInit { span: s, .. }
            | Expr::Pipeline { span: s, .. }
            | Expr::Decide { span: s, .. }
            | Expr::Match { span: s, .. }
            | Expr::Select { span: s, .. }
            | Expr::Lambda { span: s, .. }
            | Expr::ListLiteral(_, s)
            | Expr::MapLiteral(_, s)
            | Expr::ErrorPropagate(_, s)
            | Expr::OrRecovery { span: s, .. }
            | Expr::ArrayRepeatLiteral { span: s, .. }
            | Expr::Comptime { span: s, .. }
            | Expr::Wrapping(_, s)
            | Expr::Saturating(_, s)
            | Expr::Cast { span: s, .. }
            | Expr::Block(_, _, s) => s,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideArm {
    pub condition: Expr,
    pub body: Expr,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Pattern {
    Wildcard(SourceSpan),
    Identifier(String, SourceSpan),
    Literal(LiteralValue, SourceSpan),
    Variant {
        enum_name: Option<String>,
        variant_name: String,
        bindings: Vec<String>,
        span: SourceSpan,
    },
}

impl Pattern {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Pattern::Wildcard(s)
            | Pattern::Identifier(_, s)
            | Pattern::Literal(_, s)
            | Pattern::Variant { span: s, .. } => s,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectArm {
    pub condition: Expr,
    pub body: Expr,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    /// v1.4.5 W3: fixed-point decimal literal `19.99d`, stored as the raw
    /// i64 mantissa (value × 10⁴). Exact decimal arithmetic, no binary FP error.
    Dec64(i64),
    String(String),
    Bool(bool),
    Char(char),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureKind {
    ByRef,
    ByMove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedVar {
    pub name: String,
    pub kind: CaptureKind,
    pub span: SourceSpan,
}

/// Perform capture inference on a closure/lambda body to determine free variables
/// and whether they should be captured ByRef or ByMove.
pub fn infer_captures(
    params: &[Param],
    body: &Expr,
    enclosing_vars: &std::collections::HashSet<String>,
    is_escaping: bool,
) -> Vec<CapturedVar> {
    let mut param_names: std::collections::HashSet<String> =
        params.iter().map(|p| p.name.clone()).collect();
    let mut captured = Vec::new();
    let mut seen = std::collections::HashSet::new();

    fn walk_expr(
        expr: &Expr,
        params: &mut std::collections::HashSet<String>,
        enclosing: &std::collections::HashSet<String>,
        is_escaping: bool,
        captured: &mut Vec<CapturedVar>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        match expr {
            Expr::Identifier(name, span) => {
                if !params.contains(name) && enclosing.contains(name) && seen.insert(name.clone()) {
                    captured.push(CapturedVar {
                        name: name.clone(),
                        kind: if is_escaping {
                            CaptureKind::ByMove
                        } else {
                            CaptureKind::ByRef
                        },
                        span: span.clone(),
                    });
                }
            }
            Expr::Binary { left, right, .. } => {
                walk_expr(left, params, enclosing, is_escaping, captured, seen);
                walk_expr(right, params, enclosing, is_escaping, captured, seen);
            }
            Expr::Unary { expr, .. }
            | Expr::ErrorPropagate(expr, _)
            | Expr::Wrapping(expr, _)
            | Expr::Saturating(expr, _)
            | Expr::Cast { expr, .. }
            | Expr::Comptime { expr, .. } => {
                walk_expr(expr, params, enclosing, is_escaping, captured, seen);
            }
            Expr::Call { callee, args, .. } => {
                walk_expr(callee, params, enclosing, is_escaping, captured, seen);
                for a in args {
                    walk_expr(a, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::MemberAccess { object, .. } => {
                walk_expr(object, params, enclosing, is_escaping, captured, seen);
            }
            Expr::IndexAccess { object, index, .. } => {
                walk_expr(object, params, enclosing, is_escaping, captured, seen);
                walk_expr(index, params, enclosing, is_escaping, captured, seen);
            }
            Expr::Range { start, end, .. } => {
                walk_expr(start, params, enclosing, is_escaping, captured, seen);
                walk_expr(end, params, enclosing, is_escaping, captured, seen);
            }
            Expr::Tuple(exprs, _) | Expr::ListLiteral(exprs, _) => {
                for e in exprs {
                    walk_expr(e, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::MapLiteral(entries, _) => {
                for (k, v) in entries {
                    walk_expr(k, params, enclosing, is_escaping, captured, seen);
                    walk_expr(v, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::ObjectInit { fields, .. } => {
                for (_, e) in fields {
                    walk_expr(e, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::Pipeline { stages, .. } => {
                for s in stages {
                    walk_expr(s, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::InterpolatedString { expressions, .. } => {
                for e in expressions {
                    walk_expr(e, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::Decide { arms, else_arm, .. } => {
                for a in arms {
                    walk_expr(&a.condition, params, enclosing, is_escaping, captured, seen);
                    walk_expr(&a.body, params, enclosing, is_escaping, captured, seen);
                }
                if let Some(e) = else_arm {
                    walk_expr(e, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::Match { value, arms, .. } => {
                walk_expr(value, params, enclosing, is_escaping, captured, seen);
                for a in arms {
                    if let Some(g) = &a.guard {
                        walk_expr(g, params, enclosing, is_escaping, captured, seen);
                    }
                    walk_expr(&a.body, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::Select { arms, else_arm, .. } => {
                for a in arms {
                    walk_expr(&a.condition, params, enclosing, is_escaping, captured, seen);
                    walk_expr(&a.body, params, enclosing, is_escaping, captured, seen);
                }
                if let Some(e) = else_arm {
                    walk_expr(e, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::Lambda {
                params: inner_params,
                body: inner_body,
                ..
            } => {
                let mut shadowed = Vec::new();
                for p in inner_params {
                    if params.insert(p.name.clone()) {
                        shadowed.push(p.name.clone());
                    }
                }
                walk_expr(inner_body, params, enclosing, is_escaping, captured, seen);
                for s in shadowed {
                    params.remove(&s);
                }
            }
            Expr::OrRecovery { expr, arms, .. } => {
                walk_expr(expr, params, enclosing, is_escaping, captured, seen);
                for a in arms {
                    if let Some(g) = &a.guard {
                        walk_expr(g, params, enclosing, is_escaping, captured, seen);
                    }
                    walk_expr(&a.body, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::ArrayRepeatLiteral { elem, .. } => {
                walk_expr(elem, params, enclosing, is_escaping, captured, seen);
            }
            Expr::Block(stmts, trailing, ..) => {
                for s in stmts {
                    walk_stmt(s, params, enclosing, is_escaping, captured, seen);
                }
                if let Some(t) = trailing {
                    walk_expr(t, params, enclosing, is_escaping, captured, seen);
                }
            }
            Expr::Literal(..) => {}
        }
    }

    fn walk_stmt(
        stmt: &Stmt,
        params: &mut std::collections::HashSet<String>,
        enclosing: &std::collections::HashSet<String>,
        is_escaping: bool,
        captured: &mut Vec<CapturedVar>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        match stmt {
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    walk_stmt(s, params, enclosing, is_escaping, captured, seen);
                }
            }
            Stmt::Let { name, init, .. }
            | Stmt::Mut { name, init, .. }
            | Stmt::Const { name, init, .. }
            | Stmt::Val { name, init, .. }
            | Stmt::CompactBind { name, init, .. } => {
                walk_expr(init, params, enclosing, is_escaping, captured, seen);
                params.insert(name.clone());
            }
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, params, enclosing, is_escaping, captured, seen);
                walk_expr(value, params, enclosing, is_escaping, captured, seen);
            }
            Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => {
                walk_expr(e, params, enclosing, is_escaping, captured, seen);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(condition, params, enclosing, is_escaping, captured, seen);
                walk_stmt(then_branch, params, enclosing, is_escaping, captured, seen);
                if let Some(eb) = else_branch {
                    walk_stmt(eb, params, enclosing, is_escaping, captured, seen);
                }
            }
            Stmt::For {
                iterable,
                body,
                var_name,
                ..
            } => {
                walk_expr(iterable, params, enclosing, is_escaping, captured, seen);
                let was_present = !params.insert(var_name.clone());
                walk_stmt(body, params, enclosing, is_escaping, captured, seen);
                if !was_present {
                    params.remove(var_name);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                walk_expr(condition, params, enclosing, is_escaping, captured, seen);
                walk_stmt(body, params, enclosing, is_escaping, captured, seen);
            }
            Stmt::Loop { body, .. }
            | Stmt::Parallel(body, _)
            | Stmt::Simd(body, _)
            | Stmt::Unsafe { body, .. } => {
                walk_stmt(body, params, enclosing, is_escaping, captured, seen);
            }
            Stmt::With {
                resource_name,
                init,
                body,
                ..
            } => {
                walk_expr(init, params, enclosing, is_escaping, captured, seen);
                let was_present = !params.insert(resource_name.clone());
                walk_stmt(body, params, enclosing, is_escaping, captured, seen);
                if !was_present {
                    params.remove(resource_name);
                }
            }
            Stmt::Return(Some(e), _) => {
                walk_expr(e, params, enclosing, is_escaping, captured, seen);
            }
            _ => {}
        }
    }

    walk_expr(
        body,
        &mut param_names,
        enclosing_vars,
        is_escaping,
        &mut captured,
        &mut seen,
    );
    captured
}

#[derive(Debug, Clone, PartialEq)]
pub enum StaticVal {
    Int(i64),
    Float(f64),
    /// v1.4.5 W3: Dec64 raw mantissa (value × 10⁴).
    Dec64(i64),
    Bool(bool),
    String(String),
    Char(char),
    None,
    Unknown,
}

/// Evaluates an AST expression given a map of statically known variable values.
pub fn eval_static_expr(
    expr: &Expr,
    env: &std::collections::HashMap<String, StaticVal>,
) -> StaticVal {
    match expr {
        Expr::Literal(lit, _) => match lit {
            LiteralValue::Int(n) => StaticVal::Int(*n),
            LiteralValue::Float(f) => StaticVal::Float(*f),
            LiteralValue::Dec64(m) => StaticVal::Dec64(*m),
            LiteralValue::Bool(b) => StaticVal::Bool(*b),
            LiteralValue::String(s) => StaticVal::String(s.clone()),
            LiteralValue::Char(c) => StaticVal::Char(*c),
            LiteralValue::None => StaticVal::None,
        },
        Expr::Identifier(name, _) => env.get(name).cloned().unwrap_or(StaticVal::Unknown),
        Expr::Unary { op, expr, .. } => {
            let val = eval_static_expr(expr, env);
            match (op.as_str(), val) {
                ("!", StaticVal::Bool(b)) => StaticVal::Bool(!b),
                ("-", StaticVal::Int(n)) => StaticVal::Int(-n),
                ("-", StaticVal::Float(f)) => StaticVal::Float(-f),
                ("+", StaticVal::Int(n)) => StaticVal::Int(n),
                ("+", StaticVal::Float(f)) => StaticVal::Float(f),
                _ => StaticVal::Unknown,
            }
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            let left_v = eval_static_expr(left, env);
            let right_v = eval_static_expr(right, env);
            match (op.as_str(), &left_v, &right_v) {
                ("==", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Bool(a == b),
                ("!=", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Bool(a != b),
                ("<", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Bool(a < b),
                ("<=", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Bool(a <= b),
                (">", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Bool(a > b),
                (">=", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Bool(a >= b),

                ("==", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Bool(a == b),
                ("!=", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Bool(a != b),
                ("<", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Bool(a < b),
                ("<=", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Bool(a <= b),
                (">", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Bool(a > b),
                (">=", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Bool(a >= b),

                ("==", StaticVal::Bool(a), StaticVal::Bool(b)) => StaticVal::Bool(a == b),
                ("!=", StaticVal::Bool(a), StaticVal::Bool(b)) => StaticVal::Bool(a != b),

                ("==", StaticVal::String(a), StaticVal::String(b)) => StaticVal::Bool(a == b),
                ("!=", StaticVal::String(a), StaticVal::String(b)) => StaticVal::Bool(a != b),

                ("==", StaticVal::Char(a), StaticVal::Char(b)) => StaticVal::Bool(a == b),
                ("!=", StaticVal::Char(a), StaticVal::Char(b)) => StaticVal::Bool(a != b),

                ("&&", StaticVal::Bool(false), _) => StaticVal::Bool(false),
                ("&&", _, StaticVal::Bool(false)) => StaticVal::Bool(false),
                ("&&", StaticVal::Bool(true), StaticVal::Bool(true)) => StaticVal::Bool(true),

                ("||", StaticVal::Bool(true), _) => StaticVal::Bool(true),
                ("||", _, StaticVal::Bool(true)) => StaticVal::Bool(true),
                ("||", StaticVal::Bool(false), StaticVal::Bool(false)) => StaticVal::Bool(false),

                ("+", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Int(a.wrapping_add(*b)),
                ("-", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Int(a.wrapping_sub(*b)),
                ("*", StaticVal::Int(a), StaticVal::Int(b)) => StaticVal::Int(a.wrapping_mul(*b)),
                ("/", StaticVal::Int(a), StaticVal::Int(b)) if *b != 0 => StaticVal::Int(a / b),
                ("%", StaticVal::Int(a), StaticVal::Int(b)) if *b != 0 => StaticVal::Int(a % b),

                ("+", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Float(a + b),
                ("-", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Float(a - b),
                ("*", StaticVal::Float(a), StaticVal::Float(b)) => StaticVal::Float(a * b),
                ("/", StaticVal::Float(a), StaticVal::Float(b)) if *b != 0.0 => {
                    StaticVal::Float(a / b)
                }

                _ => StaticVal::Unknown,
            }
        }
        Expr::Comptime { expr, .. } => eval_static_expr(expr, env),
        _ => StaticVal::Unknown,
    }
}

/// Checks if a contract condition is statically proven true, either via literal/constant
/// evaluation or through parameter refinement invariants.
pub fn is_contract_statically_true(
    cond: &Expr,
    param_refinements: &[(String, Option<Refinement>)],
) -> bool {
    let empty_env = std::collections::HashMap::new();
    if eval_static_expr(cond, &empty_env) == StaticVal::Bool(true) {
        return true;
    }

    match cond {
        Expr::Binary {
            op, left, right, ..
        } => {
            if op == "&&" {
                return is_contract_statically_true(left, param_refinements)
                    && is_contract_statically_true(right, param_refinements);
            }
            if op == "||" {
                return is_contract_statically_true(left, param_refinements)
                    || is_contract_statically_true(right, param_refinements);
            }

            if let Expr::Identifier(var_name, _) = &**left {
                for (p_name, refn_opt) in param_refinements {
                    if p_name == var_name {
                        if let Some(refn) = refn_opt {
                            match refn {
                                Refinement::Range {
                                    start,
                                    end,
                                    inclusive,
                                } => {
                                    let s_val = eval_static_expr(start, &empty_env);
                                    let e_val = eval_static_expr(end, &empty_env);
                                    let r_val = eval_static_expr(right, &empty_env);
                                    if let (StaticVal::Int(s), StaticVal::Int(r)) = (s_val, &r_val)
                                    {
                                        if op == ">=" && s >= *r {
                                            return true;
                                        }
                                        if op == ">" && s > *r {
                                            return true;
                                        }
                                        if op == "!="
                                            && (s > *r
                                                || (matches!(e_val, StaticVal::Int(e) if e < *r)))
                                        {
                                            return true;
                                        }
                                    }
                                    if let (StaticVal::Int(e), StaticVal::Int(r)) = (e_val, &r_val)
                                    {
                                        if op == "<=" && e <= *r {
                                            return true;
                                        }
                                        if op == "<"
                                            && (*inclusive && e < *r || !*inclusive && e <= *r)
                                        {
                                            return true;
                                        }
                                    }
                                }
                                Refinement::Predicate {
                                    var_name: p_var,
                                    predicate,
                                } => {
                                    if format!("{:?}", predicate).replace(p_var, var_name)
                                        == format!("{:?}", cond)
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// True when the expression tree contains any `asm` block.
pub fn expr_contains_asm(expr: &Expr) -> bool {
    expr_has_asm_kind(expr, false)
}
