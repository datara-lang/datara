pub mod lexer;
pub mod parser;
pub mod types;

use crate::ast::*;
use crate::cimport::lexer::CLexer;
use crate::cimport::parser::CParser;
use crate::cimport::types::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use std::path::{Path, PathBuf};

/// Which struct-return ABI the native backend implements for C imports
/// (v1.3.3). Computed by the driver from the host target and backend
/// selection, then consumed by `expand_c_imports` to admit exactly the
/// by-value struct returns the backend can lower.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructReturnAbi {
    /// No struct-return lowering is available: every >1-word by-value
    /// struct return is rejected at compile time (E0962).
    None,
    /// Microsoft x64 hidden sret return slot: the caller allocates the
    /// return buffer and passes its pointer as the first integer argument.
    Sret,
    /// System V AMD64 register pair: a two-eightbyte aggregate rides
    /// RAX/RDX (INTEGER classes) and XMM0/XMM1 (SSE classes).
    SysVRegisters,
}

pub fn expand_c_imports(
    program: &mut Program,
    base_dir: Option<&Path>,
    diag: &mut DiagnosticEngine,
    struct_return_abi: StructReturnAbi,
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
                // aggregates come back through the hidden sret return slot
                // (Microsoft x64) or the SysV AMD64 register pair
                // (RAX/RDX, XMM0/XMM1), and everything else is rejected at
                // compile time. The gate must never let an unsupported
                // shape through to codegen: emitting a call to such a
                // function crashes at run time.
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
                            let mut sysv_classes: Option<[SysVClass; 2]> = None;

                            // Compile-time ABI classification for struct
                            // returns that the native ABI layer must lower:
                            // one machine word or less rides in RAX as raw
                            // bits, layout-compatible aggregates ride the
                            // hidden sret return slot (Microsoft x64) or the
                            // SysV AMD64 register pair, and every other
                            // shape is rejected with E0962 instead of
                            // emitting a call that crashes at run time
                            // (missing hidden sret slot). Variadic struct
                            // returns keep the rejection even where the
                            // struct-return ABI is supported: the varargs
                            // call convention adds ABI state (e.g. AL on
                            // System V) the import path does not model.
                            match classify_return(
                                &cf.return_type,
                                &struct_layouts,
                                struct_return_abi,
                            ) {
                                ReturnAbi::Register => {}
                                ReturnAbi::Sret(size)
                                    if !cf.is_variadic
                                        && struct_return_abi == StructReturnAbi::Sret =>
                                {
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
                                ReturnAbi::SysVRegisters { classes }
                                    if !cf.is_variadic
                                        && struct_return_abi == StructReturnAbi::SysVRegisters =>
                                {
                                    sysv_classes = Some(classes);
                                }
                                ReturnAbi::SysVRegisters { classes } => {
                                    let reason = if cf.is_variadic {
                                        "the function is variadic and the import path does not model the variadic struct-return ABI"
                                    } else {
                                        "the current compilation target does not implement the SysV AMD64 register-pair return ABI for C imports yet"
                                    };
                                    diag.warning(
                                        ErrorCode::CImportUnsupportedConstruct,
                                        format!(
                                            "C function '{}' returns a by-value struct of 16 bytes classified as {:?}/{:?} eightbytes, but {}, so the declaration is rejected at compile time instead of crashing at run time. Use an out-pointer parameter (e.g. `void f(T* out)`).",
                                            cf.name, classes[0], classes[1], reason
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
                                attributes: Vec::new(),
                                sret_size,
                                sysv_classes,
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
                                        is_comptime: false,
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
                                is_comptime: false,
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
                                        offset: None,
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
    /// SysV AMD64 register classes of the two eightbytes, when the struct is
    /// exactly two 8-byte scalars at offsets 0 and 8 (the same
    /// layout-compatible zone the sret path covers). Each field maps to its
    /// own eightbyte, so `classes[0]` is offset 0 and `classes[1]` is
    /// offset 8. `None` for every other shape.
    sysv_classes: Option<[SysVClass; 2]>,
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
#[derive(Debug)]
enum ReturnAbi {
    /// One machine word or less: returned in RAX (raw bits for small
    /// aggregates, the value itself for scalars). Existing path.
    Register,
    /// Aggregate larger than one machine word whose C layout matches the
    /// Datara object layout: returned through the hidden sret slot
    /// (Microsoft x64) or, for the two-eightbyte shape, through the SysV
    /// AMD64 register pair — `classify_return` picks the variant from the
    /// target's [`StructReturnAbi`].
    Sret(usize),
    /// Two-eightbyte aggregate classified for the SysV AMD64 register
    /// return: `classes[0]` (offset 0) rides RAX/XMM0, `classes[1]`
    /// (offset 8) rides RDX/XMM1. Emitted only when the target declares
    /// [`StructReturnAbi::SysVRegisters`].
    SysVRegisters { classes: [SysVClass; 2] },
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
///
/// For a layout-compatible two-eightbyte aggregate the `struct_return_abi`
/// of the target decides which lowering the declaration carries: the SysV
/// register pair on linux-x86_64, the hidden sret slot on Windows x64.
fn classify_return(
    ty: &CType,
    structs: &std::collections::HashMap<String, StructLayout>,
    struct_return_abi: StructReturnAbi,
) -> ReturnAbi {
    if let CType::Named(name) = ty
        && let Some(layout) = structs.get(name)
    {
        if layout.size <= 8 {
            return ReturnAbi::Register;
        }
        if layout.sret_eligible {
            if layout.size <= MAX_SRET_BYTES {
                if let (StructReturnAbi::SysVRegisters, Some(classes)) =
                    (struct_return_abi, layout.sysv_classes)
                {
                    return ReturnAbi::SysVRegisters { classes };
                }
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
/// exactly with the Datara object layout used to read the sret buffer. A
/// struct of exactly two such 8-byte scalars additionally carries its SysV
/// AMD64 register classes: one field per eightbyte, in field order.
fn compute_struct_layouts(decls: &[CDecl]) -> std::collections::HashMap<String, StructLayout> {
    let mut layouts = std::collections::HashMap::new();
    for decl in decls {
        if let CDecl::Struct(cs) = decl {
            let mut size = 0usize;
            let mut align = 1usize;
            let mut eligible = true;
            let mut classes = Some([SysVClass::Integer, SysVClass::Integer]);
            for (index, field) in cs.fields.iter().enumerate() {
                let (f_size, f_align, f_scalar8) = c_field_layout(&field.ty, &layouts);
                let offset = round_up(size, f_align);
                // The Datara reader always loads field `index` from
                // `index * 8`; any other C offset is unreadable.
                if !f_scalar8 || offset != index * 8 {
                    eligible = false;
                }
                // SysV register classification needs exactly one 8-byte
                // scalar per eightbyte, two eightbytes total.
                if let Some(cls) = c_field_reg_class(&field.ty) {
                    match (index, classes.as_mut()) {
                        (0, Some(arr)) => arr[0] = cls,
                        (1, Some(arr)) => arr[1] = cls,
                        _ => classes = None,
                    }
                } else {
                    classes = None;
                }
                size = offset + f_size;
                align = align.max(f_align);
            }
            let total = round_up(size, align);
            if cs.fields.is_empty() || total != cs.fields.len() * 8 {
                eligible = false;
            }
            if total != 16 {
                classes = None;
            }
            layouts.insert(
                cs.name.clone(),
                StructLayout {
                    size: total,
                    align,
                    sret_eligible: eligible,
                    sysv_classes: classes,
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

/// SysV AMD64 register class of one C field, when it occupies exactly one
/// eightbyte: 8-byte integers and pointers are INTEGER, 8-byte floats are
/// SSE. Sub-8-byte fields, `float`, nested aggregates and unknown named
/// types occupy a fraction of, or more than, one eightbyte and have no
/// single register class.
fn c_field_reg_class(ty: &CType) -> Option<SysVClass> {
    match ty {
        CType::Int { bits, .. } if *bits == 64 => Some(SysVClass::Integer),
        CType::Float { bits, .. } if *bits == 64 => Some(SysVClass::Sse),
        CType::String | CType::RawPtr { .. } => Some(SysVClass::Integer),
        CType::Named(name) => match name.as_str() {
            "long" | "long long" | "size_t" | "ssize_t" | "int64_t" | "uint64_t" | "intptr_t"
            | "uintptr_t" | "ptrdiff_t" => Some(SysVClass::Integer),
            "double" => Some(SysVClass::Sse),
            _ => None,
        },
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> CType {
        CType::Named(name.to_string())
    }

    fn struct_with_fields(name: &str, fields: &[(&str, CType)]) -> CDecl {
        CDecl::Struct(CStruct {
            name: name.to_string(),
            fields: fields
                .iter()
                .map(|(fname, ty)| CField {
                    name: fname.to_string(),
                    ty: ty.clone(),
                    line: 1,
                    col: 1,
                })
                .collect(),
            line: 1,
            col: 1,
        })
    }

    fn layout_of(decls: &[CDecl], name: &str) -> StructLayout {
        let layouts = compute_struct_layouts(decls);
        *layouts.get(name).expect("struct must be laid out")
    }

    fn classes_of(fields: &[(&str, CType)]) -> Option<[SysVClass; 2]> {
        let decl = struct_with_fields("S", fields);
        layout_of(std::slice::from_ref(&decl), "S").sysv_classes
    }

    #[test]
    fn sysv_class_table_two_eightbyte_structs() {
        // The classification table: struct fields -> register classes.
        // Every layout-compatible two-8-byte-scalar shape maps one field to
        // one eightbyte, in field order. Pure function: runs on any OS.
        let cases = [
            (
                &[("a", named("long long")), ("b", named("long long"))],
                [SysVClass::Integer, SysVClass::Integer],
            ),
            (
                &[("x", named("double")), ("y", named("double"))],
                [SysVClass::Sse, SysVClass::Sse],
            ),
            (
                &[("a", named("long long")), ("x", named("double"))],
                [SysVClass::Integer, SysVClass::Sse],
            ),
            (
                &[("x", named("double")), ("a", named("long long"))],
                [SysVClass::Sse, SysVClass::Integer],
            ),
            (
                &[
                    ("a", named("int64_t")),
                    ("p", CType::RawPtr { pointee: None }),
                ],
                [SysVClass::Integer, SysVClass::Integer],
            ),
            (
                &[("s", CType::String), ("x", named("double"))],
                [SysVClass::Integer, SysVClass::Sse],
            ),
        ];
        for (fields, expected) in cases {
            assert_eq!(classes_of(fields), Some(expected), "fields: {:?}", fields);
        }
    }

    #[test]
    fn sysv_class_table_rejects_out_of_zone_shapes() {
        // Sub-8-byte fields, `float`, nested aggregates and unknown types
        // have no single-eightbyte class; a third field disqualifies too.
        let decls = [
            struct_with_fields("TwoFloats", &[("x", named("float")), ("y", named("float"))]),
            struct_with_fields(
                "Wide",
                &[
                    ("a", named("int")),
                    ("b", named("int")),
                    ("c", named("long long")),
                ],
            ),
            struct_with_fields(
                "Triple",
                &[
                    ("a", named("long long")),
                    ("b", named("long long")),
                    ("c", named("long long")),
                ],
            ),
            struct_with_fields("CharLong", &[("c", CType::Char), ("a", named("long long"))]),
            struct_with_fields("Single", &[("v", named("long long"))]),
        ];
        let layouts = compute_struct_layouts(&decls);
        for name in ["TwoFloats", "Wide", "Triple", "CharLong", "Single"] {
            assert_eq!(
                layouts[name].sysv_classes, None,
                "{} must carry no SysV register classes",
                name
            );
        }
        // Triple stays layout-compatible but is above the 16-byte window.
        assert!(layouts["Triple"].sret_eligible);
        assert_eq!(layouts["Triple"].size, 24);
        // The mixed Wide shape is not even layout-compatible.
        assert!(!layouts["Wide"].sret_eligible);
    }

    #[test]
    fn classify_return_selects_variant_by_target_abi() {
        let decls = [struct_with_fields(
            "Pair",
            &[("a", named("long long")), ("b", named("long long"))],
        )];
        let layouts = compute_struct_layouts(&decls);
        let pair = named("Pair");

        // SysV target: the register-pair variant carries the classes.
        match classify_return(&pair, &layouts, StructReturnAbi::SysVRegisters) {
            ReturnAbi::SysVRegisters { classes } => {
                assert_eq!(classes, [SysVClass::Integer, SysVClass::Integer]);
            }
            other => panic!(
                "SysV target must classify Pair as SysVRegisters, got {:?}",
                other
            ),
        }
        // Windows and unsupported targets keep the sret shape; admission is
        // the caller's decision (expand_c_imports rejects when the target
        // does not implement it).
        match classify_return(&pair, &layouts, StructReturnAbi::Sret) {
            ReturnAbi::Sret(size) => assert_eq!(size, 16),
            other => panic!("Windows target must classify Pair as Sret, got {:?}", other),
        }
        assert!(matches!(
            classify_return(&pair, &layouts, StructReturnAbi::None),
            ReturnAbi::Sret(16)
        ));
    }

    #[test]
    fn classify_return_mixed_and_above_window_shapes() {
        let decls = [
            struct_with_fields(
                "IntFloat",
                &[("a", named("long long")), ("x", named("double"))],
            ),
            struct_with_fields(
                "Triple",
                &[
                    ("a", named("long long")),
                    ("b", named("long long")),
                    ("c", named("long long")),
                ],
            ),
            struct_with_fields(
                "Wide",
                &[
                    ("a", named("int")),
                    ("b", named("int")),
                    ("c", named("long long")),
                ],
            ),
        ];
        let layouts = compute_struct_layouts(&decls);
        match classify_return(&named("IntFloat"), &layouts, StructReturnAbi::SysVRegisters) {
            ReturnAbi::SysVRegisters { classes } => {
                assert_eq!(classes, [SysVClass::Integer, SysVClass::Sse]);
            }
            other => panic!("IntFloat must classify as SysVRegisters, got {:?}", other),
        }
        assert!(matches!(
            classify_return(&named("Triple"), &layouts, StructReturnAbi::SysVRegisters),
            ReturnAbi::AboveWindow(24)
        ));
        assert!(matches!(
            classify_return(&named("Wide"), &layouts, StructReturnAbi::SysVRegisters),
            ReturnAbi::Unsupported(Some(16))
        ));
        // A pointer or double scalar keeps the raw register path.
        assert!(matches!(
            classify_return(&named("double"), &layouts, StructReturnAbi::SysVRegisters),
            ReturnAbi::Register
        ));
    }
}
