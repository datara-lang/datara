//! v1.4.4: Concurrency and Autonomous Scratchpad Memory prelude.
//!
//! Registers `ThreadHandle`, `Channel`, and thread/channel/parallel/scratchpad builtins.
//! Kept in this dedicated file so `prelude.rs` remains strictly under the 60 KB limit.

use super::{DataraType, TypeChecker};
use std::collections::HashMap;

impl<'a> TypeChecker<'a> {
    /// Registers Concurrency and Scratchpad classes and functions into the prelude tables.
    pub(crate) fn register_concurrency(
        class_fields: &mut HashMap<String, HashMap<String, DataraType>>,
        class_methods: &mut HashMap<String, HashMap<String, DataraType>>,
        function_signatures: &mut HashMap<String, (Vec<DataraType>, DataraType, Vec<String>)>,
    ) {
        // --- 1. ThreadHandle Class ---
        class_fields.insert("ThreadHandle".to_string(), HashMap::new());
        let mut th_methods = HashMap::new();
        th_methods.insert("join".to_string(), DataraType::Int);
        th_methods.insert("join_timeout".to_string(), DataraType::Int);
        th_methods.insert("is_alive".to_string(), DataraType::Bool);
        th_methods.insert("free".to_string(), DataraType::Unit);
        class_methods.insert("ThreadHandle".to_string(), th_methods);

        // --- 2. Channel Class ---
        class_fields.insert("Channel".to_string(), HashMap::new());
        let mut ch_methods = HashMap::new();
        ch_methods.insert("send".to_string(), DataraType::Bool);
        ch_methods.insert(
            "recv".to_string(),
            DataraType::GenericInstance {
                name: "Outcome".into(),
                args: vec![DataraType::Int],
            },
        );
        ch_methods.insert(
            "try_recv".to_string(),
            DataraType::GenericInstance {
                name: "Maybe".into(),
                args: vec![DataraType::Int],
            },
        );
        ch_methods.insert(
            "recv_timeout".to_string(),
            DataraType::GenericInstance {
                name: "Outcome".into(),
                args: vec![DataraType::Int],
            },
        );
        ch_methods.insert("close".to_string(), DataraType::Unit);
        ch_methods.insert("len".to_string(), DataraType::Int);
        ch_methods.insert("is_closed".to_string(), DataraType::Bool);
        ch_methods.insert("free".to_string(), DataraType::Unit);
        class_methods.insert("Channel".to_string(), ch_methods);

        // --- 3. Top-Level Functions ---
        function_signatures.insert(
            "spawn".to_string(),
            (
                vec![DataraType::Val],
                DataraType::Class("ThreadHandle".into()),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "join".to_string(),
            (
                vec![DataraType::Class("ThreadHandle".into())],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "join_timeout".to_string(),
            (
                vec![DataraType::Class("ThreadHandle".into()), DataraType::Int],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "channel_create".to_string(),
            (
                vec![DataraType::Int],
                DataraType::Class("Channel".into()),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "channel_new".to_string(),
            (
                Vec::new(),
                DataraType::Class("Channel".into()),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "channel_send".to_string(),
            (
                vec![DataraType::Class("Channel".into()), DataraType::Int],
                DataraType::Bool,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "channel_recv".to_string(),
            (
                vec![DataraType::Class("Channel".into())],
                DataraType::GenericInstance {
                    name: "Outcome".into(),
                    args: vec![DataraType::Int],
                },
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "channel_try_recv".to_string(),
            (
                vec![DataraType::Class("Channel".into())],
                DataraType::GenericInstance {
                    name: "Maybe".into(),
                    args: vec![DataraType::Int],
                },
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "channel_close".to_string(),
            (
                vec![DataraType::Class("Channel".into())],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "channel_len".to_string(),
            (
                vec![DataraType::Class("Channel".into())],
                DataraType::Int,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "parallel_for".to_string(),
            (
                vec![DataraType::Int, DataraType::Int, DataraType::Val],
                DataraType::Unit,
                Vec::new(),
            ),
        );

        // --- 4. Autonomous Scratchpad Memory Builtins ---
        function_signatures.insert(
            "scratch_enter".to_string(),
            (Vec::new(), DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "scratch_alloc".to_string(),
            (
                vec![DataraType::Int, DataraType::Int],
                DataraType::RawPtr,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "scratch_exit".to_string(),
            (vec![DataraType::Int], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "scratch_promote".to_string(),
            (
                vec![DataraType::RawPtr, DataraType::Int],
                DataraType::RawPtr,
                Vec::new(),
            ),
        );
    }
}
