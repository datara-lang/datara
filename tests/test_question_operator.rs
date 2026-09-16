//! v1.4.1 Question operator (?) comprehensive test suite.
//!
//! Verifies:
//! - Success path unwrapping for Outcome<T> and Maybe<T>.
//! - Early return on error/none path for Outcome<T> and Maybe<T>.
//! - Cross-payload Outcome propagation (Outcome<Str> propagating out of Outcome<Int>).
//! - Question operator inside if branches and while loops.
//! - Checked I/O operations (file_read_checked, env_get_checked) with ?.
//! - Diagnostic gate E-TYPE-010 when ? is used in non-Result/Option returning functions.
//! - Both JIT and AOT native execution.

use forgen::diagnostics::ErrorCode;
use forgen::driver::ForgenCompiler;

static MSVC_ENV: std::sync::Once = std::sync::Once::new();
fn ensure_msvc_env() {
    MSVC_ENV.call_once(|| unsafe {
        if std::env::var_os("ProgramFiles(x86)").is_none() {
            std::env::set_var("ProgramFiles(x86)", r"C:\Program Files (x86)");
        }
        if std::env::var_os("ProgramFiles").is_none() {
            std::env::set_var("ProgramFiles", r"C:\Program Files");
        }
    });
}

fn run_datara(source: &str, name: &str) -> String {
    ensure_msvc_env();
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}",
        name, res.error
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, _stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");
    assert_eq!(code, 0, "{} exited with {}", name, code);

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    stdout.trim().replace("\r\n", "\n")
}

fn run_jit(source: &str, name: &str) -> String {
    let compiler = ForgenCompiler::new("release");
    let result = compiler.run_source(source, name, &[], true);
    let (stdout, stderr, code, _dur) = result.expect("JIT execution should succeed");
    assert_eq!(code, 0, "JIT exit code {} stderr: {}", code, stderr);
    stdout.trim().replace("\r\n", "\n")
}

#[test]
fn test_question_outcome_happy_path_jit() {
    let source = r#"
use stdlib.result.result.Outcome

fn compute(x: Int) -> Outcome<Int> {
    if x > 0 {
        return Outcome.ok(x * 10)
    }
    return Outcome<Int> { is_success: false, value: 0, error_msg: "negative or zero" }
}

fn add_computes(a: Int, b: Int) -> Outcome<Int> {
    let x = compute(a)?
    let y = compute(b)?
    return Outcome.ok(x + y)
}

fn main() {
    let r = add_computes(5, 3)
    println(fmt"ok:{r.is_ok()} val:{r.unwrap()}")
}
"#;
    let out = run_jit(source, "test_question_outcome_happy_path_jit");
    assert_eq!(out, "ok:true val:80");
}

#[test]
fn test_question_outcome_early_return_jit() {
    let source = r#"
use stdlib.result.result.Outcome

fn compute(x: Int) -> Outcome<Int> {
    if x > 0 {
        return Outcome.ok(x * 10)
    }
    return Outcome<Int> { is_success: false, value: 0, error_msg: "non-positive number" }
}

fn add_computes(a: Int, b: Int) -> Outcome<Int> {
    let x = compute(a)?
    let y = compute(b)?
    return Outcome.ok(x + y)
}

fn main() {
    let r = add_computes(5, -1)
    println(fmt"ok:{r.is_ok()} err:{r.err()}")
}
"#;
    let out = run_jit(source, "test_question_outcome_early_return_jit");
    assert_eq!(out, "ok:false err:non-positive number");
}

#[test]
fn test_question_outcome_happy_path_aot() {
    let source = r#"
use stdlib.result.result.Outcome

fn step1(n: Int) -> Outcome<Int> {
    return Outcome.ok(n + 1)
}

fn step2(n: Int) -> Outcome<Int> {
    return Outcome.ok(n * 2)
}

fn pipeline(n: Int) -> Outcome<Int> {
    let a = step1(n)?
    let b = step2(a)?
    return Outcome.ok(b)
}

fn main() {
    let r = pipeline(10)
    println(int_to_str(r.unwrap()))
}
"#;
    let out = run_datara(source, "test_question_outcome_happy_path_aot");
    assert_eq!(out, "22");
}

#[test]
fn test_question_outcome_early_return_aot() {
    let source = r#"
use stdlib.result.result.Outcome

fn step_fail() -> Outcome<Int> {
    return Outcome<Int> { is_success: false, value: 0, error_msg: "step failed" }
}

fn pipeline() -> Outcome<Int> {
    let _ = step_fail()?
    return Outcome.ok(999)
}

fn main() {
    let r = pipeline()
    println(fmt"ok:{r.is_ok()} msg:{r.err()}")
}
"#;
    let out = run_datara(source, "test_question_outcome_early_return_aot");
    assert_eq!(out, "ok:false msg:step failed");
}

#[test]
fn test_question_cross_payload_propagation_aot() {
    let source = r#"
use stdlib.result.result.Outcome

fn parse_word(code: Int) -> Outcome<Str> {
    if code == 1 {
        return Outcome.ok("success")
    }
    return Outcome.err("invalid word code")
}

fn get_word_len(code: Int) -> Outcome<Int> {
    let w = parse_word(code)?
    return Outcome.ok(w.len())
}

fn main() {
    let r1 = get_word_len(1)
    println(fmt"ok1:{r1.is_ok()} len:{r1.unwrap()}")
    let r2 = get_word_len(99)
    println(fmt"ok2:{r2.is_ok()} err:{r2.err()}")
}
"#;
    let out = run_datara(source, "test_question_cross_payload_propagation_aot");
    assert_eq!(out, "ok1:true len:7\nok2:false err:invalid word code");
}

#[test]
fn test_question_in_if_branch_and_loop() {
    let source = r#"
use stdlib.result.result.Outcome

fn check_val(x: Int) -> Outcome<Int> {
    if x < 10 {
        return Outcome.ok(x)
    }
    return Outcome<Int> { is_success: false, value: 0, error_msg: "value exceeds 10" }
}

fn sum_evens(limit: Int) -> Outcome<Int> {
    mut total = 0
    mut i = 0
    while i < limit {
        if i % 2 == 0 {
            let v = check_val(i)?
            total = total + v
        }
        i = i + 1
    }
    return Outcome.ok(total)
}

fn main() {
    let ok = sum_evens(6)
    println(fmt"sum:{ok.unwrap()}")
    let err = sum_evens(15)
    println(fmt"err:{err.err()}")
}
"#;
    let out = run_jit(source, "test_question_in_if_branch_and_loop");
    assert_eq!(out, "sum:6\nerr:value exceeds 10");
}

#[test]
fn test_question_with_checked_io() {
    let source = r#"
use stdlib.result.result.Outcome

fn check_missing_file() -> Outcome<Int> {
    unsafe(justification: "test read missing file") {
        let content = file_read_checked("C:/definitely_nonexistent_xyz_999.txt")?
        return Outcome.ok(content.len())
    }
}

fn check_missing_env() -> Outcome<Int> {
    unsafe(justification: "test missing env") {
        let val = env_get_checked("DEFINITELY_NONEXISTENT_ENV_KEY_XYZ_123")?
        return Outcome.ok(val.len())
    }
}

fn main() {
    let r_file = check_missing_file()
    println(fmt"file_err:{r_file.is_err()}")
    let r_env = check_missing_env()
    println(fmt"env_err:{r_env.is_err()}")
}
"#;
    let out = run_datara(source, "test_question_with_checked_io");
    assert_eq!(out, "file_err:true\nenv_err:true");
}

#[test]
fn test_question_maybe_happy_and_none() {
    let source = r#"
use stdlib.result.option.Maybe

fn lookup(key: Int) -> Maybe<Int> {
    if key == 1 {
        return Maybe<Int> { is_some: true, value: 100 }
    }
    return Maybe<Int> { is_some: false, value: 0 }
}

fn double_lookup(key: Int) -> Maybe<Int> {
    let v = lookup(key)?
    return Maybe<Int> { is_some: true, value: v * 2 }
}

fn main() {
    let some_res = double_lookup(1)
    println(fmt"some:{some_res.is_some} val:{some_res.value}")
    let none_res = double_lookup(2)
    println(fmt"some:{none_res.is_some}")
}
"#;
    let out = run_jit(source, "test_question_maybe_happy_and_none");
    assert_eq!(out, "some:true val:200\nsome:false");
}

#[test]
fn test_question_compile_error_e_type_010_on_unit() {
    let source = r#"
use stdlib.result.result.Outcome

fn get_val() -> Outcome<Str> {
    return Outcome.err("fail")
}

fn main() {
    let x = get_val()?
    println(int_to_str(x))
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "err_unit.dtr", None);
    assert!(!res.success, "Question operator in main (Unit) must fail");
    assert!(
        res.diagnostics
            .contains(ErrorCode::QuestionPropagation.as_str()),
        "Must emit E-TYPE-010, got:\n{}",
        res.diagnostics
    );
}

#[test]
fn test_question_compile_error_e_type_010_on_plain_type() {
    let source = r#"
use stdlib.result.result.Outcome

fn get_val() -> Outcome<Str> {
    return Outcome.err("fail")
}

fn helper() -> Int {
    let x = get_val()?
    return x
}

fn main() {
    let _ = helper()
}
"#;
    let compiler = ForgenCompiler::new("debug");
    let res = compiler.compile_source(source, "err_plain.dtr", None);
    assert!(
        !res.success,
        "Question operator in Int-returning function must fail"
    );
    assert!(
        res.diagnostics
            .contains(ErrorCode::QuestionPropagation.as_str()),
        "Must emit E-TYPE-010, got:\n{}",
        res.diagnostics
    );
}
