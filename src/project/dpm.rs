//! Unified multi-language dependency manifest (`dpm.toml` / `datara.toml`) and orchestrator.
//!
//! Provides single-command restore and compilation across all supported language bridges:
//! Datara packages, C/C++, Rust crates, Python packages, npm, Go, C# (.NET NativeAOT),
//! Zig, and JVM (GraalVM).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DpmPackageMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DpmDepValue {
    Simple(String),
    Detailed(Box<DpmDepDetailed>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpmDepDetailed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macos: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buildmode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aot: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DpmManifest {
    #[serde(default)]
    pub package: DpmPackageMeta,

    #[serde(default)]
    pub dependencies: HashMap<String, DpmDepValue>,

    #[serde(default, rename = "c-dependencies", alias = "c_dependencies")]
    pub c_dependencies: HashMap<String, DpmDepValue>,

    #[serde(default, rename = "cpp-dependencies", alias = "cpp_dependencies")]
    pub cpp_dependencies: HashMap<String, DpmDepValue>,

    #[serde(default, rename = "rust-dependencies", alias = "rust_dependencies")]
    pub rust_dependencies: HashMap<String, DpmDepValue>,

    #[serde(default, rename = "python-dependencies", alias = "python_dependencies")]
    pub python_dependencies: HashMap<String, DpmDepValue>,

    #[serde(default, rename = "npm-dependencies", alias = "npm_dependencies")]
    pub npm_dependencies: HashMap<String, DpmDepValue>,

    #[serde(default, rename = "go-dependencies", alias = "go_dependencies")]
    pub go_dependencies: HashMap<String, DpmDepValue>,

    #[serde(
        default,
        rename = "dotnet-dependencies",
        alias = "dotnet_dependencies",
        alias = "csharp-dependencies",
        alias = "csharp_dependencies"
    )]
    pub dotnet_dependencies: HashMap<String, DpmDepValue>,

    #[serde(default, rename = "zig-dependencies", alias = "zig_dependencies")]
    pub zig_dependencies: HashMap<String, DpmDepValue>,

    #[serde(
        default,
        rename = "jvm-dependencies",
        alias = "jvm_dependencies",
        alias = "java-dependencies",
        alias = "java_dependencies"
    )]
    pub jvm_dependencies: HashMap<String, DpmDepValue>,
}

impl DpmManifest {
    pub fn find_in_dir(dir: &Path) -> Option<(PathBuf, Self)> {
        for name in &["dpm.toml", "datara.toml"] {
            let p = dir.join(name);
            if p.is_file() {
                if let Ok(content) = fs::read_to_string(&p) {
                    if let Ok(manifest) = toml::from_str::<DpmManifest>(&content) {
                        return Some((p, manifest));
                    }
                }
            }
        }
        None
    }

    /// Orchestrate installation and compilation of all foreign dependencies.
    pub fn install_all(&self, project_dir: &Path) -> Result<InstallSummary, String> {
        let mut summary = InstallSummary::default();

        // 1. Rust dependencies via Cargo
        if !self.rust_dependencies.is_empty() {
            println!(
                ":: [Rust Bridge] Synchronizing {} Rust crates...",
                self.rust_dependencies.len()
            );
            for (crate_name, dep) in &self.rust_dependencies {
                let ver_arg = match dep.version() {
                    Some(v) => format!("{}@{}", crate_name, v),
                    None => crate_name.clone(),
                };
                let res = Command::new("cargo")
                    .args(["add", &ver_arg])
                    .current_dir(project_dir)
                    .output();
                if let Ok(out) = res {
                    if out.status.success() {
                        println!("[DONE] Added Rust crate '{}'", crate_name);
                        summary.rust_installed += 1;
                    }
                } else {
                    eprintln!(
                        "[WARN] 'cargo' not found in PATH. Ensure Rust and Cargo are installed."
                    );
                }
            }
        }

        // 2. Python dependencies via pip
        if !self.python_dependencies.is_empty() {
            println!(
                ":: [Python Bridge] Synchronizing {} Python packages...",
                self.python_dependencies.len()
            );
            for (pkg_name, dep) in &self.python_dependencies {
                let req_spec = match dep.version() {
                    Some(v) => format!(
                        "{}{}",
                        pkg_name,
                        if v.starts_with(|c: char| c.is_ascii_punctuation()) {
                            v.to_string()
                        } else {
                            format!("=={}", v)
                        }
                    ),
                    None => pkg_name.clone(),
                };
                let res = Command::new("python")
                    .args(["-m", "pip", "install", &req_spec])
                    .current_dir(project_dir)
                    .status()
                    .or_else(|_| {
                        Command::new("python3")
                            .args(["-m", "pip", "install", &req_spec])
                            .current_dir(project_dir)
                            .status()
                    });
                if let Ok(st) = res {
                    if st.success() {
                        println!("[DONE] Installed Python package '{}'", req_spec);
                        summary.python_installed += 1;
                    }
                } else {
                    eprintln!(
                        "[WARN] 'python' / 'pip' not found in PATH. Install Python 3.8+ to use Python bridge."
                    );
                }
            }
        }

        // 3. NPM packages
        if !self.npm_dependencies.is_empty() {
            println!(
                ":: [Node/NPM Bridge] Synchronizing {} NPM packages...",
                self.npm_dependencies.len()
            );
            for (pkg_name, dep) in &self.npm_dependencies {
                let spec = match dep.version() {
                    Some(v) => format!("{}@{}", pkg_name, v),
                    None => pkg_name.clone(),
                };
                let res = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" })
                    .args(["install", &spec])
                    .current_dir(project_dir)
                    .status();
                if let Ok(st) = res {
                    if st.success() {
                        println!("[DONE] Installed NPM package '{}'", spec);
                        summary.npm_installed += 1;
                    }
                } else {
                    eprintln!("[WARN] 'npm' not found in PATH. Install Node.js to use NPM bridge.");
                }
            }
        }

        // 4. Go dependencies (buildmode=c-shared)
        if !self.go_dependencies.is_empty() {
            println!(
                ":: [Go Bridge] Building {} Go c-shared packages...",
                self.go_dependencies.len()
            );
            for (lib_name, dep) in &self.go_dependencies {
                if let Some(src_path) = dep.path() {
                    let out_name = if cfg!(windows) {
                        format!("{}.dll", lib_name)
                    } else {
                        format!("lib{}.so", lib_name)
                    };
                    let res = Command::new("go")
                        .args(["build", "-buildmode=c-shared", "-o", &out_name, src_path])
                        .current_dir(project_dir)
                        .status();
                    if let Ok(st) = res {
                        if st.success() {
                            println!("[DONE] Compiled Go library '{}' -> {}", lib_name, out_name);
                            summary.go_compiled += 1;
                        }
                    } else {
                        eprintln!(
                            "[WARN] 'go' toolchain not found in PATH. Ensure Go 1.20+ is installed."
                        );
                    }
                }
            }
        }

        // 5. .NET NativeAOT dependencies
        if !self.dotnet_dependencies.is_empty() {
            println!(
                ":: [C#/.NET Bridge] Publishing {} NativeAOT libraries...",
                self.dotnet_dependencies.len()
            );
            for (lib_name, dep) in &self.dotnet_dependencies {
                if let Some(proj_path) = dep.path() {
                    let res = Command::new("dotnet")
                        .args(["publish", "-c", "Release", "/p:PublishAot=true", proj_path])
                        .current_dir(project_dir)
                        .status();
                    if let Ok(st) = res {
                        if st.success() {
                            println!("[DONE] Published .NET NativeAOT library '{}'", lib_name);
                            summary.dotnet_published += 1;
                        }
                    } else {
                        eprintln!(
                            "[WARN] 'dotnet' SDK not found in PATH. Install .NET 8.0+ SDK to use C# bridge."
                        );
                    }
                }
            }
        }

        // 6. C dependencies
        if !self.c_dependencies.is_empty() {
            println!(
                ":: [C Bridge] Synchronizing {} C libraries...",
                self.c_dependencies.len()
            );
            for (lib_name, dep) in &self.c_dependencies {
                match dep {
                    DpmDepValue::Detailed(_) if dep.path().is_some() => {
                        let src_path = dep.path().unwrap();
                        let src = project_dir.join(src_path);
                        let out_name = if cfg!(windows) {
                            format!("{}.dll", lib_name)
                        } else {
                            format!("lib{}.so", lib_name)
                        };
                        let res = if src.join("CMakeLists.txt").exists() {
                            Command::new("cmake")
                                .args(["-B", "build", "-DCMAKE_BUILD_TYPE=Release"])
                                .current_dir(&src)
                                .status()
                                .and_then(|_| {
                                    Command::new("cmake")
                                        .args(["--build", "build", "--config", "Release"])
                                        .current_dir(&src)
                                        .status()
                                })
                        } else {
                            let cc = if cfg!(windows) { "clang" } else { "cc" };
                            Command::new(cc)
                                .args(["-shared", "-fPIC", "-O3", "-o", &out_name])
                                .arg(src_path)
                                .current_dir(project_dir)
                                .status()
                        };
                        if let Ok(st) = res {
                            if st.success() {
                                println!("[DONE] Compiled C library '{}'", lib_name);
                                summary.c_compiled += 1;
                            }
                        } else {
                            eprintln!(
                                "[WARN] Failed to compile C library '{}'. Ensure C compiler or CMake is installed.",
                                lib_name
                            );
                        }
                    }
                    DpmDepValue::Detailed(_) if dep.link().is_some() => {
                        let l = dep.link().unwrap();
                        println!("[DONE] Linked system C library '{}'", l);
                        summary.c_compiled += 1;
                    }
                    _ => {
                        println!("[DONE] Registered C dependency '{}'", lib_name);
                        summary.c_compiled += 1;
                    }
                }
            }
        }

        // 7. C++ dependencies
        if !self.cpp_dependencies.is_empty() {
            println!(
                ":: [C++ Bridge] Synchronizing {} C++ libraries...",
                self.cpp_dependencies.len()
            );
            for (lib_name, dep) in &self.cpp_dependencies {
                match dep {
                    DpmDepValue::Detailed(_) if dep.path().is_some() => {
                        let src_path = dep.path().unwrap();
                        let src = project_dir.join(src_path);
                        let out_name = if cfg!(windows) {
                            format!("{}.dll", lib_name)
                        } else {
                            format!("lib{}.so", lib_name)
                        };
                        let res = if src.join("CMakeLists.txt").exists() {
                            Command::new("cmake")
                                .args(["-B", "build", "-DCMAKE_BUILD_TYPE=Release"])
                                .current_dir(&src)
                                .status()
                                .and_then(|_| {
                                    Command::new("cmake")
                                        .args(["--build", "build", "--config", "Release"])
                                        .current_dir(&src)
                                        .status()
                                })
                        } else {
                            let cxx = if cfg!(windows) { "clang++" } else { "c++" };
                            Command::new(cxx)
                                .args(["-shared", "-fPIC", "-std=c++17", "-O3", "-o", &out_name])
                                .arg(src_path)
                                .current_dir(project_dir)
                                .status()
                        };
                        if let Ok(st) = res {
                            if st.success() {
                                println!("[DONE] Compiled C++ library '{}'", lib_name);
                                summary.cpp_compiled += 1;
                            }
                        } else {
                            eprintln!(
                                "[WARN] Failed to compile C++ library '{}'. Ensure C++ compiler or CMake is installed.",
                                lib_name
                            );
                        }
                    }
                    DpmDepValue::Detailed(_) if dep.link().is_some() => {
                        let l = dep.link().unwrap();
                        println!("[DONE] Linked system C++ library '{}'", l);
                        summary.cpp_compiled += 1;
                    }
                    _ => {
                        println!("[DONE] Registered C++ dependency '{}'", lib_name);
                        summary.cpp_compiled += 1;
                    }
                }
            }
        }

        // 8. Zig dependencies
        if !self.zig_dependencies.is_empty() {
            println!(
                ":: [Zig Bridge] Synchronizing {} Zig packages...",
                self.zig_dependencies.len()
            );
            for (lib_name, dep) in &self.zig_dependencies {
                if let Some(src_path) = dep.path() {
                    let out_name = if cfg!(windows) {
                        format!("{}.dll", lib_name)
                    } else {
                        format!("lib{}.so", lib_name)
                    };
                    let res = Command::new("zig")
                        .args([
                            "build-lib",
                            "-dynamic",
                            "-O",
                            "ReleaseFast",
                            src_path,
                            &format!("-femit-bin={}", out_name),
                        ])
                        .current_dir(project_dir)
                        .status();
                    if let Ok(st) = res {
                        if st.success() {
                            println!("[DONE] Compiled Zig library '{}' -> {}", lib_name, out_name);
                            summary.zig_compiled += 1;
                        }
                    } else {
                        eprintln!(
                            "[WARN] 'zig' toolchain not found in PATH. Ensure Zig is installed."
                        );
                    }
                } else {
                    println!("[DONE] Registered Zig dependency '{}'", lib_name);
                    summary.zig_compiled += 1;
                }
            }
        }

        // 9. JVM dependencies
        if !self.jvm_dependencies.is_empty() {
            println!(
                ":: [JVM Bridge] Synchronizing {} JVM packages...",
                self.jvm_dependencies.len()
            );
            for (lib_name, dep) in &self.jvm_dependencies {
                if dep.aot() == Some(true) {
                    if let Some(proj_path) = dep.path() {
                    let out_name = if cfg!(windows) {
                        format!("{}.dll", lib_name)
                    } else {
                        format!("lib{}.so", lib_name)
                    };
                    let res = Command::new("native-image")
                        .args(["--shared", "-o", lib_name, "-jar", proj_path])
                        .current_dir(project_dir)
                        .status();
                    if let Ok(st) = res {
                        if st.success() {
                            println!(
                                "[DONE] Compiled GraalVM Native Image for '{}' -> {}",
                                lib_name, out_name
                            );
                            summary.jvm_compiled += 1;
                        }
                    } else {
                        eprintln!(
                            "[WARN] 'native-image' not found in PATH. Install GraalVM to compile JVM libraries to native shared libraries."
                        );
                    }
                } else {
                    println!("[DONE] Registered JVM dependency '{}'", lib_name);
                    summary.jvm_compiled += 1;
                }
                }
            }
        }

        Ok(summary)
    }
}

#[derive(Debug, Default, Clone)]
pub struct InstallSummary {
    pub rust_installed: usize,
    pub python_installed: usize,
    pub npm_installed: usize,
    pub go_compiled: usize,
    pub dotnet_published: usize,
    pub c_compiled: usize,
    pub cpp_compiled: usize,
    pub zig_compiled: usize,
    pub jvm_compiled: usize,
}


impl DpmDepValue {
    pub fn detailed(&self) -> Option<&DpmDepDetailed> {
        match self {
            DpmDepValue::Detailed(d) => Some(d),
            _ => None,
        }
    }
    pub fn version(&self) -> Option<&str> {
        match self {
            DpmDepValue::Simple(v) => Some(v),
            DpmDepValue::Detailed(d) => d.version.as_deref(),
        }
    }
    pub fn link(&self) -> Option<&str> {
        match self {
            DpmDepValue::Simple(v) => Some(v),
            DpmDepValue::Detailed(d) => d.link.as_deref(),
        }
    }
    pub fn path(&self) -> Option<&str> {
        self.detailed().and_then(|d| d.path.as_deref())
    }
    pub fn aot(&self) -> Option<bool> {
        self.detailed().and_then(|d| d.aot)
    }
}
