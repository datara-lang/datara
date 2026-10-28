//! M10 NatVar: fiber runtime checker signatures (split out of prelude.rs
//! for the 60 KB release size gate).

use crate::types::DataraType;
use std::collections::HashMap;

pub fn insert_fiber_signatures(
    function_signatures: &mut HashMap<String, (Vec<DataraType>, DataraType, Vec<String>)>,
) {
        // v1.5.0 (M10): NatVar fiber runtime signatures.
        // spawn(f, arg[, stack_kb]): f lowers via GetFuncAddr (RawPtr-ABI);
        // join -> Int; drain/counters/chan_create/chan_recv -> Int;
        // yield/sleep/send -> Unit.
        for spawn_name in &["fiber_spawn", "datara_rt_fiber_spawn"] {
            function_signatures.insert(
                spawn_name.to_string(),
                (
                    vec![DataraType::RawPtr, DataraType::Int, DataraType::Int],
                    DataraType::Int,
                    Vec::new(),
                ),
            );
        }
        for zero_unit_name in &["fiber_yield", "datara_rt_fiber_yield"] {
            function_signatures.insert(
                zero_unit_name.to_string(),
                (vec![], DataraType::Unit, Vec::new()),
            );
        }
        for sleep_name in &["fiber_sleep_ms", "datara_rt_fiber_sleep_ms"] {
            function_signatures.insert(
                sleep_name.to_string(),
                (vec![DataraType::Int], DataraType::Unit, Vec::new()),
            );
        }
        for send_name in &["fiber_chan_send", "datara_rt_fiber_chan_send"] {
            function_signatures.insert(
                send_name.to_string(),
                (vec![DataraType::Int, DataraType::Int], DataraType::Unit, Vec::new()),
            );
        }
        for zero_int_name in &[
            "fiber_run_until_done",
            "datara_rt_fiber_run_until_done",
            "fiber_count",
            "datara_rt_fiber_count",
            "fiber_done_count",
            "datara_rt_fiber_done_count",
            "fiber_chan_create",
            "datara_rt_fiber_chan_create",
        ] {
            function_signatures.insert(
                zero_int_name.to_string(),
                (vec![], DataraType::Int, Vec::new()),
            );
        }
        for take_int_name in &["fiber_join", "datara_rt_fiber_join", "fiber_chan_recv", "datara_rt_fiber_chan_recv"] {
            function_signatures.insert(
                take_int_name.to_string(),
                (vec![DataraType::Int], DataraType::Int, Vec::new()),
            );
        }
}
