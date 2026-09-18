pub mod alias_analysis;
pub(crate) mod attributes;
pub(crate) mod class_track;
pub(crate) mod emit_call;
pub(crate) mod emit_inst;
pub mod prefetch;
pub(crate) mod runtime_decls;
pub(crate) mod simd;

pub(crate) use class_track::class_type_name;

use crate::ast::Program;
use crate::codegen::CodegenBackend;
use crate::codegen::linker::{compile_with_clang, find_clang};
use crate::codegen::target::{Arch, CallingConvention, Os, TargetInfo};
use crate::dmir::{BasicBlockId, Function, Inst, Module, Terminator, ValueId};
use crate::types::{DataraType, TypeChecker};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Dedicated LLVM IR code emitter for Datara / Forgen.
pub struct LlvmEmitter<'a> {
    pub target: &'a TargetInfo,
    pub emit_debug_info: bool,
    pub profile: Option<&'a crate::pgo::ProfileData>,
}

fn get_range_for_var(
    fn_name: &str,
    var_name: &str,
    ty_hint: Option<&str>,
    types: &TypeChecker,
) -> Option<(i64, i64)> {
    if let Some(DataraType::Range { min, max, .. }) = types
        .fn_symbol_types
        .get(&(fn_name.to_string(), var_name.to_string()))
        .or_else(|| types.symbol_types.get(var_name))
    {
        return Some((*min as i64, *max as i64));
    }
    if let Some(th) = ty_hint
        && th.starts_with("Int<")
        && th.ends_with('>')
    {
        let inner = &th[4..th.len() - 1];
        if let Some((s_min, s_max)) = inner.split_once("..")
            && let (Ok(min), Ok(max)) = (s_min.trim().parse::<i64>(), s_max.trim().parse::<i64>())
        {
            return Some((min, max));
        }
    }
    None
}

fn collect_address_taken(module: &Module) -> HashSet<String> {
    let mut address_taken = HashSet::new();
    for f in module.functions.values() {
        for b in &f.blocks {
            for inst in &b.instructions {
                if let Inst::GetFuncAddr { func_name, .. } = inst {
                    address_taken.insert(func_name.clone());
                }
            }
        }
    }
    address_taken
}

impl<'a> LlvmEmitter<'a> {
    pub fn new(target: &'a TargetInfo) -> Self {
        Self {
            target,
            emit_debug_info: false,
            profile: None,
        }
    }

    pub fn with_debug(mut self, debug_info: bool) -> Self {
        self.emit_debug_info = debug_info;
        self
    }

    pub fn with_profile(mut self, profile: Option<&'a crate::pgo::ProfileData>) -> Self {
        self.profile = profile;
        self
    }

    /// Map Datara / DMIR type names to LLVM IR types.
    pub fn dmir_type_to_llvm(&self, ty: &str) -> &'static str {
        match ty {
            "Int" | "Int64" | "UInt" | "UInt64" | "i64" | "u64" | "isize" | "usize" | "USize"
            | "Bool" => "i64",
            "Int32" | "UInt32" | "i32" | "u32" => "i32",
            "Int16" | "UInt16" | "i16" | "u16" => "i16",
            "Int8" | "UInt8" | "i8" | "u8" | "Byte" => "i8",
            "i128" | "u128" => "i128",
            "Float" | "Float64" | "f64" => "double",
            "Float32" | "f32" | "f16" => "float",
            "Float4" | "float4" | "f32x4" | "<4 x float>" => "<4 x float>",
            "Float8" | "float8" | "f32x8" | "<8 x float>" => "<8 x float>",
            "Float16" | "float16" | "f32x16" | "<16 x float>" => "<16 x float>",
            "Float2" | "float2" | "f64x2" | "<2 x double>" => "<2 x double>",
            "Float4_64" | "f64x4" | "<4 x double>" => "<4 x double>",
            "Int4" | "i32x4" | "<4 x i32>" => "<4 x i32>",
            "i32x8" | "<8 x i32>" => "<8 x i32>",
            "Str" | "String" => "ptr",
            "Unit" | "void" | "Never" => "void",
            s if s.starts_with("Int<") || s.starts_with("UInt<") => "i64",
            s if s.starts_with("Float<") => "double",
            _ => "ptr",
        }
    }

    /// Escape string content for LLVM IR string literals: `c"...\00"`.
    /// Returns the escaped string and the total byte length including null terminator.
    pub fn escape_llvm_string(s: &str) -> (String, usize) {
        let mut out = String::new();
        let mut bytes_count = 0;
        for b in s.bytes() {
            bytes_count += 1;
            match b {
                b'\\' => out.push_str("\\5C"),
                b'"' => out.push_str("\\22"),
                b'\n' => out.push_str("\\0A"),
                b'\r' => out.push_str("\\0D"),
                b'\t' => out.push_str("\\09"),
                0 => out.push_str("\\00"),
                32..=126 => out.push(b as char),
                _ => out.push_str(&format!("\\{:02X}", b)),
            }
        }
        bytes_count += 1; // null terminator
        out.push_str("\\00");
        (out, bytes_count)
    }

    /// Emit complete LLVM IR module from DMIR Module.
    pub fn emit_module(
        &self,
        module: &Module,
        _program: &Program,
        types: &TypeChecker,
    ) -> Result<String, String> {
        let mut ir = String::new();

        ir.push_str(
            "; ============================================================================\n",
        );
        ir.push_str("; Auto-generated LLVM IR by Datara Forgen Compiler v1.0\n");
        ir.push_str(&format!(
            "; Target Triple: {}\n",
            self.target.triple_string()
        ));
        ir.push_str("; Architecture: x86_64 / AArch64 Native Backend\n");
        ir.push_str(
            "; ============================================================================\n\n",
        );

        // Target Layout & Triple
        match (&self.target.arch, &self.target.os) {
            (Arch::Aarch64, Os::MacOS) => {
                ir.push_str("target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\"\n");
                ir.push_str("target triple = \"arm64-apple-macosx\"\n\n");
            }
            (Arch::Aarch64, Os::Linux) => {
                ir.push_str(
                    "target datalayout = \"e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128\"\n",
                );
                ir.push_str("target triple = \"aarch64-unknown-linux-gnu\"\n\n");
            }
            (Arch::Aarch64, Os::Windows) => {
                ir.push_str(
                    "target datalayout = \"e-m:w-p:64:64-i32:32-i64:64-i128:128-n32:64-S128\"\n",
                );
                ir.push_str("target triple = \"aarch64-pc-windows-msvc\"\n\n");
            }
            (Arch::X86_64, Os::Windows) => {
                ir.push_str("target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n");
                ir.push_str("target triple = \"x86_64-pc-windows-msvc\"\n\n");
            }
            (Arch::X86_64, Os::MacOS) => {
                ir.push_str("target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n");
                ir.push_str("target triple = \"x86_64-apple-macosx\"\n\n");
            }
            _ => match self.target.calling_convention {
                CallingConvention::WindowsFastcall => {
                    ir.push_str("target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n");
                    ir.push_str("target triple = \"x86_64-pc-windows-msvc\"\n\n");
                }
                CallingConvention::Aarch64Standard => {
                    ir.push_str("target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\"\n");
                    ir.push_str("target triple = \"arm64-apple-macosx\"\n\n");
                }
                CallingConvention::SystemV => {
                    ir.push_str("target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n");
                    ir.push_str("target triple = \"x86_64-unknown-linux-gnu\"\n\n");
                }
                CallingConvention::WasmStandard => {
                    ir.push_str("target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-n32:64-S128-ni:1:10:20\"\n");
                    ir.push_str("target triple = \"wasm32-unknown-wasi\"\n\n");
                }
            },
        }

        // 1. Collect and emit string literals
        let mut string_literal_map: HashMap<String, usize> = HashMap::new();
        let mut str_id = 0;

        // Always register empty string and colon
        let mut register_str = |s: &str| {
            if !string_literal_map.contains_key(s) {
                string_literal_map.insert(s.to_string(), str_id);
                str_id += 1;
            }
        };

        register_str("");
        register_str(":");

        let mut sorted_func_names: Vec<&String> = module.functions.keys().collect();
        sorted_func_names.sort();

        for fname in &sorted_func_names {
            let func = &module.functions[*fname];
            for b in &func.blocks {
                for inst in &b.instructions {
                    match inst {
                        Inst::ConstStr { value, .. } => register_str(value),
                        Inst::FormatStr { parts, .. } => {
                            for p in parts {
                                register_str(p);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        ir.push_str("; --- Global String Literals ---\n");
        let mut sorted_strings: Vec<(&String, &usize)> = string_literal_map.iter().collect();
        sorted_strings.sort_by_key(|&(_, id)| *id);
        for (content, id) in sorted_strings {
            let (escaped, len) = Self::escape_llvm_string(content);
            ir.push_str(&format!(
                "@.str.{} = private unnamed_addr constant [{} x i8] c\"{}\", align 1\n",
                id, len, escaped
            ));
        }
        ir.push('\n');

        // 1b. Module-level global variables
        if !module.globals.is_empty() {
            ir.push_str("; --- Module-Level Global Variables ---\n");
            let mut sorted_globals: Vec<_> = module.globals.iter().collect();
            sorted_globals.sort_by_key(|(name, _)| *name);
            for (gname, (gty, _is_mut)) in sorted_globals {
                let llvm_ty = self.dmir_type_to_llvm(gty);
                let init_val = if llvm_ty == "double" {
                    "0.0"
                } else if llvm_ty == "ptr" {
                    "null"
                } else {
                    "0"
                };
                ir.push_str(&format!(
                    "@datara_global_{} = internal global {} {}, align 8\n",
                    gname, llvm_ty, init_val
                ));
            }
            ir.push('\n');
        }

        runtime_decls::emit_runtime_declarations(&mut ir);
        simd::emit_simd_declarations(&mut ir);

        // 2b. Declare user-declared extern "C" functions (FFI), mirroring the
        // Cranelift backend so FFI programs compile on both backends.
        let mut sorted_externs: Vec<_> = module.extern_functions.iter().collect();
        sorted_externs.sort_by_key(|(name, _)| *name);
        for (ef_name, (ef_params, ef_ret)) in sorted_externs {
            if ef_name.starts_with("datara_rt_") {
                continue;
            }
            let ret = if ef_ret == "Unit" || ef_ret == "Never" {
                "void".to_string()
            } else {
                self.dmir_type_to_llvm(ef_ret).to_string()
            };
            let params = ef_params
                .iter()
                .map(|p| self.dmir_type_to_llvm(p))
                .collect::<Vec<_>>()
                .join(", ");
            ir.push_str(&format!("declare {} @{}({})\n", ret, ef_name, params));
        }
        if !module.extern_functions.is_empty() {
            ir.push('\n');
        }

        // 3. Emit all functions with ownership and effect attributes
        let address_taken = collect_address_taken(module);
        let signatures = attributes::index_program_signatures(_program);
        let mut effect_analyzer = crate::effects::EffectAnalyzer::new();
        effect_analyzer.analyze_program(_program);

        let mut range_metadata_map: HashMap<(i64, i64), usize> = HashMap::new();
        let mut branch_weights_map: HashMap<(u32, u32), usize> = HashMap::new();
        let mut entry_count_map: HashMap<usize, usize> = HashMap::new();
        let mut loop_metadata_map: HashMap<usize, usize> = HashMap::new();
        branch_weights_map.insert((1, 1048576), 9);
        let mut next_meta_id = 10;
        for fname in &sorted_func_names {
            let f = &module.functions[*fname];
            let (ast_params, is_pure_ast) = signatures.get(*fname).cloned().unwrap_or_default();
            let is_pure_eff = effect_analyzer
                .function_effects
                .get(*fname)
                .map(|e| e.is_pure())
                .unwrap_or(false);
            let accesses_global = f.blocks.iter().any(|b| {
                b.instructions.iter().any(|inst| match inst {
                    crate::dmir::Inst::LoadVar { name, .. }
                    | crate::dmir::Inst::AssignVar { name, .. } => {
                        module.globals.contains_key(name)
                    }
                    _ => false,
                })
            });
            let is_pure = (is_pure_ast || is_pure_eff) && !accesses_global;
            let (is_cold, is_hot, entry_count) = if *fname == "main" {
                (false, false, None)
            } else if let Some(ref prof) = self.profile {
                if prof.is_runtime_measured() {
                    let count = prof.hot_functions.get(*fname).copied().unwrap_or(0);
                    let cold = count == 0;
                    let hot = count > 50;
                    (cold, hot, if count > 0 { Some(count) } else { None })
                } else {
                    let cold = !f.blocks.is_empty()
                        && f.blocks.iter().all(|b| attributes::is_cold_block(f, b.id));
                    (cold, false, None)
                }
            } else {
                let cold = !f.blocks.is_empty()
                    && f.blocks.iter().all(|b| attributes::is_cold_block(f, b.id));
                (cold, false, None)
            };
            let fn_ctx = attributes::FunctionAttrContext {
                is_pure,
                is_cold,
                is_hot,
                entry_count,
                ast_params,
            };
            ir.push_str(&self.emit_function_with_details(
                f,
                module,
                &string_literal_map,
                types,
                &mut range_metadata_map,
                &mut branch_weights_map,
                &mut entry_count_map,
                &mut loop_metadata_map,
                &mut next_meta_id,
                &address_taken,
                Some(&fn_ctx),
            )?);
            ir.push('\n');
        }

        // 4. Emit Loop Vectorization & Unroll Metadata (Honest contract: only enabled when target supports it)
        let vec_enabled = attributes::is_vector_supported(self.target);
        let width = if self
            .target
            .vector_support
            .contains(&crate::codegen::target::VectorExtension::Avx2)
        {
            8
        } else {
            4
        };
        ir.push_str("!0 = distinct !{!0, !1, !2, !3}\n");
        ir.push_str(&format!(
            "!1 = !{{!\"llvm.loop.vectorize.enable\", i1 {}}}\n",
            if vec_enabled { 1 } else { 0 }
        ));
        ir.push_str(&format!(
            "!2 = !{{!\"llvm.loop.vectorize.width\", i32 {}}}\n",
            width
        ));
        ir.push_str("!3 = !{!\"llvm.loop.unroll.enable\", i1 1}\n");
        ir.push_str("!9 = !{!\"branch_weights\", i32 1, i32 1048576}\n\n");

        // 5. Emit Profile-Guided and Custom Branch Weights Metadata Nodes
        let mut custom_weights: Vec<((u32, u32), usize)> = branch_weights_map
            .into_iter()
            .filter(|&(_, id)| id != 9)
            .collect();
        if !custom_weights.is_empty() {
            ir.push_str("; --- Profile-Guided Branch Weights ---\n");
            custom_weights.sort_by_key(|&(_, id)| id);
            for ((taken, not_taken), id) in custom_weights {
                ir.push_str(&format!(
                    "!{} = !{{!\"branch_weights\", i32 {}, i32 {}}}\n",
                    id, taken, not_taken
                ));
            }
            ir.push('\n');
        }

        // 5b. Emit Profile-Guided Function Entry Counts
        if !entry_count_map.is_empty() {
            ir.push_str("; --- Profile-Guided Function Entry Counts ---\n");
            let mut sorted_entries: Vec<(usize, usize)> = entry_count_map.into_iter().collect();
            sorted_entries.sort_by_key(|&(id, _)| id);
            for (id, count) in sorted_entries {
                ir.push_str(&format!(
                    "!{} = !{{!\"function_entry_count\", i64 {}}}\n",
                    id, count
                ));
            }
            ir.push('\n');
        }

        // 5c. Emit Profile-Guided Loop Unroll Metadata
        if !loop_metadata_map.is_empty() {
            ir.push_str("; --- Profile-Guided Loop Unroll Metadata ---\n");
            let mut sorted_loops: Vec<(usize, usize)> = loop_metadata_map.into_iter().collect();
            sorted_loops.sort_by_key(|&(id, _)| id);
            for (id, trip_count) in sorted_loops {
                let unroll_id = next_meta_id;
                next_meta_id += 1;
                let count_id = next_meta_id;
                next_meta_id += 1;
                ir.push_str(&format!(
                    "!{} = distinct !{{!{}, !{}, !{}}}\n!{} = !{{!\"llvm.loop.unroll.enable\", i1 1}}\n!{} = !{{!\"llvm.loop.unroll.count\", i32 {}}}\n",
                    id, id, unroll_id, count_id, unroll_id, count_id, trip_count
                ));
            }
            ir.push('\n');
        }

        // 6. Emit Formal Value Range Propagation (FVRP) Metadata Nodes
        if !range_metadata_map.is_empty() {
            ir.push_str("; --- FVRP Range Metadata Nodes ---\n");
            let mut sorted_ranges: Vec<((i64, i64), usize)> =
                range_metadata_map.into_iter().collect();
            sorted_ranges.sort_by_key(|&(_, id)| id);
            for ((min, high), id) in sorted_ranges {
                ir.push_str(&format!("!{} = !{{i64 {}, i64 {}}}\n", id, min, high));
            }
            ir.push('\n');
        }

        // 7. Emit Type-Based Alias Analysis (TBAA) Metadata Nodes
        ir.push_str(&attributes::emit_tbaa_metadata());

        Ok(ir)
    }

    /// Emit a single function to LLVM IR.
    pub fn emit_function(
        &self,
        f: &Function,
        module: &Module,
        strings: &HashMap<String, usize>,
        types: &TypeChecker,
        range_metadata: &mut HashMap<(i64, i64), usize>,
        next_meta_id: &mut usize,
    ) -> Result<String, String> {
        let address_taken = collect_address_taken(module);
        self.emit_function_with_address_taken(
            f,
            module,
            strings,
            types,
            range_metadata,
            next_meta_id,
            &address_taken,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn emit_function_with_address_taken(
        &self,
        f: &Function,
        module: &Module,
        strings: &HashMap<String, usize>,
        types: &TypeChecker,
        range_metadata: &mut HashMap<(i64, i64), usize>,
        next_meta_id: &mut usize,
        address_taken: &HashSet<String>,
    ) -> Result<String, String> {
        let mut branch_weights = HashMap::new();
        let mut entry_counts = HashMap::new();
        let mut loop_meta = HashMap::new();
        self.emit_function_with_details(
            f,
            module,
            strings,
            types,
            range_metadata,
            &mut branch_weights,
            &mut entry_counts,
            &mut loop_meta,
            next_meta_id,
            address_taken,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn emit_function_with_details(
        &self,
        f: &Function,
        module: &Module,
        strings: &HashMap<String, usize>,
        types: &TypeChecker,
        range_metadata: &mut HashMap<(i64, i64), usize>,
        branch_weights_map: &mut HashMap<(u32, u32), usize>,
        entry_count_map: &mut HashMap<usize, usize>,
        loop_metadata_map: &mut HashMap<usize, usize>,
        next_meta_id: &mut usize,
        address_taken: &HashSet<String>,
        fn_ctx: Option<&attributes::FunctionAttrContext>,
    ) -> Result<String, String> {
        let mut out = String::new();
        let is_main = f.name == "main";
        let has_main = module.functions.contains_key("main");
        let is_internal = has_main
            && !is_main
            && module.functions.contains_key(&f.name)
            && !module.extern_functions.contains_key(&f.name)
            && !address_taken.contains(&f.name);

        // Signature
        let ret_type = if is_main {
            "i32".to_string()
        } else if f.return_type == "Unit" {
            "void".to_string()
        } else {
            self.dmir_type_to_llvm(&f.return_type).to_string()
        };

        let is_pure = fn_ctx.map(|c| c.is_pure).unwrap_or(false);
        let ast_params = fn_ctx.map(|c| c.ast_params.as_slice());

        let params_sig = if is_main {
            "".to_string()
        } else {
            f.params
                .iter()
                .enumerate()
                .map(|(idx, (p_name, p_ty, p_val))| {
                    let attrs = attributes::derive_param_attributes(
                        f, idx, p_name, p_ty, *p_val, module, ast_params, is_pure,
                    );
                    format!("{}{} %v{}", self.dmir_type_to_llvm(p_ty), attrs, p_val.0)
                })
                .collect::<Vec<_>>()
                .join(", ")
        };

        let linkage_and_cc = if is_internal {
            "define internal fastcc"
        } else {
            "define"
        };

        let is_cold = fn_ctx.map(|c| c.is_cold).unwrap_or(false);
        let is_hot = fn_ctx.map(|c| c.is_hot).unwrap_or(false);
        let fn_attrs = attributes::derive_fn_attributes(f, is_pure, is_cold, is_hot);
        let fn_attrs_str = if fn_attrs.is_empty() {
            "".to_string()
        } else {
            format!(" {}", fn_attrs.join(" "))
        };
        let entry_meta = attributes::derive_fn_entry_count_metadata(
            fn_ctx.and_then(|c| c.entry_count),
            entry_count_map,
            next_meta_id,
        );

        out.push_str(&format!(
            "{} {} @{}({}){}{} {{\n",
            linkage_and_cc, ret_type, f.name, params_sig, fn_attrs_str, entry_meta
        ));

        // Track local variable types and allocas
        let mut local_vars: BTreeSet<String> = BTreeSet::new();
        let mut all_struct_inits: Vec<(ValueId, usize)> = Vec::new();
        let mut escaping_structs: HashSet<ValueId> = HashSet::new();

        for b in &f.blocks {
            for inst in &b.instructions {
                if let Inst::AssignVar { name, .. } = inst {
                    local_vars.insert(name.clone());
                } else if let Inst::StructInit { dest, fields, .. } = inst {
                    let byte_size = fields.len().saturating_mul(8).max(8);
                    all_struct_inits.push((*dest, byte_size));
                }
            }
        }

        for b in &f.blocks {
            if let Terminator::Return { value: Some(v) } = &b.terminator {
                escaping_structs.insert(*v);
            }
            for inst in &b.instructions {
                match inst {
                    Inst::Call { args, .. } => {
                        for a in args {
                            escaping_structs.insert(*a);
                        }
                    }
                    Inst::MethodCall { object, args, .. } => {
                        escaping_structs.insert(*object);
                        for a in args {
                            escaping_structs.insert(*a);
                        }
                    }
                    Inst::SetField { value, .. } => {
                        escaping_structs.insert(*value);
                    }
                    Inst::StructInit { fields, .. } => {
                        for (_, fv) in fields {
                            escaping_structs.insert(*fv);
                        }
                    }
                    _ => {}
                }
            }
        }

        if f.blocks.len() > 1 {
            for b in &f.blocks {
                if b.id != f.entry_block {
                    for inst in &b.instructions {
                        if let Inst::StructInit { dest, .. } = inst {
                            escaping_structs.insert(*dest);
                        }
                    }
                }
            }
        }

        let mut stack_structs: HashSet<ValueId> = HashSet::new();
        let mut struct_inits: Vec<(ValueId, usize)> = Vec::new();
        for (s_id, s_size) in all_struct_inits {
            if !escaping_structs.contains(&s_id) {
                stack_structs.insert(s_id);
                struct_inits.push((s_id, s_size));
            }
        }

        // Value types tracking within the function
        let mut value_types: HashMap<ValueId, &'static str> = HashMap::new();
        let mut bool_vids: HashSet<ValueId> = HashSet::new();
        let mut bool_vars: HashSet<String> = HashSet::new();
        for (p_name, p_ty, p_val) in &f.params {
            value_types.insert(*p_val, self.dmir_type_to_llvm(p_ty));
            if p_ty == "Bool" {
                bool_vids.insert(*p_val);
                bool_vars.insert(p_name.clone());
            }
        }

        // Track which class each struct-typed value belongs to so field
        // offsets resolve against the right class (exact match) instead of
        // whichever class happens to declare the field name.
        let mut value_classes: HashMap<ValueId, String> = HashMap::new();
        let mut var_classes: HashMap<String, String> = HashMap::new();
        for (p_name, p_ty, p_val) in &f.params {
            if let Some(c) = class_type_name(p_ty) {
                value_classes.insert(*p_val, c.clone());
                var_classes.insert(p_name.clone(), c.clone());
                if p_name == "this" {
                    var_classes.insert("self".to_string(), c.clone());
                } else if p_name == "self" {
                    var_classes.insert("this".to_string(), c);
                }
            }
        }
        for b in &f.blocks {
            for inst in &b.instructions {
                if let Inst::StructInit {
                    dest, class_name, ..
                } = inst
                {
                    value_classes.insert(*dest, class_name.clone());
                }
            }
        }

        // Collect incoming jump edges for basic block PHI nodes
        let mut incoming_edges: HashMap<BasicBlockId, Vec<(BasicBlockId, Vec<ValueId>)>> =
            HashMap::new();
        for b in &f.blocks {
            match &b.terminator {
                Terminator::Branch { target, args } => {
                    incoming_edges
                        .entry(*target)
                        .or_default()
                        .push((b.id, args.clone()));
                }
                Terminator::CondBranch {
                    then_block,
                    then_args,
                    else_block,
                    else_args,
                    ..
                } => {
                    incoming_edges
                        .entry(*then_block)
                        .or_default()
                        .push((b.id, then_args.clone()));
                    incoming_edges
                        .entry(*else_block)
                        .or_default()
                        .push((b.id, else_args.clone()));
                }
                _ => {}
            }
        }

        let mut var_types: HashMap<String, &'static str> = HashMap::new();
        for (gname, (gty, _)) in &module.globals {
            var_types.insert(gname.clone(), self.dmir_type_to_llvm(gty));
        }
        for (pname, pty, _) in &f.params {
            var_types.insert(pname.clone(), self.dmir_type_to_llvm(pty));
        }
        for vname in &local_vars {
            let dt = types
                .fn_symbol_types
                .get(&(f.name.clone(), vname.clone()))
                .or_else(|| {
                    let base_name = f.name.split("__spec_").next().unwrap_or(&f.name);
                    types
                        .fn_symbol_types
                        .get(&(base_name.to_string(), vname.clone()))
                })
                .or_else(|| {
                    let base_name = f.name.split("__").next().unwrap_or(&f.name);
                    types
                        .fn_symbol_types
                        .get(&(base_name.to_string(), vname.clone()))
                });
            if let Some(dt) = dt {
                let vty = match dt {
                    DataraType::Float => "double",
                    DataraType::Int | DataraType::Bool | DataraType::Char => "i64",
                    DataraType::Class(cls) => match cls.as_str() {
                        "Float4" | "float4" | "f32x4" => "<4 x float>",
                        "Float8" | "float8" | "f32x8" => "<8 x float>",
                        "Float16" | "float16" | "f32x16" => "<16 x float>",
                        "Float2" | "float2" | "f64x2" => "<2 x double>",
                        "Float4_64" | "f64x4" => "<4 x double>",
                        "Int4" | "i32x4" => "<4 x i32>",
                        "i32x8" => "<8 x i32>",
                        _ => "ptr",
                    },
                    _ => "ptr",
                };
                var_types.insert(vname.clone(), vty);
            }
        }

        // Secondary inference pass: check assignment instructions if symbol table lacked type
        for vname in &local_vars {
            if !var_types.contains_key(vname) {
                for b in &f.blocks {
                    for inst in &b.instructions {
                        if let Inst::AssignVar { name, value } = inst {
                            if name == vname {
                                for b2 in &f.blocks {
                                    for inst2 in &b2.instructions {
                                        match inst2 {
                                            Inst::ConstFloat { dest, .. } if dest == value => {
                                                var_types.insert(vname.clone(), "double");
                                            }
                                            Inst::ConstInt { dest, .. }
                                            | Inst::ConstBool { dest, .. }
                                                if dest == value =>
                                            {
                                                var_types.insert(vname.clone(), "i64");
                                            }
                                            Inst::ConstStr { dest, .. }
                                            | Inst::StructInit { dest, .. }
                                                if dest == value =>
                                            {
                                                var_types.insert(vname.clone(), "ptr");
                                            }
                                            Inst::Call { dest, func, ty, .. } if dest == value => {
                                                if func.starts_with("datara_rt_list_")
                                                    || func.starts_with("datara_rt_str_")
                                                    || func.starts_with("mem_alloc")
                                                    || func.starts_with("arena_alloc")
                                                    || func.starts_with("stack_alloc")
                                                {
                                                    var_types.insert(vname.clone(), "ptr");
                                                } else if ty == "Float" {
                                                    var_types.insert(vname.clone(), "double");
                                                } else if ty == "Float4"
                                                    || ty == "float4"
                                                    || ty == "f32x4"
                                                    || func == "float4"
                                                    || func == "datara_rt_float4"
                                                    || func == "f32x4"
                                                {
                                                    var_types.insert(vname.clone(), "<4 x float>");
                                                } else if ty == "Float8"
                                                    || ty == "f32x8"
                                                    || func == "f32x8"
                                                    || func == "datara_rt_f32x8"
                                                {
                                                    var_types.insert(vname.clone(), "<8 x float>");
                                                } else if ty == "Float16"
                                                    || ty == "f32x16"
                                                    || func == "f32x16"
                                                    || func == "datara_rt_f32x16"
                                                {
                                                    var_types.insert(vname.clone(), "<16 x float>");
                                                } else if ty == "Float2"
                                                    || ty == "f64x2"
                                                    || func == "f64x2"
                                                    || func == "datara_rt_f64x2"
                                                {
                                                    var_types.insert(vname.clone(), "<2 x double>");
                                                } else if ty == "Float4_64"
                                                    || ty == "f64x4"
                                                    || func == "f64x4"
                                                    || func == "datara_rt_f64x4"
                                                {
                                                    var_types.insert(vname.clone(), "<4 x double>");
                                                } else if ty == "Int4"
                                                    || ty == "i32x4"
                                                    || func == "i32x4"
                                                    || func == "datara_rt_i32x4"
                                                {
                                                    var_types.insert(vname.clone(), "<4 x i32>");
                                                } else if ty == "i32x8"
                                                    || func == "i32x8"
                                                    || func == "datara_rt_i32x8"
                                                {
                                                    var_types.insert(vname.clone(), "<8 x i32>");
                                                } else if ty == "Str"
                                                    || ty == "String"
                                                    || ty.starts_with("List<")
                                                {
                                                    var_types.insert(vname.clone(), "ptr");
                                                }
                                            }
                                            Inst::BinOp { dest, ty, .. } if dest == value => {
                                                if ty == "Float" {
                                                    var_types.insert(vname.clone(), "double");
                                                } else if ty == "Str" || ty == "String" {
                                                    var_types.insert(vname.clone(), "ptr");
                                                }
                                            }
                                            _ => {}
                                        }
                                        if var_types.contains_key(vname) {
                                            break;
                                        }
                                    }
                                    if var_types.contains_key(vname) {
                                        break;
                                    }
                                }
                            }
                        }
                        if var_types.contains_key(vname) {
                            break;
                        }
                    }
                    if var_types.contains_key(vname) {
                        break;
                    }
                }
            }
        }

        // Allocate local variables in entry block if any
        let has_allocas =
            !local_vars.is_empty() || !f.params.is_empty() || !struct_inits.is_empty();
        if has_allocas {
            out.push_str("entry_allocas:\n");
            for (pname, pty, pval) in &f.params {
                let llvm_pty = self.dmir_type_to_llvm(pty);
                out.push_str(&format!(
                    "  %var_{} = alloca {}, align 8\n",
                    pname, llvm_pty
                ));
                out.push_str(&format!(
                    "  store {} %v{}, ptr %var_{}, align 8\n",
                    llvm_pty, pval.0, pname
                ));
                if pname == "this" {
                    out.push_str(&format!(
                        "  %var_self = alloca {}, align 8\n",
                        llvm_pty
                    ));
                    out.push_str(&format!(
                        "  store {} %v{}, ptr %var_self, align 8\n",
                        llvm_pty, pval.0
                    ));
                } else if pname == "self" {
                    out.push_str(&format!(
                        "  %var_this = alloca {}, align 8\n",
                        llvm_pty
                    ));
                    out.push_str(&format!(
                        "  store {} %v{}, ptr %var_this, align 8\n",
                        llvm_pty, pval.0
                    ));
                }
                if let Some((min, max)) = get_range_for_var(&f.name, pname, Some(pty), types) {
                    out.push_str(&format!(
                        "  %fvrp_pmin_{} = icmp sge i64 %v{}, {}\n",
                        pname, pval.0, min
                    ));
                    out.push_str(&format!(
                        "  call void @llvm.assume(i1 %fvrp_pmin_{})\n",
                        pname
                    ));
                    out.push_str(&format!(
                        "  %fvrp_pmax_{} = icmp sle i64 %v{}, {}\n",
                        pname, pval.0, max
                    ));
                    out.push_str(&format!(
                        "  call void @llvm.assume(i1 %fvrp_pmax_{})\n",
                        pname
                    ));
                }
            }
            for vname in &local_vars {
                if !f.params.iter().any(|(p, _, _)| p == vname)
                    && !module.globals.contains_key(vname)
                {
                    let vty = var_types.get(vname).copied().unwrap_or("i64");
                    let align = if vty.starts_with('<') {
                        match vty {
                            "<16 x float>" => 64,
                            "<8 x float>" | "<4 x double>" | "<8 x i32>" => 32,
                            _ => 16,
                        }
                    } else {
                        8
                    };
                    out.push_str(&format!(
                        "  %var_{} = alloca {}, align {}\n",
                        vname, vty, align
                    ));
                }
            }
            for (s_id, s_size) in &struct_inits {
                let align = if *s_size >= 64 {
                    64
                } else if *s_size >= 32 {
                    32
                } else {
                    16
                };
                out.push_str(&format!(
                    "  %v{} = alloca [{} x i8], align {}\n",
                    s_id.0, s_size, align
                ));
            }
            if let Some(first_block) = f.blocks.first() {
                out.push_str(&format!("  br label %bb{}\n\n", first_block.id.0));
            }
        }

        // Emit blocks
        for block in &f.blocks {
            out.push_str(&format!("bb{}:\n", block.id.0));

            // Emit PHIs for block parameters if present
            if !block.params.is_empty()
                && block.id != f.entry_block
                && let Some(preds) = incoming_edges.get(&block.id)
            {
                for (param_idx, param) in block.params.iter().enumerate() {
                    let param_ty = preds
                        .iter()
                        .find_map(|(_, args)| {
                            args.get(param_idx)
                                .and_then(|a| value_types.get(a).copied())
                        })
                        .or_else(|| param.name.as_ref().and_then(|n| var_types.get(n).copied()))
                        .unwrap_or_else(|| self.dmir_type_to_llvm(&param.ty));
                    value_types.insert(param.val, param_ty);
                    if let Some(ref name) = param.name {
                        if let Some(c) = var_classes.get(name) {
                            value_classes.insert(param.val, c.clone());
                        }
                    }

                    let phi_incoming = preds
                        .iter()
                        .filter_map(|(pred_id, args)| {
                            args.get(param_idx)
                                .map(|arg_val| format!("[ %v{}, %bb{} ]", arg_val.0, pred_id.0))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");

                    if !phi_incoming.is_empty() {
                        out.push_str(&format!(
                            "  %v{} = phi {} {}\n",
                            param.val.0, param_ty, phi_incoming
                        ));
                    }
                }
            }

            // Emit instructions
            for inst in &block.instructions {
                self.emit_instruction(
                    inst,
                    module,
                    strings,
                    &mut value_types,
                    &mut var_types,
                    &mut var_classes,
                    &mut value_classes,
                    &mut bool_vids,
                    &mut bool_vars,
                    &mut out,
                    &f.name,
                    types,
                    range_metadata,
                    next_meta_id,
                    &address_taken,
                    &stack_structs,
                )?;
            }

            // Emit terminator
            match &block.terminator {
                Terminator::Branch { target, .. } => {
                    if target.0 <= block.id.0 {
                        let loop_meta = attributes::derive_loop_metadata(
                            &f.name,
                            block.id,
                            self.profile,
                            loop_metadata_map,
                            next_meta_id,
                        );
                        out.push_str(&format!("  br label %bb{}{}\n", target.0, loop_meta));
                    } else {
                        out.push_str(&format!("  br label %bb{}\n", target.0));
                    }
                }
                Terminator::CondBranch {
                    cond,
                    then_block,
                    else_block,
                    ..
                } => {
                    let cmp_reg = format!("%c_{}_{}", block.id.0, cond.0);
                    out.push_str(&format!("  {} = icmp ne i64 %v{}, 0\n", cmp_reg, cond.0));
                    let branch_meta = attributes::derive_branch_metadata(
                        &f.name,
                        block.id,
                        *then_block,
                        *else_block,
                        f,
                        self.profile,
                        branch_weights_map,
                        next_meta_id,
                    );
                    if then_block.0 <= block.id.0 || else_block.0 <= block.id.0 {
                        let loop_meta = attributes::derive_loop_metadata(
                            &f.name,
                            block.id,
                            self.profile,
                            loop_metadata_map,
                            next_meta_id,
                        );
                        out.push_str(&format!(
                            "  br i1 {}, label %bb{}, label %bb{}{}{}\n",
                            cmp_reg, then_block.0, else_block.0, branch_meta, loop_meta
                        ));
                    } else {
                        out.push_str(&format!(
                            "  br i1 {}, label %bb{}, label %bb{}{}\n",
                            cmp_reg, then_block.0, else_block.0, branch_meta
                        ));
                    }
                }
                Terminator::Return { value } => {
                    if is_main {
                        // Propagate datara_main's Int result as the process
                        // exit code instead of always returning 0.
                        if let (Some(v), "Int" | "Bool") = (value, f.return_type.as_str()) {
                            out.push_str(&format!(
                                "  %main_ret_{} = trunc i64 %v{} to i32\n",
                                v.0, v.0
                            ));
                            out.push_str(&format!("  ret i32 %main_ret_{}\n", v.0));
                        } else {
                            out.push_str("  ret i32 0\n");
                        }
                    } else if f.return_type == "Unit" || f.return_type == "Never" {
                        out.push_str("  ret void\n");
                    } else if let Some(v) = value {
                        let rty = value_types.get(v).copied().unwrap_or("i64");
                        out.push_str(&format!("  ret {} %v{}\n", rty, v.0));
                    } else {
                        out.push_str("  ret void\n");
                    }
                }
                Terminator::Unreachable => {
                    out.push_str("  unreachable\n");
                }
            }
            out.push('\n');
        }

        out.push_str("}\n");
        Ok(out)
    }

    pub(crate) fn find_field_offset(
        &self,
        module: &Module,
        class: Option<&str>,
        field: &str,
        fn_name: &str,
    ) -> Result<usize, String> {
        // Exact class match only: when the object's class is known, only
        // that class's layout may be consulted — an unrelated class that
        // happens to declare the same field name must not win. Generic
        // monomorphizations resolve through their template name.
        if let Some(cls) = class {
            let base_c = cls
                .split('<')
                .next()
                .unwrap_or(cls)
                .split('_')
                .next()
                .unwrap_or(cls);
            if let Some(fields) = module
                .class_fields
                .get(cls)
                .or_else(|| module.class_fields.get(base_c))
            {
                if module.packed_classes.contains(cls) || module.packed_classes.contains(base_c) {
                    let mut cur_offset = 0usize;
                    for f in fields {
                        if f == field {
                            return Ok(cur_offset);
                        }
                        let fkey = format!("{}.{}", cls, f);
                        let fty = module
                            .class_field_types
                            .get(&fkey)
                            .map(|s| s.as_str())
                            .unwrap_or("Int");
                        cur_offset += match fty {
                            "Byte" | "U8" | "I8" | "Bool" | "Char" => 1,
                            "U16" | "I16" => 2,
                            "U32" | "I32" | "F32" => 4,
                            _ => 8,
                        };
                    }
                    return Err(format!(
                        "LLVM codegen failed: [E0944] field '{}' does not exist in the layout of class '{}' in function '{}': refusing cross-class offset fallback",
                        field, cls, fn_name
                    ));
                }
                return fields.iter().position(|f| f == field)
                    .map(|pos| pos.saturating_mul(8))
                    .ok_or_else(|| {
                        format!(
                            "LLVM codegen failed: [E0944] field '{}' does not exist in the layout of class '{}' in function '{}': refusing cross-class offset fallback",
                            field, cls, fn_name
                        )
                    });
            }
        }
        // Class unknown: the historical fallback scan picked whichever class
        // sorted first and declared the same field name, silently reading the
        // wrong slot when two classes share a field name. That is now a
        // compile error (E0944), never a guessed offset.
        Err(format!(
            "LLVM codegen failed: [E0944] cannot resolve the receiver class for field access '.{}' in function '{}': refusing to guess the field offset",
            field, fn_name
        ))
    }
}

/// LLVM Backend implementing `CodegenBackend`.
pub struct LlvmBackend {
    pub target: TargetInfo,
    pub debug_info: bool,
    pub profile: Option<crate::pgo::ProfileData>,
}

impl LlvmBackend {
    pub fn new(target: TargetInfo) -> Self {
        Self {
            target,
            debug_info: false,
            profile: None,
        }
    }

    pub fn with_debug(mut self, debug_info: bool) -> Self {
        self.debug_info = debug_info;
        self
    }

    pub fn with_profile(mut self, profile: Option<crate::pgo::ProfileData>) -> Self {
        self.profile = profile;
        self
    }
}

impl CodegenBackend for LlvmBackend {
    fn emit(&self, module: &Module, program: &Program, types: &TypeChecker) -> String {
        let emitter = LlvmEmitter::new(&self.target)
            .with_debug(self.debug_info)
            .with_profile(self.profile.as_ref());
        emitter
            .emit_module(module, program, types)
            .unwrap_or_else(|e| format!("; LLVM EMISSION ERROR [E0944]: {}\n", e))
    }

    fn compile_to_executable(&self, source: &str, target_path: &Path) -> Result<PathBuf, String> {
        let ll_path = target_path.with_extension("ll");
        std::fs::write(&ll_path, source)
            .map_err(|e| format!("Failed to write LLVM IR to {}: {}", ll_path.display(), e))?;

        let exe_path = if cfg!(windows) {
            target_path.with_extension("exe")
        } else {
            target_path.to_path_buf()
        };

        if find_clang().is_some() {
            let rt_source = crate::runtime::runtime_source_path();
            let rt_archive = crate::runtime::runtime_lib_path();
            let rt_opt = if let Some(ref src) = rt_source {
                Some(src.as_path())
            } else if rt_archive.exists() {
                Some(rt_archive.as_path())
            } else {
                None
            };
            compile_with_clang(&ll_path, rt_opt, &exe_path, "3", None, self.debug_info)?;
            Ok(exe_path)
        } else {
            Err(format!(
                "Clang not found. LLVM IR written to {}. Install Clang to compile with --llvm.",
                ll_path.display()
            ))
        }
    }

    fn target_info(&self) -> TargetInfo {
        self.target.clone()
    }

    fn run_executable(
        &self,
        exe_path: &Path,
        args: &[String],
    ) -> Result<(String, String, i32, u128), String> {
        let start = std::time::Instant::now();
        let mut cmd = Command::new(exe_path);
        cmd.args(args);
        cmd.env_remove("LD_PRELOAD");
        // Set detect_leaks=0 to suppress LeakSanitizer on compiled child binaries.
        if std::env::var("ASAN_OPTIONS").is_ok() {
            cmd.env("ASAN_OPTIONS", "detect_leaks=0");
        } else {
            cmd.env_remove("ASAN_OPTIONS");
        }
        let out = cmd
            .output()
            .map_err(|e| format!("Failed to run executable {}: {}", exe_path.display(), e))?;
        let elapsed = start.elapsed().as_nanos();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(-1);
        Ok((stdout, stderr, code, elapsed))
    }
}
