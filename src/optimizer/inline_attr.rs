//! v1.3.4 `@inline` attribute support: frontend validation and the
//! `@inline(always)` forced inliner.
//!
//! # Attribute surface
//!
//! * `@inline` — a hint. Stored on the DMIR function (`Function::inline_hint`)
//!   and left to the existing cost-model-driven inliner.
//! * `@inline(always)` — a strong request. Under `--opt speed` the callee
//!   body is spliced into every eligible same-module call site (see the
//!   eligibility rules below). Under `--opt default` the hint is validated
//!   and stored but the forced inliner does not run.
//! * `@inline(never)` — a hard veto. The cost-model inliner, the IPO
//!   inliner, specialization cloning and the forced inliner all honor it:
//!   the function keeps outline dispatch everywhere.
//!
//! # Validation (hard errors, tier-independent)
//!
//! * `@inline(bogus)` or a duplicated `@inline` on one function fails
//!   with `E0957` (`InlineAttributeInvalid`).
//! * `@inline(always)` on an `extern "C"` declaration fails with `E0956`
//!   (`InlineAlwaysExtern`): the compiler cannot inline a body it does not
//!   have, so silently ignoring the attribute would be a lie.
//!
//! # Forced inlining eligibility (`inline_always_functions`)
//!
//! A candidate must be a same-module DMIR function with hint `Always` that
//! is *not* `main`, has exactly **one basic block** (the splicer is a
//! straight-line body copier; multi-block bodies are rejected with a trace
//! record, never silently dropped), has **at most 64 DMIR instructions**
//! (size cap), and is not self-recursive. At the call site, a callee whose
//! body calls the current caller is skipped (call-graph cycle guard), so
//! mutually recursive `@inline(always)` pairs terminate. Every
//! decision — applied or rejected — is recorded in the decision trace.

use super::Optimizer;
use crate::ast::{ClassItem, Decl, Program};
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use crate::dmir::{Function, InlineHint, Inst, Module};
use std::collections::HashMap;

/// Maximum callee body size (in DMIR instructions) the forced inliner will
/// splice. Small enough to bound compile time and code growth, large enough
/// for the accessors and tiny helpers `@inline(always)` targets.
pub(crate) const INLINE_ALWAYS_MAX_INSTS: usize = 64;

/// Validates every `@inline` attribute on the program's declarations.
///
/// Runs in the driver pipeline right after parse/derive expansion, before
/// lowering stores the hints, so a malformed attribute can never reach the
/// optimizer. Errors go through the standard diagnostics engine and fail
/// compilation.
pub fn validate_inline_attributes(program: &Program, diag: &mut DiagnosticEngine) {
    for decl in &program.declarations {
        match decl {
            Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                validate_fn_inline_attrs(&f.name, &f.attributes, false, &f.span, diag);
            }
            Decl::ExternFn(ef) => {
                validate_fn_inline_attrs(&ef.name, &ef.attributes, true, &ef.span, diag);
            }
            Decl::Class(c) => {
                for item in &c.body_items {
                    if let ClassItem::Method(m) = item {
                        let full = format!("{}_{}", c.name, m.name);
                        validate_fn_inline_attrs(&full, &m.attributes, false, &m.span, diag);
                    }
                }
            }
            Decl::Behavior(b) => {
                for item in &b.body_items {
                    if let ClassItem::Method(m) = item {
                        let full = format!("{}_{}", b.target_type, m.name);
                        validate_fn_inline_attrs(&full, &m.attributes, false, &m.span, diag);
                    }
                }
            }
            Decl::Impl(i) => {
                for m in &i.methods {
                    let full = format!("{}_{}", i.target_type, m.name);
                    validate_fn_inline_attrs(&full, &m.attributes, false, &m.span, diag);
                }
            }
            _ => {}
        }
    }
}

/// Validates the `inline` attributes of one declaration.
///
/// `is_extern` adds the E0956 rejection: `@inline(always)` on an extern
/// declaration promises something the compiler cannot deliver.
fn validate_fn_inline_attrs(
    name: &str,
    attrs: &[crate::ast::Attribute],
    is_extern: bool,
    span: &crate::diagnostics::SourceSpan,
    diag: &mut DiagnosticEngine,
) {
    let inline_attr = attrs.iter().find(|a| a.name == "inline");
    let (hint, err) = InlineHint::parse_from_attrs(attrs);
    if let Some(msg) = err {
        let attr_span = inline_attr
            .map(|a| a.span.clone())
            .unwrap_or_else(|| span.clone());
        diag.error(
            ErrorCode::InlineAttributeInvalid,
            format!("function '{}': {}", name, msg),
            Some(attr_span),
        );
        return;
    }
    if is_extern && hint == InlineHint::Always {
        let attr_span = inline_attr
            .map(|a| a.span.clone())
            .unwrap_or_else(|| span.clone());
        diag.error(
            ErrorCode::InlineAlwaysExtern,
            format!(
                "function '{}': '@inline(always)' cannot be honored on an extern declaration: there is no Datara body to inline",
                name
            ),
            Some(attr_span),
        );
    }
}

impl Optimizer {
    /// Splices every eligible `@inline(always)` callee into its call
    /// sites. Called from `optimize_module` only under `--opt speed`,
    /// before the cost-model inliner, so hint-marked callees are honored
    /// even where the heuristic would decline them.
    pub(crate) fn inline_always_functions(&mut self, module: &mut Module) {
        let mut candidates: HashMap<String, Function> = HashMap::new();
        let mut fn_names: Vec<String> = module.functions.keys().cloned().collect();
        fn_names.sort();

        for name in &fn_names {
            let f = &module.functions[name];
            if name == "main" || f.inline_hint != InlineHint::Always {
                continue;
            }
            // v1.4.0: @arena/@pool and asm frames stay outline.
            if f.alloc_hint != crate::dmir::ArenaHint::None || f.has_inline_asm {
                self.trace.record(
                    "InlineAlways",
                    name,
                    "Rejected",
                    "None",
                    "None",
                    "allocator-tier or asm-bearing function keeps outline dispatch",
                );
                continue;
            }
            if f.blocks.len() != 1 {
                self.trace.record(
                    "InlineAlways",
                    name,
                    "Rejected",
                    "None",
                    "None",
                    "multi-block callee bodies are outside the v1.3.4 forced inliner; call kept",
                );
                continue;
            }
            // The straight-line splicer copies only effect-free instruction
            // kinds and *drops* everything else. A body containing I/O, calls
            // with effects, nested control flow or stores must therefore be
            // refused here — never silently truncated.
            if !f.blocks[0].instructions.iter().all(|i| {
                matches!(
                    i,
                    Inst::ConstInt { .. }
                        | Inst::ConstFloat { .. }
                        | Inst::ConstStr { .. }
                        | Inst::ConstBool { .. }
                        | Inst::LoadVar { .. }
                        | Inst::AssignVar { .. }
                        | Inst::BinOp { .. }
                        | Inst::UnOp { .. }
                        | Inst::GetField { .. }
                        | Inst::Select { .. }
                        | Inst::Decide { .. }
                        | Inst::Return { .. }
                )
            }) {
                self.trace.record(
                    "InlineAlways",
                    name,
                    "Rejected",
                    "None",
                    "None",
                    "callee body contains effects or control flow the straight-line splicer cannot copy; call kept",
                );
                continue;
            }
            if f.blocks[0].instructions.len() > INLINE_ALWAYS_MAX_INSTS {
                self.trace.record(
                    "InlineAlways",
                    name,
                    "Rejected",
                    "None",
                    "None",
                    &format!(
                        "callee body has {} DMIR instructions, above the {}-instruction cap; call kept",
                        f.blocks[0].instructions.len(),
                        INLINE_ALWAYS_MAX_INSTS
                    ),
                );
                continue;
            }
            if f.blocks[0]
                .instructions
                .iter()
                .any(|i| matches!(i, Inst::Call { func, .. } if func == name))
            {
                self.trace.record(
                    "InlineAlways",
                    name,
                    "Rejected",
                    "None",
                    "None",
                    "self-recursive callee; inlining would not terminate",
                );
                continue;
            }
            candidates.insert(name.clone(), f.clone());
        }

        if candidates.is_empty() {
            return;
        }

        let (sites, inlined_set) = self.inline_candidates_into_callers(module, &candidates, true);

        let mut names: Vec<String> = candidates.keys().cloned().collect();
        names.sort();
        for name in &names {
            if inlined_set.contains(name) {
                self.trace.record(
                    "InlineAlways",
                    name,
                    "Applied",
                    &format!("body spliced into call sites ({} site(s) replaced this pass)", sites),
                    "None (hint-driven, callee body copied)",
                    "'@inline(always)' honored: straight-line callee body inlined with fresh ValueIds",
                );
            } else {
                self.trace.record(
                    "InlineAlways",
                    name,
                    "Rejected",
                    "None",
                    "None",
                    "'@inline(always)' callee had no eligible call site in this module",
                );
            }
        }
    }
}
