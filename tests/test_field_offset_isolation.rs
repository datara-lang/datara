//! Field-offset resolution isolation tests (E0944).
//!
//! Regression tests for the silent-wrong-data class of bugs where a
//! bare-name offset fallback table (`field_default_offsets`, first-wins
//! across ALL classes) supplied the first class's offset for a second
//! class merely because both declare a field with the same name (e.g.
//! `scope_id`). The table is gone; an unresolvable receiver class or an
//! unknown field is now a compile error, never a guessed offset.

use forgen::codegen::cranelift::RealCraneliftBackend;
use forgen::codegen::llvm::LlvmEmitter;
use forgen::codegen::target::TargetInfo;
use forgen::dmir::{Inst, ValueId};
use forgen::driver::ForgenCompiler;
use forgen::resolver::Resolver;
use forgen::types::TypeChecker;

/// Two structs share two field NAMES (`scope_id`, and via mutation `id`)
/// with different layouts. SA declares scope_id at slot 2, SB at slot 1.
/// Mutating SA must not affect what SB reads.
const ISOLATION_SOURCE: &str = r#"
struct SA {
    id: Int
    kind: Str
    scope_id: Int
    priority: Int
}

struct SB {
    name: Str
    scope_id: Int
    tag: Int
}

fn make_sa() -> SA {
    let s = SA { id: 111, kind: "alpha", scope_id: 777, priority: 9 }
    return s
}

fn make_sb() -> SB {
    let b = SB { name: "beta", scope_id: 42, tag: 5 }
    return b
}

fn main() -> Int {
    let a = make_sa()
    let b = make_sb()

    // Poison SA's layout slots, especially its scope_id.
    a.id = 999
    a.scope_id = 12345
    a.priority = 8

    // SB must still read its OWN scope_id (42), not SA's offset.
    print(b.scope_id)
    print(b.tag)
    print(b.name)

    if b.scope_id == 42 && b.tag == 5 && b.name == "beta" {
        return 0
    }
    return 1
}
"#;

#[test]
fn test_sb_scope_id_not_poisoned_by_sa() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(ISOLATION_SOURCE, "field_isolation.dtr", None);
    assert!(
        res.success,
        "Compilation should succeed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let exe = res.exe_path.unwrap();
    let (stdout, stderr, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(
        code, 0,
        "SB.scope_id must read its own slot; stdout={} stderr={}",
        stdout, stderr
    );
    // b.scope_id == 42 (SA's poisoned 12345 must not appear)
    assert!(
        stdout.contains("42"),
        "SB.scope_id must be 42, got: {}",
        stdout
    );
    assert!(stdout.contains("5"), "SB.tag must be 5, got: {}", stdout);
    assert!(
        stdout.contains("beta"),
        "SB.name must be beta, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("12345"),
        "SA's poisoned scope_id must not leak into SB reads: {}",
        stdout
    );
}

/// Same isolation property exercised through the three receiver shapes
/// that previously depended on fragile resolution: the `this` receiver
/// inside a behavior method, a chained call-result receiver, and a
/// parameter receiver.
const MULTI_SHAPE_SOURCE: &str = r#"
struct SA {
    id: Int
    kind: Str
    scope_id: Int
    priority: Int
}

struct SB {
    name: Str
    scope_id: Int
    tag: Int
}

behavior SB {
    read_scope() -> Int {
        return this.scope_id
    }
}

fn take_sb(b: SB) -> Int {
    return b.scope_id
}

fn make_sa() -> SA {
    let s = SA { id: 111, kind: "alpha", scope_id: 777, priority: 9 }
    return s
}

fn make_sb() -> SB {
    let b = SB { name: "beta", scope_id: 42, tag: 5 }
    return b
}

fn main() -> Int {
    let a = make_sa()
    a.scope_id = 12345

    let b = make_sb()
    if b.read_scope() != 42 { return 1 }
    if make_sb().scope_id != 42 { return 2 }
    if take_sb(b) != 42 { return 3 }
    return 0
}
"#;

#[test]
fn test_shared_field_names_isolated_across_receiver_shapes() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(MULTI_SHAPE_SOURCE, "field_probe.dtr", None);
    assert!(
        res.success,
        "Compilation should succeed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let exe = res.exe_path.unwrap();
    let (_stdout, stderr, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(
        code, 0,
        "every receiver shape must read its own scope_id; stderr={}",
        stderr
    );
}

/// Generic instantiation: the monomorphized class name must resolve
/// through the template layout that class_field_offsets actually
/// contains. Box<Int> declares value at slot 0; a bare-name fallback
/// would have been reachable only when resolution failed, so this
/// guards the resolution chain itself.
const GENERIC_SOURCE: &str = r#"
struct Box<T> {
    value: T
}

fn make_box_int() -> Box<Int> {
    let b = Box { value: 41 }
    return b
}

fn main() -> Int {
    let b = make_box_int()
    print(b.value)
    if b.value == 41 {
        return 0
    }
    return 1
}
"#;

#[test]
fn test_generic_instantiation_field_access_resolves_template_layout() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(GENERIC_SOURCE, "generic_field.dtr", None);
    assert!(
        res.success,
        "Compilation should succeed: {:?}\n{}",
        res.error, res.diagnostics
    );

    let exe = res.exe_path.unwrap();
    let (stdout, stderr, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(
        code, 0,
        "generic field read must resolve; stderr={}",
        stderr
    );
    assert!(
        stdout.contains("41"),
        "Box<Int>.value must be 41, got: {}",
        stdout
    );
}

/// LLVM path: the same isolation program must emit without the E0944
/// error now that value class tracking covers call results and params.
/// (IR emission only; running the LLVM pipeline needs clang.)
#[test]
fn test_llvm_emission_resolves_shared_field_names() {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(ISOLATION_SOURCE, "field_isolation_llvm.dtr", None);
    let dmir = res.dmir_module.expect("DMIR module");
    let program = res.program.expect("AST program");

    let resolver = Resolver::new();
    let types = TypeChecker::new(&resolver);
    let target = TargetInfo::native();
    let emitter = LlvmEmitter::new(&target);
    let ir = emitter
        .emit_module(&dmir, &program, &types)
        .expect("LLVM IR emission must resolve all field offsets");
    assert!(ir.contains("define"), "emitted IR must contain functions");
    assert!(
        !ir.contains("LLVM EMISSION ERROR"),
        "emitted IR must not contain an error marker"
    );
}

/// Directly tests the fail-loud contract: a GetField whose receiver has
/// NO recorded class must be a compile error (E0944), not a silently
/// guessed offset.
///
/// Constructed by injecting the instruction into a valid DMIR module
/// because no supported source syntax currently lowers to an
/// unregistered receiver (call results, params, StructInit, LoadVar all
/// record their class); previously the deleted bare-name fallback table
/// hid such cases silently instead.
fn inject_classless_getfield(dmir: &mut forgen::dmir::Module) {
    let target_fn = dmir
        .functions
        .values_mut()
        .find(|f| f.name == "make_sa")
        .expect("make_sa function");
    let mut injected = false;
    for b in target_fn.blocks.iter_mut() {
        let anchor = b
            .instructions
            .iter()
            .position(|inst| matches!(inst, Inst::ConstInt { .. }));
        let Some(anchor) = anchor else { continue };
        let object = match &b.instructions[anchor] {
            Inst::ConstInt { dest, .. } => *dest,
            _ => unreachable!("matched ConstInt"),
        };
        b.instructions.insert(
            anchor + 1,
            Inst::GetField {
                dest: ValueId(object.0 + 10_000),
                object,
                field: "scope_id".to_string(),
                ty: "Int".to_string(),
            },
        );
        injected = true;
        break;
    }
    assert!(injected, "test setup: ConstInt anchor must exist");
}

fn base_module(source: &str, name: &str) -> forgen::dmir::Module {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(source, name, None);
    assert!(res.success, "base program must compile: {:?}", res.error);
    res.dmir_module.expect("DMIR module")
}

#[test]
fn test_cranelift_unresolvable_receiver_is_compile_error() {
    let mut dmir = base_module(ISOLATION_SOURCE, "field_inject_clif.dtr");
    inject_classless_getfield(&mut dmir);

    let backend = RealCraneliftBackend::new(TargetInfo::native());
    let err = backend
        .compile_to_object_bytes(&dmir)
        .expect_err("unresolvable receiver must fail compilation");
    assert!(
        err.contains("E0944"),
        "error must carry the E0944 code, got: {}",
        err
    );
    assert!(
        err.contains("scope_id") && err.contains("make_sa"),
        "error must name the field and function, got: {}",
        err
    );
}

#[test]
fn test_llvm_unresolvable_receiver_is_compile_error() {
    let mut dmir = base_module(ISOLATION_SOURCE, "field_inject_llvm.dtr");
    inject_classless_getfield(&mut dmir);

    let program = {
        let compiler = ForgenCompiler::new("release");
        let res = compiler.compile_source(ISOLATION_SOURCE, "field_inject_llvm.dtr", None);
        res.program.expect("AST program")
    };
    let resolver = Resolver::new();
    let types = TypeChecker::new(&resolver);
    let target = TargetInfo::native();
    let emitter = LlvmEmitter::new(&target);
    let err = emitter
        .emit_module(&dmir, &program, &types)
        .expect_err("unresolvable receiver must fail LLVM emission");
    assert!(
        err.contains("E0944"),
        "error must carry the E0944 code, got: {}",
        err
    );
}

/// A field name that does not exist in the receiver's own layout must
/// also fail (the old fallback would have supplied another class's
/// offset for it).
#[test]
fn test_cranelift_unknown_field_in_known_class_is_compile_error() {
    let mut dmir = base_module(MULTI_SHAPE_SOURCE, "field_unknown_clif.dtr");
    // The optimizer inlines take_sb into main, so rewrite the SB-typed
    // scope_id field accesses (take_sb's inlined body and the chained
    // call result) to `priority`, which SB does NOT declare (only SA
    // does). The old bare-name fallback would have supplied SA's offset
    // silently; the fixed resolver must reject this at compile time.
    let mut injected = 0;
    for f in dmir.functions.values_mut() {
        if f.name != "main" {
            continue;
        }
        for b in f.blocks.iter_mut() {
            for inst in b.instructions.iter_mut() {
                if let Inst::GetField { field, dest, .. } = inst
                    && field == "scope_id"
                {
                    *field = "priority".to_string();
                    // Keep the dest/value shape; only the field name
                    // changes, so the surrounding code stays valid.
                    let _ = dest;
                    injected += 1;
                }
            }
        }
    }
    assert!(injected > 0, "test setup: main must access a field");

    let backend = RealCraneliftBackend::new(TargetInfo::native());
    let err = backend
        .compile_to_object_bytes(&dmir)
        .expect_err("SB has no priority field; must fail compilation");
    assert!(
        err.contains("E0944"),
        "error must carry the E0944 code, got: {}",
        err
    );
    assert!(
        err.contains("priority") && err.contains("SB"),
        "error must name the field and class, got: {}",
        err
    );
}
