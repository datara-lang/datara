//! `forgen doctor` (v1.4.0): compiler/environment health checks.
//!
//! `forgen doctor --bridges` runs the interop scenario matrix over every
//! foreign bridge (C, Python, JS, Rust) and prints an aligned verdict table
//! with per-bridge call overhead and an overall health verdict. See
//! `crate::interop_audit` for the probe design.

use crate::interop_audit::run_bridge_audit;

/// `forgen doctor [flags]` - diagnostics for the local toolchain.
pub(crate) fn cmd_doctor(args: &[String]) -> bool {
    let bridges = args.iter().any(|a| a == "--bridges");

    println!("[Forgen doctor] Datara/Forgen toolchain diagnostics");
    println!("  version:  {}", env!("CARGO_PKG_VERSION"));
    println!("  host:     {}", std::env::consts::OS);
    println!("  arch:     {}", std::env::consts::ARCH);
    println!("  runtime:  bundled C runtime (linked per-executable)");
    println!();

    if !bridges {
        println!("  Run 'forgen doctor --bridges' to audit the foreign-function");
        println!("  bridges (C, Python, JS, Rust) with a compiled scenario matrix:");
        println!("    call round-trip, primitives, aggregates, error transfer and a");
        println!("    1000-iteration leak smoke with per-call overhead measurement.");
        return true;
    }

    let start = std::time::Instant::now();
    let scenarios = run_bridge_audit();
    let elapsed = start.elapsed().as_millis();

    println!("Interop bridge audit (compiled probes, real execution):");
    print!("{}", crate::interop_audit::render_table(&scenarios));

    let fails = scenarios
        .iter()
        .filter(|s| s.verdict == crate::interop_audit::Verdict::Fail)
        .count();
    let passes = scenarios
        .iter()
        .filter(|s| s.verdict == crate::interop_audit::Verdict::Pass)
        .count();
    let skips = scenarios
        .iter()
        .filter(|s| s.verdict == crate::interop_audit::Verdict::Skip)
        .count();

    println!();
    println!(
        "[Forgen doctor] {} scenario(s): {} pass, {} skip, {} fail ({}ms)",
        scenarios.len(),
        passes,
        skips,
        fails,
        elapsed
    );
    if fails > 0 {
        println!("  Overall: POOR - failing bridges above; set FORGEN_DOCTOR_DEEP=1");
        println!("  to include the Rust crate-build probe.");
        std::process::exit(1);
    }
    println!("  Overall: environment healthy for interop development.");
    true
}
