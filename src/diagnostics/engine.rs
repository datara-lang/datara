use super::codes::ErrorCode;
use super::span::SourceSpan;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Whether diagnostic output may use ANSI colour.
///
/// The rule is the one every other compiler settled on, in this order:
///
///   1. `NO_COLOR` set at all (to anything, including the empty string, per
///      https://no-color.org) means **no colour**. This wins over everything,
///      because it is the explicit instruction a user gave.
///   2. `FORGEN_COLOR` / `CLICOLOR_FORCE` set to a non-zero value means **colour**
///      even when stdout is not a terminal. This is what makes `less -R`,
///      `fzf --ansi` and a CI log viewer usable.
///   3. otherwise, colour only if stdout is a real terminal.
///
/// Rule 3 is the one that was missing. `forgen check` used to emit escapes into
/// any pipe, so `forgen check | grep`, an editor reading its diagnostics, and a
/// log file all collected `\x1b[1;31m` noise - and `NO_COLOR` did nothing on
/// that path, because the only function that looked at it
/// (`src/lint/diagnostics.rs::is_terminal`) was not on it and never checked
/// whether the output was a terminal in any case.
///
/// Cached: this is consulted per diagnostic and the answer cannot change within
/// a process.
pub fn color_enabled() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        // 1. NO_COLOR wins.
        if std::env::var_os("NO_COLOR").is_some() {
            return false;
        }
        // 2. an explicit opt-in beats the terminal test.
        for key in ["FORGEN_COLOR", "CLICOLOR_FORCE"] {
            if let Ok(v) = std::env::var(key) {
                if !v.is_empty() && v != "0" {
                    return true;
                }
            }
        }
        // `CLICOLOR=0` is the long-standing BSD spelling of "not a color terminal".
        if let Ok(v) = std::env::var("CLICOLOR") {
            if v == "0" {
                return false;
            }
        }
        // 3. ask the OS.
        stdout_is_terminal()
    })
}

/// Is stdout a terminal? One `isatty` on unix, `GetConsoleMode` on windows.
///
/// Deliberately not a dependency. `atty` would be one more crate in a compiler
/// that has been careful about its tree, and this is twenty lines.
#[cfg(unix)]
fn stdout_is_terminal() -> bool {
    use std::os::unix::io::AsRawFd;
    unsafe extern "C" {
        fn isatty(fd: i32) -> i32;
    }
    // SAFETY: `isatty` takes an fd and does not retain it. Passing fd 1 is the
    // documented use; a bad fd returns 0 rather than trapping.
    unsafe { isatty(std::io::stdout().as_raw_fd()) == 1 }
}

#[cfg(windows)]
fn stdout_is_terminal() -> bool {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleMode(h: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    }
    let h = std::io::stdout().as_raw_handle();
    let mut mode: u32 = 0;
    // SAFETY: both pointers are valid for the duration of the call; the handle
    // comes from `stdout()` and the out-param is a local. When stdout is
    // redirected to a file or a pipe `GetConsoleMode` fails and returns 0, which
    // is exactly the answer we want.
    unsafe { GetConsoleMode(h, &mut mode) != 0 }
}

#[cfg(not(any(unix, windows)))]
fn stdout_is_terminal() -> bool {
    // Unknown platform: assume not a terminal. Emitting no colour is the safe
    // way to be wrong - it cannot corrupt anyone's log.
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub help: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

impl Diagnostic {
    pub fn error(code: ErrorCode, message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            code: code.as_str().to_string(),
            severity: "ERROR".to_string(),
            message: message.into(),
            span,
            help: None,
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticEngine {
    pub locale: String,
    pub diagnostics: Vec<Diagnostic>,
    pub source_map: HashMap<String, String>,
    pub error_count: usize,
}

impl DiagnosticEngine {
    pub fn new(locale: &str) -> Self {
        Self {
            locale: locale.to_string(),
            diagnostics: Vec::new(),
            source_map: HashMap::new(),
            error_count: 0,
        }
    }

    pub fn set_source(&mut self, file: &str, source: &str) {
        self.source_map.insert(file.to_string(), source.to_string());
    }

    pub fn error(&mut self, code: ErrorCode, message: String, span: Option<SourceSpan>) {
        self.error_with_help(code, message, span, None);
    }

    pub fn error_with_help(
        &mut self,
        code: ErrorCode,
        message: String,
        span: Option<SourceSpan>,
        help: Option<String>,
    ) {
        self.error_count += 1;
        self.diagnostics.push(Diagnostic {
            code: code.as_str().to_string(),
            severity: "ERROR".to_string(),
            message,
            span,
            help,
            reason: None,
        });
    }

    /// Push a pre-built diagnostic (already ERROR-severity) into the engine.
    pub fn error_raw(&mut self, d: crate::diagnostics::Diagnostic) {
        self.error_count += 1;
        self.diagnostics.push(d);
    }

    /// Push a pre-built diagnostic (already WARNING-severity) into the engine.
    pub fn warning_raw(&mut self, d: crate::diagnostics::Diagnostic) {
        self.diagnostics.push(d);
    }

    pub fn warning(&mut self, code: ErrorCode, message: String, span: Option<SourceSpan>) {
        self.diagnostics.push(Diagnostic {
            code: code.as_str().to_string(),
            severity: "WARNING".to_string(),
            message,
            span,
            help: None,
            reason: None,
        });
    }

    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    pub fn clear(&mut self) {
        self.diagnostics.clear();
        self.error_count = 0;
    }

    /// The full rendering: the severity line, the `-->` location, the source
    /// excerpt and the caret underline.
    ///
    /// **The name means "all of the rendering", not "all of the colour".** It
    /// necessarily did not when it was written - it meant `use_color: true` - and
    /// that reading is what put escapes into every pipe the compiler ever wrote
    /// to, because there are 26 call sites and none of them was in a position to
    /// know whether stdout was a terminal. Deciding here is what fixes all 26
    /// without touching one of them, and it is the only place that *can* decide:
    /// the diagnostic engine is the last thing to see the text before it goes
    /// out.
    ///
    /// For a caller that genuinely needs colour or genuinely needs none, there is
    /// `format_with_options`. Callers that need a machine-readable answer should
    /// use the JSON output instead of parsing this - the `--error-format=json`
    /// path exists for exactly that.
    pub fn format_all(&self) -> String {
        self.format_with_options(color_enabled())
    }

    /// The full rendering with no colour at all, regardless of the environment.
    pub fn format_plain(&self) -> String {
        self.format_with_options(false)
    }

    pub fn format_with_options(&self, use_color: bool) -> String {
        let (red_bold, yellow_bold, cyan_bold, blue_bold, dim, bold, reset) = if use_color {
            (
                "\x1b[1;31m",
                "\x1b[1;33m",
                "\x1b[1;36m",
                "\x1b[1;34m",
                "\x1b[2m",
                "\x1b[1m",
                "\x1b[0m",
            )
        } else {
            ("", "", "", "", "", "", "")
        };

        let mut out = String::new();
        for diag in &self.diagnostics {
            let (sev_color, sev_text) = if diag.severity == "ERROR" {
                (red_bold, "error")
            } else {
                (yellow_bold, "warning")
            };

            out.push_str(&format!(
                "{}{}[{}]{}: {}{}{}\n",
                sev_color, sev_text, diag.code, reset, bold, diag.message, reset
            ));

            if let Some(span) = &diag.span {
                let file_display = if span.file.is_empty() {
                    "<source>"
                } else {
                    &span.file
                };
                out.push_str(&format!(
                    "  {}-->{reset} {}:{}:{}\n",
                    blue_bold, file_display, span.start_line, span.start_col
                ));

                if let Some(src) = self.source_map.get(&span.file) {
                    let lines: Vec<&str> = src.lines().collect();
                    if span.start_line > 0 && span.start_line <= lines.len() {
                        let line_str = lines[span.start_line - 1];
                        let gutter = format!("{:>4} {}|{reset} ", span.start_line, blue_bold);
                        let empty_gutter = format!("     {}|{reset} ", blue_bold);

                        out.push_str(&empty_gutter);
                        out.push('\n');

                        out.push_str(&gutter);
                        out.push_str(line_str);
                        out.push('\n');

                        let indent = " ".repeat(span.start_col.saturating_sub(1));
                        let line_rem = line_str
                            .len()
                            .saturating_sub(span.start_col.saturating_sub(1))
                            .max(1);
                        let len = span
                            .end_col
                            .saturating_sub(span.start_col)
                            .max(1)
                            .min(line_rem);
                        let carets = "^".repeat(len);
                        out.push_str(&empty_gutter);
                        out.push_str(&indent);
                        out.push_str(&format!("{}{}{reset}\n", red_bold, carets));
                    }
                }
                out.push_str(&format!("     {}|{reset}\n", blue_bold));

                if let Some(reason) = &diag.reason {
                    out.push_str(&format!(
                        "     {}={reset} {}note:{reset} {}\n",
                        blue_bold, dim, reason
                    ));
                }
                if let Some(help) = &diag.help {
                    out.push_str(&format!(
                        "     {}={reset} {}help:{reset} {}\n",
                        blue_bold, cyan_bold, help
                    ));
                }
                out.push_str(&format!(
                    "     {}={reset} {}note: for more details, run 'forgen explain {}'{reset}\n\n",
                    blue_bold, dim, diag.code
                ));
            } else {
                if let Some(reason) = &diag.reason {
                    out.push_str(&format!("  {}note:{reset} {}\n", dim, reason));
                }
                if let Some(help) = &diag.help {
                    out.push_str(&format!("  {}help:{reset} {}\n", cyan_bold, help));
                }
                out.push_str(&format!(
                    "  {}note: for more details, run 'forgen explain {}'{reset}\n\n",
                    dim, diag.code
                ));
            }
        }
        out
    }

    /// Machine-readable JSON diagnostic format for IDEs, CI, and tools.
    pub fn format_json(&self) -> String {
        serde_json::to_string_pretty(&self.diagnostics).unwrap_or_else(|_| "[]".to_string())
    }
}
