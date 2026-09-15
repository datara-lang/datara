//! v1.4.0: the `StrBuf` prelude class.
//!
//! `StrBuf` is a growable string builder for hot string-assembly paths. It
//! lives in its own file to keep `prelude.rs` under the 60 KB source limit;
//! `TypeChecker::new` calls [`register_strbuf`] after the other builtins.
//!
//! # Surface
//!
//! * `StrBuf { }` (or `StrBuf.new()` where a class-name receiver resolves) —
//!   constructs a builder backed by the runtime's amortized-doubling buffer.
//! * `push(s: Str) -> StrBuf` — appends a string, returns the builder so
//!   calls chain.
//! * `push_int(i: Int) -> StrBuf` — appends the decimal form of an integer.
//! * `join() -> Str` — materializes the builder as a `Str`. The result goes
//!   through the runtime's never-freed scratch ring, so it stays valid for
//!   the rest of the call (the same ownership conventions as every other
//!   runtime string); the builder keeps accumulating afterwards.
//! * `len() -> Int` — current byte length.
//!
//! # Runtime
//!
//! Backed by `datara_rt_strbuf_{new,push,push_int,join,len}` in
//! `datara_runtime.c`, dispatched in the Cranelift backend's method-call
//! table on the receiver's declared class (`StrBuf`).

use super::{DataraType, TypeChecker};
use std::collections::HashMap;

impl<'a> TypeChecker<'a> {
    /// Registers the `StrBuf` class in the prelude tables.
    pub(crate) fn register_strbuf(
        class_fields: &mut HashMap<String, HashMap<String, DataraType>>,
        class_methods: &mut HashMap<String, HashMap<String, DataraType>>,
    ) {
        // StrBuf is opaque: no user-visible fields.
        class_fields.insert("StrBuf".to_string(), HashMap::new());

        let mut methods = HashMap::new();
        methods.insert("new".to_string(), DataraType::Class("StrBuf".into()));
        methods.insert("push".to_string(), DataraType::Class("StrBuf".into()));
        methods.insert("push_int".to_string(), DataraType::Class("StrBuf".into()));
        methods.insert("join".to_string(), DataraType::String);
        methods.insert("len".to_string(), DataraType::Int);
        class_methods.insert("StrBuf".to_string(), methods);
    }
}
