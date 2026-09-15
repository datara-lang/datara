pub mod lexer;
pub mod parser;
pub mod types;

use crate::ast::*;
use crate::cimport::lexer::CLexer;
use crate::cimport::parser::CParser;
use crate::cimport::types::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use std::path::{Path, PathBuf};

pub fn expand_c_imports(
    program: &mut Program,
    base_dir: Option<&Path>,
    diag: &mut DiagnosticEngine,
    allow_sret_returns: bool,
) {
    let mut new_declarations = Vec::new();
    let mut extra_libs = Vec::new();

    for decl in program.declarations.drain(..) {
        match decl {
            Decl::CImport(cimport) => {
                // Record link libraries
                for lib in &cimport.link_libs {
                    if !extra_libs.contains(lib) {
                        extra_libs.push(lib.clone());
                    }
                }

                // Resolve header path
                let header_path_buf = PathBuf::from(&cimport.header_path);
                let resolved_path = if header_path_buf.is_absolute() && header_path_buf.exists() {
                    Some(header_path_buf.clone())
                } else if let Some(base) = base_dir {
                    let cand = base.join(&cimport.header_path);
                    if cand.exists() { Some(cand) } else { None }
                } else {
                    None
                };

                let resolved_path = resolved_path.or_else(|| {
                    if let Ok(cwd) = std::env::current_dir() {
                        let cand = cwd.join(&cimport.header_path);
                        if cand.exists() {
                            return Some(cand);
                        }
                    }
                    if header_path_buf.exists() {
                        return Some(header_path_buf.clone());
                    }
                    None
                });

                let header_file_path = match resolved_path {
                    Some(p) => p,
                    None => {
                        diag.error(
                            ErrorCode::CImportHeaderNotFound,
                            format!(
                                "C header file '{}' not found (searched relative to source directory and working directory)",
                                cimport.header_path
                            ),
                            Some(cimport.span.clone()),
                        );
                        continue;
                    }
                };

                let header_content = match std::fs::read_to_string(&header_file_path) {
                    Ok(s) => s,
                    Err(e) => {
                        diag.error(
                            ErrorCode::CImportReadFailed,
                            format!(
                                "Failed to read C header file '{}': {}",
                                header_file_path.display(),
                                e
                            ),
                            Some(cimport.span.clone()),
                        );
                        continue;
                    }
                };

                let header_str_path = header_file_path.to_string_lossy().to_string();
                let mut lexer = CLexer::new(&header_content);
                let tokens = lexer.tokenize();
                let mut parser = CParser::new(tokens);
                let c_decls = parser.parse();

                // Compute the memory layout of all parsed structs so
                // functions returning aggregates can be classified against
                // the native calling convention: one machine word or less
                // comes back in RAX as raw bits, larger layout-compatible
                // aggregates come back through the hidden sret return slot,
                // and everything else is rejected at compile time. The gate
                // must never let an unsupported shape through to codegen:
                // emitting a call to such a function crashes at run time.
                let struct_layouts = compute_struct_layouts(&c_decls);

                for parser_diag in parser.diagnostics {
                    diag.warning(
                        ErrorCode::CImportUnsupportedConstruct,
                        parser_diag,
                        Some(cimport.span.clone()),
                    );
                }

                for c_decl in c_decls {
                    match c_decl {
                        CDecl::Function(cf) => {
                            let f_span = SourceSpan::new(
                                cf.line,
                                cf.col,
                                cf.line,
                                cf.col + cf.name.len(),
                                header_str_path.clone(),
                            );
                            let params: Vec<Param> = cf
                                .params
                                .iter()
                                .map(|p| {
                                    let p_span = SourceSpan::new(
                                        p.line,
                                        p.col,
                                        p.line,
                                        p.col + p.name.len(),
                                        header_str_path.clone(),
                                    );
                                    Param {
                                        name: p.name.clone(),
                                        type_node: p.ty.to_datara_type_node(
                                            &header_str_path,
                                            p.line,
                                            p.col,
                                        ),
                                        ownership_mode: "val".into(),
                                        span: p_span,
                                    }
                                })
                                .collect();

                            let return_type = cf.return_type.to_datara_type_node(
                                &header_str_path,
                                cf.line,
                                cf.col,
                            );

                            let mut sret_size: Option<usize> = None;

                            // Compile-time ABI classification for struct
                            // returns that the native ABI layer must lower:
                            // one machine word or less rides in RAX as raw
                            // bits, layout-compatible aggregates ride the
                            // hidden sret return slot where the backend
                            // supports it, and every other shape is rejected
                            // with E0962 instead of emitting a call that
                            // crashes at run time (missing hidden sret slot).
                            // Variadic struct returns keep the rejection even
                            // where sret is supported: the varargs call
                            // convention adds ABI state (e.g. AL on System V)
                            // the import path does not model.
                            match classify_return(&cf.return_type, &struct_layouts) {
                                ReturnAbi::Register => {}
                                ReturnAbi::Sret(size) if allow_sret_returns && !cf.is_variadic => {
                                    sret_size = Some(size);
                                }
                                ReturnAbi::Sret(size) => {
                                    let reason = if cf.is_variadic {
                                        "the function is variadic and the import path does not model the variadic struct-return ABI"
                                    } else {
                                        "the current compilation target does not implement the hidden sret return-slot ABI for C imports yet"
                                    };
                                    diag.warning(
                                        ErrorCode::CImportUnsupportedConstruct,
                                        format!(
                                            "C function '{}' returns a by-value struct of {} bytes, but {}, so the declaration is rejected at compile time instead of crashing at run time. Use an out-pointer parameter (e.g. `void f(T* out)`).",
                                            cf.name, size, reason
                                        ),
                                        Some(cimport.span.clone()),
                                    );
                                    continue;
                                }
                                ReturnAbi::AboveWindow(size) => {
                                    diag.warning(
                                        ErrorCode::CImportUnsupportedConstruct,
                                        format!(
                                            "C function '{}' returns a by-value struct of {} bytes, which exceeds the supported hidden sret return window of {} bytes (two 8-byte fields) for C imports, so the declaration is rejected at compile time instead of silently relying on an untested ABI shape. Use an out-pointer parameter (e.g. `void f(T* out)`).",
                                            cf.name, size, MAX_SRET_BYTES
                                        ),
                                        Some(cimport.span.clone()),
                                    );
                                    continue;
                                }
                                ReturnAbi::Unsupported(size) => {
                                    let size_note = match size {
                                        Some(bytes) => format!("of {} bytes", bytes),
                                        None => "of unknown size".to_string(),
                                    };
                                    diag.warning(
                                        ErrorCode::CImportUnsupportedConstruct,
                                        format!(
                                            "C function '{}' returns a by-value struct {} whose C layout cannot be mapped onto the Datara object layout used to read the sret return buffer (every field must be an 8-byte scalar at offset index*8 and the total size must equal fields*8), so the declaration is rejected at compile time instead of crashing at run time. Use an out-pointer parameter (e.g. `void f(T* out)`).",
                                            cf.name, size_note
                                        ),
                                        Some(cimport.span.clone()),
                                    );
                                    continue;
                                }
                            }

                            new_declarations.push(Decl::ExternFn(ExternFnDecl {
                                abi: "C".into(),
                                name: cf.name,
                                params,
                                return_type,
                                sret_size,
                                span: f_span,
                            }));
                        }
                        CDecl::Enum(ce) => {
                            let e_span = SourceSpan::new(
                                ce.line,
                                ce.col,
                                ce.line,
                                ce.col + 4,
                                header_str_path.clone(),
                            );
                            if let Some(enum_name) = ce.name {
                                if !new_declarations.iter().any(|d| match d {
                                    Decl::Type(t) => t.name == enum_name,
                                    Decl::Class(c) => c.name == enum_name,
                                    _ => false,
                                }) {
                                    new_declarations.push(Decl::Type(TypeDecl {
                                        name: enum_name,
                                        base_type: TypeNode::new("Int", e_span.clone()),
                                        is_export: true,
                                        span: e_span.clone(),
                                    }));
                                }
                            }

                            // Emit integer functions for each variant value for ergonomic constant access
                            for v in &ce.variants {
                                if !new_declarations.iter().any(|d| match d {
                                    Decl::Function(f) => f.name == v.name,
                                    _ => false,
                                }) {
                                    let v_span = SourceSpan::new(
                                        v.line,
                                        v.col,
                                        v.line,
                                        v.col + v.name.len(),
                                        header_str_path.clone(),
                                    );
                                    new_declarations.push(Decl::Function(FunctionDecl {
                                        name: v.name.clone(),
                                        attributes: Vec::new(),
                                        generic_params: Vec::new(),
                                        generic_constraints: Vec::new(),
                                        params: Vec::new(),
                                        return_type: Some(TypeNode::new("Int", v_span.clone())),
                                        requires: Vec::new(),
                                        ensures: Vec::new(),
                                        decreases: None,
                                        body: Box::new(Stmt::Return(
                                            Some(Expr::Literal(
                                                LiteralValue::Int(v.value),
                                                v_span.clone(),
                                            )),
                                            v_span.clone(),
                                        )),
                                        is_expression_body: true,
                                        is_export: true,
                                        span: v_span,
                                    }));
                                }
                            }
                        }
                        CDecl::DefineInt(cd) => {
                            let d_span = SourceSpan::new(
                                cd.line,
                                cd.col,
                                cd.line,
                                cd.col + cd.name.len(),
                                header_str_path.clone(),
                            );
                            new_declarations.push(Decl::Function(FunctionDecl {
                                name: cd.name,
                                attributes: Vec::new(),
                                generic_params: Vec::new(),
                                generic_constraints: Vec::new(),
                                params: Vec::new(),
                                return_type: Some(TypeNode::new("Int", d_span.clone())),
                                requires: Vec::new(),
                                ensures: Vec::new(),
                                decreases: None,
                                body: Box::new(Stmt::Return(
                                    Some(Expr::Literal(
                                        LiteralValue::Int(cd.value),
                                        d_span.clone(),
                                    )),
                                    d_span.clone(),
                                )),
                                is_expression_body: true,
                                is_export: true,
                                span: d_span,
                            }));
                        }
                        CDecl::OpaqueStruct(cs) => {
                            if !new_declarations.iter().any(|d| match d {
                                Decl::Type(t) => t.name == cs.name,
                                Decl::Class(c) => c.name == cs.name,
                                _ => false,
                            }) {
                                let s_span = SourceSpan::new(
                                    cs.line,
                                    cs.col,
                                    cs.line,
                                    cs.col + cs.name.len(),
                                    header_str_path.clone(),
                                );
                                new_declarations.push(Decl::Type(TypeDecl {
                                    name: cs.name,
                                    base_type: TypeNode::new("RawPtr", s_span.clone()),
                                    is_export: true,
                                    span: s_span,
                                }));
                            }
                        }
                        CDecl::Struct(cs) => {
                            if !new_declarations.iter().any(|d| match d {
                                Decl::Type(t) => t.name == cs.name,
                                Decl::Class(c) => c.name == cs.name,
                                _ => false,
                            }) {
                                let s_span = SourceSpan::new(
                                    cs.line,
                                    cs.col,
                                    cs.line,
                                    cs.col + cs.name.len(),
                                    header_str_path.clone(),
                                );
                                let mut body_items = Vec::new();
                                for f in &cs.fields {
                                    let f_span = SourceSpan::new(
                                        f.line,
                                        f.col,
                                        f.line,
                                        f.col + f.name.len(),
                                        header_str_path.clone(),
                                    );
                                    let type_node =
                                        f.ty.to_datara_type_node(&header_str_path, f.line, f.col);
                                    body_items.push(ClassItem::Field(FieldDecl {
                                        name: f.name.clone(),
                                        type_node,
                                        bit_field: None,
                                        default_value: None,
                                        is_mut: false,
                                        span: f_span,
                                    }));
                                }
                                new_declarations.push(Decl::Class(ClassDecl {
                                    name: cs.name.clone(),
                                    attributes: Vec::new(),
                                    generic_params: Vec::new(),
                                    base_type: None,
                                    compositions: Vec::new(),
                                    body_items,
                                    invariants: Vec::new(),
                                    is_export: true,
                                    span: s_span,
                                }));
                            }
                        }
                        CDecl::Typedef(ct) => {
                            if !new_declarations.iter().any(|d| match d {
                                Decl::Type(t) => t.name == ct.name,
                                Decl::Class(c) => c.name == ct.name,
                                _ => false,
                            }) {
                                let t_span = SourceSpan::new(
                                    ct.line,
                                    ct.col,
                                    ct.line,
                                    ct.col + ct.name.len(),
                                    header_str_path.clone(),
                                );
                                if let Some(base_type) =
                                    ct.target
                                        .to_datara_type_node(&header_str_path, ct.line, ct.col)
                                {
                                    new_declarations.push(Decl::Type(TypeDecl {
                                        name: ct.name,
                                        base_type,
                                        is_export: true,
                                        span: t_span,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
            other => new_declarations.push(other),
        }
    }

    program.declarations = new_declarations;
    for lib in extra_libs {
        if !program.link_libraries.contains(&lib) {
            program.link_libraries.push(lib);
        }
    }
}

/// Memory layout of one parsed C struct on a 64-bit target.
#[derive(Debug, Clone, Copy)]
struct StructLayout {
    /// Total size in bytes, including tail padding.
    size: usize,
    /// Natural alignment in bytes.
    align: usize,
    /// True when the C memory layout is byte-for-byte identical to the
    /// layout the native backend uses to read a Datara heap object with the
    /// same field count: every field is an 8-byte scalar sitting at offset
    /// `index * 8`, and the total size is `fields * 8`. Only such structs
    /// can be read straight out of the sret return buffer without a
    /// translating copy, because Datara field access always reads the
    /// 8-byte slot at `index * 8`.
    sret_eligible: bool,
}

/// Largest aggregate the native backend accepts through the hidden sret
/// return slot.
///
/// Microsoft x64 itself hands back any aggregate larger than one machine
/// word through the hidden slot, so this bound is a deliberate, fail-closed
/// scope limit for v1.3.2 rather than an ABI limit: only the two-machine-word
/// (16-byte) shape is covered by the native round-trip test suite, and
/// anything larger stays rejected with E0962 until it is exercised. Widening
/// this constant is the single change needed to accept larger structs.
const MAX_SRET_BYTES: usize = 16;

/// How a C function's return value crosses the native ABI boundary.
enum ReturnAbi {
    /// One machine word or less: returned in RAX (raw bits for small
    /// aggregates, the value itself for scalars). Existing path.
    Register,
    /// Aggregate larger than one machine word whose C layout matches the
    /// Datara object layout: returned through the hidden sret slot.
    Sret(usize),
    /// Layout-compatible aggregate above `MAX_SRET_BYTES`: the ABI lowering
    /// is understood, but the shape is outside the tested support window.
    AboveWindow(usize),
    /// Aggregate the ABI layer cannot lower (layout mismatch, nested
    /// aggregate, or a size the layout engine cannot establish).
    Unsupported(Option<usize>),
}

/// Classify a C return type against the layout table.
///
/// Named types that are neither parsed structs nor known scalars (enums,
/// user typedefs of scalars) keep the historical lenient treatment and are
/// assumed to be register-sized: tightening that would reject real,
/// working enum-returning imports.
fn classify_return(
    ty: &CType,
    structs: &std::collections::HashMap<String, StructLayout>,
) -> ReturnAbi {
    if let CType::Named(name) = ty
        && let Some(layout) = structs.get(name)
    {
        if layout.size <= 8 {
            return ReturnAbi::Register;
        }
        if layout.sret_eligible {
            if layout.size <= MAX_SRET_BYTES {
                return ReturnAbi::Sret(layout.size);
            }
            return ReturnAbi::AboveWindow(layout.size);
        }
        return ReturnAbi::Unsupported(Some(layout.size));
    }
    match c_type_size(ty, &std::collections::HashMap::new()) {
        Some(size) if size > 8 => ReturnAbi::Unsupported(Some(size)),
        _ => ReturnAbi::Register,
    }
}

/// Compute the memory layout of every parsed struct definition.
///
/// Field offsets follow the standard 64-bit C rules (natural alignment with
/// tail padding). A struct is `sret_eligible` only when the result coincides
/// exactly with the Datara object layout used to read the sret buffer.
fn compute_struct_layouts(decls: &[CDecl]) -> std::collections::HashMap<String, StructLayout> {
    let mut layouts = std::collections::HashMap::new();
    for decl in decls {
        if let CDecl::Struct(cs) = decl {
            let mut size = 0usize;
            let mut align = 1usize;
            let mut eligible = true;
            for (index, field) in cs.fields.iter().enumerate() {
                let (f_size, f_align, f_scalar8) = c_field_layout(&field.ty, &layouts);
                let offset = round_up(size, f_align);
                // The Datara reader always loads field `index` from
                // `index * 8`; any other C offset is unreadable.
                if !f_scalar8 || offset != index * 8 {
                    eligible = false;
                }
                size = offset + f_size;
                align = align.max(f_align);
            }
            let total = round_up(size, align);
            if cs.fields.is_empty() || total != cs.fields.len() * 8 {
                eligible = false;
            }
            layouts.insert(
                cs.name.clone(),
                StructLayout {
                    size: total,
                    align,
                    sret_eligible: eligible,
                },
            );
        }
    }
    layouts
}

fn round_up(value: usize, align: usize) -> usize {
    let a = align.max(1);
    value.div_ceil(a) * a
}

/// Byte size, byte alignment and "is an 8-byte scalar" flag of one C field.
/// Unknown named types fall back to the 16-byte heuristic the previous
/// one-machine-word gate used, which also disqualifies the struct from sret.
fn c_field_layout(
    ty: &CType,
    structs: &std::collections::HashMap<String, StructLayout>,
) -> (usize, usize, bool) {
    match ty {
        CType::Void => (0, 1, false),
        CType::Int { bits, .. } | CType::Float { bits, .. } => {
            let size = bits / 8;
            (size, size.min(8), size == 8)
        }
        CType::Char => (1, 1, false),
        CType::String | CType::RawPtr { .. } => (8, 8, true),
        CType::Named(name) => match name.as_str() {
            "char" | "int8_t" | "uint8_t" => (1, 1, false),
            "short" | "int16_t" | "uint16_t" => (2, 2, false),
            "int" | "int32_t" | "uint32_t" => (4, 4, false),
            "long" | "long long" | "size_t" | "ssize_t" | "int64_t" | "uint64_t" | "intptr_t"
            | "uintptr_t" | "ptrdiff_t" => (8, 8, true),
            "float" => (4, 4, false),
            "double" => (8, 8, true),
            other => match structs.get(other) {
                // A nested aggregate is inline in C but one 8-byte slot in
                // Datara, so the layouts can never agree.
                Some(nested) => (nested.size, nested.align, false),
                None => (16, 8, false),
            },
        },
    }
}

/// Byte size of a C type on a 64-bit target, if statically known.
/// Unknown named types resolve through the parsed struct table; an unknown
/// struct returns `None` so the caller can apply a conservative policy.
fn c_type_size(ty: &CType, structs: &std::collections::HashMap<String, usize>) -> Option<usize> {
    match ty {
        CType::Void => Some(0),
        CType::Int { bits, .. } | CType::Float { bits, .. } => Some(bits / 8),
        CType::Char => Some(1),
        CType::String | CType::RawPtr { .. } => Some(8),
        CType::Named(name) => match name.as_str() {
            "char" | "int8_t" | "uint8_t" => Some(1),
            "short" | "int16_t" | "uint16_t" => Some(2),
            "int" | "int32_t" | "uint32_t" => Some(4),
            "long" | "long long" | "size_t" | "ssize_t" | "int64_t" | "uint64_t" | "intptr_t"
            | "uintptr_t" | "ptrdiff_t" => Some(8),
            "float" => Some(4),
            "double" => Some(8),
            other => structs.get(other).copied(),
        },
    }
}
