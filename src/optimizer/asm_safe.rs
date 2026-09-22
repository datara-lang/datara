//! v1.4.0 structured `asm { ... }` validation.
//!
//! # Surface
//!
//! `asm { ... }` accepts the safe subset `mov|add|sub DST, SRC` where an
//! operand is one of the four x86-64 GP scratch registers (`rax/eax`,
//! `rbx/ebx`, `rcx/ecx`, `rdx/edx`), a Datara `Int` variable, or an integer
//! immediate. The block lowers to plain DMIR ops through the normal backend
//! (registers are simulated as DMIR temporaries) — it is a *structured
//! subset*, not a raw machine-code encoder.
//!
//! # Validation (hard errors)
//!
//! * `E1402` (`AsmUnsupportedInst`) — a line outside the subset (unknown
//!   mnemonic, memory operand, float register, unparsable text), or a
//!   register read before it was written in this block.
//! * `E1403` (`AsmOperandNotInt`) — a variable operand whose declared type is
//!   not `Int` (checked by the type checker, see `types::check::stmt`).
//! * `E1404` (`AsmRequiresUnsafe`) — the block is not enclosed in
//!   `unsafe(justification: "...")` (checked by the security verifier).
//! * `E1405` (`AsmUnsupportedBackend`) — the selected backend is LLVM or
//!   WASM, which reject asm-bearing functions (checked by the driver).
//!
//! Legacy `asm! { "template" }` blocks keep their LLVM-only raw-template
//! contract and are not touched by this pass.

use crate::ast::{AsmLine, AsmOp, AsmOperand, ClassItem, Decl, Program, Stmt};
use crate::diagnostics::{DiagnosticEngine, ErrorCode};

/// The supported-form text attached to every E1402 diagnostic.
const SUPPORTED_FORMS: &str = "supported forms: 'mov rax|eax|rbx|ebx|rcx|ecx|rdx|edx, <reg|var|imm>', 'add <reg|var>, <reg|var|imm>', 'sub <reg|var>, <reg|var|imm>'";

/// Validates every structured asm block in the program.
pub fn validate_asm_blocks(program: &Program, diag: &mut DiagnosticEngine) {
    for decl in &program.declarations {
        match decl {
            Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                validate_stmt(&f.name, &f.body, diag);
            }
            Decl::Class(c) => {
                for item in &c.body_items {
                    if let ClassItem::Method(m) = item
                        && let Some(body) = m.body.as_ref()
                    {
                        validate_stmt(&format!("{}_{}", c.name, m.name), body, diag);
                    }
                }
            }
            Decl::Behavior(b) => {
                for item in &b.body_items {
                    if let ClassItem::Method(m) = item
                        && let Some(body) = m.body.as_ref()
                    {
                        validate_stmt(&format!("{}_{}", b.target_type, m.name), body, diag);
                    }
                }
            }
            Decl::Impl(i) => {
                for m in &i.methods {
                    validate_stmt(&format!("{}_{}", i.target_type, m.name), &m.body, diag);
                }
            }
            _ => {}
        }
    }
}

fn validate_stmt(fn_name: &str, stmt: &Stmt, diag: &mut DiagnosticEngine) {
    match stmt {
        Stmt::Block(stmts, _) => {
            for s in stmts {
                validate_stmt(fn_name, s, diag);
            }
        }
        Stmt::Asm {
            structured, span, ..
        } => {
            if structured.is_empty() {
                return;
            }
            validate_lines(fn_name, structured, span, diag);
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            validate_stmt(fn_name, then_branch, diag);
            if let Some(e) = else_branch {
                validate_stmt(fn_name, e, diag);
            }
        }
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Loop { body, .. }
        | Stmt::ParallelFor { body, .. }
        | Stmt::Parallel(body, _)
        | Stmt::Unsafe { body, .. }
        | Stmt::With { body, .. } => validate_stmt(fn_name, body, diag),
        _ => {}
    }
}

/// Simulates the four scratch registers across one block and reports every
/// unsupported line or read-before-write with E1402.
fn validate_lines(
    fn_name: &str,
    lines: &[AsmLine],
    block_span: &crate::diagnostics::SourceSpan,
    diag: &mut DiagnosticEngine,
) {
    let mut regs_written = [false; 4];
    for line in lines {
        match line {
            AsmLine::Unsupported { raw, span } => {
                let at = if span.start_line == 0 {
                    block_span.clone()
                } else {
                    span.clone()
                };
                diag.error(
                    ErrorCode::AsmUnsupportedInst,
                    format!(
                        "function '{}': unsupported asm instruction '{}': {}",
                        fn_name, raw, SUPPORTED_FORMS
                    ),
                    Some(at),
                );
            }
            AsmLine::Inst {
                op, dst, src, span, ..
            } => {
                // add/sub read their destination first; mov only writes it.
                if *op != AsmOp::Mov
                    && let AsmOperand::Reg(r) = dst
                    && !regs_written[r.slot()]
                {
                    diag.error(
                        ErrorCode::AsmUnsupportedInst,
                        format!(
                            "function '{}': register '{}' is read before it is written in this asm block; {}",
                            fn_name,
                            r.name(),
                            SUPPORTED_FORMS
                        ),
                        Some(span.clone()),
                    );
                    continue;
                }
                if let AsmOperand::Reg(r) = src
                    && !regs_written[r.slot()]
                {
                    diag.error(
                        ErrorCode::AsmUnsupportedInst,
                        format!(
                            "function '{}': register '{}' is read before it is written in this asm block; {}",
                            fn_name,
                            r.name(),
                            SUPPORTED_FORMS
                        ),
                        Some(span.clone()),
                    );
                    continue;
                }
                if let AsmOperand::Reg(r) = dst {
                    regs_written[r.slot()] = true;
                }
            }
        }
    }
}
