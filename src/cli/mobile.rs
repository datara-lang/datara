//! Mobile CLI commands for Datara & Forgen (`forgen mobile`)
//!
//! Subcommands:
//! - `init <name> [--template android|ios|cross]`
//! - `build <entry.dtr> --target <android|ios> [--abi <abi>] [-o <out_dir>]`
//! - `check`

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::mobile_bridge::{
    AndroidAbi, ExportedFunction, ExportedParam, generate_jni_c_bridge, generate_kotlin_wrapper,
    generate_swift_c_header, generate_swift_wrapper,
};

pub fn cmd_mobile(args: &[String]) -> bool {
    if args.len() < 3 {
        print_mobile_help();
        return false;
    }

    match args[2].as_str() {
        "init" => handle_mobile_init(&args[3..]),
        "build" => handle_mobile_build(&args[3..]),
        "check" => handle_mobile_check(),
        "help" | "--help" | "-h" => {
            print_mobile_help();
            false
        }
        unknown => {
            eprintln!("Unknown mobile subcommand: '{}'", unknown);
            print_mobile_help();
            false
        }
    }
}

fn print_mobile_help() {
    println!("Forgen Mobile Cross-Compilation & Native Bridge Tool");
    println!();
    println!("USAGE:");
    println!("    forgen mobile <SUBCOMMAND> [OPTIONS]");
    println!();
    println!("SUBCOMMANDS:");
    println!("    init <name> [--template <android|ios|cross>]");
    println!("        Initialize a high-performance Datara mobile library project.");
    println!();
    println!("    build <entry.dtr> --target <android|ios> [OPTIONS]");
    println!(
        "        Compile Datara library to native mobile binary and generate zero-copy bindings."
    );
    println!("        Options:");
    println!("          --target <android|ios>     Target mobile operating system (required)");
    println!("          --abi <abi>                Target ABI (arm64-v8a, armeabi-v7a, x86_64)");
    println!(
        "          --package <pkg>            Java/Kotlin package name (default: com.datara.app)"
    );
    println!("          --class <name>             Bridge class name (default: DataraBridge)");
    println!("          -o, --out <dir>            Output directory (default: ./mobile_build)");
    println!();
    println!("    check");
    println!("        Inspect Android NDK, SDK, Clang, and iOS toolchain availability.");
    println!();
}

fn handle_mobile_init(args: &[String]) -> bool {
    if args.is_empty() {
        eprintln!("Error: project name is required for 'forgen mobile init <name>'");
        return false;
    }

    let project_name = &args[0];
    let mut template = "cross";

    let mut i = 1;
    while i < args.len() {
        if (args[i] == "--template" || args[i] == "-t") && i + 1 < args.len() {
            template = &args[i + 1];
            i += 2;
        } else {
            i += 1;
        }
    }

    let project_dir = PathBuf::from(project_name);
    if project_dir.exists() {
        eprintln!(
            "Error: directory '{}' already exists. Aborting.",
            project_name
        );
        return false;
    }

    if let Err(e) = fs::create_dir_all(project_dir.join("src")) {
        eprintln!("Failed to create project directory: {}", e);
        return false;
    }

    let toml_content = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2026"
type = "lib"

[mobile]
android_package = "com.datara.{}"
android_abis = ["arm64-v8a", "armeabi-v7a", "x86_64"]
ios_framework_name = "DataraCore"
min_android_sdk = 24
min_ios_version = "14.0"
"#,
        project_name, project_name
    );

    let _ = fs::write(project_dir.join("datara.toml"), toml_content);

    let src_content = r#"// High-performance Datara mobile library entry point
// Exports zero-overhead native computational routines

struct Particle {
    x: Float,
    y: Float,
    z: Float,
    vx: Float,
    vy: Float,
    vz: Float
}

fn init_particles(count: Int) -> Int {
    println(fmt"Initializing {count} mobile simulation particles...")
    return count
}

fn compute_physics(delta_time: Float) -> Float {
    let friction: Float = 0.98
    return delta_time * friction
}

fn add_vectors(a: Float, b: Float) -> Float {
    return a + b
}
"#;

    let _ = fs::write(project_dir.join("src").join("lib.dtr"), src_content);

    match template {
        "android" => {
            let _ = fs::create_dir_all(project_dir.join("android").join("app"));
        }
        "ios" => {
            let _ = fs::create_dir_all(project_dir.join("ios"));
        }
        _ => {
            let _ = fs::create_dir_all(project_dir.join("android"));
            let _ = fs::create_dir_all(project_dir.join("ios"));
        }
    }

    println!("Initialized Datara mobile project '{}'", project_name);
    println!("  Template: {}", template);
    println!("  Config:   {}/datara.toml", project_name);
    println!("  Source:   {}/src/lib.dtr", project_name);
    println!();
    println!("To build for mobile:");
    println!(
        "  cd {} && forgen mobile build src/lib.dtr --target android",
        project_name
    );
    true
}

fn handle_mobile_build(args: &[String]) -> bool {
    if args.is_empty() {
        eprintln!("Error: input file required for 'forgen mobile build <file.dtr>'");
        return false;
    }

    let source_file = &args[0];
    let mut target = "";
    let mut abi_str = "arm64-v8a";
    let mut package = "com.datara.app";
    let mut class_name = "DataraBridge";
    let mut out_dir = PathBuf::from("mobile_build");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" | "-t" => {
                if i + 1 < args.len() {
                    target = &args[i + 1];
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--abi" => {
                if i + 1 < args.len() {
                    abi_str = &args[i + 1];
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--package" | "-p" => {
                if i + 1 < args.len() {
                    package = &args[i + 1];
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--class" | "-c" => {
                if i + 1 < args.len() {
                    class_name = &args[i + 1];
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--out" | "-o" => {
                if i + 1 < args.len() {
                    out_dir = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    if target.is_empty() {
        eprintln!("Error: --target <android|ios> is required.");
        return false;
    }

    let source_path = Path::new(source_file);
    if !source_path.exists() {
        eprintln!("Error: source file '{}' not found.", source_file);
        return false;
    }

    let source_code = match fs::read_to_string(source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading '{}': {}", source_file, e);
            return false;
        }
    };

    let funcs = parse_exported_functions(&source_code);

    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("Failed to create output directory: {}", e);
        return false;
    }

    match target.to_ascii_lowercase().as_str() {
        "android" => {
            let abi = AndroidAbi::from_str(abi_str).unwrap_or(AndroidAbi::Arm64V8a);
            let jni_c = generate_jni_c_bridge(package, class_name, &funcs);
            let lib_name = class_name.to_lowercase();
            let kotlin = generate_kotlin_wrapper(package, class_name, &lib_name, &funcs);

            let jni_path = out_dir.join(format!("{}_jni.c", class_name.to_lowercase()));
            let kt_path = out_dir.join(format!("{}.kt", class_name));

            let _ = fs::write(&jni_path, jni_c);
            let _ = fs::write(&kt_path, kotlin);

            println!("Mobile Android Build Artifacts Generated:");
            println!("  Target ABI:      {}", abi.as_str());
            println!("  Target Triple:   {}", abi.target_triple());
            println!("  JNI C Bridge:    {}", jni_path.display());
            println!("  Kotlin Wrapper:  {}", kt_path.display());
            println!("  Exported Funcs:  {}", funcs.len());
            true
        }
        "ios" => {
            let header = generate_swift_c_header(class_name, &funcs);
            let swift = generate_swift_wrapper(class_name, &funcs);

            let header_path = out_dir.join(format!("{}.h", class_name));
            let swift_path = out_dir.join(format!("{}.swift", class_name));

            let _ = fs::write(&header_path, header);
            let _ = fs::write(&swift_path, swift);

            println!("Mobile iOS Build Artifacts Generated:");
            println!("  Umbrella Header: {}", header_path.display());
            println!("  Swift Wrapper:   {}", swift_path.display());
            println!("  Framework Name:  {}", class_name);
            println!("  Exported Funcs:  {}", funcs.len());
            true
        }
        unknown => {
            eprintln!(
                "Unknown target '{}'. Supported targets: android, ios",
                unknown
            );
            false
        }
    }
}

fn handle_mobile_check() -> bool {
    println!("--- Forgen Mobile Toolchain Audit ---");

    // Android NDK check
    let ndk_env = std::env::var("ANDROID_NDK_HOME")
        .or_else(|_| std::env::var("ANDROID_NDK_ROOT"))
        .or_else(|_| std::env::var("NDK_HOME"));

    match ndk_env {
        Ok(path) => println!("  Android NDK:     FOUND at {}", path),
        Err(_) => {
            println!("  Android NDK:     Not set (set ANDROID_NDK_HOME for cross-compilation)")
        }
    }

    // Android SDK check
    let sdk_env = std::env::var("ANDROID_HOME").or_else(|_| std::env::var("ANDROID_SDK_ROOT"));
    match sdk_env {
        Ok(path) => println!("  Android SDK:     FOUND at {}", path),
        Err(_) => println!("  Android SDK:     Not set (set ANDROID_HOME for AAR packaging)"),
    }

    // Clang check
    let clang_status = std::process::Command::new("clang")
        .arg("--version")
        .output();
    match clang_status {
        Ok(out) if out.status.success() => {
            let v = String::from_utf8_lossy(&out.stdout);
            let first_line = v.lines().next().unwrap_or("clang installed");
            println!("  C/C++ Compiler:  FOUND ({})", first_line);
        }
        _ => println!("  C/C++ Compiler:  clang not found in PATH"),
    }

    // iOS Xcode check (macOS only)
    if cfg!(target_os = "macos") {
        let xcode_status = std::process::Command::new("xcrun")
            .arg("--version")
            .output();
        match xcode_status {
            Ok(out) if out.status.success() => {
                println!("  Apple Xcode:     FOUND (xcrun available)")
            }
            _ => println!("  Apple Xcode:     xcrun not found"),
        }
    } else {
        println!("  Apple Xcode:     Available on macOS host");
    }

    println!("Mobile toolchain check complete.");
    true
}

fn parse_exported_functions(source: &str) -> Vec<ExportedFunction> {
    let mut funcs = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("fn ") {
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                if let Some(paren_open) = rest.find('(') {
                    let fn_name = rest[..paren_open].trim().to_string();
                    if let Some(paren_close) = rest.find(')') {
                        let params_str = &rest[paren_open + 1..paren_close];
                        let mut params = Vec::new();
                        if !params_str.trim().is_empty() {
                            for part in params_str.split(',') {
                                let p = part.trim();
                                if let Some((pname, ptype)) = p.split_once(':') {
                                    params.push(ExportedParam {
                                        name: pname.trim().to_string(),
                                        dtr_type: ptype.trim().to_string(),
                                    });
                                }
                            }
                        }

                        let ret_type = if let Some(arrow) = rest.find("->") {
                            let after_arrow = &rest[arrow + 2..];
                            let end = after_arrow.find('{').unwrap_or(after_arrow.len());
                            after_arrow[..end].trim().to_string()
                        } else {
                            "Void".to_string()
                        };

                        funcs.push(ExportedFunction {
                            name: fn_name,
                            params,
                            return_type: ret_type,
                        });
                    }
                }
            }
        }
    }

    funcs
}
