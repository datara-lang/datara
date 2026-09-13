//! Type-Based Alias Analysis (TBAA) & NoAlias Attributes for LLVM (v1.2.4)
//!
//! Emits LLVM `noalias` parameter attributes and TBAA metadata nodes for Datara
//! immutable references, slices, and borrowed collections.
//! Proves to LLVM's Loop Vectorizer and SLP Vectorizer that memory regions
//! never overlap, enabling zero-check SIMD vectorization.

/// Formats a pointer parameter with the `noalias` attribute if proven unaliased.
pub fn format_param_noalias(ty: &str, name: &str, is_immutable_ref: bool) -> String {
    if is_immutable_ref && ty == "ptr" {
        format!("ptr noalias %{}", name)
    } else {
        format!("{} %{}", ty, name)
    }
}

/// Generates standard Datara TBAA (Type-Based Alias Analysis) metadata nodes for LLVM IR.
pub fn emit_tbaa_metadata_definitions() -> String {
    let mut out = String::new();
    out.push_str("\n; --- Datara TBAA Metadata Hierarchy ---\n");
    out.push_str("!0 = !{!\"Datara Root TBAA\", null}\n");
    out.push_str("!1 = !{!\"Datara Scalar\", !0, i64 0}\n");
    out.push_str("!2 = !{!\"Datara Int\", !1, i64 0}\n");
    out.push_str("!3 = !{!\"Datara Float\", !1, i64 0}\n");
    out.push_str("!4 = !{!\"Datara List Data\", !0, i64 0}\n");
    out.push_str("!5 = !{!\"Datara Struct Field\", !0, i64 0}\n");
    out.push_str("!6 = !{!2, !2, i64 0} ; Int access tag\n");
    out.push_str("!7 = !{!3, !3, i64 0} ; Float access tag\n");
    out.push_str("!8 = !{!4, !4, i64 0} ; List access tag\n");
    out
}

/// Returns the TBAA tag string to append to a load/store instruction.
pub fn get_tbaa_tag_for_type(ty: &str) -> &'static str {
    match ty {
        "Int" | "i64" => ", !tbaa !6",
        "Float" | "double" | "f64" => ", !tbaa !7",
        "List" | "ptr" => ", !tbaa !8",
        _ => "",
    }
}
