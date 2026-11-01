//! PLAN_V151 N7 flagship: examples/showcase/pipeline_demo — the "hub in
//! one file" (fs CSV → collect → SIMD → numpy zero-copy → SSR HTTP).
//!
//! Golden contract: the demo's stdout is byte-identical on all three
//! backends (Cranelift / JIT / LLVM). Runs the real binary over the
//! example directory (Level-3 project: manifest grants fs capabilities).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forgen_pipeline_demo_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

const GOLDEN: &str = "CSV_BYTES\n56\n6\n500\n320\nSIMD_DOT\n440.0\nPY_MEAN\n110.00\ntrue\ntrue\n";

fn run_demo(dir: &std::path::Path, extra: &[&str]) -> (String, i32) {
    // Copy the example project into the temp dir (write_file creates
    // pipeline_demo.csv next to the manifest).
    let ex_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/showcase/pipeline_demo");
    for name in ["main.dtr", "datara.toml"] {
        fs::copy(ex_dir.join(name), dir.join(name)).expect("copy example file");
    }
    let out = Command::new(env!("CARGO_BIN_EXE_forgen"))
        .args(["run", "main.dtr"])
        .args(extra)
        .current_dir(dir)
        .env("DATARA_HOME", dir.join("home"))
        .output()
        .expect("forgen run must start");
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (text, out.status.code().unwrap_or(-1))
}

#[test]
fn test_pipeline_demo_golden_output_three_backends() {
    let outputs: Vec<(String, i32)> = [vec![], vec!["--jit"], vec!["--llvm"]]
        .iter()
        .map(|extra| {
            let dir = temp_dir(&format!("tier_{}", extra.len()));
            let (t, c) = run_demo(&dir, extra);
            // Suppress W-SYN-001 warnings from stdlib modules in stderr —
            // they land in stderr, stdout stays the golden channel.
            let stdout = t
                .lines()
                .filter(|l| !l.starts_with("warning") && !l.contains("W-SYN"))
                .collect::<Vec<_>>()
                .join("\n");
            (stdout, c)
        })
        .collect();

    for (i, (text, code)) in outputs.iter().enumerate() {
        assert_eq!(*code, 0, "tier {} failed: {text}", i);
        let stdout = text
            .lines()
            .filter(|l| !l.starts_with("warning") && !l.contains("W-SYN"))
            .collect::<Vec<_>>()
            .join("\n");
        // stderr may carry warnings; the golden is stdout-only. Filter to
        // stdout lines by matching the known shape.
        let actual: Vec<&str> = stdout
            .lines()
            .filter(|l| {
                !l.starts_with("warning")
                    && !l.contains("-->")
                    && !l.starts_with(' ')
                    && !l.starts_with('|')
                    && !l.starts_with('=')
                    && !l.is_empty()
            })
            .collect();
        let actual_text = actual.join("\n") + "\n";
        // Full golden parity on ALL three tiers. The historical LLVM
        // py-bind divergence (bound Python object reported len=1 despite
        // the C path receiving the correct list) was fixed by declaring
        // datara_py_call/eval/eval_safe in the LLVM runtime_decls — the
        // stdlib python.dtr module referenced them, and its LLVM-compiled
        // body was broken without the declares. DATARA_BRIDGE_TRACE in
        // datara_value.c/datara_py.c documents the verification path.
        assert_eq!(actual_text, GOLDEN, "tier {} golden mismatch", i);
    }
}
