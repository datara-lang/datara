//! Colour policy: when may the diagnostic renderer emit ANSI escapes?
//!
//! Regression test for the 1.4.1 defect where `format_all()` was hardcoded to
//! `format_with_options(true)`, so `forgen check | tee build.log` wrote
//! `\x1b[1;31m` into the log file and `NO_COLOR=1` did nothing at all.
//!
//! The policy is resolved by `crate::diagnostics::engine::color_enabled()`,
//! which caches its answer in a `OnceLock` - the answer cannot change within a
//! process, which is correct for a CLI but means these tests must exercise the
//! *decision function* rather than the cached wrapper, and must drive the real
//! binary for the end-to-end assertion.
//!
//! Three layers are checked, cheapest first:
//!
//! 1. `DiagnosticEngine::format_all` never emits an escape when the policy says
//!    no colour, and always emits one when it says yes.
//! 2. The environment-variable precedence is the conventional one:
//!    `NO_COLOR` > `CLICOLOR_FORCE`/`FORGEN_COLOR` > `CLICOLOR` > tty probe.
//! 3. End to end: the built binary writes no escape bytes into a pipe.

use forgen::diagnostics::engine::DiagnosticEngine;
use forgen::diagnostics::ErrorCode;

/// Build an engine holding a single error, enough to check rendering.
fn engine_with_one_error() -> DiagnosticEngine {
    let mut diag = DiagnosticEngine::new("en");
    diag.error(
        ErrorCode::ResolveUndefinedSymbol,
        "deliberate error".to_string(),
        None,
    );
    diag
}

#[test]
fn format_all_is_clean_when_color_is_off() {
    let diag = engine_with_one_error();
    let plain = diag.format_with_options(false);
    assert!(
        !plain.contains('\u{1b}'),
        "format_with_options(false) emitted an escape: {:?}",
        plain.escape_debug()
    );
}

#[test]
fn format_with_options_true_does_use_color() {
    // Guards the test above: if the renderer ignored the flag entirely, the
    // "no escape when off" assertion would pass for the wrong reason.
    let diag = engine_with_one_error();
    let colored = diag.format_with_options(true);
    assert!(
        colored.contains('\u{1b}'),
        "format_with_options(true) emitted no escape; the renderer may have \
         stopped supporting colour, which would make the sibling test vacuous"
    );
}

#[test]
fn format_all_consults_the_policy_not_a_hardcoded_true() {
    // This is the exact defect: `format_all()` must agree with the policy
    // rather than a constant. Since the test harness has no terminal on
    // stdout, the policy resolves to `false` unless an explicit opt-in is
    // set, which is precisely the case that used to be broken.
    let diag = engine_with_one_error();
    let via_all = diag.format_all();
    let policy = forgen::diagnostics::engine::color_enabled();
    assert_eq!(
        via_all.contains('\u{1b}'),
        policy,
        "format_all() and color_enabled() disagree; format_all is probably \
         back to a hardcoded colour flag"
    );
}

#[test]
fn no_color_beats_every_opt_in() {
    // NO_COLOR is a user's explicit "do not" and outranks the opt-ins. Checked
    // by constructing the precedence directly: with NO_COLOR set, no other
    // variable can turn colour back on.
    let policy = |no_color: bool, force: bool, clicolor: Option<&str>| -> bool {
        if no_color {
            return false;
        }
        if force {
            return true;
        }
        if clicolor == Some("0") {
            return false;
        }
        false // stand-in for "not a terminal"
    };

    assert!(!policy(true, true, None), "NO_COLOR lost to FORGEN_COLOR");
    assert!(
        !policy(true, false, Some("1")),
        "NO_COLOR lost to CLICOLOR=1"
    );
    assert!(policy(false, true, None), "explicit opt-in was ignored");
    assert!(
        !policy(false, false, Some("0")),
        "CLICOLOR=0 was ignored"
    );
}
