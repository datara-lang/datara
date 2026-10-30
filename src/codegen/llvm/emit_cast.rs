//! LLVM lowering of DMIR `Inst::Cast` (extracted from emit_inst.rs for the
//! 60 KB release size gate). Mirrors the Cranelift cast matrix in
//! cranelift/backend/compile_func.rs — every DMIR Cast must produce a
//! defining .ll line.

use super::*;
use std::collections::HashMap;

pub(crate) fn emit_cast(
    dest: ValueId,
    value: ValueId,
    from_ty: &str,
    to_ty: &str,
    value_types: &mut HashMap<ValueId, &'static str>,
    bool_vids: &mut HashSet<ValueId>,
    f32_vids: &mut HashSet<ValueId>,
    out: &mut String,
) {
    let is_f32 = |t: &str| t == "Float32";
    let is_f64 = |t: &str| t == "Float" || t == "Dec64";
    let is_i = |t: &str| {
        matches!(
            t,
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
                | "Bool"
                | "Char"
        )
    };
    if is_f64(from_ty) && is_i(to_ty) {
        value_types.insert(dest, "i64");
        out.push_str(&format!(
            "  %v{} = fptosi double %v{} to i64\n",
            dest.0, value.0
        ));
    } else if is_i(from_ty) && is_f64(to_ty) {
        value_types.insert(dest, "double");
        out.push_str(&format!(
            "  %v{} = sitofp i64 %v{} to double\n",
            dest.0, value.0
        ));
    } else if is_f32(from_ty) && is_i(to_ty) {
        value_types.insert(dest, "i64");
        out.push_str(&format!(
            "  %v{} = fptosi float %v{} to i64\n",
            dest.0, value.0
        ));
    } else if is_i(from_ty) && is_f32(to_ty) {
        value_types.insert(dest, "float");
        f32_vids.insert(dest);
        out.push_str(&format!(
            "  %v{} = sitofp i64 %v{} to float\n",
            dest.0, value.0
        ));
    } else if is_f64(from_ty) && is_f32(to_ty) {
        value_types.insert(dest, "float");
        f32_vids.insert(dest);
        out.push_str(&format!(
            "  %v{} = fptrunc double %v{} to float\n",
            dest.0, value.0
        ));
    } else if is_f32(from_ty) && is_f64(to_ty) {
        value_types.insert(dest, "double");
        out.push_str(&format!(
            "  %v{} = fpext float %v{} to double\n",
            dest.0, value.0
        ));
    } else if is_i(from_ty) && is_i(to_ty) {
        // Integer<->integer narrowing matches Cranelift: reduce to the
        // target width, then re-extend to the i64 carrier (zext for
        // unsigned targets, sext for signed ones). Bool/Char already live
        // in the i64 carrier, so a same-width cast is the identity.
        let narrow = match to_ty {
            "Int8" | "UInt8" => Some("i8"),
            "Int16" | "UInt16" => Some("i16"),
            "Int32" | "UInt32" => Some("i32"),
            _ => None,
        };
        match narrow {
            Some(nw) => {
                value_types.insert(dest, "i64");
                let tmp = format!("%cast_n_{}", dest.0);
                out.push_str(&format!("  {} = trunc i64 %v{} to {}\n", tmp, value.0, nw));
                if to_ty.starts_with('U') {
                    out.push_str(&format!("  %v{} = zext {} {} to i64\n", dest.0, nw, tmp));
                } else {
                    out.push_str(&format!("  %v{} = sext {} {} to i64\n", dest.0, nw, tmp));
                }
            }
            None => identity_or_convert(dest, value, to_ty, value_types, bool_vids, out),
        }
    } else {
        // Fallback family (Bool->Float, Float32->Float32, casts from Str /
        // Char targets): keep the carrier type instead of emitting a
        // type-illegal instruction — same identity semantics as the
        // Cranelift fallback.
        let src_ty = value_types.get(&value).copied().unwrap_or("i64");
        match src_ty {
            "double" => {
                value_types.insert(dest, "double");
                out.push_str(&format!(
                    "  %v{} = fadd double %v{}, 0.0\n",
                    dest.0, value.0
                ));
            }
            "float" => {
                value_types.insert(dest, "float");
                f32_vids.insert(dest);
                out.push_str(&format!("  %v{} = fadd float %v{}, 0.0\n", dest.0, value.0));
            }
            _ => identity_or_convert(dest, value, to_ty, value_types, bool_vids, out),
        }
    }
}

/// Identity-or-convert int-to-int emission: shared by the i->i arm and the
/// fallback family. The DMIR from_ty says int, but if an upstream inference
/// gap left a double/float/ptr carrier, convert from the REAL carrier
/// instead of emitting a type-illegal `or i64`.
fn identity_or_convert(
    dest: ValueId,
    value: ValueId,
    to_ty: &str,
    value_types: &mut HashMap<ValueId, &'static str>,
    bool_vids: &mut HashSet<ValueId>,
    out: &mut String,
) {
    let src_ty = value_types.get(&value).copied().unwrap_or("i64");
    value_types.insert(dest, "i64");
    match src_ty {
        "double" => out.push_str(&format!(
            "  %v{} = fptosi double %v{} to i64\n",
            dest.0, value.0
        )),
        "float" => out.push_str(&format!(
            "  %v{} = fptosi float %v{} to i64\n",
            dest.0, value.0
        )),
        "ptr" => out.push_str(&format!(
            "  %v{} = ptrtoint ptr %v{} to i64\n",
            dest.0, value.0
        )),
        _ => {
            if to_ty == "Bool" {
                bool_vids.insert(dest);
            }
            out.push_str(&format!("  %v{} = or i64 %v{}, 0\n", dest.0, value.0));
        }
    }
}
