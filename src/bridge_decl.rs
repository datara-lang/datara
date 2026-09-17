//! v1.4.2 Declarative Bridge Hub (`bridge lang::module { ... }`).
//!
//! Provides:
//! - Static validation of bridge foreign languages (py, js, c, rs) -> E-BRIDGE-003
//! - Supported type whitelist validation (Int, Float, Bool, Str, List<prim>, Map<Str, prim>) -> E-BRIDGE-002
//! - Call-site type mismatch checking -> E-BRIDGE-001
//! - Registration of module aliases so `module.func(...)` resolves transparently
//! - Lowering to existing runtime primitives (no new ABIs)

use crate::ast::{BridgeDecl, Decl, Program, TypeNode};
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::types::DataraType;
use std::collections::HashMap;

/// Metadata for a registered bridge function.
#[derive(Debug, Clone)]
pub struct BridgeFunctionMeta {
    pub lang: String,
    pub module: String,
    pub fn_name: String,
    pub param_types: Vec<DataraType>,
    pub return_type: DataraType,
    pub span: SourceSpan,
}

/// Global registry of declared foreign bridges in a program.
#[derive(Debug, Clone, Default)]
pub struct BridgeRegistry {
    pub functions: HashMap<String, BridgeFunctionMeta>,
    pub module_to_lang: HashMap<String, String>,
}

impl BridgeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds registry from program without diagnostic emission.
    pub fn from_program(program: &Program) -> Self {
        let mut registry = Self::default();
        for decl in &program.declarations {
            if let Decl::Bridge(b) = decl {
                let lang_lower = b.lang.to_lowercase();
                registry
                    .module_to_lang
                    .insert(b.module.clone(), lang_lower.clone());
                for f in &b.functions {
                    let param_types = f
                        .params
                        .iter()
                        .map(|p| type_node_to_datara_type(p.type_node.as_ref()))
                        .collect();
                    let return_type = type_node_to_datara_type(f.return_type.as_ref());
                    let meta = BridgeFunctionMeta {
                        lang: lang_lower.clone(),
                        module: b.module.clone(),
                        fn_name: f.name.clone(),
                        param_types,
                        return_type,
                        span: f.span.clone(),
                    };
                    let qualified_dot = format!("{}.{}", b.module, f.name);
                    let qualified_colon = format!("{}::{}::{}", lang_lower, b.module, f.name);
                    let qualified_snake = format!("{}_{}_{}", lang_lower, b.module, f.name);
                    registry.functions.insert(qualified_dot, meta.clone());
                    registry.functions.insert(qualified_colon, meta.clone());
                    registry.functions.insert(qualified_snake, meta.clone());
                    registry.functions.insert(f.name.clone(), meta);
                }
            }
        }
        registry
    }

    /// Validates all bridge declarations in the program and populates the registry.
    /// Emits E-BRIDGE-002 for unsupported types and E-BRIDGE-003 for unknown languages.
    pub fn register_program(&mut self, program: &mut Program, diag: &mut DiagnosticEngine) {
        let bridges: Vec<BridgeDecl> = program
            .declarations
            .iter()
            .filter_map(|d| match d {
                Decl::Bridge(b) => Some(b.clone()),
                _ => None,
            })
            .collect();
        for b in &bridges {
            self.register_decl(b, &mut program.module_aliases, diag);
        }
    }

    fn register_decl(
        &mut self,
        decl: &BridgeDecl,
        module_aliases: &mut HashMap<String, Vec<String>>,
        diag: &mut DiagnosticEngine,
    ) {
        let lang_lower = decl.lang.to_lowercase();
        let is_valid_lang = matches!(
            lang_lower.as_str(),
            "py" | "python"
                | "js"
                | "javascript"
                | "node"
                | "c"
                | "rs"
                | "rust"
                | "zig"
                | "cs"
                | "csharp"
                | "dotnet"
        );

        if !is_valid_lang {
            diag.error(
                ErrorCode::BridgeUnknownLanguage,
                format!(
                    "Unknown foreign bridge language '{}'. Supported languages: py, js, c, rs, zig, cs",
                    decl.lang
                ),
                Some(decl.span.clone()),
            );
            return;
        }

        self.module_to_lang
            .insert(decl.module.clone(), lang_lower.clone());

        // Register in program.module_aliases so `module.func(...)` resolves via namespace rules
        let alias_entry = module_aliases.entry(decl.module.clone()).or_default();

        for f in &decl.functions {
            if !alias_entry.contains(&f.name) {
                alias_entry.push(f.name.clone());
            }

            let mut param_types = Vec::new();
            for p in &f.params {
                let ty = type_node_to_datara_type(p.type_node.as_ref());
                if !is_supported_bridge_type(&ty) {
                    diag.error(
                        ErrorCode::BridgeUnsupportedType,
                        format!(
                            "Unsupported parameter type in bridge function '{}'. Supported: Int, Float, Bool, Str, List<primitive>, Map<Str, primitive>",
                            f.name
                        ),
                        Some(p.span.clone()),
                    );
                }
                param_types.push(ty);
            }

            let return_type = type_node_to_datara_type(f.return_type.as_ref());
            if !is_supported_bridge_return_type(&return_type) {
                diag.error(
                    ErrorCode::BridgeUnsupportedType,
                    format!(
                        "Unsupported return type in bridge function '{}'. Supported: Int, Float, Bool, Str, List<primitive>, Map<Str, primitive>",
                        f.name
                    ),
                    Some(f.span.clone()),
                );
            }

            let meta = BridgeFunctionMeta {
                lang: lang_lower.clone(),
                module: decl.module.clone(),
                fn_name: f.name.clone(),
                param_types,
                return_type,
                span: f.span.clone(),
            };

            // Register with qualified prefixes:
            // "math.sqrt", "py::math::sqrt", "py_math_sqrt", and bare "sqrt"
            let qualified_dot = format!("{}.{}", decl.module, f.name);
            let qualified_colon = format!("{}::{}::{}", lang_lower, decl.module, f.name);
            let qualified_snake = format!("{}_{}_{}", lang_lower, decl.module, f.name);

            self.functions.insert(qualified_dot, meta.clone());
            self.functions.insert(qualified_colon, meta.clone());
            self.functions.insert(qualified_snake, meta.clone());
            self.functions.insert(f.name.clone(), meta);
        }
    }

    /// Check if a function is a registered bridge function.
    pub fn lookup(&self, name: &str) -> Option<&BridgeFunctionMeta> {
        self.functions.get(name)
    }

    /// Validates argument types against the declared bridge function signature.
    /// Emits E-BRIDGE-001 on type mismatch.
    pub fn check_call_args(
        &self,
        fn_name: &str,
        arg_types: &[DataraType],
        span: &SourceSpan,
        diag: &mut DiagnosticEngine,
    ) -> bool {
        if let Some(meta) = self.lookup(fn_name) {
            let mut ok = true;
            for (idx, (expected, actual)) in
                meta.param_types.iter().zip(arg_types.iter()).enumerate()
            {
                if !actual.is_compatible(expected) {
                    diag.error(
                        ErrorCode::BridgeTypeMismatch,
                        format!(
                            "bridge {}::{}::{}: arg {} expected '{}', got '{}'",
                            meta.lang, meta.module, meta.fn_name, idx, expected, actual
                        ),
                        Some(span.clone()),
                    );
                    ok = false;
                }
            }
            ok
        } else {
            true
        }
    }
}

/// Convert AST TypeNode to DataraType.
pub fn type_node_to_datara_type(node: Option<&TypeNode>) -> DataraType {
    match node {
        None => DataraType::Unit,
        Some(n) => match n.name.as_str() {
            "Int" | "i64" | "int" => DataraType::Int,
            "Float" | "f64" | "float" => DataraType::Float,
            "Bool" | "bool" => DataraType::Bool,
            "Str" | "String" | "str" => DataraType::String,
            "Unit" | "void" => DataraType::Unit,
            "List" => {
                let inner = if let Some(arg) = n.generic_args.first() {
                    type_node_to_datara_type(Some(arg))
                } else {
                    DataraType::Int
                };
                DataraType::List(Box::new(inner))
            }
            "Map" => {
                let k = if let Some(arg) = n.generic_args.first() {
                    type_node_to_datara_type(Some(arg))
                } else {
                    DataraType::String
                };
                let v = if let Some(arg) = n.generic_args.get(1) {
                    type_node_to_datara_type(Some(arg))
                } else {
                    DataraType::Int
                };
                DataraType::Map(Box::new(k), Box::new(v))
            }
            other => DataraType::Class(other.to_string()),
        },
    }
}

/// Check if type is in the bridge whitelist:
/// Int, Float, Bool, Str, List<primitive>, Map<Str, primitive>
pub fn is_supported_bridge_type(ty: &DataraType) -> bool {
    match ty {
        DataraType::Int | DataraType::Float | DataraType::Bool | DataraType::String => true,
        DataraType::List(inner) => is_primitive(inner),
        DataraType::Map(key, val) => matches!(**key, DataraType::String) && is_primitive(val),
        _ => false,
    }
}

pub fn is_supported_bridge_return_type(ty: &DataraType) -> bool {
    matches!(ty, DataraType::Unit) || is_supported_bridge_type(ty)
}

fn is_primitive(ty: &DataraType) -> bool {
    matches!(
        ty,
        DataraType::Int | DataraType::Float | DataraType::Bool | DataraType::String
    )
}

/// Expands bridge declarations in the program:
/// 1. Registers them in BridgeRegistry and program.module_aliases.
/// 2. For C and Rust foreign bridges, synthesizes Decl::ExternFn so standard C ABI linking occurs.
pub fn expand_bridge_declarations(program: &mut Program, diag: &mut DiagnosticEngine) {
    let mut registry = BridgeRegistry::new();
    registry.register_program(program, diag);

    let mut new_externs = Vec::new();
    for decl in &program.declarations {
        if let Decl::Bridge(b) = decl {
            let lang_lower = b.lang.to_lowercase();
            if matches!(lang_lower.as_str(), "c" | "rs" | "rust") {
                for f in &b.functions {
                    new_externs.push(Decl::ExternFn(crate::ast::ExternFnDecl {
                        abi: "C".into(),
                        name: f.name.clone(),
                        params: f.params.clone(),
                        return_type: f.return_type.clone(),
                        attributes: Vec::new(),
                        sret_size: None,
                        sysv_classes: None,
                        span: f.span.clone(),
                    }));
                }
            }
        }
    }
    program.declarations.extend(new_externs);
}
