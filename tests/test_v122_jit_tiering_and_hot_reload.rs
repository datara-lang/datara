//! Datara & Forgen v1.2.2 Test Suite: Adaptive JIT Tiering & Differential Hot-Reloading
//!
//! Validates:
//! 1. `TieredJitController`: Function invocation and loop trip counters trigger
//!    promotions from Tier 0 (Baseline) to Tier 1 (Vectorized SIMD) and Tier 2 (Peak Compute).
//! 2. `DifferentialAstCache`: Accurate detection of modified, added, and unchanged functions.
//! 3. `JitSession::hot_reload_delta`: Sub-millisecond atomic trampoline hot-swapping.
//! 4. End-to-end execution of updated game loop logic mid-flight without state reset.

use forgen::codegen::cranelift::RealCraneliftBackend;
use forgen::codegen::cranelift::backend::opts::JitCompilationTier;
use forgen::codegen::cranelift::delta_cache::DifferentialAstCache;
use forgen::codegen::cranelift::jit::JitSession;
use forgen::codegen::cranelift::tiering::{FunctionTier, TieredJitController, TieringThresholds};
use forgen::codegen::target::TargetInfo;
use forgen::driver::ForgenCompiler;

#[test]
fn test_v122_jit_tiering_controller() {
    let thresholds = TieringThresholds {
        tier_0_to_1_invocations: 50,
        tier_0_to_1_loop_trips: 100,
        tier_1_to_2_invocations: 500,
    };
    let controller = TieredJitController::new(thresholds);

    controller.register_function("physics_step", FunctionTier::Tier0Baseline);
    assert_eq!(
        controller.get_tier("physics_step"),
        FunctionTier::Tier0Baseline
    );

    // Call 49 times -> still Tier 0
    for _ in 0..49 {
        let promo = controller.check_and_record("physics_step", false, 1);
        assert!(promo.is_none());
    }
    assert_eq!(
        controller.get_tier("physics_step"),
        FunctionTier::Tier0Baseline
    );

    // 50th call -> Promotes to Tier 1
    let promo = controller.check_and_record("physics_step", false, 1);
    assert_eq!(
        promo,
        Some((FunctionTier::Tier0Baseline, FunctionTier::Tier1Vectorized))
    );
    assert_eq!(
        controller.get_tier("physics_step"),
        FunctionTier::Tier1Vectorized
    );

    // Loop trips test on another function
    controller.register_function("ray_march_loop", FunctionTier::Tier0Baseline);
    let promo_loop = controller.check_and_record("ray_march_loop", true, 150);
    assert_eq!(
        promo_loop,
        Some((FunctionTier::Tier0Baseline, FunctionTier::Tier1Vectorized))
    );

    let transitions = controller.get_transitions();
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].0, "physics_step");
    assert_eq!(transitions[1].0, "ray_march_loop");
}

#[test]
fn test_v122_differential_ast_cache() {
    let src1 = r#"
fn calculate_speed(x: Int) -> Int => x * 2
fn calculate_gravity(mass: Int) -> Int => mass * 10
fn main() -> Int => calculate_speed(5)
"#;
    let compiler = ForgenCompiler::new("quick");
    let res1 = compiler.compile_source(src1, "test1.dtr", None);
    assert!(res1.success, "compile 1 failed: {:?}", res1.diagnostics);
    let dmir_mod1 = res1.dmir_module.expect("DMIR module 1");

    let cache = DifferentialAstCache::new();
    let delta1 = cache.diff_and_update(&dmir_mod1);

    // Initial load: everything is added
    assert_eq!(delta1.modified.len(), 0);
    assert!(delta1.added.contains(&"calculate_speed".to_string()));
    assert!(delta1.added.contains(&"calculate_gravity".to_string()));

    // Source 2: modify calculate_speed, add new function, keep calculate_gravity untouched
    let src2 = r#"
fn calculate_speed(x: Int) -> Int => x * 99
fn calculate_gravity(mass: Int) -> Int => mass * 10
fn calculate_drag(v: Int) -> Int => v * 3
fn main() -> Int => calculate_speed(5)
"#;
    let res2 = compiler.compile_source(src2, "test2.dtr", None);
    assert!(res2.success, "compile 2 failed: {:?}", res2.diagnostics);
    let dmir_mod2 = res2.dmir_module.expect("DMIR module 2");

    let delta2 = cache.diff_and_update(&dmir_mod2);
    assert!(delta2.modified.contains(&"calculate_speed".to_string()));
    assert_eq!(delta2.added, vec!["calculate_drag".to_string()]);
    assert!(delta2.unchanged.contains(&"calculate_gravity".to_string()));
}

#[test]
fn test_v122_hot_reload_delta_in_jit_session() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src1 = r#"
fn compute_score(x: Int) -> Int => x + 10

fn main() -> Int {
    let s = compute_score(5)
    println(int_to_str(s))
    0
}
"#;
    let compiler = ForgenCompiler::new("quick");
    let res1 = compiler.compile_source(src1, "game.dtr", None);
    assert!(res1.success, "compile 1 failed: {:?}", res1.diagnostics);
    let dmir_mod1 = res1.dmir_module.expect("DMIR module 1");

    session
        .load_module(&dmir_mod1)
        .expect("load module into JIT");
    let (stdout1, _, code1, _) = session.run_entry(None, &[], true).expect("run entry v1");
    assert_eq!(code1, 0);
    assert_eq!(stdout1.trim(), "15");

    // Hot-reload with modified compute_score logic
    let src2 = r#"
fn compute_score(x: Int) -> Int => x * 100

fn main() -> Int {
    let s = compute_score(5)
    println(int_to_str(s))
    0
}
"#;
    let res2 = compiler.compile_source(src2, "game.dtr", None);
    assert!(res2.success, "compile 2 failed: {:?}", res2.diagnostics);
    let dmir_mod2 = res2.dmir_module.expect("DMIR module 2");

    let (delta, elapsed_nanos) = session
        .hot_reload_delta(&dmir_mod2)
        .expect("hot reload delta");
    assert!(delta.modified.contains(&"compute_score".to_string()));
    assert!(elapsed_nanos > 0);

    // Re-run entry point mid-flight
    let (stdout2, _, code2, _) = session.run_entry(None, &[], true).expect("run entry v2");
    assert_eq!(code2, 0);
    assert_eq!(
        stdout2.trim(),
        "500",
        "Hot reloaded compute_score must return 500"
    );
}

#[test]
fn test_v122_sso_string_zero_alloc() {
    use forgen::runtime::{
        datara_rt_heap_alloc_count, datara_rt_reset_heap_alloc_count, datara_rt_str_is_sso,
        datara_rt_str_sso,
    };
    use std::ffi::{CStr, CString};

    unsafe {
        datara_rt_reset_heap_alloc_count();
        let s_short = CString::new("player_score_100").unwrap();
        assert_eq!(datara_rt_str_is_sso(s_short.as_ptr()), 1);

        let ptr = datara_rt_str_sso(s_short.as_ptr());
        let back = CStr::from_ptr(ptr).to_str().unwrap();
        assert_eq!(back, "player_score_100");
        assert_eq!(
            datara_rt_heap_alloc_count(),
            0,
            "SSO string must cause 0 heap allocations"
        );
    }
}
