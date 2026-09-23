use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_phase5_docs_consistency_script() {
    let python_cmd = if Command::new("python3").arg("--version").output().is_ok() {
        "python3"
    } else {
        "python"
    };
    let output = Command::new(python_cmd)
        .arg("scripts/check_docs_consistency.py")
        .output()
        .expect("failed to execute check_docs_consistency.py");
    assert!(
        output.status.success(),
        "check_docs_consistency.py failed (exit: {:?})\nSTDOUT:\n{}\nSTDERR:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_phase5_glossary_and_changelog() {
    let glossary_path = Path::new("docs/GLOSSARY.md");
    assert!(glossary_path.exists(), "missing docs/GLOSSARY.md");
    let glossary = fs::read_to_string(glossary_path).expect("read glossary");
    assert!(
        glossary.contains("Affine Ownership"),
        "missing Affine Ownership in glossary"
    );
    assert!(
        glossary.contains("Capability Lattice"),
        "missing Capability Lattice in glossary"
    );
    assert!(
        glossary.contains("Evidence Gates"),
        "missing Evidence Gates in glossary"
    );

    let changelog_path = Path::new("CHANGELOG.md");
    assert!(changelog_path.exists(), "missing CHANGELOG.md");
    let changelog = fs::read_to_string(changelog_path).expect("read changelog");
    assert!(
        changelog.contains("## [1.1.0]"),
        "missing [1.1.0] in CHANGELOG.md"
    );
    assert!(changelog.contains("Native Async/Await to Completion"));
}

#[test]
fn test_phase5_readme_navigation() {
    // The documentation portal page is `docs/DOCUMENTATION.md`; `docs/README.md`
    // was renamed because GitHub Pages does not publish README files.
    let docs_path = Path::new("docs/DOCUMENTATION.md");
    assert!(docs_path.exists(), "missing docs/DOCUMENTATION.md");
    let docs = fs::read_to_string(docs_path).expect("read docs/DOCUMENTATION.md");
    assert!(
        docs.contains("TUTORIAL.md"),
        "missing TUTORIAL.md in docs/DOCUMENTATION.md"
    );
    assert!(
        docs.contains("GLOSSARY.md"),
        "missing GLOSSARY.md in docs/DOCUMENTATION.md"
    );
    assert!(
        docs.contains("Historical & Archival Specifications"),
        "missing archival specs section"
    );
}
