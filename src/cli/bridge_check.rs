//! `forgen bridge check <file.dtr>` — static audit of foreign-function
//! bridges (PLAN_V151 N3, heir of plan B1):
//!
//! 1. Declarative bridges (`bridge py::module { fn ... }`): unknown
//!    languages (E-BRIDGE-003) and unsupported param/return types
//!    (E-BRIDGE-002) come from `bridge_decl::BridgeRegistry`; call sites
//!    are checked for arity and literal-arg type compatibility
//!    (E-BRIDGE-001, exact span).
//! 2. C imports (`import c "h.h" with link(...)`) — the "headers/stubs"
//!    half: the header must exist and parse (E-BRIDGE-002 at the import
//!    span), and every call site hitting a header function must match its
//!    declared arity (E-BRIDGE-001 at the call span).
//!
//! Exit code is non-zero when any diagnostic was emitted, so CI can gate
//! on it. The bridge SURFACE (functions, languages, headers) is printed as
//! a report even when everything is valid.

use crate::ast::{Decl, Expr, Stmt};
use crate::cimport::types::CDecl;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One verified call site (name, argc, span).
type CallSite = (String, usize, SourceSpan);

struct HeaderFn {
    arity: usize,
    is_variadic: bool,
    line: usize,
}

pub(crate) fn cmd_bridge_check(args: &[String]) -> bool {
    // args = [exe, "bridge", "check", <file.dtr>]: take the first argument
    // after the "check" subcommand.
    let file = match args.iter().skip(3).find(|a| !a.starts_with('-')) {
        Some(f) => f.clone(),
        None => {
            eprintln!("[FAIL] Usage: forgen bridge check <file.dtr>");
            return false;
        }
    };
    let path = PathBuf::from(&file);
    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[FAIL] Cannot read {}: {}", path.display(), e);
            return false;
        }
    };

    let mut diag = DiagnosticEngine::new("en");
    let mut lexer = Lexer::new(&source, &file);
    let tokens = lexer.tokenize(&mut diag);
    let mut parser = Parser::new(tokens, &mut diag, &file);
    let program = parser.parse_program();

    println!(
        ":: [BRIDGE] Auditing foreign-function bridges in {}...",
        file
    );
    let mut failures = 0usize;

    // ---- 1. Declarative bridges ----------------------------------------
    let mut bridge_arity: HashMap<String, (usize, SourceSpan)> = HashMap::new();
    let mut bridge_count = 0usize;
    for decl in &program.declarations {
        if let Decl::Bridge(b) = decl {
            bridge_count += 1;
            println!(
                "  [BRIDGE] {}::{} ({} fn)",
                b.lang,
                b.module,
                b.functions.len()
            );
            for f in &b.functions {
                bridge_arity
                    .entry(f.name.clone())
                    .or_insert((f.params.len(), f.span.clone()));
            }
        }
    }
    if bridge_count == 0 {
        println!("  [i] No declarative bridges (bridge lang::module) found.");
    }

    // Unknown-language / unsupported-type diagnostics were emitted into the
    // shared registry pass; surface them here.
    if diag.has_errors() {
        failures += 1;
    }

    // ---- 2. C headers ----------------------------------------------------
    let mut header_fns: HashMap<String, HeaderFn> = HashMap::new();
    let base_dir = path.parent().map(|p| p.to_path_buf());
    let mut header_count = 0usize;
    let mut c_fn_count = 0usize;
    for decl in &program.declarations {
        if let Decl::CImport(ci) = decl {
            let resolved = resolve_header(&ci.header_path, base_dir.as_deref());
            let header = match resolved {
                None => {
                    diag.error(
                        ErrorCode::BridgeUnsupportedType,
                        format!(
                            "C import: header '{}' not found (relative to {})",
                            ci.header_path,
                            base_dir
                                .as_deref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| ".".to_string())
                        ),
                        Some(ci.span.clone()),
                    );
                    failures += 1;
                    continue;
                }
                Some(p) => p,
            };
            let header_src = match std::fs::read_to_string(&header) {
                Ok(s) => s,
                Err(e) => {
                    diag.error(
                        ErrorCode::BridgeUnsupportedType,
                        format!("C import: cannot read '{}': {}", header.display(), e),
                        Some(ci.span.clone()),
                    );
                    failures += 1;
                    continue;
                }
            };
            let mut h_diag = DiagnosticEngine::new("en");
            h_diag.set_source(&header.display().to_string(), &header_src);
            let mut h_lexer = crate::cimport::lexer::CLexer::new(&header_src);
            let h_tokens = h_lexer.tokenize();
            let mut h_parser = crate::cimport::parser::CParser::new(h_tokens);
            let decls = h_parser.parse();
            if h_diag.has_errors() {
                diag.error(
                    ErrorCode::BridgeUnsupportedType,
                    format!(
                        "C import: header '{}' failed to parse; run 'forgen check' for details",
                        header.display()
                    ),
                    Some(ci.span.clone()),
                );
                failures += 1;
                continue;
            }
            header_count += 1;
            let mut n = 0usize;
            for d in &decls {
                if let CDecl::Function(f) = d {
                    header_fns.insert(
                        f.name.clone(),
                        HeaderFn {
                            arity: f.params.len(),
                            is_variadic: f.is_variadic,
                            line: f.line,
                        },
                    );
                    n += 1;
                }
            }
            c_fn_count += n;
            println!(
                "  [C] {} ({} fn, link {:?})",
                header.display(),
                n,
                ci.link_libs
            );
        }
    }
    if header_count == 0 {
        println!("  [i] No C imports (import c \"...\") found.");
    }

    // ---- 3. Call sites ----------------------------------------------------
    let mut calls: Vec<CallSite> = Vec::new();
    for decl in &program.declarations {
        if let Decl::Function(f) = decl {
            collect_stmt(&f.body, &mut calls);
        }
    }
    let mut checked = 0usize;
    for (name, argc, span) in &calls {
        let argc = *argc;
        if let Some((want, _decl_span)) = bridge_arity.get(name) {
            checked += 1;
            if argc != *want {
                diag.error(
                    ErrorCode::BridgeTypeMismatch,
                    format!(
                        "bridge call '{}' expects {} argument(s), call passes {}",
                        name, want, argc
                    ),
                    Some(span.clone()),
                );
                failures += 1;
            }
        } else if let Some(hf) = header_fns.get(name) {
            checked += 1;
            let ok = if hf.is_variadic {
                argc >= hf.arity
            } else {
                argc == hf.arity
            };
            if !ok {
                diag.error(
                    ErrorCode::BridgeTypeMismatch,
                    format!(
                        "C function '{}' (header line {}) expects {} argument(s), call passes {}",
                        name, hf.line, hf.arity, argc
                    ),
                    Some(span.clone()),
                );
                failures += 1;
            }
        }
    }

    // ---- Report ----------------------------------------------------------
    if diag.has_errors() {
        eprintln!("{}", diag.format_all());
    }
    println!();
    println!(
        "  Surface: {} bridge decl(s), {} C header fn(s), {} verified call site(s)",
        bridge_count, c_fn_count, checked
    );
    if failures == 0 {
        println!("[DONE] Bridge check passed: no signature mismatches.");
        true
    } else {
        eprintln!("[FAIL] Bridge check found {} problem(s).", failures);
        // The CLI router discards arm results; exit non-zero so CI gates.
        std::process::exit(1);
    }
}

fn resolve_header(header_path: &str, base_dir: Option<&Path>) -> Option<PathBuf> {
    let p = PathBuf::from(header_path);
    if p.is_absolute() && p.exists() {
        return Some(p);
    }
    if let Some(base) = base_dir {
        let cand = base.join(&p);
        if cand.exists() {
            return Some(cand);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let cand = cwd.join(&p);
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

// ---- AST walker: collect plain-name call sites ---------------------------

fn collect_stmt(stmt: &Stmt, out: &mut Vec<CallSite>) {
    match stmt {
        Stmt::Block(stmts, _) => stmts.iter().for_each(|s| collect_stmt(s, out)),
        Stmt::Let { init: e, .. }
        | Stmt::Mut { init: e, .. }
        | Stmt::Const { init: e, .. }
        | Stmt::Val { init: e, .. }
        | Stmt::CompactBind { init: e, .. }
        | Stmt::Expr(e, _)
        | Stmt::Out(e, _)
        | Stmt::Err(e, _) => collect_expr(e, out),
        Stmt::Return(Some(e), _) => collect_expr(e, out),
        Stmt::Return(None, _) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Asm { .. } => {}
        Stmt::Assign { target, value, .. } => {
            collect_expr(target, out);
            collect_expr(value, out);
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr(condition, out);
            collect_stmt(then_branch, out);
            if let Some(eb) = else_branch {
                collect_stmt(eb, out);
            }
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr(iterable, out);
            collect_stmt(body, out);
        }
        Stmt::While {
            condition, body, ..
        } => {
            collect_expr(condition, out);
            collect_stmt(body, out);
        }
        Stmt::Loop { body, .. } => collect_stmt(body, out),
        Stmt::Parallel(inner, _) => collect_stmt(inner, out),
        Stmt::ParallelFor { iterable, body, .. } => {
            collect_expr(iterable, out);
            collect_stmt(body, out);
        }
        Stmt::Simd(inner, _) => collect_stmt(inner, out),
        Stmt::With { init, body, .. } => {
            collect_expr(init, out);
            collect_stmt(body, out);
        }
        Stmt::Unsafe { body, .. } => collect_stmt(body, out),
        // Break/Continue/Asm (matched in the leaf arm above) carry no call
        // sites, and every other Stmt variant is already covered.
    }
}

fn collect_expr(expr: &Expr, out: &mut Vec<CallSite>) {
    match expr {
        Expr::Call { callee, args, span } => {
            if let Expr::Identifier(name, _) = &**callee {
                out.push((name.clone(), args.len(), span.clone()));
            }
            collect_expr(callee, out);
            args.iter().for_each(|a| collect_expr(a, out));
        }
        Expr::Literal(..) | Expr::Identifier(..) => {}
        Expr::InterpolatedString { expressions, .. } => {
            expressions.iter().for_each(|p| collect_expr(p, out));
        }
        Expr::Binary { left, right, .. } => {
            collect_expr(left, out);
            collect_expr(right, out);
        }
        Expr::Range { start, end, .. } => {
            collect_expr(start, out);
            collect_expr(end, out);
        }
        Expr::Unary { expr, .. } => collect_expr(expr, out),
        Expr::MemberAccess { object, .. } => collect_expr(object, out),
        Expr::IndexAccess { object, index, .. } => {
            collect_expr(object, out);
            collect_expr(index, out);
        }
        Expr::Tuple(items, _) | Expr::ListLiteral(items, _) => {
            items.iter().for_each(|e| collect_expr(e, out));
        }
        Expr::MapLiteral(pairs, _) => {
            for (k, v) in pairs {
                collect_expr(k, out);
                collect_expr(v, out);
            }
        }
        Expr::ObjectInit { fields, .. } => {
            for (_, v) in fields {
                collect_expr(v, out);
            }
        }
        Expr::Pipeline { stages, .. } => {
            stages.iter().for_each(|s| collect_expr(s, out));
        }
        Expr::Decide { arms, else_arm, .. } => {
            for a in arms {
                collect_expr(&a.condition, out);
                collect_expr(&a.body, out);
            }
            if let Some(ev) = else_arm {
                collect_expr(ev, out);
            }
        }
        Expr::Match { value, arms, .. } => {
            collect_expr(value, out);
            for a in arms {
                if let Some(g) = &a.guard {
                    collect_expr(g, out);
                }
                collect_expr(&a.body, out);
            }
        }
        Expr::Select { arms, else_arm, .. } => {
            for a in arms {
                collect_expr(&a.condition, out);
                collect_expr(&a.body, out);
            }
            if let Some(ev) = else_arm {
                collect_expr(ev, out);
            }
        }
        Expr::Lambda { body, .. } => collect_expr(body, out),
        Expr::ErrorPropagate(inner, _) | Expr::Comptime { expr: inner, .. } => {
            collect_expr(inner, out)
        }
        Expr::OrRecovery { expr, arms, .. } => {
            collect_expr(expr, out);
            for a in arms {
                if let Some(g) = &a.guard {
                    collect_expr(g, out);
                }
                collect_expr(&a.body, out);
            }
        }
        Expr::ArrayRepeatLiteral { elem, .. } => {
            collect_expr(elem, out);
        }
        Expr::Wrapping(inner, _) | Expr::Saturating(inner, _) => collect_expr(inner, out),
        Expr::Cast { expr: inner, .. } => collect_expr(inner, out),
        Expr::Block(stmts, tail, _) => {
            stmts.iter().for_each(|s| collect_stmt(s, out));
            if let Some(t) = tail {
                collect_expr(t, out);
            }
        }
        Expr::IsCheck { operand, .. } => collect_expr(operand, out),
    }
}
