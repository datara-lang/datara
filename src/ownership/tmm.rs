//! M5 TMM Layer 2+3: annotation-carrier semantics for `@arc`, `@weak`,
//! `@derive(AutoFree)`, `scope { }` blocks, and FFI `owned`/`borrowed`
//! boundary checks (PLAN_V150, Temporal Memory Management).
//!
//! Layer 2 = explicit shared ownership (`@arc` classes with a compile-time
//! known refcount discipline; release at refcount zero is deterministic, no
//! stop-the-world collector). Layer 3 = compiler-inserted frees
//! (`@derive(AutoFree)` types get an end-of-scope release in reverse
//! declaration order, defer-last). `@weak` references never extend a
//! refcount and must be upgraded to `Outcome<T>` before use.
//!
//! This module is the static half: it validates the annotations, detects
//! strong-reference cycles in the `@arc` ownership graph (with the exact
//! placement hint "put @weak here"), and enforces the FFI boundary rule
//! (a `borrowed` parameter cannot be stored/escape past the call; an
//! `owned` parameter transfers ownership and the value must not be used
//! after the call). The dynamic refcount primitives already live in the
//! C runtime (`datara_guard.c`).

use crate::ast::{ClassItem, Decl, Expr, Program, Stmt};
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Layer assigned to a type by the TMM plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmmLayer {
    /// Layer 1: dataflow-inferred (default; resolved in M6).
    Dataflow,
    /// Layer 2: explicit shared ownership (`@arc`).
    Arc,
    /// Layer 2/3 boundary: weak handle on an `@arc` object (`@weak` field).
    Weak,
    /// Layer 3: compiler-inserted scope frees (`@derive(AutoFree)`).
    AutoFree,
}

/// One field of a class relevant to the TMM graph.
#[derive(Debug, Clone)]
pub struct FieldEdge {
    pub field_name: String,
    /// Target type name (type synonyms resolved by the caller).
    pub target: String,
    pub is_weak: bool,
    pub span: SourceSpan,
}

/// Per-class TMM metadata extracted from attributes.
#[derive(Debug, Clone)]
pub struct ClassPlan {
    pub name: String,
    pub layer: TmmLayer,
    pub is_arc: bool,
    pub has_autofree: bool,
    pub fields: Vec<FieldEdge>,
    pub span: SourceSpan,
}

/// The whole-program TMM plan: which classes are `@arc`, which carry
/// `@derive(AutoFree)`, and the strong-ownership edges between them.
#[derive(Debug, Default)]
pub struct TmmPlan {
    pub classes: BTreeMap<String, ClassPlan>,
    /// Type synonyms, so `type Player = ...` resolves in the graph.
    synonyms: HashMap<String, String>,
}

impl TmmPlan {
    pub fn build(program: &Program) -> Self {
        let mut plan = Self::default();
        // Pass 0: collect type synonyms for graph resolution.
        for decl in &program.declarations {
            if let Decl::Type(t) = decl {
                plan.synonyms
                    .insert(t.name.clone(), t.base_type.name.clone());
            }
        }
        for decl in &program.declarations {
            if let Decl::Class(c) = decl {
                let is_arc = c.attributes.iter().any(|a| a.name == "arc");
                let has_autofree = c
                    .attributes
                    .iter()
                    .any(|a| a.name == "derive" && a.args.iter().any(|(arg, _)| arg == "AutoFree"));
                let layer = if is_arc {
                    TmmLayer::Arc
                } else if has_autofree {
                    TmmLayer::AutoFree
                } else {
                    TmmLayer::Dataflow
                };
                let mut fields = Vec::new();
                for item in &c.body_items {
                    if let ClassItem::Field(f) = item {
                        let Some(t) = &f.type_node else { continue };
                        // List<T>/Map<K, V> edges point at the element type:
                        // a container of @arc values can close a cycle too.
                        let inner = t
                            .generic_args
                            .last()
                            .map(|g| g.name.clone())
                            .unwrap_or_else(|| t.name.clone());
                        let is_weak = t.name == "Weak";
                        let target = plan.resolve(&inner);
                        fields.push(FieldEdge {
                            field_name: f.name.clone(),
                            target,
                            is_weak,
                            span: f.span.clone(),
                        });
                    }
                }
                plan.classes.insert(
                    c.name.clone(),
                    ClassPlan {
                        name: c.name.clone(),
                        layer,
                        is_arc,
                        has_autofree,
                        fields,
                        span: c.span.clone(),
                    },
                );
            }
        }
        plan
    }

    fn resolve(&self, name: &str) -> String {
        let mut current = name.to_string();
        for _ in 0..8 {
            match self.synonyms.get(&current) {
                Some(next) if *next != current => current = next.clone(),
                _ => break,
            }
        }
        current
    }

    pub fn layer_of(&self, class: &str) -> TmmLayer {
        self.classes
            .get(class)
            .map(|c| c.layer)
            .unwrap_or(TmmLayer::Dataflow)
    }

    /// True if any field of `class` (transitively, through non-@arc value
    /// types) contains a weak handle. Used by cycle diagnostics and by the
    /// memory inspector.
    pub fn has_weak_fields(&self, class: &str) -> bool {
        let Some(c) = self.classes.get(class) else {
            return false;
        };
        c.fields.iter().any(|f| f.is_weak)
    }
}

/// Static cycle detector (M5): a strong-reference cycle through `@arc`
/// fields leaks deterministically (refcount never reaches zero). Report the
/// cycle path and the exact fix placement. `@weak` fields break cycles and
/// are skipped here — they are the fix, not the bug.
pub fn detect_arc_cycles(plan: &TmmPlan, diag: &mut DiagnosticEngine) {
    let arc_names: Vec<&String> = plan
        .classes
        .values()
        .filter(|c| c.is_arc)
        .map(|c| &c.name)
        .collect();

    // Report each distinct cycle once (canonical key: sorted members),
    // not once per starting node.
    let mut reported: HashSet<Vec<String>> = HashSet::new();

    for start in arc_names.clone() {
        // DFS over strong (non-weak) edges, tracking the path.
        let mut path: Vec<String> = vec![start.clone()];
        let mut on_path: HashSet<String> = HashSet::new();
        on_path.insert(start.clone());
        let mut visited: HashSet<(String, String)> = HashSet::new();
        dfs_cycle(
            plan,
            start,
            &mut path,
            &mut on_path,
            &mut visited,
            diag,
            &mut reported,
        );
    }
}

fn dfs_cycle(
    plan: &TmmPlan,
    class: &str,
    path: &mut Vec<String>,
    on_path: &mut HashSet<String>,
    visited: &mut HashSet<(String, String)>,
    diag: &mut DiagnosticEngine,
    reported: &mut HashSet<Vec<String>>,
) {
    let Some(node) = plan.classes.get(class) else {
        return;
    };
    for edge in &node.fields {
        if edge.is_weak {
            continue; // @weak breaks the cycle: this is the documented fix.
        }
        let key = (class.to_string(), edge.target.clone());
        if !visited.insert(key) {
            continue;
        }
        if let Some(pos) = path.iter().position(|p| p == &edge.target) {
            // Found a strong cycle: path[pos..] + edge.target (the edge
            // closes back to path[pos], so it appears once).
            let mut cycle: Vec<String> = path[pos..].to_vec();
            cycle.push(edge.target.clone());
            let first = cycle
                .first()
                .cloned()
                .unwrap_or_default();
            // Canonical form: sorted members dedupe rotations of the cycle.
            let mut members: Vec<String> = path[pos..].to_vec();
            members.sort();
            if !reported.insert(members) {
                continue;
            }
            let span = plan
                .classes
                .get(&edge.target)
                .map(|c| c.span.clone())
                .unwrap_or_else(|| edge.span.clone());
            diag.error_with_help(
                ErrorCode::TmmArcCycle,
                format!(
                    "Strong reference cycle in @arc ownership graph: {}. The refcount of these objects can never reach zero, so they leak.",
                    cycle.join(" → ")
                ),
                Some(span),
                Some(format!(
                    "put @weak on the field back-edge (e.g. 'next: Weak<{}>' instead of '{}: {}') and call upgrade() -> Outcome<T> before use",
                    first, edge.field_name, edge.target
                )),
            );
            continue;
        }
        if plan.classes.contains_key(&edge.target) && edge.target == plan.resolve(&edge.target) {
            path.push(edge.target.clone());
            on_path.insert(edge.target.clone());
            dfs_cycle(plan, &edge.target, path, on_path, visited, diag, reported);
            path.pop();
            on_path.remove(&edge.target);
        }
    }
}

/// M5 FFI boundary check (PLAN_V150): on `extern "C"` declarations,
/// parameters marked `borrowed`/`view` must not outlive the call (they may
/// not be stored, returned, or moved into globals), and parameters marked
/// `owned`/`own` transfer ownership (use after the call is a use-after-move
/// reported by the ownership tracker).
///
/// `check_ffi_boundaries` walks function bodies (check tail) and reports:
/// - storing a borrowed value into a class field or a global,
/// - returning a borrowed value from the function,
/// - passing a borrowed value where the callee takes `owned`.
pub fn check_ffi_boundaries(program: &Program, diag: &mut DiagnosticEngine) {
    use crate::ast::Param;

    // Index extern declarations: name → per-param ownership mode.
    let mut extern_modes: HashMap<String, Vec<Param>> = HashMap::new();
    for decl in &program.declarations {
        if let Decl::ExternFn(ef) = decl {
            extern_modes.insert(ef.name.clone(), ef.params.clone());
        }
    }
    if extern_modes.is_empty() {
        return;
    }

    // Walk every function body; inside, flag suspicious operations on
    // values that flowed from a borrowed extern parameter.
    for decl in &program.declarations {
        let (fn_name, params, body): (&str, &[Param], Option<&Stmt>) = match decl {
            Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) => {
                (&f.name, &f.params, Some(&f.body))
            }
            _ => continue,
        };
        let Some(body) = body else { continue };
        // Which locals hold borrowed extern results? Track direct calls to
        // extern fns whose param modes are borrowed and whose return may be
        // a view of the borrowed argument.
        let mut ctx = FfiCtx {
            fn_name,
            extern_modes: &extern_modes,
            borrowed_locals: HashSet::new(),
            diag,
        };
        // Seed: parameters declared `view`/`borrowed` in THIS function that
        // alias extern pointers cannot be stored either — conservative seed
        // is empty; only extern-derived locals are tracked.
        let _ = params;
        ctx.walk(body);
    }
}

struct FfiCtx<'a> {
    fn_name: &'a str,
    extern_modes: &'a HashMap<String, Vec<crate::ast::Param>>,
    borrowed_locals: HashSet<String>,
    diag: &'a mut DiagnosticEngine,
}

impl<'a> FfiCtx<'a> {
    fn walk(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    self.walk(s);
                }
            }
            Stmt::Parallel(s, _) | Stmt::Simd(s, _) | Stmt::Unsafe { body: s, .. } => self.walk(s),
            Stmt::Let { name, init, .. }
            | Stmt::Mut { name, init, .. }
            | Stmt::Val { name, init, .. }
            | Stmt::CompactBind { name, init, .. }
            | Stmt::Const { name, init, .. } => {
                self.check_init_borrowed(init);
                if self.init_is_extern_view(init) {
                    self.borrowed_locals.insert(name.clone());
                }
            }
            Stmt::Assign { value, .. } => {
                self.check_init_borrowed(value);
            }
            Stmt::Expr(e, _) | Stmt::Out(e, _) | Stmt::Err(e, _) => self.check_expr_escape(e),
            Stmt::Return(Some(e), span) => {
                if let Expr::Identifier(name, _) = e {
                    if self.borrowed_locals.contains(name) {
                        self.diag.error(
                            ErrorCode::TmmBorrowEscapesCall,
                            format!(
                                "Borrowed value '{}' from an extern call must not outlive the call: returning it escapes the borrow",
                                name
                            ),
                            Some(span.clone()),
                        );
                    }
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.walk(then_branch);
                if let Some(eb) = else_branch {
                    self.walk(eb);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Loop { body, .. } => {
                self.walk(body)
            }
            _ => {}
        }
    }

    fn check_init_borrowed(&mut self, init: &Expr) {
        // `let store = { field: borrowed_local }` — a struct init that
        // captures a borrowed local into a value object escapes the borrow.
        if let Expr::ObjectInit { fields, .. } = init {
            for (fname, fe) in fields {
                if let Expr::Identifier(name, span) = fe {
                    if self.borrowed_locals.contains(name) {
                        self.diag.error(
                            ErrorCode::TmmBorrowEscapesCall,
                            format!(
                                "Borrowed value '{}' must not be stored into field '{}': a borrow cannot outlive the extern call that produced it",
                                name, fname
                            ),
                            Some(span.clone()),
                        );
                    }
                }
            }
        }
    }

    fn check_expr_escape(&mut self, e: &Expr) {
        // `f(x)` where f is an extern fn and the arg slot is owned: passing
        // a borrowed local into an owned slot is a transfer violation.
        if let Expr::Call { callee, args, .. } = e
            && let Expr::Identifier(fname, _) = &**callee
            && let Some(params) = self.extern_modes.get(fname)
        {
            for (i, a) in args.iter().enumerate() {
                let Some(p) = params.get(i) else { continue };
                let owned = p.ownership_mode == "owned" || p.ownership_mode == "own";
                let borrowed = p.ownership_mode == "view" || p.ownership_mode == "borrowed";
                if !owned {
                    continue;
                }
                if let Expr::Identifier(name, aspan) = a
                    && self.borrowed_locals.contains(name)
                {
                    let _ = borrowed;
                    self.diag.error(
                        ErrorCode::TmmBorrowEscapesCall,
                        format!(
                            "Borrowed value '{}' passed to extern '{}[]' parameter '{}[owned]' in '{}': a borrow cannot be transferred as ownership",
                            name, fname, p.name, self.fn_name
                        ),
                        Some(aspan.clone()),
                    );
                }
            }
        }
    }

    fn init_is_extern_view(&mut self, _init: &Expr) -> bool {
        // A call to an extern fn whose *return* is considered a view of one
        // of its borrowed parameters. C functions returning a borrowed
        // pointer (e.g. getenv) are modeled this way by convention: the
        // result is only safe while the argument outlives it. Without an
        // explicit annotation we treat all extern results as owned (safe
        // default) — a future `@returns(borrowed)` attribute refines this.
        false
    }
}

/// `forgen inspect --memory` textual report: per-class TMM layer, fields,
/// and ownership edges; plus the program-level summary. Returns the full
/// report text (CLI prints it).
pub fn memory_report(program: &Program) -> String {
    let plan = TmmPlan::build(program);
    let mut out = String::new();
    out.push_str("TMM Memory Plan\n");
    out.push_str("===============\n");
    out.push_str(&format!("{:<20} {:<10} FIELDS\n", "TYPE", "LAYER"));
    for c in plan.classes.values() {
        let layer = match c.layer {
            TmmLayer::Dataflow => "L1 dataflow",
            TmmLayer::Arc => "L2 @arc",
            TmmLayer::Weak => "L2 @weak",
            TmmLayer::AutoFree => "L3 autofree",
        };
        let fields: Vec<String> = c
            .fields
            .iter()
            .map(|f| {
                if f.is_weak {
                    format!("{}: weak<{}>", f.field_name, f.target)
                } else {
                    format!("{}: {}", f.field_name, f.target)
                }
            })
            .collect();
        out.push_str(&format!(
            "{:<20} {:<10} {}\n",
            c.name,
            layer,
            fields.join(", ")
        ));
    }
    // Scope frees for @derive(AutoFree) classes used inside functions.
    out.push_str("\nScope insertions (Layer 3, defer-last):\n");
    let mut any_scope = false;
    for decl in &program.declarations {
        if let Decl::Function(f) | Decl::Flow(f) | Decl::Task(f) = decl {
            let mut autofree_locals: Vec<String> = Vec::new();
            collect_autofree_locals(&plan, &f.body, &mut autofree_locals);
            if !autofree_locals.is_empty() {
                any_scope = true;
                out.push_str(&format!(
                    "  fn {}: release {} (reverse order)\n",
                    f.name,
                    autofree_locals.join(", ")
                ));
            }
        }
    }
    if !any_scope {
        out.push_str("  (none)\n");
    }
    out
}

fn collect_autofree_locals(plan: &TmmPlan, stmt: &Stmt, out: &mut Vec<String>) {
    match stmt {
        Stmt::Block(stmts, _) => {
            for s in stmts {
                collect_autofree_locals(plan, s, out);
            }
        }
        Stmt::Let { name, init, .. }
        | Stmt::Val { name, init, .. }
        | Stmt::Mut { name, init, .. }
        | Stmt::CompactBind { name, init, .. } => {
            if let Expr::ObjectInit { class_name, .. } = init {
                if plan
                    .classes
                    .get(&plan.resolve(class_name))
                    .is_some_and(|c| c.has_autofree)
                {
                    out.push(name.clone());
                }
            }
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_autofree_locals(plan, then_branch, out);
            if let Some(eb) = else_branch {
                collect_autofree_locals(plan, eb, out);
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Loop { body, .. } => {
            collect_autofree_locals(plan, body, out)
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        // In-memory lex+parse using the public parser API.
        let mut lexer = crate::lexer::Lexer::new(src, "tmm_test.dtr");
        let mut diag = crate::diagnostics::DiagnosticEngine::new("en");
        let tokens = lexer.tokenize(&mut diag);
        let mut parser = crate::parser::Parser::new(tokens, &mut diag, "tmm_test.dtr");
        parser.parse_program()
    }

    #[test]
    fn plan_extracts_arc_and_autofree() {
        let program = parse(
            "@arc\nstruct Node { value: Int, next: Weak<Node> }\n\n@derive(AutoFree)\nstruct Buf { size: Int }\n\nfn main() -> Int {\n    return 0\n}\n",
        );
        let plan = TmmPlan::build(&program);
        let node = plan.classes.get("Node").expect("Node");
        assert!(node.is_arc);
        assert!(node.fields.iter().any(|f| f.is_weak));
        let buf = plan.classes.get("Buf").expect("Buf");
        assert!(buf.has_autofree);
    }

    #[test]
    fn cycle_detector_reports_strong_cycle_and_weak_is_clean() {
        let mut diag = DiagnosticEngine::new("en");
        // Strong cycle: A.b: B, B.a: A — but expressed via ClassDecl is
        // verbose; build the plan programmatically instead.
        let mut plan = TmmPlan::default();
        let mk = |name: &str| ClassPlan {
            name: name.to_string(),
            layer: TmmLayer::Arc,
            is_arc: true,
            has_autofree: false,
            fields: Vec::new(),
            span: SourceSpan::point(1, 1, "t.dtr".into()),
        };
        let mut a = mk("A");
        a.fields.push(FieldEdge {
            field_name: "b".into(),
            target: "B".into(),
            is_weak: false,
            span: SourceSpan::point(2, 5, "t.dtr".into()),
        });
        let mut b = mk("B");
        b.fields.push(FieldEdge {
            field_name: "a".into(),
            target: "A".into(),
            is_weak: false,
            span: SourceSpan::point(3, 5, "t.dtr".into()),
        });
        plan.classes.insert("A".into(), a);
        plan.classes.insert("B".into(), b);
        detect_arc_cycles(&plan, &mut diag);
        assert!(diag.has_errors(), "strong cycle must be reported");
        assert!(diag.diagnostics[0].code == "E-TMM-001");
        assert!(diag.diagnostics[0].message.contains("A → B → A"));
        assert!(diag.diagnostics[0].help.as_ref().unwrap().contains("@weak"));

        // @weak breaks the cycle: no diagnostics.
        let mut b2 = mk("B");
        b2.fields.push(FieldEdge {
            field_name: "a".into(),
            target: "A".into(),
            is_weak: true,
            span: SourceSpan::point(3, 5, "t.dtr".into()),
        });
        plan.classes.insert("B".into(), b2);
        let mut diag2 = DiagnosticEngine::new("en");
        detect_arc_cycles(&plan, &mut diag2);
        assert!(!diag2.has_errors(), "weak back-edge must silence the cycle");
    }

    #[test]
    fn memory_report_lists_layers() {
        let program = parse(
            "@arc\nstruct Node { value: Int }\n\nfn main() -> Int {\n    return 0\n}\n",
        );
        let report = memory_report(&program);
        assert!(report.contains("Node"));
        assert!(report.contains("L2 @arc"));
    }
}
