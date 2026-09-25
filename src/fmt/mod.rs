//! Datara & Forgen Official Code Formatter Engine
//!
//! Provides API and CLI execution for formatting `.dtr` and `.forge` files.

pub mod rules;

pub use rules::{
    FormatDiff, FormatOptions, format_loops_in_code, format_operators_in_code, format_source,
};

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct FormatSummary {
    pub files_checked: usize,
    pub files_formatted: usize,
    pub total_diffs: usize,
    pub has_unformatted: bool,
}

/// Reads a Datara source file as UTF-8, mapping the "not valid UTF-8" IO
/// error to an actionable message. PowerShell 5.1 still defaults to OEM
/// codepages, so a `.dtr` saved from it (or from any non-UTF-8 editor) would
/// otherwise surface as an opaque `stream did not contain valid UTF-8` and
/// the tooling would silently skip the file.
pub fn read_source_utf8(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            format!(
                "File '{}' is not valid UTF-8. Datara sources must be UTF-8 \
                 (BOM optional). Re-save the file with UTF-8 encoding in your \
                 editor; when redirecting from PowerShell 5.1 use \
                 '[IO.File]::WriteAllText(...)' or 'Out-File -Encoding utf8'.",
                path.display()
            )
        } else {
            format!("Failed to read file '{}': {}", path.display(), e)
        }
    })
}

/// Formats a single file in-place or checks it
pub fn format_file(path: &Path, opts: &FormatOptions) -> Result<Vec<FormatDiff>, String> {
    let content = read_source_utf8(path)?;

    let (formatted, diffs) = format_source(&content, opts);

    if !opts.check && !diffs.is_empty() {
        fs::write(path, &formatted)
            .map_err(|e| format!("Failed to write file '{}': {}", path.display(), e))?;
    }

    Ok(diffs)
}

/// Recursively finds all `.dtr` and `.forge` files in a directory or file path
pub fn collect_datara_files(target: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if target.is_file() {
        let ext = target.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext == "dtr" || ext == "forge" {
            files.push(target.to_path_buf());
        }
    } else if target.is_dir()
        && let Ok(entries) = fs::read_dir(target)
    {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let dir_name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if dir_name != "target" && dir_name != ".git" && dir_name != "node_modules" {
                    files.extend(collect_datara_files(&p));
                }
            } else {
                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "dtr" || ext == "forge" {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    files
}

/// Runs formatting across multiple files or a directory
pub fn format_paths(paths: &[PathBuf], opts: &FormatOptions) -> Result<FormatSummary, String> {
    let mut summary = FormatSummary::default();

    for path in paths {
        let diffs = format_file(path, opts)?;
        summary.files_checked += 1;

        if !diffs.is_empty() {
            summary.has_unformatted = true;
            summary.total_diffs += diffs.len();
            if !opts.check {
                summary.files_formatted += 1;
            }
        }
    }

    Ok(summary)
}
