use forgen::driver::ForgenCompiler;

#[test]
fn test_bridge_valid_py_math() {
    let code = r#"
use py::math

bridge py::math {
    fn sqrt(x: Float) -> Float
    fn pow(base: Float, exp: Float) -> Float
}

fn main() {
    let s = math.sqrt(16.0)
    let p = math.pow(2.0, 3.0)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_valid_py_bridge.dtr", None);
    assert!(
        res.success,
        "Valid py bridge must compile successfully. Error: {:?}\nDiag: {}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_bridge_arg_type_mismatch_e_bridge_001() {
    let code = r#"
use py::math

bridge py::math {
    fn sqrt(x: Float) -> Float
}

fn main() {
    let s = math.sqrt("not_a_float")
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_bridge_type_mismatch.dtr", None);
    assert!(
        !res.success,
        "Bridge call with wrong arg type must fail compilation"
    );
    assert!(
        res.diagnostics.contains("E-BRIDGE-001"),
        "Diagnostics must contain E-BRIDGE-001: {}",
        res.diagnostics
    );
}

#[test]
fn test_bridge_unsupported_param_type_e_bridge_002() {
    let code = r#"
struct ComplexCustomObj {
    val: Int
}

use py::math

bridge py::math {
    fn process(obj: ComplexCustomObj) -> Int
}

fn main() {}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_bridge_unsupported_param.dtr", None);
    assert!(
        !res.success,
        "Bridge with unwhitelisted type must fail with E-BRIDGE-002"
    );
    assert!(
        res.diagnostics.contains("E-BRIDGE-002"),
        "Diagnostics must contain E-BRIDGE-002: {}",
        res.diagnostics
    );
}

#[test]
fn test_bridge_unknown_language_e_bridge_003() {
    let code = r#"
use ruby::math

bridge ruby::math {
    fn calculate(x: Float) -> Float
}

fn main() {}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_bridge_unknown_lang.dtr", None);
    assert!(
        !res.success,
        "Bridge with unknown language must fail with E-BRIDGE-003"
    );
    assert!(
        res.diagnostics.contains("E-BRIDGE-003"),
        "Diagnostics must contain E-BRIDGE-003: {}",
        res.diagnostics
    );
}

#[test]
fn test_bridge_valid_c_declaration() {
    let code = r#"
use c::mathlib

bridge c::mathlib {
    fn abs(n: Int) -> Int
}

fn main() {
    unsafe(justification: "test C extern bridge") {
        let x = mathlib.abs(-10)
    }
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_valid_c_bridge.dtr", None);
    assert!(
        res.success,
        "Valid C bridge declaration must compile successfully. Error: {:?}\nDiag: {}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_bridge_valid_rust_declaration() {
    let code = r#"
use rs::fastmath

bridge rs::fastmath {
    fn fast_sin(x: Float) -> Float
}

fn main() {
    unsafe(justification: "test Rust extern bridge") {
        let v = fastmath.fast_sin(1.57)
    }
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.check_source(code, "test_valid_rs_bridge.dtr");
    assert!(
        res.success,
        "Valid Rust bridge declaration must compile successfully. Error: {:?}\nDiag: {}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_bridge_valid_js_declaration() {
    let code = r#"
use js::stringutil

bridge js::stringutil {
    fn trim(s: Str) -> Str
}

fn main() {
    let t = stringutil.trim(" hello ")
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_valid_js_bridge.dtr", None);
    assert!(
        res.success,
        "Valid JS bridge declaration must compile successfully. Error: {:?}\nDiag: {}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_bridge_supported_list_and_map_types() {
    let code = r#"
use py::stats

bridge py::stats {
    fn mean(vals: List<Float>) -> Float
    fn counts(m: Map<Str, Int>) -> Int
}

fn main() {}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_bridge_list_map.dtr", None);
    assert!(
        res.success,
        "Bridge with List<Float> and Map<Str, Int> must compile successfully. Error: {:?}\nDiag: {}",
        res.error, res.diagnostics
    );
}

#[test]
fn test_bridge_multiple_functions() {
    let code = r#"
use py::geometry

bridge py::geometry {
    fn area(w: Float, h: Float) -> Float
    fn perimeter(w: Float, h: Float) -> Float
    fn is_square(w: Float, h: Float) -> Bool
}

fn main() {
    let a = geometry.area(10.0, 20.0)
    let p = geometry.perimeter(10.0, 20.0)
    let s = geometry.is_square(10.0, 20.0)
}
"#;
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source(code, "test_bridge_multi_fn.dtr", None);
    assert!(
        res.success,
        "Multi-function bridge must compile successfully. Error: {:?}\nDiag: {}",
        res.error, res.diagnostics
    );
}
