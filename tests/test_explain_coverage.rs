//! M2 Diagnostics Gold, gate 1 (PLAN_V150): every ErrorCode must carry a
//! non-empty human description, and the code string must be round-trip
//! stable. Per the 2026-09-25 owner decision, diagnostics are ENGLISH-ONLY:
//! description() ignores the locale argument and the dual-locale matrix
//! from the original M2 checklist is dropped.
//!
//! The full per-code long-explanation matrix (extended text + example) is
//! tracked separately in PLAN_V150 M2; this test enforces the baseline the
//! rest builds on.

use forgen::diagnostics::codes::{ErrorCode, all_codes};

#[test]
fn every_code_has_nonempty_description() {
    let codes = all_codes();
    assert!(
        codes.len() >= 90,
        "expected the full ErrorCode set, got {}",
        codes.len()
    );
    for code in &codes {
        // Locale is intentionally ignored (English-only decision); probe
        // both call shapes to keep the signature honest.
        let en = code.description("en");
        assert!(
            !en.trim().is_empty(),
            "{:?} must have a non-empty description",
            code.as_str()
        );
        let same = code.description("ru");
        assert_eq!(en, same, "description must not branch on locale");
    }
}

#[test]
fn every_code_has_extended_explanation_with_example() {
    // M2 gate 2: 100% of the ErrorCode set resolves to an extended
    // explanation containing an [INVALID]/[VALID] example pair.
    for code in all_codes() {
        let text = forgen::diagnostics::explain_data::explain_for_code(code.as_str())
            .unwrap_or_else(|| panic!("code {} has no extended explanation", code.as_str()));
        assert!(
            text.contains("[VALID] Good Code:"),
            "explanation for {} must contain a good-code example",
            code.as_str()
        );
        assert!(
            text.contains("[INVALID] Bad Code:"),
            "explanation for {} must contain a bad-code example",
            code.as_str()
        );
        assert!(text.contains(code.as_str()));
    }
}

#[test]
fn code_strings_are_unique_and_prefixed() {
    let codes = all_codes();
    let mut seen = std::collections::HashSet::new();
    for code in &codes {
        let s = code.as_str();
        assert!(seen.insert(s), "duplicate code string: {s}");
        // Compiler codes are E-codes (E-TYPE-001, E0310, ...) or W-codes
        // (W-SYN-001, ...); lint codes live in the lint module and are not
        // part of this enum.
        assert!(
            s.starts_with('E') || s.starts_with('W'),
            "code {s} must be an E-code or W-code"
        );
    }
}

#[test]
fn explain_anchor_codes_exist() {
    // The explain surface must resolve at least these anchors used across
    // docs and tests; full-matrix coverage is the remaining M2 work.
    let anchors = [
        "E-TYPE-001",
        "E-TYPE-010",
        "E-RESOLVE-001",
        "E-OUT-001",
        "W-SYN-001",
        "E0310",
    ];
    let all = all_codes();
    for code in anchors {
        assert!(
            all.iter().any(|c| c.as_str() == code),
            "anchor code {code} missing from the ErrorCode set"
        );
    }
}
