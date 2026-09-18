//! Integration test suite for Entity-Guided Evidence Optimization (Datara v1.5.0).
//! Verifies the 5 core language constructs as contracts of proof:
//! 1. struct: Concrete data layout, rejects interior methods (E-STRUCT-001).
//! 2. component: POD layout (Copy + memcpy safe), rejects methods (E-COMP-001) and non-POD fields (E-COMP-002). Auto SoA qualified.
//! 3. behavior: State-free (rejects fields E-BEH-001), pure by default (E-BEH-002 on unannotated side effects), 2x inlining multiplier.
//! 4. role: Behavioral contract with typed capability bounds (E-ROLE-001).
//! 5. trait/impl: Coherence (single impl E-IMPL-001, super-traits E-IMPL-002), single-impl devirtualization.
//! Complete eradication of `class` keyword (E0100).

use forgen::codegen::TargetInfo;
use forgen::codegen::cranelift::backend::RealCraneliftBackend;
use forgen::codegen::cranelift::backend::opts::JitCompilationTier;
use forgen::codegen::cranelift::jit::JitSession;
use forgen::driver::ForgenCompiler;

#[test]
fn test_entity_class_forbidden_e0100() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
class OldModel {
    id: Int
}
"#;
    let res = compiler.check_source(src, "test_class.dtr");
    assert!(!res.success, "class keyword must be forbidden");
    assert!(
        res.diagnostics.contains("E0100"),
        "expected E0100, got: {}",
        res.diagnostics
    );
    assert!(
        res.diagnostics
            .contains("The 'class' keyword does not exist in Datara"),
        "expected explanation, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_struct_rejects_interior_methods_e_struct_001() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
struct Point {
    x: Int,
    y: Int,
    fn dist() -> Int {
        return 0
    }
}
"#;
    let res = compiler.check_source(src, "test_struct_methods.dtr");
    assert!(!res.success, "struct must reject interior methods");
    assert!(
        res.diagnostics.contains("E-STRUCT-001"),
        "expected E-STRUCT-001, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_component_rejects_interior_methods_e_comp_001() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
component Transform {
    x: Float,
    y: Float,
    fn move_by(dx: Float) {
    }
}
"#;
    let res = compiler.check_source(src, "test_comp_methods.dtr");
    assert!(!res.success, "component must reject interior methods");
    assert!(
        res.diagnostics.contains("E-COMP-001"),
        "expected E-COMP-001, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_component_rejects_non_pod_fields_e_comp_002() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
component Sprite {
    name: Str,
    width: Int
}
"#;
    let res = compiler.check_source(src, "test_comp_non_pod.dtr");
    assert!(!res.success, "component must reject non-POD fields");
    assert!(
        res.diagnostics.contains("E-COMP-002"),
        "expected E-COMP-002, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_behavior_rejects_fields_e_beh_001() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
behavior Point {
    offset: Int
    fn get_offset() -> Int {
        return this.offset
    }
}
"#;
    let res = compiler.check_source(src, "test_beh_fields.dtr");
    assert!(!res.success, "behavior must reject fields");
    assert!(
        res.diagnostics.contains("E-BEH-001"),
        "expected E-BEH-001, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_behavior_rejects_unannotated_impure_methods_e_beh_002() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
struct Logger {
    id: Int
}

behavior Logger {
    fn log_msg(msg: Str) {
        println(msg)
    }
}
"#;
    let res = compiler.check_source(src, "test_beh_impure.dtr");
    assert!(
        !res.success,
        "behavior must reject unannotated impure methods"
    );
    assert!(
        res.diagnostics.contains("E-BEH-002"),
        "expected E-BEH-002, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_behavior_permits_annotated_impure_methods() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
struct Logger {
    id: Int
}

behavior Logger {
    fn log_msg(msg: Str) / IO {
        println(msg)
    }
}

fn main() -> Int {
    return 0
}
"#;
    let res = compiler.check_source(src, "test_beh_annotated_io.dtr");
    assert!(
        res.success,
        "behavior with / IO must pass: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_impl_coherence_duplicate_rejected_e_impl_001() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
trait Greet {
    fn greet() -> Str
}

struct Person {
    name: Str
}

impl Greet for Person {
    fn greet() -> Str {
        return "Hello"
    }
}

impl Greet for Person {
    fn greet() -> Str {
        return "Duplicate"
    }
}
"#;
    let res = compiler.check_source(src, "test_impl_duplicate.dtr");
    assert!(!res.success, "duplicate impl must be rejected");
    assert!(
        res.diagnostics.contains("E-IMPL-001"),
        "expected E-IMPL-001, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_impl_missing_super_trait_rejected_e_impl_002() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
trait Base {
    fn base_val() -> Int
}

trait Derived: Base {
    fn derived_val() -> Int
}

struct Entity {
    id: Int
}

impl Derived for Entity {
    fn derived_val() -> Int {
        return 1
    }
}
"#;
    let res = compiler.check_source(src, "test_impl_supertrait.dtr");
    assert!(!res.success, "missing super-trait impl must be rejected");
    assert!(
        res.diagnostics.contains("E-IMPL-002"),
        "expected E-IMPL-002, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_role_capability_enforcement_e_role_001() {
    let compiler = ForgenCompiler::new("check");
    let src = r#"
role NetWorker {
    fn transmit() / IO needs Network
}

struct Node {
    using NetWorker
}

behavior Node {
    fn transmit() / IO {
        println("transmitted")
    }
}

fn main() -> Int {
    return 0
}
"#;
    let res = compiler.check_source(src, "test_role_cap.dtr");
    assert!(!res.success, "missing role capability must be rejected");
    assert!(
        res.diagnostics.contains("E-ROLE-001"),
        "expected E-ROLE-001, got: {}",
        res.diagnostics
    );
}

#[test]
fn test_entity_struct_and_behavior_jit_execution() {
    let backend = RealCraneliftBackend::new(TargetInfo::host());
    let mut session =
        JitSession::new(backend, JitCompilationTier::MaxSpeed).expect("Create JIT session");

    let src = r#"
struct Vector2D {
    x: Int
    y: Int
}

behavior Vector2D {
    fn manhattan_dist() -> Int {
        return this.x + this.y
    }
}

fn main() -> Int {
    let v = Vector2D { x: 20, y: 22 }
    let d = v.manhattan_dist()
    println(int_to_str(d))
    return 0
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(src, "test_vec2d_jit.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");

    session
        .load_module(&dmir_mod)
        .expect("load module into JIT");
    let (stdout, _, code, _) = session.run_entry(None, &[], true).expect("run entry");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn test_entity_component_auto_soa_registration() {
    let compiler = ForgenCompiler::new("release");
    let src = r#"
component Particle {
    x: Float
    y: Float
    vx: Float
    vy: Float
}

fn main() -> Int {
    return 0
}
"#;
    let res = compiler.compile_source(src, "test_comp_soa.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.diagnostics);
    let dmir_mod = res.dmir_module.expect("DMIR module");
    assert!(
        dmir_mod.component_classes.contains("Particle"),
        "Particle component must be registered in component_classes for Auto SoA"
    );
}
