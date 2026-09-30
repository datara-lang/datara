//! M4 Large-Program Stability: symbol namespace, interning, mangling, and
//! the link-phase duplicate check with `E-LINK-001` diagnostics (PLAN_V150).
//!
//! `lower_program` inserts into `module.functions` (a `HashMap`) via
//! `insert()`, which *silently overwrites* on a name collision. Before the
//! interner, two distinct declarations that mangle to the same symbol
//! (e.g. a top-level `fn Foo_bar` and the method `bar` of class `Foo`, both
//! becoming `Foo_bar`) lost one definition and produced a wrong program or
//! an opaque backend failure. The `SymbolTable` records every insertion and
//! `check_module` turns the collisions into precise, source-anchored
//! diagnostics before codegen runs.

use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::dmir::ir::Module;
use std::collections::HashMap;

/// Interned symbol ID (M4): stable small integer for a symbol name.
/// Construction goes through [`Interner::intern`]; IDs are dense per
/// `Interner` instance, so a `u32` is sufficient for any single module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(u32);

impl SymbolId {
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// String interner: repeated symbol names (very common in large programs —
/// `main`, method names, mangled specializations) map to one `SymbolId` and
/// one allocation. Lookup is O(1); conversion back to text is O(1).
#[derive(Debug, Default)]
pub struct Interner {
    map: HashMap<String, SymbolId>,
    strings: Vec<String>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            map: HashMap::with_capacity(cap),
            strings: Vec::with_capacity(cap),
        }
    }

    /// Get (or allocate) the ID for `name`.
    pub fn intern(&mut self, name: &str) -> SymbolId {
        if let Some(&id) = self.map.get(name) {
            return id;
        }
        let id = SymbolId(self.strings.len() as u32);
        self.strings.push(name.to_string());
        self.map.insert(name.to_string(), id);
        id
    }

    /// Resolve an ID back to its name. Always succeeds for IDs produced by
    /// this interner; an out-of-range ID returns `None` (defensive).
    pub fn resolve(&self, id: SymbolId) -> Option<&str> {
        self.strings.get(id.0 as usize).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

/// One definition site of a symbol in the module being linked.
#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub id: SymbolId,
    pub name: String,
    pub span: Option<SourceSpan>,
    /// Kind used for the diagnostic wording (`function`, `method`, `global`,
    /// `extern`). Free-form: purely presentational.
    pub kind: &'static str,
}

/// Records symbol definitions during lowering and detects collisions.
/// Mirrors the insert calls in `lower_program` so a collision here is a
/// collision at link time.
#[derive(Debug, Default)]
pub struct SymbolTable {
    /// symbol name → every definition site, in insertion order.
    defs: HashMap<String, Vec<SymbolDef>>,
    interner: Interner,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    /// Record one definition of `name`. Returns the definition's symbol id.
    pub fn define(
        &mut self,
        name: &str,
        kind: &'static str,
        span: Option<SourceSpan>,
    ) -> SymbolId {
        let id = self.interner.intern(name);
        self.defs.entry(name.to_string()).or_default().push(SymbolDef {
            id,
            name: name.to_string(),
            span,
            kind,
        });
        id
    }

    /// Symbols with more than one definition, sorted by name for
    /// deterministic diagnostics.
    pub fn duplicates(&self) -> Vec<(&str, &Vec<SymbolDef>)> {
        let mut dups: Vec<(&str, &Vec<SymbolDef>)> = self
            .defs
            .iter()
            .filter(|(_, defs)| defs.len() > 1)
            .map(|(name, defs)| (name.as_str(), defs))
            .collect();
        dups.sort_by_key(|(name, _)| (*name).to_string());
        dups
    }
}

/// v1.5.0 mangling (M4): the canonical mangled form for a method or impl
/// function on `type_name`, and for a generic specialization with concrete
/// type arguments. Used by lowering for method symbols and by tests to
/// assert the scheme.
pub fn mangle_method(type_name: &str, method_name: &str) -> String {
    format!("{}_{}", type_name, method_name)
}

pub fn mangle_specialization(fn_name: &str, type_args: &[&str]) -> String {
    let mut out = String::with_capacity(fn_name.len() + 1 + type_args.iter().map(|s| s.len() + 1).sum::<usize>());
    out.push_str(fn_name);
    for arg in type_args {
        out.push('_');
        out.push_str(arg);
    }
    out
}

/// Link-phase duplicate check (M4, PLAN_V150 "проверка дублей в link-фазе
/// с E-LINK-диагностикой"): report every symbol that has more than one
/// definition with a precise E-LINK-001 error anchored at the *second and
/// later* definition site (the first definition is named in the message).
///
/// `function_spans` provides fallback spans when the definition was recorded
/// without one. Returns the number of diagnostics emitted.
pub fn check_module_link(
    module: &Module,
    table: &SymbolTable,
    diag: &mut DiagnosticEngine,
) -> usize {
    let mut reported = 0usize;
    for (name, defs) in table.duplicates() {
        // Self-alias definitions are legal: `lower_behavior` inserts the same
        // lowered body under `Type_method` and `B_Type_method`, and
        // `lower_impl` may insert a trait-dispatch alias. Those duplicates
        // share the same definition *file* and are intentional aliases; only
        // report when the duplicate definitions come from genuinely distinct
        // declarations (different spans in the same file, or different files
        // for a non-module-qualified symbol).
        let first = &defs[0];
        for dup in &defs[1..] {
            if is_intentional_alias(first, dup) {
                continue;
            }
            let first_loc = describe_span(first, module);
            let span = dup
                .span
                .clone()
                .or_else(|| module.function_spans.get(name).cloned());
            let help = format!(
                "Every symbol must mangle to a unique link name. If this is a method, rename the method or the colliding top-level function so the '{}' prefix keeps it unique.",
                if dup.kind == "method" || first.kind == "method" { "<type>" } else { "top-level" }
            );
            diag.error_with_help(
                ErrorCode::LinkDuplicateSymbol,
                format!(
                    "[E-LINK-001] Duplicate symbol '{}' in link phase: defined as {} here and already defined as {} at {}. Rename one definition or qualify the name.",
                    name, dup.kind, first.kind, first_loc
                ),
                span,
                Some(help),
            );
            reported += 1;
        }
    }
    reported
}

fn is_intentional_alias(a: &SymbolDef, b: &SymbolDef) -> bool {
    // Alias inserts always carry the same span (the same method decl) or no
    // spans at all; genuine collisions carry distinct spans.
    match (&a.span, &b.span) {
        (Some(sa), Some(sb)) => {
            sa.file == sb.file
                && sa.start_line == sb.start_line
                && sa.start_col == sb.start_col
                && sa.end_line == sb.end_line
                && sa.end_col == sb.end_col
        }
        (None, None) => true,
        _ => false,
    }
}

fn describe_span(def: &SymbolDef, module: &Module) -> String {
    let span = def
        .span
        .as_ref()
        .or_else(|| module.function_spans.get(&def.name));
    match span {
        Some(s) => format!("{}:{}", s.file, s.start_line),
        None => "<unknown location>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interner_roundtrip_and_dedup() {
        let mut i = Interner::new();
        let a = i.intern("foo");
        let b = i.intern("foo");
        let c = i.intern("bar");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(i.resolve(a), Some("foo"));
        assert_eq!(i.resolve(c), Some("bar"));
        assert_eq!(i.len(), 2);
    }

    #[test]
    fn symbol_table_detects_duplicates() {
        let mut t = SymbolTable::new();
        t.define("Foo_bar", "function", Some(SourceSpan::point(1, 1, "a.dtr".into())));
        t.define("Foo_bar", "method", Some(SourceSpan::point(9, 5, "b.dtr".into())));
        let dups = t.duplicates();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].0, "Foo_bar");
        assert_eq!(dups[0].1.len(), 2);
    }

    #[test]
    fn same_span_definitions_are_alias_not_collision() {
        use crate::diagnostics::DiagnosticEngine;
        use crate::dmir::ir::Module;

        // Alias re-insert (same name, same span — the `B_` self-alias and
        // inherited-method paths in lowering) must NOT produce a diagnostic;
        // a genuine collision (same name, distinct spans) must produce one.
        let mut t = SymbolTable::new();
        let span = SourceSpan::point(3, 5, "m.dtr".into());
        t.define("Type_m", "method", Some(span.clone()));
        t.define("B_Type_m", "method", Some(span.clone())); // different name
        t.define("Type_m", "method", Some(span)); // same name + same span

        let mut diag = DiagnosticEngine::new("en");
        let module = Module::new("t");
        assert_eq!(check_module_link(&module, &t, &mut diag), 0);
        assert!(!diag.has_errors());

        // Different spans for the same name = real collision.
        t.define("Type_m", "method", Some(SourceSpan::point(30, 1, "m.dtr".into())));
        let mut diag2 = DiagnosticEngine::new("en");
        assert_eq!(check_module_link(&module, &t, &mut diag2), 1);
        assert!(diag2.has_errors());
        assert!(diag2.diagnostics[0].code == "E-LINK-001");
        assert!(diag2.diagnostics[0].message.contains("Type_m"));
    }

    #[test]
    fn mangling_scheme() {
        assert_eq!(mangle_method("Vec2", "len"), "Vec2_len");
        assert_eq!(mangle_specialization("pair_swap", &["Int", "Str"]), "pair_swap_Int_Str");
    }
}
