use cranelift_codegen::ir::{InstBuilder, MachMemFlags, Value as ClifValue, types as clif_types};
use cranelift_frontend::FunctionBuilder;
use std::collections::HashMap;

/// Resolves the byte offset of `field` on a receiver of class `cls`,
/// falling back to the generic template name only (never to another
/// class's layout). Missing field is E0944, not a guessed offset.
pub fn resolve_field_offset(
    class_field_offsets: &HashMap<String, HashMap<String, i32>>,
    cls: &str,
    field: &str,
    fn_name: &str,
) -> Result<i32, String> {
    let base_c = cls
        .split('<')
        .next()
        .unwrap_or(cls)
        .split('_')
        .next()
        .unwrap_or(cls);
    class_field_offsets
        .get(cls)
        .or_else(|| class_field_offsets.get(base_c))
        .and_then(|m| m.get(field).copied())
        .ok_or_else(|| {
            format!(
                "Code generation failed: [E0944] field '{}' does not exist in the layout of class '{}' in function '{}': refusing cross-class offset fallback",
                field, cls, fn_name
            )
        })
}

pub fn get_field_type_size(fty: &str) -> u32 {
    crate::dmir::ir::dm_repr_byte_size(fty)
}

pub fn emit_field_store(
    builder: &mut FunctionBuilder,
    flags: MachMemFlags,
    base_addr: ClifValue,
    offset: i32,
    val: ClifValue,
    field_type: &str,
    is_packed: bool,
) {
    let v_ty = builder.func.dfg.value_type(val);
    if is_packed {
        match field_type {
            "Byte" | "U8" | "UInt8" | "I8" | "Int8" | "Bool" | "Char" => {
                let v_i8 = if v_ty == clif_types::I8 {
                    val
                } else {
                    builder.ins().ireduce(clif_types::I8, val)
                };
                builder.ins().store(flags, v_i8, base_addr, offset);
            }
            "U16" | "UInt16" | "I16" | "Int16" => {
                let v_i16 = if v_ty == clif_types::I16 {
                    val
                } else {
                    builder.ins().ireduce(clif_types::I16, val)
                };
                builder.ins().store(flags, v_i16, base_addr, offset);
            }
            "U32" | "UInt32" | "I32" | "Int32" => {
                let v_i32 = if v_ty == clif_types::I32 {
                    val
                } else {
                    builder.ins().ireduce(clif_types::I32, val)
                };
                builder.ins().store(flags, v_i32, base_addr, offset);
            }
            "F32" | "Float32" => {
                let v_f32 = if v_ty == clif_types::F32 {
                    val
                } else {
                    builder.ins().fdemote(clif_types::F32, val)
                };
                builder.ins().store(flags, v_f32, base_addr, offset);
            }
            _ => {
                let val_to_store =
                    if v_ty == clif_types::I8 || v_ty == clif_types::I32 || v_ty == clif_types::I16
                    {
                        builder.ins().sextend(clif_types::I64, val)
                    } else {
                        val
                    };
                builder.ins().store(flags, val_to_store, base_addr, offset);
            }
        }
    } else {
        let val_to_store = if v_ty == clif_types::I8 || v_ty == clif_types::I32 {
            builder.ins().sextend(clif_types::I64, val)
        } else {
            val
        };
        builder.ins().store(flags, val_to_store, base_addr, offset);
    }
}

pub fn emit_field_load(
    builder: &mut FunctionBuilder,
    flags: MachMemFlags,
    base_addr: ClifValue,
    offset: i32,
    field_type: &str,
    is_float: bool,
    is_packed: bool,
) -> ClifValue {
    if is_float {
        if is_packed && field_type == "F32" {
            let raw = builder
                .ins()
                .load(clif_types::F32, flags, base_addr, offset);
            builder.ins().fpromote(clif_types::F64, raw)
        } else {
            builder
                .ins()
                .load(clif_types::F64, flags, base_addr, offset)
        }
    } else    if is_packed {
        match field_type {
            // v1.4.5 W1: canonical narrow names route identically to the
            // legacy short reprs via explicit arms below.
            "Byte" | "U8" | "UInt8" | "Bool" | "Char" => {
                let raw = builder.ins().load(clif_types::I8, flags, base_addr, offset);
                builder.ins().uextend(clif_types::I64, raw)
            }
            "I8" | "Int8" => {
                let raw = builder.ins().load(clif_types::I8, flags, base_addr, offset);
                builder.ins().sextend(clif_types::I64, raw)
            }
            "U16" | "UInt16" => {
                let raw = builder
                    .ins()
                    .load(clif_types::I16, flags, base_addr, offset);
                builder.ins().uextend(clif_types::I64, raw)
            }
            "I16" | "Int16" => {
                let raw = builder
                    .ins()
                    .load(clif_types::I16, flags, base_addr, offset);
                builder.ins().sextend(clif_types::I64, raw)
            }
            "U32" | "UInt32" => {
                let raw = builder
                    .ins()
                    .load(clif_types::I32, flags, base_addr, offset);
                builder.ins().uextend(clif_types::I64, raw)
            }
            "I32" | "Int32" => {
                let raw = builder
                    .ins()
                    .load(clif_types::I32, flags, base_addr, offset);
                builder.ins().sextend(clif_types::I64, raw)
            }
            _ => builder
                .ins()
                .load(clif_types::I64, flags, base_addr, offset),
        }
    } else {
        builder
            .ins()
            .load(clif_types::I64, flags, base_addr, offset)
    }
}
