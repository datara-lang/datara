use forgen::lint::diagnostics::LintSeverity;
use forgen::lint::profile::LintProfile;
use forgen::lint::lint_source_with_profile;
use forgen::project::manifest::DataraManifest;
use std::str::FromStr;

#[test]
fn test_profile_standard_loop_concat_is_warning() {
    let src = r#"
fn run() {
    mut s = ""
    while true {
        s = s + "more"
    }
}
"#;
    let diags = lint_source_with_profile(src, "test.dtr", LintProfile::Standard).unwrap();
    let concat_diag = diags.iter().find(|d| d.code == "L1401");
    assert!(concat_diag.is_some(), "L1401 must be detected in standard profile");
    assert_eq!(concat_diag.unwrap().severity, LintSeverity::Warning);
}

#[test]
fn test_profile_student_loop_concat_escalates_to_error() {
    let src = r#"
fn run() {
    mut s = ""
    while true {
        s = s + "more"
    }
}
"#;
    let diags = lint_source_with_profile(src, "test.dtr", LintProfile::Student).unwrap();
    let concat_diag = diags.iter().find(|d| d.code == "L1401");
    assert!(concat_diag.is_some(), "L1401 must be detected in student profile");
    assert_eq!(concat_diag.unwrap().severity, LintSeverity::Error, "L1401 must escalate to Error in student profile");
}

#[test]
fn test_profile_systems_loop_concat_is_warning() {
    let src = r#"
fn run() {
    mut s = ""
    while true {
        s = s + "more"
    }
}
"#;
    let diags = lint_source_with_profile(src, "test.dtr", LintProfile::Systems).unwrap();
    let concat_diag = diags.iter().find(|d| d.code == "L1401");
    assert!(concat_diag.is_some(), "L1401 must still be reported in systems profile");
    assert_eq!(concat_diag.unwrap().severity, LintSeverity::Warning);
}

#[test]
fn test_profile_standard_style_warning() {
    let src = r#"
fn badFunctionName() {
}
"#;
    let diags = lint_source_with_profile(src, "test.dtr", LintProfile::Standard).unwrap();
    let style_diag = diags.iter().find(|d| d.code == "style::non_snake_case");
    assert!(style_diag.is_some(), "Style warning must be present in standard profile");
    assert_eq!(style_diag.unwrap().severity, LintSeverity::Warning);
}

#[test]
fn test_profile_student_style_warning() {
    let src = r#"
fn badFunctionName() {
}
"#;
    let diags = lint_source_with_profile(src, "test.dtr", LintProfile::Student).unwrap();
    let style_diag = diags.iter().find(|d| d.code == "style::non_snake_case");
    assert!(style_diag.is_some(), "Style warning must still be present in student profile");
    assert_eq!(style_diag.unwrap().severity, LintSeverity::Warning);
}

#[test]
fn test_profile_systems_suppresses_style_warning() {
    let src = r#"
fn badFunctionName() {
}
"#;
    let diags = lint_source_with_profile(src, "test.dtr", LintProfile::Systems).unwrap();
    let style_diag = diags.iter().find(|d| d.code == "style::non_snake_case");
    assert!(style_diag.is_none(), "Style warning must be suppressed in systems profile");
}

#[test]
fn test_profile_student_unused_variable_escalates_to_error() {
    let src = r#"
fn run() {
    let unused_val = 42
}
"#;
    let diags = lint_source_with_profile(src, "test.dtr", LintProfile::Student).unwrap();
    let unused_diag = diags.iter().find(|d| d.code == "style::unused_variable");
    assert!(unused_diag.is_some(), "Unused variable must be detected in student profile");
    assert_eq!(unused_diag.unwrap().severity, LintSeverity::Error, "Unused variable must escalate to Error in student profile");
}

#[test]
fn test_profile_parse_from_str() {
    assert_eq!(LintProfile::from_str("student").unwrap(), LintProfile::Student);
    assert_eq!(LintProfile::from_str("Student").unwrap(), LintProfile::Student);
    assert_eq!(LintProfile::from_str("standard").unwrap(), LintProfile::Standard);
    assert_eq!(LintProfile::from_str("STANDARD").unwrap(), LintProfile::Standard);
    assert_eq!(LintProfile::from_str("systems").unwrap(), LintProfile::Systems);
    assert!(LintProfile::from_str("invalid_profile").is_err());
}

#[test]
fn test_profile_manifest_toml_parsing() {
    let toml_str = r#"
[package]
name = "my_algo"
version = "0.1.0"

[lint]
profile = "student"
"#;
    let manifest: DataraManifest = toml::from_str(toml_str).expect("must parse datara.toml with [lint]");
    assert!(manifest.lint.is_some());
    let lint_cfg = manifest.lint.unwrap();
    assert_eq!(lint_cfg.profile.as_deref(), Some("student"));
    let prof: LintProfile = lint_cfg.profile.unwrap().parse().unwrap();
    assert_eq!(prof, LintProfile::Student);
}
