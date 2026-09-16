//! Control-flow parity pins: these tests do not add behaviour, they lock in
//! what the backends already emit so future refactors cannot silently change
//! it. Where a semantic is implementation-defined it is quoted at the emit
//! site and pinned with a comment.
//!
//! Pinned semantics:
//!   * `%` is emitted in src/codegen/cranelift/backend/inst_binop.rs as
//!     CHECKED `srem`:
//!     - divisor == 0       -> trapnz(INTEGER_DIVISION_BY_ZERO)
//!     - INT_MIN % -1       -> trapnz(INTEGER_OVERFLOW)
//!     - power-of-2 divisor -> sign-bias fast path (`sshr_imm(lv, 63)` /
//!       `ushr_imm(sign, 64-k)` / `iadd` / `sshr_imm(k)` / `ishl_imm(k)` /
//!       `isub`), truncation-correct for negative dividends
//!     - otherwise          -> `builder.ins().srem(lv, rv)`
//!       All paths therefore follow Rust truncation semantics: the result
//!       takes the sign of the DIVIDEND (-7 % 3 == -1, not 2).
//!   * `continue` in a `while` re-evaluates the whole condition (the loop
//!     header is the continue target), while `continue` in a `for`-range
//!     jumps to the increment block so the induction variable still advances
//!     (src/dmir/lowering/stmt.rs).
//!   * `if`/`else` is a STATEMENT (the value form is `decide`); assigning to a
//!     mutable declared before the `if` is the documented workaround, pinned
//!     in test_if_else_statement_assigns_to_mutable.

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

fn write_source(dir: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("failed to write test source");
    path
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("exe"));
    let _ = std::fs::remove_file(path.with_extension("pdb"));
    let _ = std::fs::remove_file(path.with_extension("obj"));
}

fn run_stdout(path: &std::path::Path) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_file(path, None);
    assert!(
        res.success,
        "Compilation should succeed:\n{}",
        res.diagnostics
    );
    let (stdout, stderr, code, _) = compiler
        .codegen
        .run_executable(&res.exe_path.unwrap(), &[])
        .unwrap();
    if code != 0 {
        eprintln!("stderr: {}", stderr);
    }
    (stdout, code)
}

fn run_lines(src: &str, name: &str) -> Vec<String> {
    let path = write_source(&std::env::temp_dir(), name, src);
    let (stdout, code) = run_stdout(&path);
    assert_eq!(code, 0);
    cleanup(&path);
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect()
}

#[test]
fn test_modulo_in_loop() {
    ensure_msvc_env();
    let lines = run_lines(
        r#"fn main() {
    mut acc = 0
    mut i = 0
    while i < 10 {
        if i % 3 == 0 {
            acc = acc + i
        }
        i = i + 1
    }
    println(int_to_str(acc))
    mut odd = 0
    for j in 0..7 {
        if j % 2 == 1 {
            odd = odd + 1
        }
    }
    println(int_to_str(odd))
}
"#,
        "cf_mod_loop.dtr",
    );
    assert_eq!(lines, vec!["18", "3"], "0+3+6+9 = 18; odd in 0..7 = 3");
}

#[test]
fn test_modulo_negative_operands_truncates_toward_zero() {
    ensure_msvc_env();
    // Divisor through a guarded variable: E0941 requires a statically proven
    // non-zero divisor, and a negative literal is not accepted as proven.
    let lines = run_lines(
        r#"fn main() {
    let neg_a = 0 - 7
    let pos_b = 3
    if pos_b != 0 {
        println(int_to_str(neg_a % pos_b))
    }
    let pos_a = 7
    let neg_b = 0 - 3
    if neg_b != 0 {
        println(int_to_str(pos_a % neg_b))
    }
}
"#,
        "cf_mod_negative.dtr",
    );
    // Truncation (srem) semantics: sign follows the dividend.
    assert_eq!(lines, vec!["-1", "1"], "-7 % 3 = -1 and 7 % -3 = 1");
}

#[test]
fn test_modulo_negative_dividend_power_of_two_path() {
    ensure_msvc_env();
    // 4 is a positive literal, so the checked path admits it directly; the
    // backend takes the power-of-2 sign-bias fast path here (inst_binop.rs).
    let lines = run_lines(
        r#"fn main() {
    let a = 0 - 7
    println(int_to_str(a % 4))
    let b = 0 - 8
    println(int_to_str(b % 4))
    println(int_to_str(7 % 4))
}
"#,
        "cf_mod_pow2.dtr",
    );
    assert_eq!(
        lines,
        vec!["-3", "0", "3"],
        "the power-of-2 fast path must stay truncation-correct for negatives"
    );
}

#[test]
fn test_break_out_of_while() {
    ensure_msvc_env();
    let lines = run_lines(
        r#"fn main() {
    mut i = 0
    mut found = 0
    while i < 100 {
        if i == 7 {
            found = i
            break
        }
        i = i + 1
    }
    println(int_to_str(found))
}
"#,
        "cf_break_while.dtr",
    );
    assert_eq!(lines, vec!["7"]);
}

#[test]
fn test_continue_in_while_and_for_range() {
    ensure_msvc_env();
    let lines = run_lines(
        r#"fn main() {
    // continue in while: the header (whole condition) is re-evaluated, so
    // the induction variable must be advanced before the continue.
    mut i = 0
    mut wsum = 0
    while i < 6 {
        i = i + 1
        if i == 3 {
            continue
        }
        wsum = wsum + i
    }
    println(int_to_str(wsum))

    // continue in for-range: jumps to the increment block, so the loop still
    // terminates and the induction variable advances.
    mut fsum = 0
    for j in 0..6 {
        if j == 3 {
            continue
        }
        fsum = fsum + j
    }
    println(int_to_str(fsum))
}
"#,
        "cf_continue.dtr",
    );
    // while: 1+2+4+5+6 = 18 (3 skipped)
    // for:   0+1+2+4+5 = 12 (3 skipped)
    assert_eq!(lines, vec!["18", "12"]);
}

#[test]
fn test_nested_break_continue_scoping() {
    ensure_msvc_env();
    let lines = run_lines(
        r#"fn main() {
    mut outer_hits = 0
    mut total = 0
    for a in 0..3 {
        mut b = 0
        while b < 5 {
            b = b + 1
            if b == 2 {
                // inner continue: only this while iteration is skipped
                continue
            }
            if b == 4 {
                // inner break: only the while loop exits, the for continues
                break
            }
            if a == 1 {
                outer_hits = outer_hits + 1
            }
            total = total + 1
        }
    }
    println(int_to_str(total))
    println(int_to_str(outer_hits))
}
"#,
        "cf_nested.dtr",
    );
    // Inner while per `a` iteration: b = 1, (skip 2), 3, (break at 4) -> two
    // accumulating iterations; 3 outer iterations -> 6 total.
    assert_eq!(lines, vec!["6", "2"], "inner break must not exit the for");
}

/// `if`/`else` is a statement in Datara; the expression form is `decide`.
/// The documented workaround -- declare a mutable, then assign in each branch
/// -- is pinned here so it keeps compiling and producing the same value.
#[test]
fn test_if_else_statement_assigns_to_mutable() {
    ensure_msvc_env();
    let lines = run_lines(
        r#"fn main() {
    mut label = 0
    let score = 82
    if score >= 75 {
        label = 1
    } else {
        label = 2
    }
    println(int_to_str(label))

    // `decide` is the expression form of the same conditional.
    let rank = decide {
        score >= 90 => 1,
        score >= 75 => 2,
        else => 3
    }
    println(int_to_str(rank))
}
"#,
        "cf_if_statement.dtr",
    );
    assert_eq!(lines, vec!["1", "2"]);
}
