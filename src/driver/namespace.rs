//! v1.3.3 module-namespace helpers (split out of modules.rs to keep the
//! 60 KB source-size budget): ambiguity detection for merged declarations.

use super::super::ast::Decl;
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use std::path::Path;

/// Report an ambiguous import when two distinct modules export the same
/// top-level name. Re-exports of the same underlying file through diamond
/// imports stay silent; two different files exporting the same name make
/// any unqualified use of that name undefined, which used to be resolved
/// first-wins silently — that hid real bugs.
pub(super) fn check_ambiguous_import(
    program_decls: &[Decl],
    new_decl: &Decl,
    module_file: &Path,
    diag: &mut DiagnosticEngine,
) {
    let (dup_name, dup_span) = match new_decl {
        Decl::Class(c) => (c.name.clone(), Some(&c.span)),
        Decl::Enum(e) => (e.name.clone(), Some(&e.span)),
        Decl::Function(f) => (f.name.clone(), Some(&f.span)),
        Decl::Component(c) => (c.name.clone(), Some(&c.span)),
        Decl::Role(r) => (r.name.clone(), Some(&r.span)),
        Decl::Trait(t) => (t.name.clone(), Some(&t.span)),
        Decl::Type(td) => (td.name.clone(), Some(&td.span)),
        _ => return,
    };

    fn decl_file<'d>(d: &'d Decl, dup_name: &str) -> Option<&'d String> {
        match d {
            Decl::Class(c) if c.name == dup_name => Some(&c.span.file),
            Decl::Enum(e) if e.name == dup_name => Some(&e.span.file),
            Decl::Function(f) if f.name == dup_name => Some(&f.span.file),
            Decl::Component(c) if c.name == dup_name => Some(&c.span.file),
            Decl::Role(r) if r.name == dup_name => Some(&r.span.file),
            Decl::Trait(t) if t.name == dup_name => Some(&t.span.file),
            Decl::Type(td) if td.name == dup_name => Some(&td.span.file),
            _ => None,
        }
    }

    let existing_file = program_decls.iter().find_map(|d| decl_file(d, &dup_name));
    let new_file: &str = &new_decl.span().file;
    let same_origin = existing_file
        .map(|ef| crate::diagnostics::is_same_file_or_module(ef, new_file))
        .unwrap_or(false);
    if same_origin {
        return;
    }

    let origin = existing_file.map(|f| f.as_str()).unwrap_or("<current>");
    diag.error(
        ErrorCode::ResolveDuplicateSymbol,
        format!(
            "Ambiguous import: '{}' is exported by two different modules ({} and {}); qualify the name or rename one export",
            dup_name, origin, module_file.display()
        ),
        dup_span.cloned(),
    );
}
