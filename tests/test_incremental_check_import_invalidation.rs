use forgen::driver::ForgenCompiler;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forgen_import_cache_regression_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(root.join("src")).expect("create project source directory");
        Self(root)
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::write(&path, contents).expect("write test source");
        path
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn check_cache_does_not_hide_direct_or_transitive_import_edits() {
    let project = Project::new();
    let root = project.write(
        "src/main.dtr",
        "use helper.answer\nfn main() -> Int => answer()\n",
    );
    let helper = project.write(
        "src/helper.dtr",
        "use leaf.value\npub fn answer() -> Int => value()\n",
    );
    let leaf = project.write("src/leaf.dtr", "pub fn value() -> Int => 42\n");
    let compiler = ForgenCompiler::new("check");

    let first = compiler.check_file(&root);
    assert!(
        first.success,
        "initial project check failed: {}",
        first.diagnostics
    );
    let cached = compiler.check_file(&root);
    assert!(
        cached.success,
        "unchanged cached check failed: {}",
        cached.diagnostics
    );

    fs::write(&leaf, "pub fn value() -> Int => \"wrong\"\n")
        .expect("replace transitive module with invalid implementation");
    let transitive_edit = compiler.check_file(&root);
    assert!(
        !transitive_edit.success,
        "editing a transitive dependency must invalidate the root check result"
    );
    assert!(
        transitive_edit.diagnostics.to_lowercase().contains("type"),
        "expected a type diagnostic, got: {}",
        transitive_edit.diagnostics
    );

    fs::write(&leaf, "pub fn value() -> Int => 42\n").expect("restore leaf");
    let restored = compiler.check_file(&root);
    assert!(
        restored.success,
        "restored dependency must check: {}",
        restored.diagnostics
    );

    fs::write(
        &helper,
        "use leaf.value\npub fn answer() -> Str => \"changed\"\n",
    )
    .expect("replace direct module");
    let direct_edit = compiler.check_file(&root);
    assert!(
        !direct_edit.success,
        "editing a directly imported module must invalidate the root check result"
    );
}
