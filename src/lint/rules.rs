use crate::ast::*;
use crate::lint::diagnostics::LintDiagnostic;
use std::collections::{HashMap, HashSet};

pub fn is_snake_case(s: &str) -> bool {
    let s = s.trim_start_matches('_');
    if s.is_empty() {
        return true;
    }
    s.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !s.contains("__")
}

pub fn is_pascal_case(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let first = s.chars().next().unwrap();
    first.is_ascii_uppercase() && !s.contains('_')
}

pub fn is_screaming_snake_case(s: &str) -> bool {
    let s = s.trim_start_matches('_');
    if s.is_empty() {
        return true;
    }
    s.chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

pub fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !chars[i - 1].is_ascii_uppercase() && chars[i - 1] != '_' {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn to_pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut cap_next = true;
    for c in s.chars() {
        if c == '_' {
            cap_next = true;
        } else if cap_next {
            out.push(c.to_ascii_uppercase());
            cap_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub fn run_all_rules(program: &Program) -> Vec<LintDiagnostic> {
    let mut diags = Vec::new();
    check_declarations(program, &mut diags);
    check_string_concat_loops(program, &mut diags);
    check_dead_code(program, &mut diags);
    diags
}

/// Dead-code detection for top-level functions (S1).
///
/// A top-level function is reported when its name is never referenced
/// anywhere in the program: no `Expr::Identifier(name)`, no direct call, and
/// no method call whose member matches the name. `main` (the entry point),
/// exported functions, attributed functions, and underscore-prefixed names
/// are roots and never reported. The check is intentionally conservative to
/// keep false positives at zero: if the AST references a name anywhere -
/// even in an unreachable branch - the function is considered live.
///
/// The reference scan is exhaustive over the whole AST: expression bodies,
/// `require`/`ensure`/`decreases` contracts, global initializers, class
/// invariants and field defaults, trait default bodies, match guards, and
/// closures are all scanned, so a function used only inside any of these is
/// never reported. Call this once per whole program (see
/// `lint::lint_files_with_profile`), not once per file - per-file calls
/// cannot see cross-module references and produce false positives.
fn check_dead_code(program: &Program, diags: &mut Vec<LintDiagnostic>) {
    // 1. Collect candidate definitions.
    let mut defined: HashMap<String, SourceSpan> = HashMap::new();
    for decl in &program.declarations {
        match decl {
            Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                let is_root = f.name == "main"
                    || f.is_export
                    || !f.attributes.is_empty()
                    || f.name.starts_with('_');
                if !is_root {
                    defined.insert(f.name.clone(), f.span.clone());
                }
            }
            _ => {}
        }
    }
    if defined.is_empty() {
        return;
    }

    // 2. Collect every referenced name across the whole program.
    let mut referenced: HashSet<String> = HashSet::new();
    for decl in &program.declarations {
        scan_decl_for_references(decl, &mut referenced);
    }

    // 3. Report defined-but-never-referenced functions.
    let mut dead: Vec<(String, SourceSpan)> = defined
        .into_iter()
        .filter(|(name, _)| !referenced.contains(name))
        .collect();
    dead.sort_by_key(|a| a.1.start_line);
    for (name, span) in dead {
        diags.push(
            LintDiagnostic::new(
                "dead_code::unused_function",
                format!("function `{}` is never used", name),
                span,
            )
            .with_help(format!(
                "remove it, rename to `_{}` if intentionally unused, or mark #[export]",
                name
            ))
            .with_note(
                "this function is never called and its name is never referenced".into(),
            ),
        );
    }
}

fn scan_decl_for_references(decl: &Decl, referenced: &mut HashSet<String>) {
    match decl {
        Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => scan_function(f, referenced),
        Decl::Class(c) => {
            for item in &c.body_items {
                scan_class_item(item, referenced);
            }
            for inv in &c.invariants {
                scan_expr(inv, referenced);
            }
        }
        Decl::Component(c) => {
            for item in &c.body_items {
                scan_class_item(item, referenced);
            }
        }
        Decl::Behavior(b) => {
            for item in &b.body_items {
                scan_class_item(item, referenced);
            }
        }
        Decl::Role(r) => {
            for m in &r.methods {
                scan_method_decl(m, referenced);
            }
        }
        Decl::Trait(t) => {
            for m in &t.methods {
                if let Some(body) = m.default_body.as_ref() {
                    scan_stmt(body, referenced);
                }
            }
        }
        Decl::Impl(i) => {
            for m in &i.methods {
                scan_function(m, referenced);
            }
        }
        Decl::Global(g) => scan_expr(&g.init, referenced),
        _ => {}
    }
}

/// Scans a `FunctionDecl` (also used for `impl` methods): body plus all
/// contract clauses (`require`/`ensure`/`decreases`).
fn scan_function(f: &FunctionDecl, referenced: &mut HashSet<String>) {
    scan_stmt(&f.body, referenced);
    for c in &f.requires {
        scan_expr(&c.condition, referenced);
    }
    for c in &f.ensures {
        scan_expr(&c.condition, referenced);
    }
    if let Some(d) = f.decreases.as_ref() {
        scan_expr(d, referenced);
    }
}

/// Scans a `MethodDecl` (class/component/behavior/role methods): optional
/// body plus all contract clauses.
fn scan_method_decl(m: &MethodDecl, referenced: &mut HashSet<String>) {
    if let Some(body) = m.body.as_ref() {
        scan_stmt(body, referenced);
    }
    for c in &m.requires {
        scan_expr(&c.condition, referenced);
    }
    for c in &m.ensures {
        scan_expr(&c.condition, referenced);
    }
    if let Some(d) = m.decreases.as_ref() {
        scan_expr(d, referenced);
    }
}

fn scan_class_item(item: &ClassItem, referenced: &mut HashSet<String>) {
    match item {
        ClassItem::Method(m) => scan_method_decl(m, referenced),
        ClassItem::Field(f) => {
            if let Some(init) = f.default_value.as_ref() {
                scan_expr(init, referenced);
            }
        }
        ClassItem::Invariant(e, _) => scan_expr(e, referenced),
        ClassItem::Using(_, _) => {}
    }
}

fn scan_stmt(stmt: &Stmt, referenced: &mut HashSet<String>) {
    match stmt {
        Stmt::Block(stmts, _) => {
            for s in stmts {
                scan_stmt(s, referenced);
            }
        }
        Stmt::Let { init, .. }
        | Stmt::Mut { init, .. }
        | Stmt::Const { init, .. }
        | Stmt::Val { init, .. }
        | Stmt::CompactBind { init, .. } => scan_expr(init, referenced),
        Stmt::Assign { target, value, .. } => {
            scan_expr(target, referenced);
            scan_expr(value, referenced);
        }
        Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => scan_expr(e, referenced),
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            scan_expr(condition, referenced);
            scan_stmt(then_branch, referenced);
            if let Some(els) = else_branch.as_ref() {
                scan_stmt(els, referenced);
            }
        }
        Stmt::For {
            iterable, body, ..
        }
        | Stmt::ParallelFor {
            iterable, body, ..
        } => {
            scan_expr(iterable, referenced);
            scan_stmt(body, referenced);
        }
        Stmt::While {
            condition, body, ..
        } => {
            scan_expr(condition, referenced);
            scan_stmt(body, referenced);
        }
        Stmt::Loop { body, .. } => scan_stmt(body, referenced),
        Stmt::Break(_) | Stmt::Continue(_) => {}
        Stmt::Parallel(inner, _) | Stmt::Simd(inner, _) => scan_stmt(inner, referenced),
        Stmt::With {
            init, body, ..
        } => {
            scan_expr(init, referenced);
            scan_stmt(body, referenced);
        }
        Stmt::Unsafe { body, .. } => scan_stmt(body, referenced),
        Stmt::Asm { .. } => {}
        Stmt::Return(Some(e), _) => scan_expr(e, referenced),
        Stmt::Return(None, _) => {}
    }
}

/// Exhaustive reference scan over an expression tree. Every `Expr` variant
/// is handled explicitly (no catch-all), so adding a new AST node breaks
/// compilation here instead of silently regressing the analysis.
fn scan_expr(expr: &Expr, referenced: &mut HashSet<String>) {
    match expr {
        Expr::Literal(..) => {}
        Expr::Identifier(name, _) => {
            referenced.insert(name.clone());
        }
        Expr::InterpolatedString { expressions, .. } => {
            for e in expressions {
                scan_expr(e, referenced);
            }
        }
        Expr::Binary { left, right, .. } => {
            scan_expr(left, referenced);
            scan_expr(right, referenced);
        }
        Expr::Unary { expr, .. } => scan_expr(expr, referenced),
        Expr::Call { callee, args, .. } => {
            // A method call `obj.name(...)` makes `name` a used symbol.
            if let Expr::MemberAccess { member, .. } = callee.as_ref() {
                referenced.insert(member.clone());
            }
            scan_expr(callee, referenced);
            for a in args {
                scan_expr(a, referenced);
            }
        }
        Expr::MemberAccess { object, .. } => scan_expr(object, referenced),
        Expr::IndexAccess { object, index, .. } => {
            scan_expr(object, referenced);
            scan_expr(index, referenced);
        }
        Expr::Range { start, end, .. } => {
            scan_expr(start, referenced);
            scan_expr(end, referenced);
        }
        Expr::Tuple(items, _) | Expr::ListLiteral(items, _) => {
            for e in items {
                scan_expr(e, referenced);
            }
        }
        Expr::MapLiteral(entries, _) => {
            for (k, v) in entries {
                scan_expr(k, referenced);
                scan_expr(v, referenced);
            }
        }
        Expr::ArrayRepeatLiteral { elem, .. } => scan_expr(elem, referenced),
        Expr::ObjectInit { fields, .. } => {
            for (_, e) in fields {
                scan_expr(e, referenced);
            }
        }
        Expr::Pipeline { stages, .. } => {
            for s in stages {
                scan_expr(s, referenced);
            }
        }
        Expr::Decide { arms, else_arm, .. } => {
            for arm in arms {
                scan_expr(&arm.condition, referenced);
                scan_expr(&arm.body, referenced);
            }
            if let Some(ea) = else_arm.as_ref() {
                scan_expr(ea, referenced);
            }
        }
        Expr::Match { value, arms, .. } => {
            scan_expr(value, referenced);
            for arm in arms {
                if let Some(g) = arm.guard.as_ref() {
                    scan_expr(g, referenced);
                }
                scan_expr(&arm.body, referenced);
            }
        }
        Expr::Select { arms, else_arm, .. } => {
            for arm in arms {
                scan_expr(&arm.body, referenced);
            }
            if let Some(ea) = else_arm.as_ref() {
                scan_expr(ea, referenced);
            }
        }
        Expr::Lambda { body, .. } => scan_expr(body, referenced),
        Expr::ErrorPropagate(e, _) | Expr::Wrapping(e, _) | Expr::Saturating(e, _) => {
            scan_expr(e, referenced)
        }
        Expr::OrRecovery { expr, arms, .. } => {
            scan_expr(expr, referenced);
            for arm in arms {
                if let Some(g) = arm.guard.as_ref() {
                    scan_expr(g, referenced);
                }
                scan_expr(&arm.body, referenced);
            }
        }
        Expr::Comptime { expr, .. } => scan_expr(expr, referenced),
        Expr::Cast { expr, .. } => scan_expr(expr, referenced),
        Expr::Block(stmts, trailing, _) => {
            for s in stmts {
                scan_stmt(s, referenced);
            }
            if let Some(t) = trailing.as_ref() {
                scan_expr(t, referenced);
            }
        }
    }
}

/// v1.4.0 (L1401): detects `s = s + <string>` self-concatenation inside
/// while/for/loop bodies - the quadratic string-building anti-pattern - and
/// suggests `StrBuf`, whose runtime builder appends in amortized O(1).
///
/// Linting runs on the raw AST before type checking, so string-ness is
/// approximated: the rule only fires when the concatenated operand is a
/// string-shaped literal (`"..."` or an interpolated `fmt"..."`), which
/// keeps integer accumulation loops (`i = i + 1`) silent.
fn check_string_concat_loops(program: &Program, diags: &mut Vec<LintDiagnostic>) {
    for decl in &program.declarations {
        match decl {
            Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                scan_loops_for_concat(&f.body, diags);
            }
            Decl::Class(c) => {
                for item in &c.body_items {
                    if let ClassItem::Method(m) = item
                        && let Some(body) = m.body.as_ref()
                    {
                        scan_loops_for_concat(body, diags);
                    }
                }
            }
            Decl::Behavior(b) => {
                for item in &b.body_items {
                    if let ClassItem::Method(m) = item
                        && let Some(body) = m.body.as_ref()
                    {
                        scan_loops_for_concat(body, diags);
                    }
                }
            }
            Decl::Impl(i) => {
                for m in &i.methods {
                    scan_loops_for_concat(&m.body, diags);
                }
            }
            _ => {}
        }
    }
}

/// Recurses the statement tree and scans every loop body it enters.
fn scan_loops_for_concat(stmt: &Stmt, diags: &mut Vec<LintDiagnostic>) {
    match stmt {
        Stmt::Block(stmts, _) => {
            for s in stmts {
                scan_loops_for_concat(s, diags);
            }
        }
        Stmt::While { body, .. }
        | Stmt::For { body, .. }
        | Stmt::ParallelFor { body, .. }
        | Stmt::Loop { body, .. } => {
            scan_concat_assigns(body, diags);
            scan_loops_for_concat(body, diags);
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            scan_loops_for_concat(then_branch, diags);
            if let Some(e) = else_branch {
                scan_loops_for_concat(e, diags);
            }
        }
        Stmt::Unsafe { body, .. } | Stmt::Parallel(body, _) | Stmt::With { body, .. } => {
            scan_loops_for_concat(body, diags)
        }
        _ => {}
    }
}

/// Reports `x = x + <string literal>` / `x = <string literal> + x` inside
/// one loop body (including nested blocks and branches).
fn scan_concat_assigns(stmt: &Stmt, diags: &mut Vec<LintDiagnostic>) {
    match stmt {
        Stmt::Assign {
            target,
            value: Expr::Binary {
                op, left, right, ..
            },
            span,
            ..
        } if op == "+" => {
            fn var_of(e: &Expr) -> Option<&str> {
                match e {
                    Expr::Identifier(n, _) => Some(n.as_str()),
                    _ => None,
                }
            }
            let self_ref = var_of(left).is_some_and(|n| var_of(target) == Some(n))
                || var_of(right).is_some_and(|n| var_of(target) == Some(n));
            let other_is_string = |e: &Expr| {
                matches!(
                    e,
                    Expr::InterpolatedString { .. } | Expr::Literal(LiteralValue::String(_), _)
                )
            };
            let concat_with_string = var_of(left).is_some_and(|n| var_of(target) == Some(n))
                && other_is_string(right)
                || var_of(right).is_some_and(|n| var_of(target) == Some(n))
                    && other_is_string(left);
            if self_ref && concat_with_string {
                let name = var_of(target).unwrap_or("?");
                diags.push(
                    LintDiagnostic::new(
                        "L1401",
                        format!(
                            "string concatenation `{} = {} + ...` inside a loop copies the whole string every iteration",
                            name, name
                        ),
                        span.clone(),
                    )
                    .with_help(
                        "build the string with StrBuf instead: `StrBuf { }`, `sb.push(...)`, `sb.join()`".to_string(),
                    )
                    .with_note(
                        "repeated `s = s + ...` is O(n^2); StrBuf.push is amortized O(1)".to_string(),
                    ),
                );
            }
        }
        Stmt::Block(stmts, _) => {
            for s in stmts {
                scan_concat_assigns(s, diags);
            }
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            scan_concat_assigns(then_branch, diags);
            if let Some(e) = else_branch {
                scan_concat_assigns(e, diags);
            }
        }
        Stmt::While { body, .. }
        | Stmt::For { body, .. }
        | Stmt::ParallelFor { body, .. }
        | Stmt::Loop { body, .. }
        | Stmt::Unsafe { body, .. }
        | Stmt::Parallel(body, _)
        | Stmt::With { body, .. } => scan_concat_assigns(body, diags),
        _ => {}
    }
}

fn check_declarations(program: &Program, diags: &mut Vec<LintDiagnostic>) {
    for decl in &program.declarations {
        match decl {
            Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                // Function name must be snake_case
                if !is_snake_case(&f.name) {
                    let suggested = to_snake_case(&f.name);
                    diags.push(
                        LintDiagnostic::new(
                            "style::non_snake_case",
                            format!("function `{}` should have a snake_case name", f.name),
                            f.span.clone(),
                        )
                        .with_help(format!("convert to snake_case: `{}`", suggested))
                        .with_note(
                            "Datara naming convention enforces snake_case for functions".into(),
                        ),
                    );
                }

                // Check parameter names
                for p in &f.params {
                    if !is_snake_case(&p.name) {
                        let suggested = to_snake_case(&p.name);
                        diags.push(
                            LintDiagnostic::new(
                                "style::non_snake_case",
                                format!("parameter `{}` should have a snake_case name", p.name),
                                p.span.clone(),
                            )
                            .with_help(format!("convert to snake_case: `{}`", suggested))
                            .with_note(
                                "Datara naming convention enforces snake_case for parameters"
                                    .into(),
                            ),
                        );
                    }
                }

                // Check statements in function body
                check_body(&f.body, diags);
            }

            Decl::Class(c) => {
                if !is_pascal_case(&c.name) {
                    let suggested = to_pascal_case(&c.name);
                    diags.push(
                        LintDiagnostic::new(
                            "style::non_camel_case_types",
                            format!("class `{}` should have a PascalCase name", c.name),
                            c.span.clone(),
                        )
                        .with_help(format!("convert to PascalCase: `{}`", suggested))
                        .with_note(
                            "Datara naming convention enforces PascalCase for types and classes"
                                .into(),
                        ),
                    );
                }
                for item in &c.body_items {
                    if let ClassItem::Method(m) = item {
                        if !is_snake_case(&m.name) {
                            let suggested = to_snake_case(&m.name);
                            diags.push(
                                LintDiagnostic::new(
                                    "style::non_snake_case",
                                    format!("method `{}` should have a snake_case name", m.name),
                                    m.span.clone(),
                                )
                                .with_help(format!("convert to snake_case: `{}`", suggested)),
                            );
                        }
                        if let Some(body) = &m.body {
                            check_body(body, diags);
                        }
                    }
                }
            }

            Decl::Enum(e) => {
                if !is_pascal_case(&e.name) {
                    let suggested = to_pascal_case(&e.name);
                    diags.push(
                        LintDiagnostic::new(
                            "style::non_camel_case_types",
                            format!("enum `{}` should have a PascalCase name", e.name),
                            e.span.clone(),
                        )
                        .with_help(format!("convert to PascalCase: `{}`", suggested)),
                    );
                }
            }

            Decl::Component(c) => {
                if !is_pascal_case(&c.name) {
                    let suggested = to_pascal_case(&c.name);
                    diags.push(
                        LintDiagnostic::new(
                            "style::non_camel_case_types",
                            format!("component `{}` should have a PascalCase name", c.name),
                            c.span.clone(),
                        )
                        .with_help(format!("convert to PascalCase: `{}`", suggested)),
                    );
                }
            }

            Decl::Role(r) => {
                if !is_pascal_case(&r.name) {
                    let suggested = to_pascal_case(&r.name);
                    diags.push(
                        LintDiagnostic::new(
                            "style::non_camel_case_types",
                            format!("role `{}` should have a PascalCase name", r.name),
                            r.span.clone(),
                        )
                        .with_help(format!("convert to PascalCase: `{}`", suggested)),
                    );
                }
            }

            Decl::Packet(p) if !is_pascal_case(&p.name) => {
                let suggested = to_pascal_case(&p.name);
                diags.push(
                    LintDiagnostic::new(
                        "style::non_camel_case_types",
                        format!("packet `{}` should have a PascalCase name", p.name),
                        p.span.clone(),
                    )
                    .with_help(format!("convert to PascalCase: `{}`", suggested)),
                );
            }

            Decl::Trait(t) if !is_pascal_case(&t.name) => {
                let suggested = to_pascal_case(&t.name);
                diags.push(
                    LintDiagnostic::new(
                        "style::non_camel_case_types",
                        format!("trait `{}` should have a PascalCase name", t.name),
                        t.span.clone(),
                    )
                    .with_help(format!("convert to PascalCase: `{}`", suggested)),
                );
            }

            Decl::Impl(i) => {
                for m in &i.methods {
                    check_body(&m.body, diags);
                }
            }

            _ => {}
        }
    }
}

fn check_body(stmt: &Stmt, diags: &mut Vec<LintDiagnostic>) {
    let mut tracker = VariableUsageTracker::new();
    tracker.analyze_stmt(stmt);

    // 1. Check unnecessary mut: declared mut, but never reassigned
    for (name, (span, is_mut)) in &tracker.declared {
        if *is_mut && !tracker.mutated.contains(name) {
            diags.push(
                LintDiagnostic::new(
                    "perf::unnecessary_mut",
                    format!("variable `{}` does not need to be mutable", name),
                    span.clone(),
                )
                .with_help(format!(
                    "remove `mut` to declare an immutable variable: `let {}`",
                    name
                ))
                .with_note(format!(
                    "`{}` is never reassigned after initialization",
                    name
                ))
                .with_fix(format!("let {}", name)),
            );
        }

        // 2. Check unused variables: declared, but never read
        if !tracker.read.contains(name) && !name.starts_with('_') {
            diags.push(
                LintDiagnostic::new(
                    "style::unused_variable",
                    format!("unused variable `{}`", name),
                    span.clone(),
                )
                .with_help(format!(
                    "if this is intentional, prefix with an underscore: `_{}`",
                    name
                ))
                .with_note(format!(
                    "`{}` is defined but its value is never evaluated",
                    name
                )),
            );
        }

        // 3. Check variable naming
        if !is_snake_case(name) {
            let suggested = to_snake_case(name);
            diags.push(
                LintDiagnostic::new(
                    "style::non_snake_case",
                    format!("variable `{}` should have a snake_case name", name),
                    span.clone(),
                )
                .with_help(format!("convert to snake_case: `{}`", suggested))
                .with_note(
                    "Datara naming convention enforces snake_case for local variables".into(),
                ),
            );
        }
    }

    // 4. Recursive structural checks for loops and expressions
    check_stmt_structure(stmt, diags);
}

fn check_stmt_structure(stmt: &Stmt, diags: &mut Vec<LintDiagnostic>) {
    match stmt {
        Stmt::Block(stmts, _) => {
            for s in stmts {
                check_stmt_structure(s, diags);
            }
        }

        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            check_expr_idioms(condition, diags);
            check_stmt_structure(then_branch, diags);
            if let Some(eb) = else_branch {
                check_stmt_structure(eb, diags);
            }
        }

        Stmt::While {
            condition,
            body,
            span,
        } => {
            check_expr_idioms(condition, diags);
            // Check if this while loop is an index loop increment: while i < 100 { ... i = i + 1 }
            if let Expr::Binary { op, left, .. } = condition
                && (op == "<" || op == "<=")
                && let Expr::Identifier(var_name, _) = left.as_ref()
                && body_increments_var(body, var_name)
            {
                diags.push(
                    LintDiagnostic::new(
                        "style::prefer_for_loop",
                        format!("manual while loop index increment for `{}` detected", var_name),
                        span.clone(),
                    )
                    .with_help(format!("use an idiomatic range for-loop: `for {} in 0..N {{ ... }}`", var_name))
                    .with_note("range for-loops are optimized into zero-cost vector loops by Evidence Gate".into()),
                );
            }
            check_stmt_structure(body, diags);
        }

        Stmt::For {
            var_name,
            iterable,
            body,
            span,
        } => {
            if !is_snake_case(var_name) {
                diags.push(
                    LintDiagnostic::new(
                        "style::non_snake_case",
                        format!("loop variable `{}` should have a snake_case name", var_name),
                        span.clone(),
                    )
                    .with_help(format!(
                        "convert to snake_case: `{}`",
                        to_snake_case(var_name)
                    )),
                );
            }
            check_expr_idioms(iterable, diags);
            check_stmt_structure(body, diags);
        }

        Stmt::Assign {
            target: _, value, ..
        } => {
            check_expr_idioms(value, diags);
        }

        Stmt::Let { init, .. } | Stmt::Mut { init, .. } | Stmt::Val { init, .. } => {
            check_expr_idioms(init, diags);
        }

        Stmt::Expr(expr, _) | Stmt::Out(expr, _) | Stmt::Err(expr, _) => {
            check_expr_idioms(expr, diags);
        }

        Stmt::Return(Some(expr), _) => {
            check_expr_idioms(expr, diags);
        }

        _ => {}
    }
}

fn check_expr_idioms(expr: &Expr, diags: &mut Vec<LintDiagnostic>) {
    match expr {
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            // Check bool comparisons like x == true or x == false
            if op == "=="
                && let Expr::Literal(LiteralValue::Bool(b), _) = right.as_ref()
            {
                if *b {
                    diags.push(
                        LintDiagnostic::new(
                            "style::bool_comparison",
                            "redundant comparison with `true`".into(),
                            span.clone(),
                        )
                        .with_help(
                            "simplify condition to evaluate boolean expression directly".into(),
                        ),
                    );
                } else {
                    diags.push(
                        LintDiagnostic::new(
                            "style::bool_comparison",
                            "comparison with `false` can be inverted".into(),
                            span.clone(),
                        )
                        .with_help("simplify condition by prefixing with negation `!`".into()),
                    );
                }
            }

            check_expr_idioms(left, diags);
            check_expr_idioms(right, diags);
        }

        Expr::Unary { expr, .. } => {
            check_expr_idioms(expr, diags);
        }

        Expr::Call { callee, args, .. } => {
            check_expr_idioms(callee, diags);
            for a in args {
                check_expr_idioms(a, diags);
            }
        }

        _ => {}
    }
}

fn body_increments_var(stmt: &Stmt, var: &str) -> bool {
    match stmt {
        Stmt::Block(stmts, _) => stmts.iter().any(|s| body_increments_var(s, var)),
        Stmt::Assign { target, value, .. } => {
            if let Expr::Identifier(name, _) = target
                && name == var
                && let Expr::Binary { op, left, .. } = value
                && op == "+"
                && let Expr::Identifier(l_name, _) = left.as_ref()
            {
                return l_name == var;
            }
            false
        }
        _ => false,
    }
}

struct VariableUsageTracker {
    declared: HashMap<String, (crate::diagnostics::SourceSpan, bool)>, // (span, is_mut)
    mutated: HashSet<String>,
    read: HashSet<String>,
}

impl VariableUsageTracker {
    fn new() -> Self {
        Self {
            declared: HashMap::new(),
            mutated: HashSet::new(),
            read: HashSet::new(),
        }
    }

    fn analyze_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name, init, span, ..
            } => {
                self.declared.insert(name.clone(), (span.clone(), false));
                self.analyze_expr(init);
            }
            Stmt::Mut {
                name, init, span, ..
            } => {
                self.declared.insert(name.clone(), (span.clone(), true));
                self.analyze_expr(init);
            }
            Stmt::Val {
                name,
                init,
                is_mut,
                span,
                ..
            } => {
                self.declared.insert(name.clone(), (span.clone(), *is_mut));
                self.analyze_expr(init);
            }
            Stmt::Assign { target, value, .. } => {
                if let Expr::Identifier(name, _) = target {
                    self.mutated.insert(name.clone());
                } else {
                    self.analyze_expr(target);
                }
                self.analyze_expr(value);
            }
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    self.analyze_stmt(s);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.analyze_expr(condition);
                self.analyze_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.analyze_stmt(eb);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.analyze_expr(condition);
                self.analyze_stmt(body);
            }
            Stmt::For {
                var_name,
                iterable,
                body,
                span,
                ..
            } => {
                self.declared
                    .insert(var_name.clone(), (span.clone(), false));
                self.analyze_expr(iterable);
                self.analyze_stmt(body);
            }
            Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => {
                self.analyze_expr(e);
            }
            Stmt::Return(Some(e), _) => {
                self.analyze_expr(e);
            }
            _ => {}
        }
    }

    fn analyze_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(name, _) => {
                self.read.insert(name.clone());
            }
            Expr::InterpolatedString { expressions, .. } => {
                for e in expressions {
                    self.analyze_expr(e);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.analyze_expr(left);
                self.analyze_expr(right);
            }
            Expr::Unary { expr, .. } => {
                self.analyze_expr(expr);
            }
            Expr::Call { callee, args, .. } => {
                self.analyze_expr(callee);
                for a in args {
                    self.analyze_expr(a);
                }
            }
            Expr::MemberAccess { object, .. } => {
                self.analyze_expr(object);
            }
            Expr::IndexAccess { object, index, .. } => {
                self.analyze_expr(object);
                self.analyze_expr(index);
            }
            Expr::Range { start, end, .. } => {
                self.analyze_expr(start);
                self.analyze_expr(end);
            }
            Expr::Tuple(exprs, _) | Expr::ListLiteral(exprs, _) => {
                for e in exprs {
                    self.analyze_expr(e);
                }
            }
            Expr::ObjectInit { fields, .. } => {
                for (_, field_expr) in fields {
                    self.analyze_expr(field_expr);
                }
            }
            Expr::MapLiteral(entries, _) => {
                for (k, v) in entries {
                    self.analyze_expr(k);
                    self.analyze_expr(v);
                }
            }
            Expr::Pipeline { stages, .. } => {
                for s in stages {
                    self.analyze_expr(s);
                }
            }
            Expr::Decide { arms, else_arm, .. } => {
                for arm in arms {
                    self.analyze_expr(&arm.condition);
                    self.analyze_expr(&arm.body);
                }
                if let Some(ea) = else_arm {
                    self.analyze_expr(ea);
                }
            }
            Expr::Match { value, arms, .. } => {
                self.analyze_expr(value);
                for arm in arms {
                    self.analyze_expr(&arm.body);
                }
            }
            Expr::Select { arms, else_arm, .. } => {
                for arm in arms {
                    self.analyze_expr(&arm.body);
                }
                if let Some(ea) = else_arm {
                    self.analyze_expr(ea);
                }
            }
            Expr::Lambda { body, .. } => {
                self.analyze_expr(body);
            }
            Expr::ErrorPropagate(e, _)
            | Expr::Comptime { expr: e, .. }
            | Expr::Wrapping(e, _)
            | Expr::Saturating(e, _)
            | Expr::Cast { expr: e, .. } => {
                self.analyze_expr(e);
            }
            Expr::OrRecovery { expr, arms, .. } => {
                self.analyze_expr(expr);
                for arm in arms {
                    self.analyze_expr(&arm.body);
                }
            }
            Expr::ArrayRepeatLiteral { elem, .. } => {
                self.analyze_expr(elem);
            }
            Expr::Block(stmts, trailing, _) => {
                for s in stmts {
                    self.analyze_stmt(s);
                }
                if let Some(t) = trailing {
                    self.analyze_expr(t);
                }
            }
            Expr::Literal(..) => {}
        }
    }
}
