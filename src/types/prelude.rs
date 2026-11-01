use super::{DataraType, TypeChecker};
use crate::resolver::Resolver;
use std::collections::{HashMap, HashSet};

// List<T> builtin-method documentation (v1.4.1). List methods are matched
// structurally in src/types/check/call.rs, not registered here, so the
// canonical method contract lives in this comment:
//
//   push(v)   -- CANONICAL method to append a value and grow the receiver.
//   append(v) -- explicit "alias of push": identical lowering, identical
//                runtime symbol (datara_rt_list_append), kept for
//                discoverability.
//   add(v)    -- accepted third spelling of push at the dispatch sites
//                (lowering + codegen); documented as an alias too.
//   pop()     -- checked: returns Outcome<T> ("empty list" on an empty
//                receiver), never a bare element.
//   first()/last() -- checked element accessors returning Outcome<T>.
//   sort()/reverse()/clear()/insert_at() -- in-place mutators returning the
//                receiver handle; remove_at/remove_value return Int (1 ok,
//                0 nothing changed); contains/is_empty -> Bool;
//                index_of -> Int (-1 when absent); slice -> fresh List<T>.
impl<'a> TypeChecker<'a> {
    pub fn new(resolver: &'a Resolver) -> Self {
        let mut class_fields = HashMap::new();
        let mut class_methods = HashMap::new();
        let mut generic_templates = HashMap::new();
        let mut function_signatures = HashMap::new();
        let fn_symbol_types = HashMap::new();

        // Built-in prelude function signatures
        // Capabilities & Safe OS Resources prelude
        let mut sys_caps_fields = HashMap::new();
        sys_caps_fields.insert(
            "files".to_string(),
            DataraType::Class("FileCapabilityProvider".into()),
        );
        sys_caps_fields.insert(
            "net".to_string(),
            DataraType::Class("NetCapabilityProvider".into()),
        );
        sys_caps_fields.insert(
            "proc".to_string(),
            DataraType::Class("ProcessCapabilityProvider".into()),
        );
        class_fields.insert("SystemCapabilities".to_string(), sys_caps_fields);

        let mut file_prov_methods = HashMap::new();
        file_prov_methods.insert(
            "grant_readonly".to_string(),
            DataraType::GenericInstance {
                name: "Capability".into(),
                args: vec![DataraType::Class("FileRead".into())],
            },
        );
        file_prov_methods.insert(
            "grant_readwrite".to_string(),
            DataraType::GenericInstance {
                name: "Capability".into(),
                args: vec![DataraType::Class("FileWrite".into())],
            },
        );
        class_methods.insert("FileCapabilityProvider".to_string(), file_prov_methods);

        let mut net_prov_methods = HashMap::new();
        net_prov_methods.insert(
            "grant_connect".to_string(),
            DataraType::GenericInstance {
                name: "Capability".into(),
                args: vec![DataraType::Class("NetworkConnect".into())],
            },
        );
        net_prov_methods.insert(
            "grant_listen".to_string(),
            DataraType::GenericInstance {
                name: "Capability".into(),
                args: vec![DataraType::Class("NetworkListen".into())],
            },
        );
        class_methods.insert("NetCapabilityProvider".to_string(), net_prov_methods);

        let mut proc_prov_methods = HashMap::new();
        proc_prov_methods.insert(
            "grant_exec".to_string(),
            DataraType::GenericInstance {
                name: "Capability".into(),
                args: vec![DataraType::Class("ProcessExec".into())],
            },
        );
        class_methods.insert("ProcessCapabilityProvider".to_string(), proc_prov_methods);

        let mut file_handle_methods = HashMap::new();
        file_handle_methods.insert("read_all".to_string(), DataraType::String);
        file_handle_methods.insert("read_line".to_string(), DataraType::String);
        file_handle_methods.insert("close".to_string(), DataraType::Unit);
        class_methods.insert("FileHandle".to_string(), file_handle_methods);

        let mut file_write_handle_methods = HashMap::new();
        file_write_handle_methods.insert("write".to_string(), DataraType::Int);
        file_write_handle_methods.insert("write_all".to_string(), DataraType::Int);
        file_write_handle_methods.insert("close".to_string(), DataraType::Unit);
        class_methods.insert("FileWriteHandle".to_string(), file_write_handle_methods);

        Self::register_core_signatures(&mut function_signatures);

        for (name, sym) in &resolver.functions {
            let mut p_types = Vec::new();
            if let Some(params) = &sym.type_node {
                p_types.push(Self::resolve_tn(resolver, params));
            }
            let ret = sym
                .return_type
                .as_ref()
                .map(|t| Self::resolve_tn(resolver, t))
                .unwrap_or(DataraType::Unit);
            function_signatures.insert(name.clone(), (p_types, ret, sym.generic_params.clone()));
        }

        for (name, sym) in &resolver.classes {
            let mut fields = HashMap::new();
            // 1. Base class fields (recursive)
            let mut curr_base = sym.base_type.clone();
            let mut visited_bases = Vec::new();
            while let Some(b_name) = curr_base {
                if visited_bases.contains(&b_name) {
                    break;
                }
                visited_bases.push(b_name.clone());
                if let Some(b_sym) = resolver.classes.get(&b_name) {
                    for (f_name, f_sym) in &b_sym.fields {
                        let f_type = f_sym
                            .type_node
                            .as_ref()
                            .map(|t| Self::resolve_tn(resolver, t))
                            .unwrap_or(DataraType::String);
                        fields.insert(f_name.clone(), f_type);
                    }
                    for comp_name in &b_sym.compositions {
                        if let Some(comp_sym) = resolver.components.get(comp_name) {
                            for (f_name, f_sym) in &comp_sym.fields {
                                let f_type = f_sym
                                    .type_node
                                    .as_ref()
                                    .map(|t| Self::resolve_tn(resolver, t))
                                    .unwrap_or(DataraType::String);
                                fields.insert(f_name.clone(), f_type);
                            }
                        }
                    }
                    curr_base = b_sym.base_type.clone();
                } else {
                    break;
                }
            }

            // 2. Composed components
            for comp_name in &sym.compositions {
                if let Some(comp_sym) = resolver.components.get(comp_name) {
                    for (f_name, f_sym) in &comp_sym.fields {
                        let f_type = f_sym
                            .type_node
                            .as_ref()
                            .map(|t| Self::resolve_tn(resolver, t))
                            .unwrap_or(DataraType::String);
                        fields.insert(f_name.clone(), f_type);
                    }
                }
            }

            // 3. Own fields
            for (f_name, f_sym) in &sym.fields {
                let f_type = f_sym
                    .type_node
                    .as_ref()
                    .map(|t| Self::resolve_tn(resolver, t))
                    .unwrap_or(DataraType::String);
                fields.insert(f_name.clone(), f_type);
            }
            class_fields.insert(name.clone(), fields.clone());

            let mut methods = HashMap::new();
            for (m_name, m_sym) in &sym.methods {
                let m_type = m_sym
                    .return_type
                    .as_ref()
                    .map(|t| Self::resolve_tn(resolver, t))
                    .unwrap_or(DataraType::Unit);
                methods.insert(m_name.clone(), m_type);
            }
            class_methods.insert(name.clone(), methods);

            if !sym.generic_params.is_empty() {
                generic_templates.insert(name.clone(), (sym.generic_params.clone(), fields));
            }
        }

        // v1.4.0: StrBuf prelude class lives in its own file (prelude.rs
        // stays under the 60 KB limit).
        Self::register_strbuf(&mut class_fields, &mut class_methods);
        Self::register_concurrency(
            &mut class_fields,
            &mut class_methods,
            &mut function_signatures,
        );
        Self::register_systems(
            &mut class_fields,
            &mut class_methods,
            &mut function_signatures,
        );

        Self {
            resolver,
            symbol_types: HashMap::new(),
            symbol_mutability: HashMap::new(),
            symbol_decl_spans: HashMap::new(),
            function_signatures,
            class_fields,
            class_methods,
            generic_templates,
            generic_specializations: HashMap::new(),
            current_return_type: None,
            propagation_sites: Vec::new(),
            var_element_types: HashMap::new(),
            last_list_element: None,
            current_fn_name: None,
            fn_symbol_types,
            var_refinements: HashMap::new(),
            function_param_nodes: HashMap::new(),
            var_array_lengths: HashMap::new(),
            traits: HashMap::new(),
            roles: HashMap::new(),
            impls: HashMap::new(),
            trait_bounds: HashMap::new(),
            current_target_type: None,
            program_module_aliases: HashMap::new(),
            bridge_functions: HashSet::new(),
            expr_depth: 0,
            loop_depth: 0,
            active_requires: Vec::new(),
            in_unsafe: false,
            allowed_devices: HashSet::new(),
            current_assign_target: None,
            source_text: None,
        }
    }
}
