//! `forgen migrate <from> <to> [path] [--dry-run]` — mechanical
//! modernization of 1.4-era Datara sources to the 1.5 canonical surface
//! (PLAN_V152 N2): `let` → `val`/`mut`, `String` → `Str`, single-argument
//! `println(x)` → `out x`.
//!
//! The rewrite is line-based and string-literal/comment aware at the level
//! needed for these three rules: replacements never touch the contents of
//! `"` strings or `//` comments. A per-file report lists every edit with
//! its 1-based line; `--dry-run` writes nothing.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Apply one textual rule outside of string literals and line comments.
/// Returns (new_line, changed) plus nothing else — keeps the pass simple
/// and auditable.
fn apply_rule(line: &str, rule: &str) -> (String, bool) {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 8);
    let mut i = 0usize;
    let mut changed = false;
    let mut in_string = false;
    let mut pending: Option<(usize, String)> = None;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' {
            in_string = !in_string;
            out.push(c);
            i += 1;
            continue;
        }
        if !in_string && c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            // Line comment: copy verbatim.
            out.push_str(&line[i..]);
            return (out, changed);
        }
        if in_string {
            out.push(c);
            i += 1;
            continue;
        }
        let rest = &line[i..];
        // (matched_len, replacement) — explicit lengths, no off-by-ones.
        let matched: Option<(usize, &str)> = if rule == "let-mut" {
            if (i == 0 || bytes[i - 1].is_ascii_whitespace())
                && (rest.starts_with("let mut ") || rest == "let mut")
            {
                Some((rest.len().min(8), "mut "))
            } else {
                None
            }
        } else if rule == "let" {
            if (i == 0 || bytes[i - 1].is_ascii_whitespace())
                && (rest.starts_with("let ") || rest == "let")
            {
                Some((rest.len().min(3), "val"))
            } else {
                None
            }
        } else if rule == "String" {
            let next_ok = match rest.len().cmp(&6) {
                std::cmp::Ordering::Equal => true,
                std::cmp::Ordering::Greater => !rest[6..]
                    .chars()
                    .next()
                    .map(|n| n.is_ascii_alphanumeric() || n == '_')
                    .unwrap_or(false),
                std::cmp::Ordering::Less => false,
            };
            if (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
                && rest.starts_with("String")
                && next_ok
            {
                Some((6, "Str"))
            } else {
                None
            }
        } else if rule == "println" {
            // `println(X)` → `out X` — drop the call parens. Multi-arg
            // println (format-string style) is left untouched: only the
            // single-argument form has an `out` equivalent.
            if (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
                && rest.starts_with("println(")
            {
                let open = i + 8; // 'println(' is 8 chars
                let mut depth = 1i32;
                let mut j = open;
                let mut in_str = false;
                while j < bytes.len() {
                    let ch = bytes[j] as char;
                    if ch == '"' {
                        in_str = !in_str;
                    } else if !in_str && ch == '(' {
                        depth += 1;
                    } else if !in_str && ch == ')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    j += 1;
                }
                if depth == 0 {
                    let inner = &line[open..j];
                    if !inner.contains(',') {
                        pending = Some((j + 1 - i, format!("out {}", inner)));
                    }
                }
            }
            None
        } else {
            None
        };
        if let Some((matched_len, replacement)) = matched {
            out.push_str(replacement);
            i += matched_len;
            changed = true;
            continue;
        }
        if let Some((skip_len, replacement)) = pending.take() {
            out.push_str(&replacement);
            i += skip_len;
            changed = true;
            continue;
        }
        out.push(c);
        i += 1;
    }
    (out, changed)
}

/// Rewrite one file, returning (new_contents, report lines).
fn migrate_source(contents: &str, rule: &str) -> (String, Vec<(usize, &'static str)>) {
    let mut report = Vec::new();
    let mut out_lines = Vec::new();
    for (idx, line) in contents.lines().enumerate() {
        // Pass 1: `let mut` → `mut` (before plain `let`, or `let mut` would
        // become `val mut`).
        let (l1, c1) = apply_rule(line, "let-mut");
        // Pass 2: plain `let` → `val`.
        let (l2, c2) = apply_rule(&l1, "let");
        // Pass 3: String → Str.
        let (l3, c3) = apply_rule(&l2, "String");
        // Pass 4: single-arg println → out.
        let (l4, c4) = apply_rule(&l3, "println");
        if c1 {
            report.push((idx + 1, "let mut -> mut"));
        }
        if c2 {
            report.push((idx + 1, "let -> val"));
        }
        if c3 {
            report.push((idx + 1, "String -> Str"));
        }
        if c4 {
            report.push((idx + 1, "println -> out"));
        }
        out_lines.push(l4);
    }
    let mut new_contents = out_lines.join("\n");
    if contents.ends_with('\n') && !new_contents.is_empty() {
        new_contents.push('\n');
    }
    let _ = rule;
    (new_contents, report)
}

pub(crate) fn cmd_migrate(args: &[String]) -> bool {
    // args: [exe, "migrate", "<from>", "<to>", path?, flags?]
    let mut path = PathBuf::from(".");
    let mut dry_run = false;
    for a in args.iter().skip(4) {
        if a == "--dry-run" {
            dry_run = true;
        } else if !a.starts_with('-') {
            path = PathBuf::from(a);
        }
    }
    let from = args.get(2).map(|s| s.as_str()).unwrap_or("1.4");
    let to = args.get(3).map(|s| s.as_str()).unwrap_or("1.5");
    println!(
        ":: [MIGRATE] Datara {} -> {}: rewriting let/val/mut, String/Str, println/out",
        from, to
    );

    if !path.exists() {
        eprintln!("[FAIL] Path does not exist: {}", path.display());
        std::process::exit(1);
    }

    let mut files = Vec::new();
    collect_dtr(&path, &mut files);
    if files.is_empty() {
        eprintln!("[FAIL] No .dtr files found under {}", path.display());
        std::process::exit(1);
    }

    let mut total_edits = 0usize;
    let mut touched = 0usize;
    for file in &files {
        let contents = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[WARN] Cannot read {}: {}", file.display(), e);
                continue;
            }
        };
        let (new_contents, report) = migrate_source(&contents, "");
        if report.is_empty() {
            continue;
        }
        touched += 1;
        total_edits += report.len();
        println!("  {} ({} edit(s)):", file.display(), report.len());
        for (line_no, rule) in &report {
            println!("    line {}: {}", line_no, rule);
        }
        if !dry_run {
            if let Err(e) = std::fs::write(file, &new_contents) {
                eprintln!("[FAIL] Cannot write {}: {}", file.display(), e);
                continue;
            }
        } else if std::env::var("DATARA_MIGRATE_DIFF").is_ok() {
            // Optional diff view for CI/tooling.
            for (idx, (old_l, new_l)) in contents.lines().zip(new_contents.lines()).enumerate() {
                if old_l != new_l {
                    println!("    -{}", old_l);
                    println!("    +{}", new_l);
                    let _ = idx;
                }
            }
        }
    }

    let mode = if dry_run {
        " (dry-run: nothing written)"
    } else {
        ""
    };
    println!(
        "[DONE] {} file(s) rewritten, {} edit(s){}",
        touched, total_edits, mode
    );
    let _ = std::io::stdout().flush();
    true
}

fn collect_dtr(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.is_file() {
        if dir.extension().and_then(|s| s.to_str()) == Some("dtr") {
            out.push(dir.to_path_buf());
        }
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" || name == "packages" {
                continue;
            }
            collect_dtr(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("dtr") {
            out.push(p);
        }
    }
}
