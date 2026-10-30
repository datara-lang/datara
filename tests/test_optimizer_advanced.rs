use forgen::driver::ForgenCompiler;

#[test]
fn test_advanced_cse_optimization() {
    let source = r#"
fn compute(a: Int, b: Int) -> Int {
    mut x = 0
    x = a + b
    mut y = 0
    y = a + b
    return x + y
}

fn main() {
    mut res = 0

    res = compute(10, 20)
    out res
}
"#;

    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "cse_test.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);

    let trace = res
        .optimization_report
        .as_ref()
        .map(|r| &r.decision_trace)
        .unwrap();
    let cse_records: Vec<_> = trace.iter().filter(|d| d.pass == "CSE").collect();
    assert!(
        !cse_records.is_empty(),
        "CSE optimization pass must record decision traces"
    );

    let exe = res.exe_path.unwrap();
    let (stdout, _, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "60");
}

#[test]
fn test_advanced_licm_loop_optimization() {
    let source = r#"
fn loop_with_invariant(n: Int) -> Int {
    mut sum = 0
    mut i = 0
    while i < n {
        mut inv = 0
        inv = n * 2
        sum = sum + inv
        i = i + 1
    }
    return sum
}

fn main() {
    mut res = 0

    res = loop_with_invariant(10)
    out res
}
"#;

    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "licm_test.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);

    let trace = res
        .optimization_report
        .as_ref()
        .map(|r| &r.decision_trace)
        .unwrap();
    let licm_records: Vec<_> = trace.iter().filter(|d| d.pass == "LICM").collect();
    assert!(
        !licm_records.is_empty(),
        "LICM optimization pass must record decision traces"
    );

    let exe = res.exe_path.unwrap();
    let (stdout, _, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(code, 0);
    // The invariant must be a runtime value (here the parameter n), not a
    // compile-time constant: since v1.3.3 the LoopFold pass consumes loops
    // whose invariant is a constant expression (`inv = 100 * 2`) before LICM
    // ever runs, so a constant invariant no longer leaves an LICM record.
    // sum += 2*n per iteration, 10 iterations with n = 10 gives 200.
    assert_eq!(stdout.trim(), "200");
}

#[test]
fn test_global_cse_across_blocks() {
    // The second `a * b` is in a block dominated by entry, where the first
    // `a * b` lives. Local (per-block) CSE cannot see this pair — only
    // dominance-based global CSE can. The record must say Applied, and the
    // result must be exact.
    let source = r#"
fn pick(a: Int, b: Int) -> Int {
    mut t = 0
    t = a * b
    if a > 0 {
        return t
    }
    return t + a * b
}

fn main() {
    out pick(3, 4)
    out pick(-3, 4)
}
"#;

    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "global_cse_test.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);

    let trace = res
        .optimization_report
        .as_ref()
        .map(|r| &r.decision_trace)
        .unwrap();
    let cse_records: Vec<_> = trace
        .iter()
        .filter(|d| d.pass == "CSE" && d.decision == "Applied")
        .collect();
    assert!(
        !cse_records.is_empty(),
        "global CSE must fire on the cross-block duplicate a*b"
    );

    let exe = res.exe_path.unwrap();
    let (stdout, _, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(code, 0);
    // pick(3, 4): a > 0 -> return t = 12. pick(-3, 4): t + a*b = -12 + -12 = -24.
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0].trim(), "12");
    assert_eq!(lines[1].trim(), "-24");
}

// Regression (PLAN_V151 N1 bug registry): DCE's collect_used_values had no
// Inst::Cast arm, so the operand of a surviving cast was treated as unused
// and its definition was deleted — E0901 "use of undefined value" after the
// intraproc pass. Int->Float casts escaped because const-fold folded them
// away; Bool->Int casts survive folding (bool constants live in a separate
// map), which is why a Bool variable cast is the minimal repro.
#[test]
fn test_dce_bool_cast_operand_survives() {
    let source = r#"
fn main() {
    mut b = true
    out b as Int
}
"#;

    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "dce_cast_test.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);
    let exe = res.exe_path.unwrap();
    let (stdout, _, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "1");
}

// Regression (PLAN_V151 N1 bug registry): inline's substitute_operands fixed
// every inst's dest but skipped Inst::Cast's value operand, leaving a stale
// pre-inline value id — E0901 "use of undefined value" after the inline pass
// (caught by the resultx package consumer: a Bool helper result cast with
// `as Int` at the call site).
#[test]
fn test_inline_cast_operand_substituted() {
    let source = r#"
fn is_ready(n: Int) -> Bool {
    return n > 0
}

fn main() {
    mut r = is_ready(5)
    out r as Int
    out int_to_str(r as Int + 1)
}
"#;

    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "inline_cast_test.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);
    let exe = res.exe_path.unwrap();
    let (stdout, _, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0].trim(), "1");
    assert_eq!(lines[1].trim(), "2");
}

// Regression (PLAN_V151 N1 bug registry): the LLVM backend mapped Int `abs`
// to the DOUBLE runtime prototype `datara_rt_math_abs(double)` while
// emitting `call i64 @datara_rt_math_abs(i64)` — a declare/call type
// mismatch that opt/llc miscompiles at -mcpu=native (a package-consumer
// gcd loop printed 4 instead of 12; bare llc -O0 printed 36). The call now
// routes to `datara_rt_math_abs_int` when the argument carrier is i64.
#[test]
fn test_llvm_int_abs_loop_parity() {
    // The pre-fix repro used a user-defined `fn abs(Int) -> Int` called
    // from a loop — exactly the package shape (mathx consumer) whose plain
    // "abs" name was hijacked to the double builtin in the LLVM name map.
    let source = r#"
fn abs(v: Int) -> Int {
    if v < 0 {
        return 0 - v
    }
    return v
}

fn main() {
    mut x = 36
    mut y = abs(-24)
    while y != 0 {
        val t = y
        y = x % y
        x = t
    }
    out int_to_str(x)
    out int_to_str(abs(-36))
}
"#;

    let compiler = ForgenCompiler::new("domain");
    let res = compiler.compile_source(source, "llvm_int_abs.dtr", None);
    assert!(res.success, "Compilation failed: {:?}", res.error);
    let exe = res.exe_path.unwrap();
    let (stdout, _, code, _) = compiler.codegen.run_executable(&exe, &[]).unwrap();
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0].trim(), "12", "gcd loop miscompiled: {stdout:?}");
    assert_eq!(lines[1].trim(), "36", "Int abs miscompiled: {stdout:?}");
}
