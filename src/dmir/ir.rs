use crate::ast::{ContractClause, Refinement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct ValueId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct BasicBlockId(pub usize);

impl std::fmt::Display for ValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{}", self.0)
    }
}

impl std::fmt::Display for BasicBlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Terminator {
    Branch {
        target: BasicBlockId,
        args: Vec<ValueId>,
    },
    CondBranch {
        cond: ValueId,
        then_block: BasicBlockId,
        then_args: Vec<ValueId>,
        else_block: BasicBlockId,
        else_args: Vec<ValueId>,
    },
    Return {
        value: Option<ValueId>,
    },
    Unreachable,
}

impl Default for Terminator {
    fn default() -> Self {
        Terminator::Return { value: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockParam {
    pub val: ValueId,
    pub ty: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Inst {
    ConstInt {
        dest: ValueId,
        value: i64,
    },
    ConstFloat {
        dest: ValueId,
        value: f64,
    },
    ConstStr {
        dest: ValueId,
        value: String,
    },
    ConstBool {
        dest: ValueId,
        value: bool,
    },
    LoadVar {
        dest: ValueId,
        name: String,
    },
    AssignVar {
        name: String,
        value: ValueId,
    },
    BinOp {
        dest: ValueId,
        op: String,
        left: ValueId,
        right: ValueId,
        ty: String,
    },
    UnOp {
        dest: ValueId,
        op: String,
        operand: ValueId,
        ty: String,
    },
    Call {
        dest: ValueId,
        func: String,
        args: Vec<ValueId>,
        ty: String,
    },
    MethodCall {
        dest: ValueId,
        object: ValueId,
        method: String,
        args: Vec<ValueId>,
        ty: String,
    },
    StructInit {
        dest: ValueId,
        class_name: String,
        fields: Vec<(String, ValueId)>,
    },
    GetField {
        dest: ValueId,
        object: ValueId,
        field: String,
        ty: String,
    },
    SetField {
        object: ValueId,
        field: String,
        value: ValueId,
    },
    Out {
        value: ValueId,
    },
    Err {
        value: ValueId,
    },
    FormatStr {
        dest: ValueId,
        parts: Vec<String>,
        values: Vec<ValueId>,
    },
    Decide {
        dest: ValueId,
        arms: Vec<(ValueId, ValueId)>,
        else_val: Option<ValueId>,
        ty: String,
    },
    WhileLoop {
        condition_insts: Vec<Inst>,
        cond_val: ValueId,
        body_insts: Vec<Inst>,
    },
    TryCatch {
        try_insts: Vec<Inst>,
        err_var: String,
        catch_insts: Vec<Inst>,
    },
    Return {
        value: Option<ValueId>,
    },
    GetFuncAddr {
        dest: ValueId,
        func_name: String,
    },
    Select {
        dest: ValueId,
        cond: ValueId,
        then_val: ValueId,
        else_val: ValueId,
        ty: String,
    },
    InlineAsm {
        template: String,
        outputs: Vec<(String, ValueId)>,
        inputs: Vec<(String, ValueId)>,
        clobbers: Vec<String>,
        options: Vec<String>,
    },
}

impl Inst {
    /// Traverses all ValueIds referenced by this instruction (both definitions and uses).
    pub fn visit_vids<F: FnMut(&ValueId) + ?Sized>(&self, f: &mut F) {
        match self {
            Inst::ConstInt { dest, .. }
            | Inst::ConstFloat { dest, .. }
            | Inst::ConstStr { dest, .. }
            | Inst::ConstBool { dest, .. }
            | Inst::GetFuncAddr { dest, .. } => f(dest),
            Inst::LoadVar { dest, .. } => f(dest),
            Inst::AssignVar { value, .. } => f(value),
            Inst::BinOp {
                dest, left, right, ..
            } => {
                f(dest);
                f(left);
                f(right);
            }
            Inst::UnOp { dest, operand, .. } => {
                f(dest);
                f(operand);
            }
            Inst::Call { dest, args, .. } => {
                f(dest);
                for a in args {
                    f(a);
                }
            }
            Inst::MethodCall {
                dest, object, args, ..
            } => {
                f(dest);
                f(object);
                for a in args {
                    f(a);
                }
            }
            Inst::StructInit { dest, fields, .. } => {
                f(dest);
                for (_, v) in fields {
                    f(v);
                }
            }
            Inst::GetField { dest, object, .. } => {
                f(dest);
                f(object);
            }
            Inst::SetField { object, value, .. } => {
                f(object);
                f(value);
            }
            Inst::Out { value } | Inst::Err { value } => f(value),
            Inst::FormatStr { dest, values, .. } => {
                f(dest);
                for v in values {
                    f(v);
                }
            }
            Inst::Select {
                dest,
                cond,
                then_val,
                else_val,
                ..
            } => {
                f(dest);
                f(cond);
                f(then_val);
                f(else_val);
            }
            Inst::Decide {
                dest,
                arms,
                else_val,
                ..
            } => {
                f(dest);
                for (c, v) in arms {
                    f(c);
                    f(v);
                }
                if let Some(e) = else_val {
                    f(e);
                }
            }
            Inst::WhileLoop {
                condition_insts,
                cond_val,
                body_insts,
            } => {
                f(cond_val);
                for i in condition_insts {
                    i.visit_vids(f);
                }
                for i in body_insts {
                    i.visit_vids(f);
                }
            }
            Inst::TryCatch {
                try_insts,
                catch_insts,
                ..
            } => {
                for i in try_insts {
                    i.visit_vids(f);
                }
                for i in catch_insts {
                    i.visit_vids(f);
                }
            }
            Inst::Return { value } => {
                if let Some(v) = value {
                    f(v);
                }
            }
            Inst::InlineAsm {
                outputs, inputs, ..
            } => {
                for (_, d) in outputs {
                    f(d);
                }
                for (_, i) in inputs {
                    f(i);
                }
            }
        }
    }
}

impl std::hash::Hash for Inst {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Inst::ConstInt { dest, value } => {
                dest.hash(state);
                value.hash(state);
            }
            Inst::ConstFloat { dest, value } => {
                dest.hash(state);
                value.to_bits().hash(state);
            }
            Inst::ConstStr { dest, value } => {
                dest.hash(state);
                value.hash(state);
            }
            Inst::ConstBool { dest, value } => {
                dest.hash(state);
                value.hash(state);
            }
            Inst::LoadVar { dest, name } => {
                dest.hash(state);
                name.hash(state);
            }
            Inst::AssignVar { name, value } => {
                name.hash(state);
                value.hash(state);
            }
            Inst::BinOp {
                dest,
                op,
                left,
                right,
                ty,
            } => {
                dest.hash(state);
                op.hash(state);
                left.hash(state);
                right.hash(state);
                ty.hash(state);
            }
            Inst::UnOp {
                dest,
                op,
                operand,
                ty,
            } => {
                dest.hash(state);
                op.hash(state);
                operand.hash(state);
                ty.hash(state);
            }
            Inst::Call {
                dest,
                func,
                args,
                ty,
            } => {
                dest.hash(state);
                func.hash(state);
                args.hash(state);
                ty.hash(state);
            }
            Inst::MethodCall {
                dest,
                object,
                method,
                args,
                ty,
            } => {
                dest.hash(state);
                object.hash(state);
                method.hash(state);
                args.hash(state);
                ty.hash(state);
            }
            Inst::StructInit {
                dest,
                class_name,
                fields,
            } => {
                dest.hash(state);
                class_name.hash(state);
                fields.hash(state);
            }
            Inst::GetField {
                dest,
                object,
                field,
                ty,
            } => {
                dest.hash(state);
                object.hash(state);
                field.hash(state);
                ty.hash(state);
            }
            Inst::SetField {
                object,
                field,
                value,
            } => {
                object.hash(state);
                field.hash(state);
                value.hash(state);
            }
            Inst::Out { value } => {
                value.hash(state);
            }
            Inst::Err { value } => {
                value.hash(state);
            }
            Inst::FormatStr {
                dest,
                parts,
                values,
            } => {
                dest.hash(state);
                parts.hash(state);
                values.hash(state);
            }
            Inst::Decide {
                dest,
                arms,
                else_val,
                ty,
            } => {
                dest.hash(state);
                arms.hash(state);
                else_val.hash(state);
                ty.hash(state);
            }
            Inst::WhileLoop {
                condition_insts,
                cond_val,
                body_insts,
            } => {
                condition_insts.hash(state);
                cond_val.hash(state);
                body_insts.hash(state);
            }
            Inst::TryCatch {
                try_insts,
                err_var,
                catch_insts,
            } => {
                try_insts.hash(state);
                err_var.hash(state);
                catch_insts.hash(state);
            }
            Inst::Return { value } => {
                value.hash(state);
            }
            Inst::GetFuncAddr { dest, func_name } => {
                dest.hash(state);
                func_name.hash(state);
            }
            Inst::Select {
                dest,
                cond,
                then_val,
                else_val,
                ty,
            } => {
                dest.hash(state);
                cond.hash(state);
                then_val.hash(state);
                else_val.hash(state);
                ty.hash(state);
            }
            Inst::InlineAsm {
                template,
                outputs,
                inputs,
                clobbers,
                options,
            } => {
                template.hash(state);
                outputs.hash(state);
                inputs.hash(state);
                clobbers.hash(state);
                options.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub label: String,
    pub params: Vec<BlockParam>,
    pub instructions: Vec<Inst>,
    pub terminator: Terminator,
}

/// v1.3.4: the `@inline`, `@inline(always)` and `@inline(never)`
/// attributes on Datara functions, carried through DMIR so the optimizer
/// (forced inlining) and future backends can honor them. `None` is the
/// default for every function without an inline attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InlineHint {
    #[default]
    None,
    Inline,
    Always,
    Never,
}

impl InlineHint {
    /// Extracts the inline hint from a declaration's attribute list.
    ///
    /// Returns the hint plus an error description when the attribute is
    /// malformed (`@inline(sometimes)`, duplicated `@inline`, or an
    /// unknown argument). The caller turns the error into a diagnostic with
    /// the right code; this function only classifies.
    pub fn parse_from_attrs(attrs: &[crate::ast::Attribute]) -> (InlineHint, Option<String>) {
        let mut hint = InlineHint::None;
        let mut seen = false;
        for a in attrs {
            if a.name != "inline" {
                continue;
            }
            if seen {
                return (
                    InlineHint::None,
                    Some("duplicate '@inline' attribute on the same function".to_string()),
                );
            }
            seen = true;
            if a.args.is_empty() {
                hint = InlineHint::Inline;
                continue;
            }
            if a.args.len() > 1 {
                return (
                    InlineHint::None,
                    Some(format!(
                        "'@inline' accepts at most one argument, got {}",
                        a.args.len()
                    )),
                );
            }
            let (arg, _val) = &a.args[0];
            match arg.as_str() {
                "always" => hint = InlineHint::Always,
                "never" => hint = InlineHint::Never,
                other => {
                    return (
                        InlineHint::None,
                        Some(format!(
                            "unknown '@inline' argument '{}': expected 'always' or 'never'",
                            other
                        )),
                    );
                }
            }
        }
        (hint, None)
    }
}

/// v1.4.0: the allocator-tier attributes `@arena` and `@pool(n)` on Datara
/// functions, carried through DMIR so the backend can route the function's
/// heap allocations to the runtime's region allocators.
///
/// * `Arena` — escaping class allocations and list-literal headers are
///   bump-allocated through the runtime's thread-local arena
///   (`datara_rt_arena_alloc`); a checkpoint taken at function entry is
///   restored at every return, so the whole region is reclaimed wholesale.
///   The checkpoint lives in a per-call local, which makes recursion safe
///   (each invocation restores its own frame's checkpoint).
/// * `Pool(n)` — a fixed slab of `n` object slots is bump-allocated once per
///   call; class allocations are served from the slab under a runtime slot
///   counter with an explicit capacity trap. Provable compile-time overflow
///   is rejected with E1406 before codegen ever runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArenaHint {
    #[default]
    None,
    Arena,
    Pool(u64),
}

impl ArenaHint {
    /// Extracts the allocator-tier hint from a declaration's attribute list.
    ///
    /// Returns the hint plus an error description when the attribute is
    /// malformed (unknown argument, duplicate attribute, non-numeric or zero
    /// `@pool` capacity). The caller turns the error into a diagnostic with
    /// the right code; this function only classifies.
    pub fn parse_from_attrs(attrs: &[crate::ast::Attribute]) -> (ArenaHint, Option<String>) {
        let mut hint = ArenaHint::None;
        let mut seen = false;
        for a in attrs {
            let tier = match a.name.as_str() {
                "arena" => ArenaHint::Arena,
                "pool" => ArenaHint::Pool(0),
                _ => continue,
            };
            if seen {
                return (
                    ArenaHint::None,
                    Some(
                        "duplicate '@arena'/'@pool' attribute on the same function; the tiers are mutually exclusive"
                            .to_string(),
                    ),
                );
            }
            seen = true;
            hint = tier;
            if a.name == "pool" {
                // The parser stores `@pool(n)` as ("n", "") and
                // `@pool(size: n)` as ("size", "n"); accept both spellings.
                let capacity = a
                    .args
                    .first()
                    .map(|(k, v)| {
                        if v.trim().is_empty() {
                            k.trim().to_string()
                        } else {
                            v.trim().to_string()
                        }
                    })
                    .unwrap_or_default();
                match capacity.parse::<u64>() {
                    Ok(n) if n > 0 => hint = ArenaHint::Pool(n),
                    _ => {
                        let shown = if capacity.is_empty() {
                            "<none>".to_string()
                        } else {
                            capacity
                        };
                        return (
                            ArenaHint::None,
                            Some(format!(
                                "'@pool' expects a positive integer capacity, got '{}'",
                                shown
                            )),
                        );
                    }
                }
            } else if !a.args.is_empty() {
                return (
                    ArenaHint::None,
                    Some("'@arena' does not take arguments".to_string()),
                );
            }
        }
        (hint, None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, String, ValueId)>,
    #[serde(default)]
    pub param_refinements: Vec<(String, Option<Refinement>)>,
    #[serde(default)]
    pub requires: Vec<ContractClause>,
    pub return_type: String,
    pub entry_block: BasicBlockId,
    pub blocks: Vec<BasicBlock>,
    /// v1.3.4: inline attribute carried from the parsed `@inline` family.
    #[serde(default)]
    pub inline_hint: InlineHint,
    /// v1.4.0: allocator tier carried from the parsed `@arena` / `@pool(n)`
    /// attributes. The driver-level validation pass rejects malformed
    /// attributes before lowering stores the hint.
    #[serde(default)]
    pub alloc_hint: ArenaHint,
    /// v1.4.0: true when the function body contains a structured `asm { ...}`
    /// block. Structured asm lowers to plain DMIR ops the Cranelift backend
    /// can compile; LLVM and WASM reject structured-asm functions with E1405.
    /// Legacy `asm!` template blocks keep their existing backend contract and
    /// do NOT set this marker.
    #[serde(default)]
    pub has_inline_asm: bool,
    /// v1.3.4: `BinOp` results (`dest` ValueIds) whose arithmetic the
    /// optimizer statically proved cannot overflow, so the backend may
    /// emit unchecked (`iadd`/`isub`/`imul`) code for exactly these
    /// instructions. Only populated under `--opt speed`; user body
    /// arithmetic is never added to this set.
    #[serde(default)]
    pub proven_no_overflow: std::collections::BTreeSet<ValueId>,
}

impl Default for Function {
    fn default() -> Self {
        Self {
            name: String::new(),
            params: Vec::new(),
            param_refinements: Vec::new(),
            requires: Vec::new(),
            return_type: String::new(),
            entry_block: BasicBlockId(0),
            blocks: Vec::new(),
            inline_hint: InlineHint::None,
            alloc_hint: ArenaHint::None,
            has_inline_asm: false,
            proven_no_overflow: std::collections::BTreeSet::new(),
        }
    }
}

impl Function {
    pub fn get_block(&self, id: BasicBlockId) -> Option<&BasicBlock> {
        if id.0 < self.blocks.len() && self.blocks[id.0].id == id {
            return Some(&self.blocks[id.0]);
        }
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn get_block_mut(&mut self, id: BasicBlockId) -> Option<&mut BasicBlock> {
        if id.0 < self.blocks.len() && self.blocks[id.0].id == id {
            return Some(&mut self.blocks[id.0]);
        }
        self.blocks.iter_mut().find(|b| b.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub functions: HashMap<String, Function>,
    pub extern_functions: HashMap<String, (Vec<String>, String)>,
    /// Imported C functions that return a by-value struct through the
    /// hidden sret return-slot ABI, mapped to the struct size in bytes.
    /// Only the native Cranelift backends consume this; every other backend
    /// keeps rejecting such declarations at the cimport gate.
    #[serde(default)]
    pub extern_sret: HashMap<String, usize>,
    /// Imported C functions that return a 16-byte layout-compatible struct
    /// through the SysV AMD64 register pair (v1.3.3), mapped to the
    /// register classes of the two eightbytes in field order
    /// (`classes[0]` = offset 0 rides RAX/XMM0, `classes[1]` = offset 8
    /// rides RDX/XMM1). Only the native Cranelift backend under the SystemV
    /// call convention consumes this; every other backend keeps rejecting
    /// such declarations at the cimport gate.
    #[serde(default)]
    pub extern_sysv: HashMap<String, [crate::ast::SysVClass; 2]>,
    pub class_fields: HashMap<String, Vec<String>>,
    pub class_field_types: HashMap<String, String>,
    #[serde(default)]
    pub function_spans: HashMap<String, crate::diagnostics::SourceSpan>,
    #[serde(default)]
    pub function_line_spans: HashMap<String, Vec<crate::diagnostics::SourceSpan>>,
    #[serde(default)]
    pub link_libraries: Vec<String>,
    #[serde(default)]
    pub globals: HashMap<String, (String, bool)>,
}

impl Module {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            functions: HashMap::new(),
            extern_functions: HashMap::new(),
            extern_sret: HashMap::new(),
            extern_sysv: HashMap::new(),
            class_fields: HashMap::new(),
            class_field_types: HashMap::new(),
            function_spans: HashMap::new(),
            function_line_spans: HashMap::new(),
            link_libraries: Vec::new(),
            globals: HashMap::new(),
        }
    }
}
