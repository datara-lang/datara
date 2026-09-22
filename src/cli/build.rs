//! Compilation-driving commands: check, run, test, bench, build, domain, profile.

use super::{extract_target_arg, keyword_set, to_json_value, to_pretty_json};
use crate::driver::ForgenCompiler;
use crate::optimizer::OptTier;
use crate::pgo::ProfileData;
use crate::project::{ProjectDiscovery, ProjectRunner};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// v1.3.4: parses the `--opt <tier>` flag (accepted as `--opt speed` or
/// `--opt=speed`). Returns `Ok(None)` when the flag is absent and `Err` for
/// an unknown tier value, which the caller turns into a hard CLI error.
fn parse_opt_tier(args: &[String]) -> Result<Option<OptTier>, String> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--opt=") {
            return OptTier::from_cli(value).map(Some).ok_or_else(|| {
                format!(
                    "unknown --opt tier '{}': expected 'speed' or 'default'",
                    value
                )
            });
        }
        if arg == "--opt" {
            let value = iter.next().cloned().ok_or_else(|| {
                "missing value for --opt: expected 'speed' or 'default'".to_string()
            })?;
            return OptTier::from_cli(&value).map(Some).ok_or_else(|| {
                format!(
                    "unknown --opt tier '{}': expected 'speed' or 'default'",
                    value
                )
            });
        }
    }
    Ok(None)
}

/// `forgen check` — fast static verification, no binaries.
pub(crate) fn cmd_check(args: &[String]) -> bool {
    let target_opt = extract_target_arg(args, 2);
    let layout = match ProjectDiscovery::discover(target_opt) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Check error: {}", e);
            std::process::exit(1);
        }
    };

    let compiler = ForgenCompiler::new("check");
    let start = Instant::now();

    let res = if layout.source_files.len() == 1 {
        compiler.check_file(&layout.source_files[0])
    } else {
        compiler.check_files(&layout.source_files)
    };

    let elapsed = start.elapsed().as_millis();

    if res.success {
        // Surface warnings (W-*) even on success so canonical-form nudges
        // (W-SYN-001/W-SYN-002/W-TYPE-002) are actually visible to users.
        let warning_records: Vec<&_> = res
            .diagnostic_records
            .iter()
            .filter(|d| d.severity == "WARNING")
            .collect();
        if !warning_records.is_empty() {
            eprintln!("{}", res.diagnostics);
        }

        // Dead-code lint (S1): report never-used top-level functions.
        // `--allow-dead` silences the whole dead_code:: family.
        let allow_dead = args.iter().any(|a| a == "--allow-dead");
        let mut dead_count = 0usize;
        if !allow_dead {
            // Whole-program dead-code analysis: all module files are merged
            // into one program so a function used from another module is not
            // reported as unused. Per-file linting cannot see cross-module
            // references. On parse failure the compiler check above has
            // already reported it, so the lint is silently skipped.
            if let Ok(diags) =
                crate::lint::lint_files_with_profile(&layout.source_files, crate::lint::LintProfile::Standard)
            {
                for d in &diags {
                    if d.code.starts_with("dead_code::") {
                        eprintln!(
                            "warning[{}]: {}\n  --> {}:{}:{}\n  help: {}",
                            d.code, d.message, d.span.file, d.span.start_line, d.span.start_col, d.help.clone().unwrap_or_default()
                        );
                        dead_count += 1;
                    }
                }
            }
        }

        println!(
            "[Forgen check] Verified 100% OK in {}ms ({} modules, 0 errors, {} warnings, valid ownership & effects)",
            elapsed,
            layout.source_files.len(),
            warning_records.len() + dead_count
        );
    } else {
        eprintln!("{}", res.diagnostics);
        std::process::exit(1);
    }
    true
}

pub fn check_target_backend(target_triple: Option<&str>, is_llvm: bool) -> Result<(), String> {
    if !is_llvm {
        if let Some(triple) = target_triple {
            let t = triple.to_lowercase();
            if t.starts_with("thumb") || t.contains("cortex-m") || t.contains("-none-") {
                return Err(format!(
                    "Error [E-TARGET-001]: Target '{}' is only supported via LLVM backend. Use '--llvm'",
                    triple
                ));
            }
        }
    }
    Ok(())
}

/// `forgen run` / `forgen quick` / `forgen start`.
pub(crate) fn cmd_run(command: &str, args: &[String]) -> bool {
    let mut target_arg: Option<&str> = None;
    let mut run_args = Vec::new();
    let mut after_dash_dash = false;
    let mut skip_next = false;

    for arg in args.iter().skip(2) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if after_dash_dash {
            run_args.push(arg.clone());
        } else if arg == "--" {
            after_dash_dash = true;
        } else if arg == "-o" || arg == "--out" || arg == "--pgo" || arg == "--target" {
            skip_next = true;
        } else if arg.starts_with("-") {
            // compiler flag, e.g. --llvm, --domain, -g, --debug
        } else if target_arg.is_none() {
            target_arg = Some(arg.as_str());
        } else {
            run_args.push(arg.clone());
        }
    }
    let target_opt = target_arg.map(Path::new);
    let layout = match ProjectDiscovery::discover(target_opt) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Run error: {}", e);
            std::process::exit(1);
        }
    };

    let is_domain = command == "domain" || args.iter().any(|a| a == "--domain");
    let mode = if command == "quick" || command == "start" {
        "quick"
    } else if is_domain {
        "domain"
    } else {
        "release"
    };
    let is_sandbox = args
        .iter()
        .any(|a| a == "--sandbox" || a.starts_with("--sandbox="));
    if is_sandbox {
        unsafe {
            std::env::set_var("DATARA_SANDBOX", "1");
        }
    }
    let is_llvm = args.iter().any(|a| a == "--llvm");
    let is_native = args
        .iter()
        .any(|a| a == "--native" || a == "--tune=native" || a.starts_with("--tune="));
    let target_triple = args
        .iter()
        .position(|a| a == "--target")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--target="))
                .and_then(|a| a.strip_prefix("--target=").map(|s| s.to_string()))
        })
        .or_else(|| {
            layout
                .manifest
                .as_ref()
                .and_then(|m| m.target.as_ref())
                .and_then(|t| t.arch.clone())
        });
    if let Err(e) = check_target_backend(target_triple.as_deref(), is_llvm) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    let debug_info = args.iter().any(|a| a == "-g" || a == "--debug");
    let is_pgo_train = args
        .iter()
        .any(|a| a == "--pgo-train" || a.starts_with("--pgo-train="));
    let profile_gen = if args
        .iter()
        .any(|a| a == "--profile" || a == "--profile-generate")
        || is_pgo_train
    {
        let prof_dir = layout.root.join(".forgen_profile");
        let _ = fs::create_dir_all(&prof_dir);
        if is_pgo_train {
            Some(layout.root.join("app.profdata"))
        } else {
            Some(prof_dir.join(format!("{}.json", layout.binary_name())))
        }
    } else {
        None
    };
    let pgo_profile = args
        .iter()
        .position(|a| a == "--pgo" || a == "--pgo-use")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--pgo-use="))
                .map(|a| PathBuf::from(a.strip_prefix("--pgo-use=").unwrap()))
        })
        .or_else(|| {
            if args.iter().any(|a| a == "--pgo-use" || a == "--pgo") {
                let candidate = layout.root.join("app.profdata");
                if candidate.exists() {
                    Some(candidate)
                } else {
                    let c2 = layout
                        .root
                        .join(".forgen_profile")
                        .join(format!("{}.json", layout.binary_name()));
                    if c2.exists() { Some(c2) } else { None }
                }
            } else {
                None
            }
        });
    let compiler = ForgenCompiler::new(mode)
        .with_llvm(is_llvm)
        .with_pgo(pgo_profile)
        .with_profile_generate(profile_gen.clone())
        .with_debug(debug_info)
        .with_target(target_triple)
        .with_native(is_native)
        .with_opt_tier(match parse_opt_tier(args) {
            Ok(tier) => tier.unwrap_or_default(),
            Err(e) => {
                eprintln!("Run error: {}", e);
                std::process::exit(1);
            }
        });

    // Cranelift in-memory JIT execution: zero disk artifacts, sub-millisecond launch
    if !is_llvm {
        match compiler.run_project_captured(&layout, &run_args, false) {
            Ok((stdout, stderr, code, _)) => {
                if let Some(ref p) = profile_gen
                    && p.exists()
                    && let Ok(prof) = crate::pgo::ProfileData::load_from_file(p)
                {
                    println!(
                        "[Forgen Profile] Generated runtime profile: {} (hot_funcs={}, branches={})",
                        p.display(),
                        prof.hot_functions.len(),
                        prof.branch_frequencies.len()
                    );
                }
                if !stdout.is_empty() {
                    print!("{}", stdout);
                }
                if !stderr.is_empty() {
                    eprint!("{}", stderr);
                }
                if code != 0 {
                    std::process::exit(code);
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        return false;
    }

    // Incremental caching check: if target binary is newer than all source files, run directly
    let bin_name = layout.binary_name();
    let exe_target = if layout.source_files.len() == 1 && layout.manifest.is_none() {
        layout.entry_point.with_extension("exe")
    } else {
        layout.root.join(format!("{}.exe", bin_name))
    };

    let cache_dir = layout.root.join(".forgen_cache");
    let mut inc_cache = crate::incremental::IncrementalCache::load_from_dir(&cache_dir);
    let mut all_fresh = !inc_cache.fingerprints.is_empty();
    for sf in &layout.source_files {
        if let Ok(content) = fs::read_to_string(sf) {
            // Transitive check: a module is only fresh when its whole
            // recorded dependency closure is fresh as well.
            if !inc_cache.is_module_fresh_transitive(sf, &content) {
                all_fresh = false;
            }
        } else {
            all_fresh = false;
        }
    }

    let mut newest_source_mod = None;
    for sf in &layout.source_files {
        if let Ok(meta) = fs::metadata(sf)
            && let Ok(mod_time) = meta.modified()
        {
            newest_source_mod = Some(
                newest_source_mod
                    .map_or(mod_time, |curr: std::time::SystemTime| curr.max(mod_time)),
            );
        }
    }
    if let Ok(meta) = fs::metadata(layout.root.join("datara.toml"))
        && let Ok(mod_time) = meta.modified()
    {
        newest_source_mod = Some(
            newest_source_mod.map_or(mod_time, |curr: std::time::SystemTime| curr.max(mod_time)),
        );
    }

    let exe_mod = fs::metadata(&exe_target)
        .map(|m| m.modified().ok())
        .ok()
        .flatten();
    let need_recompile = is_llvm
        || !all_fresh
        || match (newest_source_mod, exe_mod) {
            (Some(s), Some(e)) => s > e,
            _ => true,
        };

    if !need_recompile && exe_target.exists() {
        // Execute cached artifact immediately
        if let Ok((stdout, stderr, code, _)) =
            compiler.codegen.run_executable(&exe_target, &run_args)
        {
            print!("{}", stdout);
            if !stderr.is_empty() {
                eprint!("{}", stderr);
            }
            if code != 0 {
                std::process::exit(code);
            }
            return false;
        }
    }

    match compiler.run_project(&layout, &run_args) {
        Ok((stdout, stderr, code, _)) => {
            for sf in &layout.source_files {
                if let Ok(content) = fs::read_to_string(sf) {
                    inc_cache.update_module(sf, &content, Vec::new());
                }
            }
            let _ = inc_cache.save_to_dir(&cache_dir);

            print!("{}", stdout);
            if !stderr.is_empty() {
                eprint!("{}", stderr);
            }
            if code != 0 {
                std::process::exit(code);
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
    true
}

/// `forgen test` — run project integration tests.
pub(crate) fn cmd_test(args: &[String]) -> bool {
    let (target_opt, filter_str) = {
        let raw_target = extract_target_arg(args, 2);
        if let Some(p) = raw_target {
            if p.exists() {
                let p_str = p.to_str().unwrap_or("");
                let mut filter = None;
                let mut found_target = false;
                for a in args.iter().skip(2) {
                    if a.starts_with('-') {
                        continue;
                    }
                    if !found_target && a == p_str {
                        found_target = true;
                        continue;
                    }
                    if found_target {
                        filter = Some(a.clone());
                        break;
                    }
                }
                (Some(p), filter)
            } else {
                (None, Some(p.to_string_lossy().to_string()))
            }
        } else {
            (None, None)
        }
    };

    let layout = match ProjectDiscovery::discover(target_opt) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Test discovery error: {}", e);
            std::process::exit(1);
        }
    };

    if args.iter().any(|a| a == "--list") {
        let tests = ProjectRunner::list_tests(&layout, filter_str.as_deref());
        for t in &tests {
            println!("{}: test", t);
        }
        println!("\n{} tests", tests.len());
        return true;
    }

    let is_llvm = args.iter().any(|a| a == "--llvm");
    let is_native = args
        .iter()
        .any(|a| a == "--native" || a == "--tune=native" || a.starts_with("--tune="));
    let target_triple = args
        .iter()
        .position(|a| a == "--target")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--target="))
                .and_then(|a| a.strip_prefix("--target=").map(|s| s.to_string()))
        });
    let debug_info = args.iter().any(|a| a == "-g" || a == "--debug");
    let compiler = ForgenCompiler::new("release")
        .with_llvm(is_llvm)
        .with_debug(debug_info)
        .with_target(target_triple)
        .with_native(is_native);
    let rep = ProjectRunner::run_tests_filtered(&layout, &compiler, filter_str.as_deref());

    println!(
        "\nrunning {} test{}",
        rep.total,
        if rep.total == 1 { "" } else { "s" }
    );
    for item in &rep.results {
        if item.passed {
            println!("test {} ... ok ({}ms)", item.name, item.duration_ms);
        } else {
            println!("test {} ... FAILED ({}ms)", item.name, item.duration_ms);
            if let Some(ref err) = item.error {
                println!("  Error: {}", err);
            }
            if !item.output.is_empty() {
                println!("  Output: {}", item.output.trim());
            }
        }
    }

    let status_str = if rep.failed == 0 { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {}. {} passed; {} failed; finished in {:.2}s\n",
        status_str,
        rep.passed,
        rep.failed,
        (rep.total_duration_ms as f64) / 1000.0
    );

    if rep.failed > 0 {
        std::process::exit(1);
    }
    true
}

/// `forgen bench` — run benchmarks.
pub(crate) fn cmd_bench(args: &[String]) -> bool {
    let target_opt = extract_target_arg(args, 2);
    let layout = match ProjectDiscovery::discover(target_opt) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Benchmark discovery error: {}", e);
            std::process::exit(1);
        }
    };

    let is_llvm = args.iter().any(|a| a == "--llvm");
    let is_native = args
        .iter()
        .any(|a| a == "--native" || a == "--tune=native" || a.starts_with("--tune="));
    let target_triple = args
        .iter()
        .position(|a| a == "--target")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--target="))
                .and_then(|a| a.strip_prefix("--target=").map(|s| s.to_string()))
        });
    let compiler = ForgenCompiler::new("release")
        .with_llvm(is_llvm)
        .with_target(target_triple)
        .with_native(is_native);
    if let Err(e) = ProjectRunner::run_benches(&layout, &compiler) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
    true
}

/// `forgen build` / `release` / `debug` / `verify` — AOT native compilation.
pub(crate) fn cmd_build(command: &str, args: &[String]) -> bool {
    let target_opt = extract_target_arg(args, 2);
    let layout = match ProjectDiscovery::discover(target_opt) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Build discovery error: {}", e);
            std::process::exit(1);
        }
    };

    let is_pgo_train = args
        .iter()
        .any(|a| a == "--pgo-train" || a.starts_with("--pgo-train="));
    let is_pgo_use = args
        .iter()
        .any(|a| a == "--pgo-use" || a.starts_with("--pgo-use="));

    let pgo_profile = if is_pgo_use {
        let explicit_use = args
            .iter()
            .position(|a| a == "--pgo-use")
            .and_then(|i| args.get(i + 1))
            .filter(|a| !a.starts_with("-"))
            .map(PathBuf::from)
            .or_else(|| {
                args.iter()
                    .find(|a| a.starts_with("--pgo-use="))
                    .map(|a| PathBuf::from(a.strip_prefix("--pgo-use=").unwrap()))
            });
        explicit_use.or_else(|| {
            let c1 = layout.root.join("app.profdata");
            if c1.exists() {
                Some(c1)
            } else {
                let c2 = layout
                    .root
                    .join(".forgen_profile")
                    .join(format!("{}.json", layout.binary_name()));
                if c2.exists() { Some(c2) } else { None }
            }
        })
    } else {
        let explicit_pgo = args
            .iter()
            .position(|a| a == "--pgo")
            .and_then(|i| args.get(i + 1))
            .filter(|a| !a.starts_with("-"))
            .map(PathBuf::from)
            .or_else(|| {
                args.iter()
                    .find(|a| a.starts_with("--pgo="))
                    .map(|a| PathBuf::from(a.strip_prefix("--pgo=").unwrap()))
            });
        explicit_pgo.or_else(|| {
            if args.iter().any(|a| a == "--pgo" || a.starts_with("--pgo=")) {
                let c1 = layout.root.join("app.profdata");
                if c1.exists() {
                    Some(c1)
                } else {
                    let c2 = layout
                        .root
                        .join(".forgen_profile")
                        .join(format!("{}.json", layout.binary_name()));
                    if c2.exists() { Some(c2) } else { None }
                }
            } else {
                None
            }
        })
    };
    let target_triple = args
        .iter()
        .position(|a| a == "--target")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--target="))
                .and_then(|a| a.strip_prefix("--target=").map(|s| s.to_string()))
        })
        .or_else(|| {
            layout
                .manifest
                .as_ref()
                .and_then(|m| m.target.as_ref())
                .and_then(|t| t.arch.clone())
        });
    let is_tiny = args
        .iter()
        .any(|a| a == "--tiny" || a == "-Oz" || a == "--profile=tiny");
    if is_tiny {
        // Propagate the tiny profile to codegen: `link_args()` switches to the
        // ultra-compact link line (/NODEFAULTLIB + /FILEALIGN:512) when
        // FORGEN_TINY is set. The CLI runs single-threaded at this point, so
        // the unsafe env write is safe (same pattern as DATARA_SANDBOX above).
        unsafe {
            std::env::set_var("FORGEN_TINY", "1");
        }
    }
    let is_embed = args.iter().any(|a| a == "--embed");
    let debug_info = command == "debug" || args.iter().any(|a| a == "-g" || a == "--debug");
    let mode = if is_tiny {
        "tiny"
    } else if args.iter().any(|a| a == "--domain") {
        "domain"
    } else if command == "build" {
        if debug_info { "debug" } else { "release" }
    } else {
        command
    };
    let is_llvm = args.iter().any(|a| a == "--llvm");
    if let Err(e) = check_target_backend(target_triple.as_deref(), is_llvm) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    let is_native = args
        .iter()
        .any(|a| a == "--native" || a == "--tune=native" || a.starts_with("--tune="));
    let is_profile_generate = is_pgo_train
        || args
            .iter()
            .any(|a| a == "--profile-generate" || a.starts_with("--profile-generate="));
    let profile_generate = if is_profile_generate {
        let prof_gen_path = args
            .iter()
            .position(|a| a == "--profile-generate" || a == "--pgo-train")
            .and_then(|i| args.get(i + 1))
            .filter(|a| !a.starts_with("-"))
            .map(PathBuf::from)
            .or_else(|| {
                args.iter()
                    .find(|a| a.starts_with("--profile-generate="))
                    .map(|a| PathBuf::from(a.strip_prefix("--profile-generate=").unwrap()))
            })
            .or_else(|| {
                args.iter()
                    .find(|a| a.starts_with("--pgo-train="))
                    .map(|a| PathBuf::from(a.strip_prefix("--pgo-train=").unwrap()))
            });
        let prof_dir = layout.root.join(".forgen_profile");
        let _ = fs::create_dir_all(&prof_dir);
        Some(prof_gen_path.unwrap_or_else(|| {
            if is_pgo_train {
                layout.root.join("app.profdata")
            } else {
                prof_dir.join(format!("{}.json", layout.binary_name()))
            }
        }))
    } else {
        None
    };

    let compiler = ForgenCompiler::new(mode)
        .with_llvm(is_llvm)
        .with_pgo(pgo_profile)
        .with_profile_generate(profile_generate.clone())
        .with_debug(debug_info)
        .with_target(target_triple)
        .with_native(is_native)
        .with_opt_tier(match parse_opt_tier(args) {
            Ok(tier) => tier.unwrap_or_default(),
            Err(e) => {
                eprintln!("Build error: {}", e);
                std::process::exit(1);
            }
        });

    let start = Instant::now();
    let bin_name = layout.binary_name();
    let is_wasm_target = args.iter().any(|a| {
        a == "--wasm"
            || a == "--target=wasm"
            || a == "--target=wasm32"
            || a == "wasm32"
            || a == "wasm"
    }) || args.windows(2).any(|w| {
        w[0] == "--target" && (w[1] == "wasm" || w[1] == "wasm32" || w[1].starts_with("wasm32-"))
    }) || args
        .windows(2)
        .any(|w| (w[0] == "-o" || w[0] == "--out") && w[1].ends_with(".wasm"));
    let is_python_target = args.iter().any(|a| a == "--python");
    let is_lib_target = args.iter().any(|a| a == "--lib" || a == "--embed");
    let lib_ext = if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let output_exe = if let Some(pos) = args.iter().position(|a| a == "-o" || a == "--out") {
        if let Some(val) = args.get(pos + 1) {
            PathBuf::from(val)
        } else if is_wasm_target {
            layout.root.join(format!("{}.wasm", bin_name))
        } else if is_python_target || is_lib_target {
            layout.root.join(format!("{}.{}", bin_name, lib_ext))
        } else {
            layout.root.join(format!("{}.exe", bin_name))
        }
    } else if is_wasm_target {
        layout.root.join(format!("{}.wasm", bin_name))
    } else if is_python_target || is_lib_target {
        layout.root.join(format!("{}.{}", bin_name, lib_ext))
    } else {
        layout.root.join(format!("{}.exe", bin_name))
    };

    if is_wasm_target {
        let dmir_res = if layout.source_files.len() == 1 {
            compiler.compile_file_to_dmir(&layout.source_files[0])
        } else {
            compiler.compile_files_to_dmir(&layout.source_files)
        };
        match dmir_res {
            Ok(dmir_mod) => {
                match crate::codegen::wasm::WasmEmitter::emit_wasm_binary(&dmir_mod, &output_exe) {
                    Ok(wasm_path) => {
                        let elapsed = start.elapsed().as_millis();
                        println!(
                            "[Forgen WASM] WebAssembly compilation succeeded in {}ms",
                            elapsed
                        );
                        println!("[Forgen WASM] Target Architecture: wasm32-unknown-wasi");
                        println!("[Forgen WASM] Output: {}", wasm_path.display());
                        println!(
                            "[Forgen WASM] WAT:    {}",
                            wasm_path.with_extension("wat").display()
                        );
                        println!(
                            "[Forgen WASM] JS:     {}",
                            wasm_path.with_extension("js").display()
                        );
                        println!(
                            "[Forgen WASM] HTML:   {}",
                            wasm_path.with_extension("html").display()
                        );
                        return false;
                    }
                    Err(e) => {
                        eprintln!("[Forgen WASM] Codegen error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("[Forgen WASM] Compilation error:\n{}", e);
                std::process::exit(1);
            }
        }
    }

    let res = if layout.source_files.len() == 1 {
        compiler.compile_file(&layout.source_files[0], Some(&output_exe))
    } else {
        compiler.compile_files(&layout.source_files, Some(&output_exe))
    };
    let elapsed = start.elapsed().as_millis();

    if res.success {
        print_optimizer_warnings(&res);
        if is_llvm {
            println!("[Forgen LLVM] Ultra-optimized AOT LLVM pipeline completed.");
        }
        if is_lib_target {
            println!(
                "[Forgen] Shared library build succeeded in {}ms ({} mode)",
                elapsed, mode
            );
        } else {
            println!("[Forgen] Build succeeded in {}ms ({} mode)", elapsed, mode);
        }
        println!(
            "[Forgen] Project: {} ({} source files)",
            layout.name,
            layout.source_files.len()
        );
        if let Some(exe_p) = res.exe_path.as_ref() {
            println!("[Forgen] Output:  {}", exe_p.display());
            if let Ok(meta) = fs::metadata(exe_p) {
                let bytes = meta.len();
                let kb = bytes as f64 / 1024.0;
                println!("[Forgen] Size:    {:.1} KB ({} bytes)", kb, bytes);
            }
            if is_tiny {
                println!("[Forgen] Profile: Ultra-Compact (dead code stripped, minimal footprint)");
            }
            if is_embed {
                println!(
                    "[Forgen] Profile: C-ABI Embeddable Library (game engine / host scripting integration)"
                );
                let h_path = exe_p.with_extension("h");
                if let Some(src) = layout.source_files.first() {
                    if let Ok(p) = crate::export::export_c_header(src, &h_path) {
                        println!("[Forgen Embed] Generated C ABI header: {}", p.display());
                    }
                }
                if let Some(parent) = exe_p.parent() {
                    let embed_h = parent.join("datara_embed.h");
                    if let Ok(p) = crate::export::export_embed_header(&embed_h) {
                        println!(
                            "[Forgen Embed] Generated embed runtime header: {}",
                            p.display()
                        );
                    }
                }
            }

            if is_pgo_train && !is_lib_target {
                println!(
                    "[Forgen PGO] Auto-running instrumented training binary to generate profile..."
                );
                let mut train_cmd = std::process::Command::new(exe_p);
                if let Some(pos) = args.iter().position(|a| a == "--") {
                    train_cmd.args(&args[pos + 1..]);
                }
                let status = train_cmd.status();
                if let Ok(st) = status {
                    println!(
                        "[Forgen PGO] Training execution completed with status: {}",
                        st
                    );
                }
                if let Some(ref prof_p) = profile_generate {
                    if prof_p.exists() {
                        println!(
                            "[Forgen PGO] Generated runtime profile data: {}",
                            prof_p.display()
                        );
                    }
                }
            }

            if is_python_target {
                let py_path = exe_p.with_extension("py");
                // Paths and identifiers are embedded verbatim into
                // generated Python source; escape them so a path with
                // a quote or a foreign identifier cannot break or
                // inject into the generated module.
                let escape_py = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
                let mut py_code = format!(
                    "# Auto-generated Datara Python Bridge for {}\nimport ctypes\nimport os\n\n_dll_path = os.path.abspath(r\"{}\")\n_lib = ctypes.CDLL(_dll_path)\n\n",
                    escape_py(&bin_name),
                    escape_py(&exe_p.display().to_string())
                );

                if let Some(ref dmir) = res.dmir_module {
                    for fn_name in dmir.functions.keys() {
                        // Only valid Python identifiers are exported;
                        // everything else would produce a syntax
                        // error (or worse) in the generated module.
                        let is_ident = !fn_name.is_empty()
                            && fn_name
                                .chars()
                                .next()
                                .map(|c| c.is_ascii_alphabetic() || c == '_')
                                .unwrap_or(false)
                            && fn_name
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '_')
                            && !keyword_set().contains(fn_name.as_str());
                        if !is_ident {
                            continue;
                        }
                        py_code.push_str(&format!(
                                "def {}(*args):\n    _fn = getattr(_lib, \"{}\")\n    _fn.restype = ctypes.c_int64\n    return _fn(*args)\n\n",
                                fn_name, fn_name
                            ));
                    }
                }

                if let Err(e) = fs::write(&py_path, py_code) {
                    eprintln!("Warning: Failed to write Python wrapper: {}", e);
                } else {
                    println!(
                        "[Forgen Python FFI] Synthesized Python module: {}",
                        py_path.display()
                    );
                }
            }

            if args.iter().any(|a| a == "--ledger")
                && let Some(report) = &res.optimization_report
            {
                let ledger_path = exe_p.with_extension("ledger.json");
                let ledger_data = serde_json::json!({
                    "version": "1.0",
                    "compiler": "forgen",
                    "mode": mode,
                    "summary": {
                        "variables_promoted": report.variables_promoted,
                        "constants_folded": report.constants_folded,
                        "dead_instructions_removed": report.dead_instructions_removed,
                        "functions_inlined": report.functions_inlined,
                        "allocations_eliminated": report.allocations_eliminated,
                        "evidence_downgrades": report.evidence_downgrades,
                    },
                    "decision_trace": report.decision_trace,
                });
                if let Ok(json) = serde_json::to_string_pretty(&ledger_data)
                    && fs::write(&ledger_path, json).is_ok()
                {
                    println!("[Forgen] Ledger:  {}", ledger_path.display());
                }
            }

            if args.iter().any(|a| a == "--graph")
                && let Some(graph) = &res.semantic_graph
            {
                let graph_path = exe_p.with_extension("graph.json");
                // Round-trip through serde_json::Value so the graph's
                // HashMap fields emit with sorted (deterministic) keys.
                if let Ok(json) = serde_json::to_string_pretty(&to_json_value(graph))
                    && fs::write(&graph_path, json).is_ok()
                {
                    println!("[Forgen] Graph:   {}", graph_path.display());
                }
            }
        }
    } else {
        eprintln!(
            "{}",
            res.error.unwrap_or_else(|| "Compilation failed".into())
        );
        std::process::exit(1);
    }
    true
}

/// `forgen domain` — whole-program specialization report and specialized build/run execution.
pub(crate) fn cmd_domain(args: &[String]) -> bool {
    // Subcommand dispatch: `forgen domain build ...` or `forgen domain run ...`
    if args.len() > 2 {
        if args[2] == "build" {
            let mut sub_args = vec![args[0].clone(), "build".to_string(), "--domain".to_string()];
            sub_args.extend_from_slice(&args[3..]);
            return cmd_build("domain", &sub_args);
        } else if args[2] == "run" {
            let mut sub_args = vec![args[0].clone(), "run".to_string(), "--domain".to_string()];
            sub_args.extend_from_slice(&args[3..]);
            return cmd_run("domain", &sub_args);
        }
    }

    let is_llvm = args.iter().any(|a| a == "--llvm");
    let is_native = args
        .iter()
        .any(|a| a == "--native" || a == "--tune=native" || a.starts_with("--tune="));
    let compiler = ForgenCompiler::new("domain")
        .with_llvm(is_llvm)
        .with_native(is_native);

    let mut pgo_profile = None;
    let mut filter_args: Vec<String> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--pgo" && i + 1 < args.len() {
            pgo_profile = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else if args[i] == "--json" || args[i] == "--llvm" || args[i] == "--native" {
            i += 1;
        } else {
            filter_args.push(args[i].clone());
            i += 1;
        }
    }

    let target_opt = filter_args.first().map(|s| Path::new(s.as_str()));
    let layout = match ProjectDiscovery::discover(target_opt) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Domain discovery error: {}", e);
            std::process::exit(1);
        }
    };

    if pgo_profile.is_none() {
        let c1 = layout.root.join("app.profdata");
        if c1.exists() {
            pgo_profile = Some(c1);
        } else {
            let c2 = layout
                .root
                .join(".forgen_profile")
                .join(format!("{}.json", layout.binary_name()));
            if c2.exists() {
                pgo_profile = Some(c2);
            }
        }
    }

    let compiler = compiler.with_pgo(pgo_profile.clone());
    let res = if layout.source_files.len() == 1 {
        compiler.compile_file(&layout.source_files[0], None)
    } else {
        compiler.compile_files(&layout.source_files, None)
    };
    if res.success {
        print_optimizer_warnings(&res);
        if is_llvm {
            println!(
                "[Forgen LLVM] Whole-program Domain compilation with LLVM pipeline completed."
            );
        }
        let rep = res.optimization_report.unwrap_or_default();
        let t = res.timings;

        if args.iter().any(|a| a == "--json") {
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("optimizationReport".into(), to_json_value(&rep));
            json_obj.insert("timings".into(), to_json_value(&t));
            json_obj.insert(
                "outputBinary".into(),
                serde_json::Value::String(
                    res.exe_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                ),
            );
            if let Some(ref p) = pgo_profile {
                json_obj.insert(
                    "pgoProfile".into(),
                    serde_json::Value::String(p.to_string_lossy().to_string()),
                );
            }
            println!("{}", to_pretty_json(&json_obj));
            return false;
        }

        println!("============================================================");
        println!("             FORGEN DOMAIN SPECIALIZATION REPORT            ");
        println!("============================================================");
        println!(" Project name:               {}", layout.name);
        println!(" Modules analyzed:           {}", rep.modules_analyzed);
        println!(" Symbols analyzed:           {}", rep.symbols_analyzed);
        println!(" Reachable symbols:          {}", rep.reachable_symbols);
        println!(" Removed dead symbols:       {}", rep.removed_symbols);
        println!(
            " Generic specializations:    {:?}",
            rep.generic_specializations
        );
        println!(" Functions inlined:          {}", rep.functions_inlined);
        println!(
            " Allocations eliminated:     {}",
            rep.allocations_eliminated
        );
        println!(" Constants folded:           {}", rep.constants_folded);
        println!(
            " Dead instructions removed:  {}",
            rep.dead_instructions_removed
        );
        println!(
            " Linked runtime modules:     {:?}",
            rep.runtime_modules_linked
        );
        println!(
            " Stripped runtime modules:   {:?}",
            rep.runtime_modules_stripped
        );
        if let Some(ref p) = pgo_profile {
            println!(" PGO Profile applied:        {}", p.display());
        }
        println!("------------------------------------------------------------");
        println!(" Pipeline Timings Breakdown:");
        println!("   Discovery:   {:>4}ms", t.discovery_ms);
        println!("   Parse:       {:>4}ms", t.parse_ms);
        println!("   Resolve:     {:>4}ms", t.resolve_ms);
        println!("   TypeCheck:   {:>4}ms", t.typecheck_ms);
        println!("   Effects:     {:>4}ms", t.effects_ms);
        println!("   Ownership:   {:>4}ms", t.ownership_ms);
        println!("   Graph:       {:>4}ms", t.graph_ms);
        println!("   Optimizer:   {:>4}ms", t.optimizer_ms);
        println!("   Codegen:     {:>4}ms", t.codegen_ms);
        println!("   Link:        {:>4}ms", t.link_ms);
        println!("   Total:       {:>4}ms", t.total_ms);
        println!(
            " Output binary:              {}",
            res.exe_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        );
        println!("============================================================");
    } else {
        eprintln!(
            "{}",
            res.error.unwrap_or_else(|| "Compilation failed".into())
        );
        std::process::exit(1);
    }
    true
}

/// `forgen profile` — compile with instrumentation, execute, and write a runtime profile.
pub(crate) fn cmd_profile(args: &[String]) -> bool {
    let target_opt = args
        .iter()
        .skip(2)
        .find(|a| !a.starts_with("-") && *a != "--")
        .map(Path::new);
    let layout = match ProjectDiscovery::discover(target_opt) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Profile discovery error: {}", e);
            std::process::exit(1);
        }
    };

    let prof_dir = layout.root.join(".forgen_profile");
    let _ = fs::create_dir_all(&prof_dir);
    let prof_file = prof_dir.join(format!("{}.json", layout.binary_name()));

    let is_mem_profile = args.iter().any(|a| a == "--mem" || a == "--memory");
    let mem_prof_file = prof_dir.join(format!("{}_mem.json", layout.binary_name()));
    if is_mem_profile {
        unsafe {
            std::env::set_var("DATARA_MEMPROFILE_OUT", &mem_prof_file);
        }
    }

    let compiler = ForgenCompiler::new("release").with_profile_generate(Some(prof_file.clone()));
    let res = if layout.source_files.len() == 1 {
        compiler.compile_file(&layout.source_files[0], None)
    } else {
        compiler.compile_files(&layout.source_files, None)
    };

    if !res.success {
        if is_mem_profile {
            unsafe {
                std::env::remove_var("DATARA_MEMPROFILE_OUT");
            }
        }
        eprintln!("Profile build failed: {}", res.error.unwrap_or_default());
        std::process::exit(1);
    }

    // Actually execute the program to measure runtime behavior.
    let exe = match res.exe_path.clone() {
        Some(p) => p,
        None => {
            if is_mem_profile {
                unsafe {
                    std::env::remove_var("DATARA_MEMPROFILE_OUT");
                }
            }
            eprintln!("Profile build produced no executable");
            std::process::exit(1);
        }
    };
    let run = compiler.codegen.run_executable(&exe, &[]);

    if is_mem_profile {
        unsafe {
            std::env::remove_var("DATARA_MEMPROFILE_OUT");
        }
    }

    match run {
        Ok((stdout, stderr, code, elapsed_ns)) => {
            println!(
                "[Forgen Profile] Ran {} (exit {}, {:.2} ms)",
                exe.display(),
                code,
                elapsed_ns as f64 / 1_000_000.0
            );
            if !stdout.is_empty() {
                println!("[Forgen Profile] stdout: {}", stdout.trim_end());
            }
            if !stderr.is_empty() {
                println!("[Forgen Profile] stderr: {}", stderr.trim_end());
            }
        }
        Err(e) => {
            eprintln!("[Forgen Profile] Program failed to run: {}", e);
        }
    }

    if is_mem_profile {
        if let Ok(mem_json) = fs::read_to_string(&mem_prof_file) {
            println!("============================================================");
            println!("       DATARA RUNTIME MEMORY PROFILE REPORT                 ");
            println!("============================================================");
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&mem_json) {
                let alloc_cnt = v
                    .get("total_alloc_count")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let free_cnt = v
                    .get("total_free_count")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let alloc_b = v
                    .get("total_allocated_bytes")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let freed_b = v
                    .get("total_freed_bytes")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let peak_b = v
                    .get("peak_live_bytes")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let cur_b = v
                    .get("current_live_bytes")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let arena_b = v.get("arena_bytes").and_then(|x| x.as_u64()).unwrap_or(0);
                let promo_cnt = v
                    .get("promotions_count")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let promo_b = v
                    .get("promoted_bytes")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let slab_h = v.get("slab_hits").and_then(|x| x.as_u64()).unwrap_or(0);
                let slab_m = v.get("slab_misses").and_then(|x| x.as_u64()).unwrap_or(0);

                println!(
                    "  Allocations:            {} (peak live: {} B)",
                    alloc_cnt, peak_b
                );
                println!(
                    "  Frees:                  {} (live at exit: {} B)",
                    free_cnt, cur_b
                );
                println!("  Cumulative Allocated:   {} B", alloc_b);
                println!("  Cumulative Freed:       {} B", freed_b);
                println!("  Generational Arena:     {} B active", arena_b);
                println!(
                    "  Promotions (zero-copy): {} promotions ({} B moved)",
                    promo_cnt, promo_b
                );
                let total_slab = slab_h + slab_m;
                let slab_ratio = if total_slab > 0 {
                    (slab_h as f64 / total_slab as f64) * 100.0
                } else {
                    0.0
                };
                println!(
                    "  Slab Cache Hits:        {} / {} ({:.1}%)",
                    slab_h, total_slab, slab_ratio
                );
            }
            println!("  Memory JSON dump:       {}", mem_prof_file.display());
            println!("============================================================");
        }
    }

    if let Ok(prof) = ProfileData::load_from_file(&prof_file) {
        println!(
            "[Forgen Profile] Wrote RUNTIME instrumented profile: {}",
            prof_file.display()
        );
        println!(
            "[Forgen Profile] Measured {} hot functions, {} branch decisions (runtime provenance verified).",
            prof.hot_functions.len(),
            prof.branch_frequencies.len()
        );
    } else {
        println!(
            "[Forgen Profile] Profile generated at: {}",
            prof_file.display()
        );
    }

    let should_build = args
        .iter()
        .any(|a| a == "--build" || a == "--apply" || a == "--aot");
    if should_build {
        println!(
            "[Forgen Profile] Closing PGO loop: Compiling optimized AOT binary using measured profile..."
        );
        let is_llvm = args.iter().any(|a| a == "--llvm");
        let aot_compiler = ForgenCompiler::new("release")
            .with_llvm(is_llvm)
            .with_pgo(Some(prof_file.clone()));
        let aot_res = if layout.source_files.len() == 1 {
            aot_compiler.compile_file(&layout.source_files[0], None)
        } else {
            aot_compiler.compile_files(&layout.source_files, None)
        };
        if aot_res.success {
            println!(
                "[Forgen Profile] PGO Optimization Complete: {}",
                aot_res
                    .exe_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            );
        } else {
            eprintln!(
                "[Forgen Profile] PGO compile failed: {}",
                aot_res.error.unwrap_or_default()
            );
            return false;
        }
    }

    true
}

/// Print WARNING-severity diagnostics (e.g. E-OPT-001 layout gate) after a
/// successful build: correctness advisories must reach the user even when
/// compilation succeeds.
fn print_optimizer_warnings(res: &crate::driver::CompilationResult) {
    for d in &res.diagnostic_records {
        if d.severity == "WARNING" {
            eprintln!("warning[{}]: {}", d.code, d.message);
            if let Some(h) = &d.help {
                eprintln!("  help: {}", h);
            }
        }
    }
}
