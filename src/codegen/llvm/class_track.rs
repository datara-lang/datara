//! Value class tracking for LLVM field-offset resolution.
//!
//! GetField/SetField must resolve offsets against the receiver's OWN
//! class layout. These helpers decide whether a declared type denotes a
//! struct and look up the declared class of a field so that chained
//! access (a.b.c) tracks per value. Mirrors the Cranelift backend's
//! class recording; an unresolvable receiver is a compile error
//! (E0944), never a cross-class offset guess.

use crate::dmir::Module;

use super::LlvmEmitter;

/// Returns the class name when `ty` denotes a struct type (never a
/// primitive, collection or Unit).
pub(crate) fn class_type_name(ty: &str) -> Option<String> {
    let stripped = ty.split('<').next().unwrap_or(ty);
    if stripped.is_empty()
        || matches!(
            stripped,
            "Int"
                | "Int8"
                | "Int16"
                | "Int32"
                | "Int64"
                | "UInt"
                | "UInt8"
                | "UInt16"
                | "UInt32"
                | "UInt64"
                | "USize"
                | "isize"
                | "usize"
                | "Float"
                | "Float32"
                | "Float64"
                | "f16"
                | "f32"
                | "f64"
                | "Bool"
                | "Str"
                | "String"
                | "Char"
                | "Byte"
                | "Unit"
                | "Void"
                | "Never"
                | "List"
                | "Map"
                | "Any"
                | "dec64"
                | "dec128"
        )
        || stripped.starts_with('[')
    {
        return None;
    }
    Some(stripped.to_string())
}

impl<'a> LlvmEmitter<'a> {
    /// Returns the declared class of a field when the receiver's class is
    /// known, consulting the module's class field types (with the generic
    /// template name as fallback). Used to track chained field access.
    pub(crate) fn field_declared_class(
        &self,
        module: &Module,
        class: Option<&str>,
        field: &str,
    ) -> Option<String> {
        let cls = class?;
        let base_c = cls
            .split('<')
            .next()
            .unwrap_or(cls)
            .split('_')
            .next()
            .unwrap_or(cls);
        for key in [
            format!("{}.{}", cls, field),
            format!("{}.{}", base_c, field),
        ] {
            if let Some(ft) = module.class_field_types.get(&key)
                && let Some(c) = class_type_name(ft)
            {
                return Some(c);
            }
        }
        None
    }
}
