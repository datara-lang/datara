//! v1.4.0 `@arena` / `@pool(n)` allocator-tier validation.
//!
//! # Attribute surface
//!
//! * `@arena` — the function's escaping class allocations and list-literal
//!   headers are bump-allocated from the runtime's thread-local region
//!   allocator. A checkpoint taken at function entry is restored at every
//!   return, reclaiming the whole region wholesale (per-call frame, so
//!   recursion is safe: each invocation restores its own checkpoint).
//! * `@pool(n)` — a slab of `n` object slots is bump-allocated once per
//!   call; class allocations are served from the slab under a runtime slot
//!   counter with an explicit capacity trap.
//!
//! # Validation (hard errors, tier-independent)
//!
//! * `E1401` — a heap value allocated inside an `@arena`/`@pool` function
//!   escapes the region: returned, stored into a name that is not a local of
//!   this function (module-level binding or a parameter), or written through
//!   a field of another object. Region memory is reclaimed on return, so
//!   such a value would dangle. Only Copy results (`Int`, `Float`, `Bool`,
//!   `Str`) may be returned.
//! * `E1406` — the pool cannot hold the function's provable allocations
//!   (constant-count sites outside any loop), the `@pool` argument is not a
//!   positive integer, both tiers are requested at once, or a pooled class
//!   needs more than the slot size (8 fields).
//!
//! # Honest scope
//!
//! The escape rule is syntactic and deliberately conservative: it follows
//! direct allocation literals (`[...]`, `{...}` maps, `Class { ... }`) and
//! local names bound from them, inside the validated function. Values handed
//! to calls (which may or may not store them) and stores through an index are
//! *not* tracked — this is documented as a known limitation, not silently
//! accepted as sound.

use crate::ast::{ClassItem, Decl, Expr, FunctionDecl, MethodDecl, Program, Stmt};
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::dmir::ArenaHint;
use std::collections::{HashMap, HashSet};

/// Field count above which a class no longer fits a 64-byte pool slot
/// (`8 fields x 8 bytes`), mirroring `compile_func::POOL_SLOT_SIZE`.
pub(crate) const POOL_MAX_FIELDS: usize = 8;

/// Validates every `@arena` / `@pool(n)` attribute in the program.
///
/// Runs in the driver pipeline after derive expansion and before lowering
/// stores the hints, so a malformed attribute or an escaping allocation can
/// never reach codegen.
pub fn validate_alloc_attributes(program: &Program, diag: &mut DiagnosticEngine) {
    let mut class_field_counts: HashMap<String, usize> = HashMap::new();
    for decl in &program.declarations {
        if let Decl::Class(c) = decl {
            let fields = c
                .body_items
                .iter()
                .filter(|i| matches!(i, ClassItem::Field(_)))
                .count();
            class_field_counts.insert(c.name.clone(), fields);
        }
    }

    for decl in &program.declarations {
        match decl {
            Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                validate_fn(
                    &f.name,
                    &f.attributes,
                    &f.body,
                    &f.span,
                    &class_field_counts,
                    diag,
                );
            }
            Decl::Class(c) => {
                for item in &c.body_items {
                    if let ClassItem::Method(m) = item {
                        validate_method(&c.name, m, &class_field_counts, diag);
                    }
                }
            }
            Decl::Behavior(b) => {
                for item in &b.body_items {
                    if let ClassItem::Method(m) = item {
                        validate_method(&b.target_type, m, &class_field_counts, diag);
                    }
                }
            }
            Decl::Impl(i) => {
                for m in &i.methods {
                    validate_fn(
                        &format!("{}_{}", i.target_type, m.name),
                        &m.attributes,
                        &m.body,
                        &m.span,
                        &class_field_counts,
                        diag,
                    );
                }
            }
            _ => {}
        }
    }
}

fn validate_method(
    target_type: &str,
    m: &MethodDecl,
    class_field_counts: &HashMap<String, usize>,
    diag: &mut DiagnosticEngine,
) {
    let Some(body) = m.body.as_ref() else {
        return;
    };
    validate_fn(
        &format!("{}_{}", target_type, m.name),
        &m.attributes,
        body,
        &m.span,
        class_field_counts,
        diag,
    );
}

fn validate_fn(
    name: &str,
    attrs: &[crate::ast::Attribute],
    body: &Stmt,
    span: &SourceSpan,
    class_field_counts: &HashMap<String, usize>,
    diag: &mut DiagnosticEngine,
) {
    let attr = attrs.iter().find(|a| a.name == "arena" || a.name == "pool");
    let (hint, err) = ArenaHint::parse_from_attrs(attrs);
    if let Some(msg) = err {
        diag.error(
            ErrorCode::PoolCapacityExceeded,
            format!("function '{}': {}", name, msg),
            Some(attr.map(|a| a.span.clone()).unwrap_or_else(|| span.clone())),
        );
        return;
    }
    if hint == ArenaHint::None {
        return;
    }
    let tier = match hint {
        ArenaHint::Arena | ArenaHint::ArenaSized(_) => "@arena",
        ArenaHint::Pool(_) => "@pool",
        ArenaHint::None => return,
    };

    // 1. Escape rule (E1401).
    let mut locals: HashSet<String> = HashSet::new();
    collect_locals(body, &mut locals);
    let mut alloc_locals: HashSet<String> = HashSet::new();
    check_escape(name, tier, body, &locals, &mut alloc_locals, diag);

    // 2. Pool-specific gates (E1406).
    if let ArenaHint::Pool(capacity) = hint {
        let provable = count_provable_allocs(body);
        if provable > capacity {
            diag.error(
                ErrorCode::PoolCapacityExceeded,
                format!(
                    "function '{}': @pool({}) cannot hold the {} value(s) allocated outside any loop; raise the capacity or wrap the allocations in a loop (dynamic counts are checked at runtime)",
                    name, capacity, provable
                ),
                Some(attr.map(|a| a.span.clone()).unwrap_or_else(|| span.clone())),
            );
        }
        check_pool_slot_sizes(name, body, class_field_counts, diag);
    }
}

/// Collects every name bound inside the function body (lets, mutable
/// bindings, for-loop variables, `with` resources, catch variables).
fn collect_locals(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::Block(stmts, _) => stmts.iter().for_each(|s| collect_locals(s, out)),
        Stmt::Let { name, .. }
        | Stmt::Mut { name, .. }
        | Stmt::Const { name, .. }
        | Stmt::Val { name, .. }
        | Stmt::CompactBind { name, .. } => {
            out.insert(name.clone());
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_locals(then_branch, out);
            if let Some(e) = else_branch {
                collect_locals(e, out);
            }
        }
        Stmt::For { var_name, body, .. } | Stmt::ParallelFor { var_name, body, .. } => {
            out.insert(var_name.clone());
            collect_locals(body, out);
        }
        Stmt::While { body, .. }
        | Stmt::Loop { body, .. }
        | Stmt::Parallel(body, _)
        | Stmt::Unsafe { body, .. } => collect_locals(body, out),
        Stmt::With {
            resource_name,
            body,
            ..
        } => {
            out.insert(resource_name.clone());
            collect_locals(body, out);
        }
        _ => {}
    }
}

/// True when the expression is a direct heap-allocation literal.
fn is_alloc_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::ListLiteral(..) | Expr::MapLiteral(..) | Expr::ObjectInit { .. }
    )
}

fn check_escape(
    fn_name: &str,
    tier: &str,
    stmt: &Stmt,
    locals: &HashSet<String>,
    alloc_locals: &mut HashSet<String>,
    diag: &mut DiagnosticEngine,
) {
    match stmt {
        Stmt::Block(stmts, _) => {
            for s in stmts {
                check_escape(fn_name, tier, s, locals, alloc_locals, diag);
            }
        }
        Stmt::Let { name, init, .. }
        | Stmt::Mut { name, init, .. }
        | Stmt::Const { name, init, .. }
        | Stmt::Val { name, init, .. }
        | Stmt::CompactBind { name, init, .. } => {
            if is_alloc_expr(init)
                || matches!(init, Expr::Identifier(src, _) if alloc_locals.contains(src))
            {
                alloc_locals.insert(name.clone());
            }
        }
        Stmt::Assign {
            target,
            value,
            span,
        } => {
            let source_is_alloc = is_alloc_expr(value)
                || matches!(value, Expr::Identifier(src, _) if alloc_locals.contains(src));
            if source_is_alloc {
                match target {
                    Expr::Identifier(name, _) => {
                        if locals.contains(name) {
                            alloc_locals.insert(name.clone());
                        } else {
                            diag.error(
                                ErrorCode::ArenaEscape,
                                format!(
                                    "function '{}': value allocated in an {} function escapes the region: '{}' is not a local of this function (module binding or parameter), and region memory is reclaimed when the function returns",
                                    fn_name, tier, name
                                ),
                                Some(span.clone()),
                            );
                        }
                    }
                    Expr::MemberAccess { member, .. } => {
                        diag.error(
                            ErrorCode::ArenaEscape,
                            format!(
                                "function '{}': value allocated in an {} function escapes the region: storing it into field '{}' of another object outlives the region",
                                fn_name, tier, member
                            ),
                            Some(span.clone()),
                        );
                    }
                    _ => {}
                }
            }
        }
        Stmt::Return(Some(expr), span) => {
            let escapes = is_alloc_expr(expr)
                || matches!(expr, Expr::Identifier(src, _) if alloc_locals.contains(src));
            if escapes {
                diag.error(
                    ErrorCode::ArenaEscape,
                    format!(
                        "function '{}': value allocated in an {} function escapes the region on return; only Copy results (Int, Float, Bool, Str) may leave an {} function",
                        fn_name, tier, tier
                    ),
                    Some(span.clone()),
                );
            }
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            check_escape(fn_name, tier, then_branch, locals, alloc_locals, diag);
            if let Some(e) = else_branch {
                check_escape(fn_name, tier, e, locals, alloc_locals, diag);
            }
        }
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Loop { body, .. }
        | Stmt::ParallelFor { body, .. } => {
            check_escape(fn_name, tier, body, locals, alloc_locals, diag);
        }
        Stmt::Parallel(body, _) | Stmt::Unsafe { body, .. } => {
            check_escape(fn_name, tier, body, locals, alloc_locals, diag);
        }
        Stmt::With { body, .. } => {
            check_escape(fn_name, tier, body, locals, alloc_locals, diag);
        }
        _ => {}
    }
}

/// Counts allocations whose count is statically known: every allocation
/// literal outside any loop. Literal lengths count as that many values
/// (a `[a, b, c]` list needs three slots), class literals count one.
/// Sites inside loops are *not* counted — they are proven only at runtime by
/// the slot counter, which traps on overflow.
fn count_provable_allocs(stmt: &Stmt) -> u64 {
    match stmt {
        Stmt::Block(stmts, _) => stmts.iter().map(count_provable_allocs).sum(),
        Stmt::Let { init, .. }
        | Stmt::Mut { init, .. }
        | Stmt::Const { init, .. }
        | Stmt::Val { init, .. }
        | Stmt::CompactBind { init, .. } => count_expr_allocs(init),
        Stmt::Assign { value, .. } => count_expr_allocs(value),
        Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => count_expr_allocs(e),
        Stmt::If {
            then_branch,
            else_branch,
            condition,
            ..
        } => {
            count_expr_allocs(condition)
                + count_provable_allocs(then_branch)
                + else_branch
                    .as_ref()
                    .map(|e| count_provable_allocs(e))
                    .unwrap_or(0)
        }
        // Loop bodies allocate an unbounded number of times.
        Stmt::For { iterable, .. } | Stmt::ParallelFor { iterable, .. } => {
            count_expr_allocs(iterable)
        }
        Stmt::While { condition, .. } => count_expr_allocs(condition),
        Stmt::Loop { .. } => 0,
        Stmt::Parallel(body, _) | Stmt::Unsafe { body, .. } => count_provable_allocs(body),
        Stmt::With { init, body, .. } => count_expr_allocs(init) + count_provable_allocs(body),
        Stmt::Return(Some(e), _) => count_expr_allocs(e),
        _ => 0,
    }
}

fn count_expr_allocs(expr: &Expr) -> u64 {
    match expr {
        Expr::ListLiteral(items, _) => items.iter().map(count_expr_allocs).sum::<u64>() + 1,
        Expr::MapLiteral(entries, _) => {
            entries
                .iter()
                .map(|(k, v)| count_expr_allocs(k) + count_expr_allocs(v))
                .sum::<u64>()
                + 1
        }
        Expr::ObjectInit { fields, .. } => {
            fields
                .iter()
                .map(|(_, v)| count_expr_allocs(v))
                .sum::<u64>()
                + 1
        }
        Expr::Call { callee, args, .. } => {
            count_expr_allocs(callee) + args.iter().map(count_expr_allocs).sum::<u64>()
        }
        Expr::Binary { left, right, .. } => count_expr_allocs(left) + count_expr_allocs(right),
        Expr::InterpolatedString { expressions, .. } => {
            expressions.iter().map(count_expr_allocs).sum()
        }
        Expr::Block(stmts, trailing, _) => {
            stmts.iter().map(count_provable_allocs).sum::<u64>()
                + trailing.as_ref().map(|t| count_expr_allocs(t)).unwrap_or(0)
        }
        _ => 0,
    }
}

/// Rejects pooled functions that instantiate a class too large for a
/// 64-byte pool slot.
fn check_pool_slot_sizes(
    fn_name: &str,
    stmt: &Stmt,
    class_field_counts: &HashMap<String, usize>,
    diag: &mut DiagnosticEngine,
) {
    let mut offenders: Vec<(String, SourceSpan)> = Vec::new();
    collect_oversized_class_inits(stmt, class_field_counts, &mut offenders);
    for (class, span) in offenders {
        diag.error(
            ErrorCode::PoolCapacityExceeded,
            format!(
                "function '{}': class '{}' needs more than the {} fields that fit a @pool slot; use @arena or leave the function unpooled",
                fn_name, class, POOL_MAX_FIELDS
            ),
            Some(span),
        );
    }
}

fn collect_oversized_class_inits(
    stmt: &Stmt,
    class_field_counts: &HashMap<String, usize>,
    out: &mut Vec<(String, SourceSpan)>,
) {
    fn scan_expr(
        expr: &Expr,
        class_field_counts: &HashMap<String, usize>,
        out: &mut Vec<(String, SourceSpan)>,
    ) {
        if let Expr::ObjectInit {
            class_name,
            span,
            fields,
            ..
        } = expr
        {
            if class_field_counts
                .get(class_name)
                .map(|n| *n > POOL_MAX_FIELDS)
                .unwrap_or(false)
            {
                out.push((class_name.clone(), span.clone()));
            }
            for (_, v) in fields {
                scan_expr(v, class_field_counts, out);
            }
        }
    }

    match stmt {
        Stmt::Block(stmts, _) => {
            for s in stmts {
                collect_oversized_class_inits(s, class_field_counts, out);
            }
        }
        Stmt::Let { init, .. }
        | Stmt::Mut { init, .. }
        | Stmt::Const { init, .. }
        | Stmt::Val { init, .. }
        | Stmt::CompactBind { init, .. } => scan_expr(init, class_field_counts, out),
        Stmt::Assign { value, .. } => scan_expr(value, class_field_counts, out),
        Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => {
            scan_expr(e, class_field_counts, out);
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_oversized_class_inits(then_branch, class_field_counts, out);
            if let Some(e) = else_branch {
                collect_oversized_class_inits(e, class_field_counts, out);
            }
        }
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Loop { body, .. }
        | Stmt::ParallelFor { body, .. }
        | Stmt::Parallel(body, _)
        | Stmt::Unsafe { body, .. }
        | Stmt::With { body, .. } => {
            collect_oversized_class_inits(body, class_field_counts, out);
        }
        Stmt::Return(Some(e), _) => scan_expr(e, class_field_counts, out),
        _ => {}
    }
}

/// Convenience for tests and the CLI: the hint carried by a declaration's
/// attributes, or `None` when the attributes are malformed.
pub fn alloc_hint_of_fn(f: &FunctionDecl) -> ArenaHint {
    ArenaHint::parse_from_attrs(&f.attributes).0
}
