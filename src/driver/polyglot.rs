//! Polyglot module and native runtime resolution for Datara v1.3.0.
//!
//! Provides zero-latency binding and resolution for:
//! - Zig (.zig source files, Zig standard packages)
//! - C# / .NET (NativeAOT cdylibs, .dll / .so exports)
//! - Lua / LuaJIT (.lua scripts, LuaJIT bytecodes)

use crate::ast::UseDecl;
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use std::path::PathBuf;

pub struct PolyglotResolver;

impl PolyglotResolver {
    /// Detect and bind Zig source files or standard libraries
    pub fn resolve_zig(
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        if u.path.len() < 2 {
            return None;
        }
        let target = &u.path[1];
        let mut candidates = Vec::new();

        // 1. Direct file in base directories
        for base in base_dirs {
            let zig_file = base.join(format!("{}.zig", target));
            if zig_file.is_file() {
                candidates.push(zig_file);
            }
            let raw_file = base.join(target);
            if raw_file.is_file() && raw_file.extension().and_then(|s| s.to_str()) == Some("zig") {
                candidates.push(raw_file);
            }
        }

        // 2. Current directory
        let local_zig = PathBuf::from(format!("{}.zig", target));
        if local_zig.is_file() {
            candidates.push(local_zig);
        }

        if let Some(found) = candidates.into_iter().next() {
            println!(
                "[Forgen Polyglot] Bound Zig module '{}' -> {}",
                target,
                found.display()
            );
            return Some(found);
        }

        // Check if zig compiler is in PATH for standard packages
        let zig_check = std::process::Command::new("zig").arg("version").output();
        if zig_check.is_ok() || target == "std" || target == "math" {
            println!(
                "[Forgen Polyglot] Bound Zig system module '{}' (Native C-ABI link)",
                target
            );
            return None;
        }

        diag.error(
            ErrorCode::ResolveUnreachableModule,
            format!(
                "Zig module '{}' not found in project directories or system Zig installation.\n  --> Ensure {}.zig exists or 'zig' is installed in PATH.",
                target, target
            ),
            Some(u.span.clone()),
        );
        None
    }

    /// Detect and bind C# / .NET NativeAOT libraries
    pub fn resolve_csharp(
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        if u.path.len() < 2 {
            return None;
        }
        let target = &u.path[1];
        let extensions = if cfg!(windows) {
            vec!["dll"]
        } else if cfg!(target_os = "macos") {
            vec!["dylib", "so"]
        } else {
            vec!["so"]
        };

        for ext in &extensions {
            // 1. Direct binary path in base directories
            for base in base_dirs {
                let dll_file = base.join(format!("{}.{}", target, ext));
                if dll_file.is_file() {
                    println!(
                        "[Forgen Polyglot] Bound C#/.NET NativeAOT library '{}' -> {}",
                        target,
                        dll_file.display()
                    );
                    return Some(dll_file);
                }
                let native_aot_dir = base
                    .join("bin")
                    .join("Release")
                    .join("net8.0")
                    .join("publish");
                let aot_file = native_aot_dir.join(format!("{}.{}", target, ext));
                if aot_file.is_file() {
                    println!(
                        "[Forgen Polyglot] Bound C#/.NET NativeAOT publish binary '{}' -> {}",
                        target,
                        aot_file.display()
                    );
                    return Some(aot_file);
                }
            }

            // 2. Current working directory
            let local_dll = PathBuf::from(format!("{}.{}", target, ext));
            if local_dll.is_file() {
                println!(
                    "[Forgen Polyglot] Bound C#/.NET NativeAOT library '{}' -> {}",
                    target,
                    local_dll.display()
                );
                return Some(local_dll);
            }
        }

        // Known standard .NET runtime bindings
        const KNOWN_DOTNET: &[&str] = &["core", "system", "math", "collections", "numerics"];
        if KNOWN_DOTNET.contains(&target.to_lowercase().as_str()) {
            println!(
                "[Forgen Polyglot] Bound C#/.NET runtime module '{}' (NativeAOT export)",
                target
            );
            return None;
        }

        diag.error(
            ErrorCode::ResolveUnreachableModule,
            format!(
                "C#/.NET NativeAOT library '{}.dll' (or .so/.dylib) not found.\n  --> Compile your C# project with 'dotnet publish -c Release -r win-x64 /p:PublishAot=true'.",
                target
            ),
            Some(u.span.clone()),
        );
        None
    }

    /// Detect and bind Lua / LuaJIT scripts
    pub fn resolve_lua(
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        if u.path.len() < 2 {
            return None;
        }
        let target = &u.path[1];

        // 1. Direct .lua file in base directories
        for base in base_dirs {
            let lua_file = base.join(format!("{}.lua", target));
            if lua_file.is_file() {
                println!(
                    "[Forgen Polyglot] Bound Lua script '{}' -> {}",
                    target,
                    lua_file.display()
                );
                return Some(lua_file);
            }
            let raw_file = base.join(target);
            if raw_file.is_file() && raw_file.extension().and_then(|s| s.to_str()) == Some("lua") {
                println!(
                    "[Forgen Polyglot] Bound Lua script '{}' -> {}",
                    target,
                    raw_file.display()
                );
                return Some(raw_file);
            }
        }

        // 2. Current directory
        let local_lua = PathBuf::from(format!("{}.lua", target));
        if local_lua.is_file() {
            println!(
                "[Forgen Polyglot] Bound Lua script '{}' -> {}",
                target,
                local_lua.display()
            );
            return Some(local_lua);
        }

        // Known standard Lua modules
        const KNOWN_LUA: &[&str] = &[
            "math",
            "string",
            "table",
            "io",
            "os",
            "coroutine",
            "package",
        ];
        if KNOWN_LUA.contains(&target.to_lowercase().as_str()) {
            println!(
                "[Forgen Polyglot] Bound Lua standard module '{}' (Embedded LuaJIT VM)",
                target
            );
            return None;
        }

        diag.error(
            ErrorCode::ResolveUnreachableModule,
            format!(
                "Lua script '{}.lua' not found in project base directories.\n  --> Ensure the file exists or pass the relative path.",
                target
            ),
            Some(u.span.clone()),
        );
        None
    }
}
