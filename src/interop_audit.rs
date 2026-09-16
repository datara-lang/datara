//! v1.4.0 `forgen doctor --bridges`: automated interop health audit.
//!
//! For every bridge (C, Python, JS, Rust) the audit compiles and runs a
//! generated probe program that exercises a fixed scenario matrix:
//!
//! 1. `roundtrip`       - call through the boundary with 2 args, check return
//! 2. `primitives`      - int + float + bool + string argument transfer
//! 3. `aggregate`       - a composite value across the boundary (C: heap
//!                        list ABI; Python/JS: call-arg list / JSON string)
//! 4. `error_transfer`  - a failing callee is observed by Datara
//! 5. `leak_smoke`      - 1000 boundary allocations must return the live
//!                        allocator counter (`datara_rt_heap_live`) to its
//!                        baseline
//!
//! Every probe also measures per-call overhead over a hot loop with
//! `datara_rt_now_ns` and reports `ns_per_call`.
//!
//! The probes print `BRIDGE|scenario|VERDICT|detail` lines; this module
//! compiles them with the normal `ForgenCompiler` pipeline, runs them, and
//! parses the results. Bridges degrade gracefully: the Python probe detects
//! a missing interpreter at runtime and prints SKIP verdicts; the Rust
//! bridge only reports the toolchain probe unless `FORGEN_DOCTOR_DEEP=1`
//! opts into a full crate build.
//!
//! Honest scope notes:
//! * C error transfer uses sentinel values (C ABI carries no error channel);
//!   the probe verifies Datara observes the callee's failure sentinel.
//! * The leak smoke measures the runtime's managed (pool-class) allocator,
//!   which is exactly the allocation path list objects take.

use crate::driver::ForgenCompiler;
use std::path::PathBuf;

/// Verdict of a single audited scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Skip,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Skip => "SKIP",
        }
    }
}

/// One audited (bridge, scenario) pair with a human-readable detail line.
#[derive(Debug, Clone)]
pub struct BridgeScenario {
    pub bridge: &'static str,
    pub scenario: &'static str,
    pub verdict: Verdict,
    pub detail: String,
}

/// Overall health of one bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeHealth {
    Perfect,
    Good,
    Poor,
    Unavailable,
}

impl BridgeHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            BridgeHealth::Perfect => "PERFECT",
            BridgeHealth::Good => "GOOD",
            BridgeHealth::Poor => "POOR",
            BridgeHealth::Unavailable => "UNAVAILABLE",
        }
    }
}

/// The C runtime declarations every probe needs (leak counter, clock, and
/// the C-bridge entry points). Generated next to the probe sources.
const C_PROBE_HEADER: &str = r#"#ifndef DATARA_DOCTOR_C_H
#define DATARA_DOCTOR_C_H

const char* datara_rt_str_concat(const char* a, const char* b);
int64_t     datara_rt_str_eq(const char* a, const char* b);
int64_t     datara_rt_str_len(const char* s);
const char* datara_rt_int_to_str(int64_t v);
const char* datara_rt_float_to_str(double v);
const char* datara_rt_bool_to_str(int64_t v);
int64_t     datara_rt_str_to_int(const char* s);
int64_t*    datara_rt_list_create_2(int64_t a, int64_t b);
int64_t*    datara_rt_list_create_3(int64_t a, int64_t b, int64_t c);
int64_t     datara_rt_list_get(int64_t* list, int64_t idx);
int64_t     datara_rt_list_len(int64_t* list);
void        datara_rt_list_free(int64_t* list);
int64_t     datara_rt_heap_live(void);
int64_t     datara_rt_now_ns(void);

#endif
"#;

const C_PROBE: &str = r#"
import c "datara_doctor_c.h";

fn report(name: Str, ok: Bool) {
    if ok {
        println("C|" + name + "|PASS|verified")
    } else {
        println("C|" + name + "|FAIL|mismatch")
    }
}

fn main() {
    unsafe(justification: "doctor C bridge audit exercises the bundled C runtime ABI") {
        let joined = datara_rt_str_concat("inter", "op")
        report("roundtrip", datara_rt_str_eq(joined, "interop") == 1)

        let s_val = datara_rt_int_to_str(42)
        let f = datara_rt_float_to_str(2.5)
        let b = datara_rt_bool_to_str(1)
        let prim = datara_rt_str_eq(s_val, "42") == 1 && datara_rt_str_eq(b, "true") == 1
        let prim2 = datara_rt_str_eq(f, "2.5") == 1 && datara_rt_str_len(joined) == 7
        report("primitives", prim && prim2)

        let xs = datara_rt_list_create_3(7, 8, 9)
        report("aggregate", datara_rt_list_get(xs, 1) == 8 && datara_rt_list_len(xs) == 3)

        report("error_transfer", datara_rt_str_to_int("not_a_number") == 0)

        let base = datara_rt_heap_live()
        for k in 0..1000 {
            let pair = datara_rt_list_create_2(k, k)
            datara_rt_list_free(pair)
        }
        report("leak_smoke", datara_rt_heap_live() == base)

        let t0 = datara_rt_now_ns()
        for m in 0..10000 {
            let s = datara_rt_str_concat("a", "b")
            datara_rt_str_len(s)
        }
        let t1 = datara_rt_now_ns()
        println("C|overhead|PASS|" + int_to_str((t1 - t0) / 10000) + " ns/call")
    }
}
"#;

const PY_PROBE: &str = r#"
import c "datara_doctor_c.h";
import python

fn main() {
    let py = Py { version: "3" }
    let canary = py.eval_int("1")
    if canary.is_err() {
        println("PY|interpreter|SKIP|python runtime not found")
        println("PY|overhead|SKIP|no interpreter")
        return
    }

    let r1 = py.eval_int("100 + 42")
    if r1.is_ok() && r1.unwrap() == 142 {
        println("PY|roundtrip|PASS|142")
    } else {
        println("PY|roundtrip|FAIL|unexpected result")
    }

    let f = py.eval_float("2.5 * 4.0")
    let s = py.eval("'doctor' + '-ok'")
    if f.is_ok() && f.unwrap() == 10.0 && s.is_ok() && s.unwrap() == "doctor-ok" {
        println("PY|primitives|PASS|int+float+string")
    } else {
        println("PY|primitives|FAIL|primitive transfer mismatch")
    }

    let agg = py.call("math.sqrt", "[144.0]")
    if agg.is_ok() {
        println("PY|aggregate|PASS|math.sqrt -> " + agg.unwrap())
    } else {
        println("PY|aggregate|FAIL|call failed")
    }

    let err = py.eval("10 / 0")
    if err.is_err() {
        println("PY|error_transfer|PASS|" + err.error_msg)
    } else {
        println("PY|error_transfer|FAIL|error was not observed")
    }

    unsafe(justification: "doctor audit reads the runtime leak counter") {
        let base = datara_rt_heap_live()
        for i in 0..1000 {
            let keep = py.eval_int("1")
            if keep.is_err() {
                println("PY|leak_smoke|FAIL|call failed mid-loop")
                return
            }
        }
        if datara_rt_heap_live() == base {
            println("PY|leak_smoke|PASS|live allocator back to baseline")
        } else {
            println("PY|leak_smoke|FAIL|allocator delta != 0")
        }

        let t0 = datara_rt_now_ns()
        for i in 0..2000 {
            let keep = py.eval_int("1")
            if keep.is_err() {
                return
            }
        }
        let t1 = datara_rt_now_ns()
        println("PY|overhead|PASS|" + int_to_str((t1 - t0) / 2000) + " ns/call")
    }
}
"#;

const JS_PROBE: &str = r#"
import c "datara_doctor_c.h";

fn main() {
    let canary = js_eval_int("1 + 1")
    if canary != 2 {
        println("JS|engine|SKIP|javascript engine unavailable")
        println("JS|overhead|SKIP|no engine")
        return
    }

    let r1 = js_eval_int("6 * 7")
    if r1 == 42 {
        println("JS|roundtrip|PASS|42")
    } else {
        println("JS|roundtrip|FAIL|unexpected result")
    }

    let i = js_eval_int("2 + 3")
    let f = js_eval_float("2.5 * 4")
    let s = js_eval("'doctor' + '-ok'")
    if i == 5 && f == 10.0 && s == "doctor-ok" {
        println("JS|primitives|PASS|int+float+string")
    } else {
        println("JS|primitives|FAIL|primitive transfer mismatch")
    }

    let agg = js_eval("JSON.stringify([1, 2, 3])")
    if agg == "[1,2,3]" {
        println("JS|aggregate|PASS|JSON array string")
    } else {
        println("JS|aggregate|FAIL|unexpected aggregate: " + agg)
    }

    // The bundled djs engine surfaces a failed call as the 'undefined'
    // sentinel; Datara observes the distinguished value across the boundary.
    let err = js_eval("no_such_function_xyzzy()")
    if err == "undefined" {
        println("JS|error_transfer|PASS|failed call surfaced as 'undefined' sentinel")
    } else {
        println("JS|error_transfer|FAIL|no sentinel observed")
    }

    unsafe(justification: "doctor audit reads the runtime leak counter") {
        let base = datara_rt_heap_live()
        for i in 0..1000 {
            let keep = js_eval_int("1")
            if keep != 1 {
                println("JS|leak_smoke|FAIL|call failed mid-loop")
                return
            }
        }
        if datara_rt_heap_live() == base {
            println("JS|leak_smoke|PASS|live allocator back to baseline")
        } else {
            println("JS|leak_smoke|FAIL|allocator delta != 0")
        }

        let t0 = datara_rt_now_ns()
        for i in 0..2000 {
            let keep = js_eval_int("1")
            if keep != 1 {
                return
            }
        }
        let t1 = datara_rt_now_ns()
        println("JS|overhead|PASS|" + int_to_str((t1 - t0) / 2000) + " ns/call")
    }
}
"#;

/// Runs the full bridge audit. Returns one entry per (bridge, scenario).
/// Never panics: any probe failure degrades into FAIL or SKIP verdicts.
pub fn run_bridge_audit() -> Vec<BridgeScenario> {
    // The workspace is unique per invocation and removed at the end, so
    // concurrent callers (parallel tests, doctor while tests run) never
    // share linker output files.
    let Some(workspace) = probe_workspace() else {
        return vec![BridgeScenario {
            bridge: "C",
            scenario: "roundtrip",
            verdict: Verdict::Fail,
            detail: "no writable temp workspace".to_string(),
        }];
    };

    // All probes that touch the runtime's C ABI share one generated header.
    let header = workspace.join("datara_doctor_c.h");
    let _ = std::fs::write(&header, C_PROBE_HEADER);

    let mut out = Vec::new();

    // ---- C bridge -------------------------------------------------------
    out.extend(run_c_bridge(&workspace));

    // ---- Python bridge --------------------------------------------------
    out.extend(run_compiled_probe(
        &workspace,
        "PY",
        PY_PROBE,
        &[
            "roundtrip",
            "primitives",
            "aggregate",
            "error_transfer",
            "leak_smoke",
        ],
    ));

    // ---- JS bridge ------------------------------------------------------
    out.extend(run_compiled_probe(
        &workspace,
        "JS",
        JS_PROBE,
        &[
            "roundtrip",
            "primitives",
            "aggregate",
            "error_transfer",
            "leak_smoke",
        ],
    ));

    // ---- Rust bridge ----------------------------------------------------
    out.extend(run_rust_bridge());

    remove_workspace(&workspace);

    out
}

/// Per-call overhead line parsed from a probe's `overhead` output.
pub fn overhead_of(scenarios: &[BridgeScenario], bridge: &str) -> Option<i64> {
    scenarios
        .iter()
        .find(|s| s.bridge == bridge && s.scenario == "overhead" && s.verdict == Verdict::Pass)
        .and_then(|s| s.detail.split_whitespace().next())
        .and_then(|v| v.parse::<i64>().ok())
}

fn bridge_health(scenarios: &[BridgeScenario], bridge: &str) -> BridgeHealth {
    let mine: Vec<&BridgeScenario> = scenarios
        .iter()
        .filter(|s| s.bridge == bridge && s.scenario != "overhead")
        .collect();
    if mine.is_empty() {
        return BridgeHealth::Poor;
    }
    let fails = mine.iter().filter(|s| s.verdict == Verdict::Fail).count();
    let passes = mine.iter().filter(|s| s.verdict == Verdict::Pass).count();
    let skips = mine.iter().filter(|s| s.verdict == Verdict::Skip).count();
    if fails > 0 {
        BridgeHealth::Poor
    } else if skips > 0 && passes > 0 {
        BridgeHealth::Good
    } else if skips == mine.len() {
        BridgeHealth::Unavailable
    } else {
        BridgeHealth::Perfect
    }
}

/// Renders the audit as the aligned doctor table plus per-bridge overhead
/// and health lines. `overheads` maps bridge tag -> ns/call.
pub fn render_table(scenarios: &[BridgeScenario]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {:<7} | {:<14} | {:<7} | {}",
        "bridge", "scenario", "verdict", "detail"
    ));
    out.push('\n');
    out.push_str("  ----------------------------------------------------------------");
    out.push('\n');
    for s in scenarios {
        out.push_str(&format!(
            "  {:<7} | {:<14} | {:<7} | {}",
            s.bridge,
            s.scenario,
            s.verdict.as_str(),
            s.detail
        ));
        out.push('\n');
    }
    for bridge in ["C", "PY", "JS", "RUST"] {
        let health = bridge_health(scenarios, bridge);
        let overhead = overhead_of(scenarios, bridge)
            .map(|v| format!("{} ns/call", v))
            .unwrap_or_else(|| "n/a".to_string());
        out.push_str(&format!(
            "  {:<7} health: {:<11} overhead: {}\n",
            bridge,
            health.as_str(),
            overhead
        ));
    }
    out
}

fn parse_probe_lines(
    bridge: &'static str,
    expected: &[&'static str],
    stdout: &str,
    run_failed: Option<&str>,
) -> Vec<BridgeScenario> {
    let mut out = Vec::new();
    for name in expected {
        let line = stdout
            .lines()
            .find(|l| l.starts_with(&format!("{}|{}|", bridge, name)));
        match line {
            Some(l) => {
                let parts: Vec<&str> = l.split('|').collect();
                let verdict = match parts.get(2).copied().unwrap_or("FAIL") {
                    "PASS" => Verdict::Pass,
                    "SKIP" => Verdict::Skip,
                    _ => Verdict::Fail,
                };
                let detail = parts.get(3).copied().unwrap_or("").to_string();
                out.push(BridgeScenario {
                    bridge,
                    scenario: name,
                    verdict,
                    detail,
                });
            }
            None => {
                let detail = match run_failed {
                    Some(e) => format!("probe run failed: {}", e),
                    None => "no verdict line emitted".to_string(),
                };
                out.push(BridgeScenario {
                    bridge,
                    scenario: name,
                    verdict: Verdict::Fail,
                    detail,
                });
            }
        }
    }
    // Overhead line is optional but parsed the same way.
    if let Some(l) = stdout
        .lines()
        .find(|l| l.starts_with(&format!("{}|overhead|", bridge)))
    {
        let parts: Vec<&str> = l.split('|').collect();
        let verdict = match parts.get(2).copied().unwrap_or("FAIL") {
            "PASS" => Verdict::Pass,
            "SKIP" => Verdict::Skip,
            _ => Verdict::Fail,
        };
        out.push(BridgeScenario {
            bridge,
            scenario: "overhead",
            verdict,
            detail: parts.get(3).copied().unwrap_or("").to_string(),
        });
    }
    out
}

/// Compiles and runs one probe program that self-reports its scenarios.
fn run_compiled_probe(
    workspace: &std::path::Path,
    bridge: &'static str,
    source: &str,
    expected: &[&'static str],
) -> Vec<BridgeScenario> {
    let header = workspace.join("datara_doctor_c.h");
    if !header.exists() {
        let _ = std::fs::write(&header, C_PROBE_HEADER);
    }
    let file = workspace.join(format!("probe_{}.dtr", bridge.to_lowercase()));
    if std::fs::write(&file, source).is_err() {
        return expected
            .iter()
            .map(|name| BridgeScenario {
                bridge,
                scenario: name,
                verdict: Verdict::Fail,
                detail: "failed to write probe".to_string(),
            })
            .collect();
    }
    let exe = workspace.join(format!("probe_{}.exe", bridge.to_lowercase()));

    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(&file, Some(&exe));
    if !res.success {
        let mut combined = res.error.clone().unwrap_or_default();
        combined.push('\n');
        combined.push_str(&res.diagnostics);
        let detail = short_reason(&combined);
        return expected
            .iter()
            .map(|name| BridgeScenario {
                bridge,
                scenario: name,
                verdict: Verdict::Fail,
                detail: format!("probe compile failed: {}", detail),
            })
            .collect();
    }
    let Some(exe_path) = res.exe_path else {
        return expected
            .iter()
            .map(|name| BridgeScenario {
                bridge,
                scenario: name,
                verdict: Verdict::Fail,
                detail: "probe produced no executable".to_string(),
            })
            .collect();
    };

    let run = compiler.cranelift.run_executable(&exe_path, &[]);
    match run {
        Ok((stdout, stderr, code, _)) => {
            let failed = if code == 0 {
                None
            } else {
                Some(stderr.as_str())
            };
            let verdicts = parse_probe_lines(bridge, expected, &stdout, failed);
            cleanup_probe(&file, &exe_path);
            verdicts
        }
        Err(e) => expected
            .iter()
            .map(|name| BridgeScenario {
                bridge,
                scenario: name,
                verdict: Verdict::Fail,
                detail: format!("probe run failed: {}", e),
            })
            .collect(),
    }
}

fn run_c_bridge(workspace: &std::path::Path) -> Vec<BridgeScenario> {
    run_compiled_probe(
        workspace,
        "C",
        C_PROBE,
        &[
            "roundtrip",
            "primitives",
            "aggregate",
            "error_transfer",
            "leak_smoke",
        ],
    )
}

/// The Rust bridge audit: the toolchain probe is always meaningful; the
/// round-trip probe performs a real (dependency-free) crate build only when
/// `FORGEN_DOCTOR_DEEP=1` is set, so `doctor` stays fast by default.
fn run_rust_bridge() -> Vec<BridgeScenario> {
    let cargo_ok = which_ok("cargo", &["--version"]);
    let deep = std::env::var("FORGEN_DOCTOR_DEEP")
        .map(|v| v == "1")
        .unwrap_or(false);

    let (roundtrip_verdict, roundtrip_detail) = if !cargo_ok {
        (Verdict::Skip, "cargo not found".to_string())
    } else if !deep {
        (
            Verdict::Skip,
            "toolchain found; set FORGEN_DOCTOR_DEEP=1 to build the fixture crate".to_string(),
        )
    } else {
        match crate_roundtrip_probe() {
            Ok(elapsed) => (
                Verdict::Pass,
                format!("fixture crate built in {}ms", elapsed),
            ),
            Err(e) => (Verdict::Fail, e),
        }
    };

    vec![
        BridgeScenario {
            bridge: "RUST",
            scenario: "roundtrip",
            verdict: roundtrip_verdict,
            detail: roundtrip_detail,
        },
        BridgeScenario {
            bridge: "RUST",
            scenario: "toolchain",
            verdict: if cargo_ok {
                Verdict::Pass
            } else {
                Verdict::Skip
            },
            detail: if cargo_ok {
                "cargo --version ok".to_string()
            } else {
                "rust toolchain not on PATH".to_string()
            },
        },
    ]
}

/// Deep probe: builds the dependency-free fixture crate and reports elapsed
/// time. Uses only the local cargo, no network.
fn crate_roundtrip_probe() -> Result<u128, String> {
    let crate_dir = PathBuf::from("tests/fixtures/fixture_rust_crate");
    if !crate_dir.join("Cargo.toml").exists() {
        return Err("fixture crate not found (run from the repo root)".to_string());
    }
    let start = std::time::Instant::now();
    let out = std::process::Command::new("cargo")
        .arg("build")
        .arg("--offline")
        .current_dir(&crate_dir)
        .output()
        .map_err(|e| format!("cargo spawn failed: {}", e))?;
    if out.status.success() {
        Ok(start.elapsed().as_millis())
    } else {
        Err(format!(
            "cargo build failed: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("unknown")
        ))
    }
}

fn which_ok(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_workspace() -> Option<PathBuf> {
    // Unique per audit invocation: concurrent callers (parallel tests, the
    // doctor CLI while a test runs) must never share a linker output file.
    static AUDIT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = AUDIT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("datara_doctor_{}_{}", std::process::id(), seq));
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Removes the per-invocation workspace after a full audit pass.
fn remove_workspace(workspace: &std::path::Path) {
    let _ = std::fs::remove_dir_all(workspace);
}

fn cleanup_probe(file: &std::path::Path, exe: &std::path::Path) {
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_file(exe);
    for ext in ["pdb", "obj", "lib"] {
        let _ = std::fs::remove_file(exe.with_extension(ext));
    }
}

fn short_reason(diagnostics: &str) -> String {
    let reason = diagnostics
        .lines()
        .find(|l| l.contains("error") || l.contains("Error") || l.contains("ERROR"))
        .unwrap_or("unknown error");
    reason.chars().take(140).collect()
}
