//! Polyglot module and native runtime resolution for Datara (v1.4.4).
//!
//! Provides zero-latency binding and resolution for:
//! - Zig (.zig source files, Zig standard packages)
//! - C# / .NET (NativeAOT cdylibs, .dll / .so exports)
//! - Lua / LuaJIT / Luau (.lua / .luau scripts, bytecode)
//! - Go (c-shared archives, .go source packages, C-ABI exports)
//! - Java / Kotlin (GraalVM Native Image cdylibs, JAR packages)

use crate::ast::UseDecl;
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use std::path::PathBuf;

pub struct PolyglotResolver;

impl PolyglotResolver {
    /// Tests whether an import prefix belongs to a supported foreign language bridge.
    pub fn is_polyglot_prefix(prefix: &str) -> bool {
        matches!(
            prefix,
            "cpp"
                | "cxx"
                | "zig"
                | "csharp"
                | "cs"
                | "dotnet"
                | "lua"
                | "luajit"
                | "luau"
                | "go"
                | "golang"
                | "java"
                | "kotlin"
                | "jvm"
        )
    }

    /// Unified dispatcher for foreign polyglot module resolution.
    pub fn resolve_polyglot(
        kind: &str,
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        match kind {
            "cpp" | "cxx" => Self::resolve_cpp(u, base_dirs, diag),
            "zig" => Self::resolve_zig(u, base_dirs, diag),
            "csharp" | "cs" | "dotnet" => Self::resolve_csharp(u, base_dirs, diag),
            "lua" | "luajit" => Self::resolve_lua(u, base_dirs, diag),
            "luau" => Self::resolve_luau(u, base_dirs, diag),
            "go" | "golang" => Self::resolve_go(u, base_dirs, diag),
            "java" | "kotlin" | "jvm" => Self::resolve_jvm(u, base_dirs, diag),
            _ => None,
        }
    }

    /// Detect and bind C++ static / shared libraries and header-only packages.
    ///
    /// Search order:
    ///   1. `<base>/<target>.lib` / `.a` (MSVC / GCC static archive)
    ///   2. `<base>/<target>.dll` / `.so` / `.dylib` (shared library)
    ///   3. `<base>/<target>.hpp` header-only sentinel
    ///
    /// If a binary is found alongside an `.hpp` header, the pairing is reported
    /// for C++ ABI awareness (name-mangling, `extern "C"` wrapper requirement).
    ///
    /// When nothing is found the diagnostic includes the exact compile command
    /// for the most common toolchains (MSVC `cl.exe`, GCC `g++`, Clang `clang++`).
    pub fn resolve_cpp(
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        if u.path.len() < 2 {
            return None;
        }
        let target = &u.path[1];

        // Platform-appropriate static and shared library extensions
        let static_exts: &[&str] = if cfg!(windows) { &["lib"] } else { &["a"] };
        let shared_exts: &[&str] = if cfg!(windows) {
            &["dll"]
        } else if cfg!(target_os = "macos") {
            &["dylib", "so"]
        } else {
            &["so"]
        };

        // --- 1. Search static archives ---
        for ext in static_exts {
            for base in base_dirs {
                let candidate = base.join(format!("{}.{}", target, ext));
                if candidate.is_file() {
                    let has_header = base.join(format!("{}.hpp", target)).is_file()
                        || base.join(format!("{}.h", target)).is_file();
                    if has_header {
                        println!(
                            "[Forgen Polyglot] Bound C++ static library '{}' -> {} (with header; extern \"C\" wrapper recommended)",
                            target,
                            candidate.display()
                        );
                    } else {
                        println!(
                            "[Forgen Polyglot] Bound C++ static library '{}' -> {}",
                            target,
                            candidate.display()
                        );
                    }
                    return Some(candidate);
                }
            }
            let local_candidate = PathBuf::from(format!("{}.{}", target, ext));
            if local_candidate.is_file() {
                println!(
                    "[Forgen Polyglot] Bound C++ static library '{}' -> {}",
                    target,
                    local_candidate.display()
                );
                return Some(local_candidate);
            }
        }

        // --- 2. Search shared libraries ---
        for ext in shared_exts {
            for base in base_dirs {
                let candidate = base.join(format!("{}.{}", target, ext));
                if candidate.is_file() {
                    println!(
                        "[Forgen Polyglot] Bound C++ shared library '{}' -> {}",
                        target,
                        candidate.display()
                    );
                    return Some(candidate);
                }
            }
            let local_candidate = PathBuf::from(format!("{}.{}", target, ext));
            if local_candidate.is_file() {
                println!(
                    "[Forgen Polyglot] Bound C++ shared library '{}' -> {}",
                    target,
                    local_candidate.display()
                );
                return Some(local_candidate);
            }
        }

        // --- 3. Header-only library sentinel ---
        for base in base_dirs {
            let hpp = base.join(format!("{}.hpp", target));
            if hpp.is_file() {
                println!(
                    "[Forgen Polyglot] Bound C++ header-only library '{}' -> {}",
                    target,
                    hpp.display()
                );
                return Some(hpp);
            }
        }

        // --- 4. Emit actionable diagnostic ---
        diag.error(
            ErrorCode::ResolveUnreachableModule,
            format!(
                "C/C++ library '{target}' not found (.lib/.a/.dll/.so/.hpp).\n\
                 --> Compile your C++ library and place the output in the project or a base directory.\n\
                 \n\
                 MSVC:   cl.exe /c /EHsc /O2 {target}.cpp  &&  lib /OUT:{target}.lib {target}.obj\n\
                 GCC:    g++ -c -O3 -ffunction-sections -fdata-sections {target}.cpp -o {target}.o  &&  ar rcs lib{target}.a {target}.o\n\
                 Clang:  clang++ -c -O3 -ffunction-sections -fdata-sections {target}.cpp -o {target}.o  &&  ar rcs lib{target}.a {target}.o\n\
                 \n\
                 Note: All extern functions callable from Datara must be declared with extern \"C\" {{ ... }}\n\
                 to disable C++ name mangling. Wrap with `import cpp \"{target}.h\" with link(\"{target}.lib\");`"
            ),
            Some(u.span.clone()),
        );
        None
    }

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

        let local_lua = PathBuf::from(format!("{}.lua", target));
        if local_lua.is_file() {
            println!(
                "[Forgen Polyglot] Bound Lua script '{}' -> {}",
                target,
                local_lua.display()
            );
            return Some(local_lua);
        }

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

    /// Detect and bind Luau scripts
    pub fn resolve_luau(
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        if u.path.len() < 2 {
            return None;
        }
        let target = &u.path[1];

        for base in base_dirs {
            let luau_file = base.join(format!("{}.luau", target));
            if luau_file.is_file() {
                println!(
                    "[Forgen Polyglot] Bound Luau script '{}' -> {}",
                    target,
                    luau_file.display()
                );
                return Some(luau_file);
            }
        }

        let local_luau = PathBuf::from(format!("{}.luau", target));
        if local_luau.is_file() {
            println!(
                "[Forgen Polyglot] Bound Luau script '{}' -> {}",
                target,
                local_luau.display()
            );
            return Some(local_luau);
        }

        diag.error(
            ErrorCode::ResolveUnreachableModule,
            format!(
                "Luau script '{}.luau' not found in project base directories.\n  --> Ensure the file exists or pass the relative path.",
                target
            ),
            Some(u.span.clone()),
        );
        None
    }

    /// Detect and bind Go packages (c-shared or cgo archives)
    pub fn resolve_go(
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        if u.path.len() < 2 {
            return None;
        }
        let target = &u.path[1];
        let extensions = if cfg!(windows) {
            vec!["dll", "lib", "a"]
        } else if cfg!(target_os = "macos") {
            vec!["dylib", "so", "a"]
        } else {
            vec!["so", "a"]
        };

        for ext in &extensions {
            for base in base_dirs {
                let bin_file = base.join(format!("{}.{}", target, ext));
                if bin_file.is_file() {
                    println!(
                        "[Forgen Polyglot] Bound Go c-shared library '{}' -> {}",
                        target,
                        bin_file.display()
                    );
                    return Some(bin_file);
                }
            }
            let local_bin = PathBuf::from(format!("{}.{}", target, ext));
            if local_bin.is_file() {
                println!(
                    "[Forgen Polyglot] Bound Go c-shared library '{}' -> {}",
                    target,
                    local_bin.display()
                );
                return Some(local_bin);
            }
        }

        // Check if .go file exists to give an actionable compiler command
        for base in base_dirs {
            let go_src = base.join(format!("{}.go", target));
            if go_src.is_file() {
                diag.error(
                    ErrorCode::ResolveUnreachableModule,
                    format!(
                        "Go source file '{}' exists but has not been compiled to a native c-shared library.\n  --> Run: go build -buildmode=c-shared -o {}.dll {}",
                        go_src.display(),
                        target,
                        go_src.display()
                    ),
                    Some(u.span.clone()),
                );
                return None;
            }
        }

        diag.error(
            ErrorCode::ResolveUnreachableModule,
            format!(
                "Go c-shared library '{}.dll' (or .so/.a) not found.\n  --> Compile your Go package with: go build -buildmode=c-shared -o {}.dll",
                target, target
            ),
            Some(u.span.clone()),
        );
        None
    }

    /// Detect and bind JVM (Java / Kotlin) Native Image shared libraries or JARs
    pub fn resolve_jvm(
        u: &UseDecl,
        base_dirs: &[PathBuf],
        diag: &mut DiagnosticEngine,
    ) -> Option<PathBuf> {
        if u.path.len() < 2 {
            return None;
        }
        let target = &u.path[1];
        let extensions = if cfg!(windows) {
            vec!["dll", "jar"]
        } else if cfg!(target_os = "macos") {
            vec!["dylib", "jar"]
        } else {
            vec!["so", "jar"]
        };

        for ext in &extensions {
            for base in base_dirs {
                let file = base.join(format!("{}.{}", target, ext));
                if file.is_file() {
                    println!(
                        "[Forgen Polyglot] Bound JVM (Java/Kotlin) library '{}' -> {}",
                        target,
                        file.display()
                    );
                    return Some(file);
                }
            }
            let local_file = PathBuf::from(format!("{}.{}", target, ext));
            if local_file.is_file() {
                println!(
                    "[Forgen Polyglot] Bound JVM (Java/Kotlin) library '{}' -> {}",
                    target,
                    local_file.display()
                );
                return Some(local_file);
            }
        }

        diag.error(
            ErrorCode::ResolveUnreachableModule,
            format!(
                "JVM / Java / Kotlin library '{}.dll' or '{}.jar' not found.\n  --> Compile with GraalVM: native-image -O3 --shared -o {}.dll -jar {}.jar",
                target, target, target, target
            ),
            Some(u.span.clone()),
        );
        None
    }
}
