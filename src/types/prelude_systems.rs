//! v1.4.4: Universal Systems, Networking & Binary Layout Primitives prelude.
//!
//! Registers `SliceView` and endian conversion / byte-swapping functions.
//! Kept in this dedicated file so `prelude.rs` remains strictly under the 60 KB limit.

use super::{DataraType, TypeChecker};
use std::collections::HashMap;

impl<'a> TypeChecker<'a> {
    /// Registers Systems, Networking and Binary primitives into the prelude tables.
    pub(crate) fn register_systems(
        class_fields: &mut HashMap<String, HashMap<String, DataraType>>,
        class_methods: &mut HashMap<String, HashMap<String, DataraType>>,
        function_signatures: &mut HashMap<String, (Vec<DataraType>, DataraType, Vec<String>)>,
    ) {
        // --- 1. SliceView Class ---
        let mut slice_fields = HashMap::new();
        slice_fields.insert("ptr".to_string(), DataraType::RawPtr);
        slice_fields.insert("len".to_string(), DataraType::Int);
        slice_fields.insert("capacity".to_string(), DataraType::Int);
        class_fields.insert("SliceView".to_string(), slice_fields);

        let mut slice_methods = HashMap::new();
        slice_methods.insert("len".to_string(), DataraType::Int);
        slice_methods.insert("get_byte".to_string(), DataraType::Int);
        slice_methods.insert("set_byte".to_string(), DataraType::Unit);
        slice_methods.insert("read_u16_be".to_string(), DataraType::Int);
        slice_methods.insert("read_u16_le".to_string(), DataraType::Int);
        slice_methods.insert("read_u32_be".to_string(), DataraType::Int);
        slice_methods.insert("read_u32_le".to_string(), DataraType::Int);
        slice_methods.insert("read_u64_be".to_string(), DataraType::Int);
        slice_methods.insert("read_u64_le".to_string(), DataraType::Int);
        slice_methods.insert("write_u16_be".to_string(), DataraType::Unit);
        slice_methods.insert("write_u16_le".to_string(), DataraType::Unit);
        slice_methods.insert("write_u32_be".to_string(), DataraType::Unit);
        slice_methods.insert("write_u32_le".to_string(), DataraType::Unit);
        slice_methods.insert("write_u64_be".to_string(), DataraType::Unit);
        slice_methods.insert("write_u64_le".to_string(), DataraType::Unit);
        slice_methods.insert("subslice".to_string(), DataraType::Class("SliceView".into()));
        slice_methods.insert("free".to_string(), DataraType::Unit);
        class_methods.insert("SliceView".to_string(), slice_methods);

        // --- 2. Endian & Byte-Swap Functions ---
        function_signatures.insert(
            "hton16".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "ntoh16".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "hton32".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "ntoh32".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "hton64".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "ntoh64".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "bswap16".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "bswap32".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "bswap64".to_string(),
            (vec![DataraType::Int], DataraType::Int, Vec::new()),
        );

        // --- 3. Slice Constructors & Freers ---
        function_signatures.insert(
            "slice_alloc".to_string(),
            (
                vec![DataraType::Int],
                DataraType::Class("SliceView".into()),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "slice_from_buffer".to_string(),
            (
                vec![DataraType::RawPtr, DataraType::Int],
                DataraType::Class("SliceView".into()),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "slice_free".to_string(),
            (
                vec![DataraType::Class("SliceView".into())],
                DataraType::Unit,
                Vec::new(),
            ),
        );

        // --- 4. Level 4: VolatilePtr Class (MMIO & Hardware Registers) ---
        let mut volatile_fields = HashMap::new();
        volatile_fields.insert("addr".to_string(), DataraType::RawPtr);
        class_fields.insert("VolatilePtr".to_string(), volatile_fields);

        let mut volatile_methods = HashMap::new();
        volatile_methods.insert("read8".to_string(), DataraType::Int);
        volatile_methods.insert("read16".to_string(), DataraType::Int);
        volatile_methods.insert("read32".to_string(), DataraType::Int);
        volatile_methods.insert("read64".to_string(), DataraType::Int);
        volatile_methods.insert("write8".to_string(), DataraType::Unit);
        volatile_methods.insert("write16".to_string(), DataraType::Unit);
        volatile_methods.insert("write32".to_string(), DataraType::Unit);
        volatile_methods.insert("write64".to_string(), DataraType::Unit);
        class_methods.insert("VolatilePtr".to_string(), volatile_methods);

        // --- 5. Level 4: Top-Level Hardware Intrinsics ---
        function_signatures.insert(
            "volatile_ptr".to_string(),
            (
                vec![DataraType::RawPtr],
                DataraType::Class("VolatilePtr".into()),
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "volatile_read8".to_string(),
            (vec![DataraType::RawPtr], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "volatile_read16".to_string(),
            (vec![DataraType::RawPtr], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "volatile_read32".to_string(),
            (vec![DataraType::RawPtr], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "volatile_read64".to_string(),
            (vec![DataraType::RawPtr], DataraType::Int, Vec::new()),
        );
        function_signatures.insert(
            "volatile_write8".to_string(),
            (
                vec![DataraType::RawPtr, DataraType::Int],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "volatile_write16".to_string(),
            (
                vec![DataraType::RawPtr, DataraType::Int],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "volatile_write32".to_string(),
            (
                vec![DataraType::RawPtr, DataraType::Int],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "volatile_write64".to_string(),
            (
                vec![DataraType::RawPtr, DataraType::Int],
                DataraType::Unit,
                Vec::new(),
            ),
        );
        function_signatures.insert(
            "atomic_fence_acquire".to_string(),
            (vec![], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "atomic_fence_release".to_string(),
            (vec![], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "atomic_fence_acq_rel".to_string(),
            (vec![], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "atomic_fence_seq_cst".to_string(),
            (vec![], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "atomic_fence".to_string(),
            (vec![DataraType::String], DataraType::Unit, Vec::new()),
        );
        function_signatures.insert(
            "typed_zero_init".to_string(),
            (vec![DataraType::Int], DataraType::RawPtr, Vec::new()),
        );
    }
}
