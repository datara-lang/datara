//! Automated LLVM / Clang installation and interactive setup.
//!
//! Provides `install_llvm()` and `prompt_and_install_llvm_if_interactive()`
//! to enable seamless LLVM toolchain provisioning for `--llvm` compilation.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::codegen::linker::{find_clang, find_llc};

/// Checks whether an LLVM/Clang executable is available on the system.
pub fn is_llvm_available() -> bool {
    find_clang().is_some() || find_llc().is_some()
}

/// Prompts the user to install LLVM if running in an interactive terminal and LLVM is missing.
/// Returns `true` if installation was performed and verified.
pub fn prompt_and_install_llvm_if_interactive() -> bool {
    if is_llvm_available() {
        return true;
    }

    // Check if stdin is a terminal
    let is_interactive = atty_check();
    if !is_interactive {
        return false;
    }

    println!("\n================================================================================");
    println!(" [Forgen LLVM] LLVM / Clang is not found on your system.");
    println!("================================================================================");
    println!(" The '--llvm' compilation profile requires Clang/LLVM for peak optimizations:");
    println!("   - Full Link-Time Optimization (LTO)");
    println!("   - AVX2 / AVX-512 / ARM Neon SIMD vectorization");
    println!("   - Maximum machine-speed production binaries\n");
    print!(" Would you like Forgen to automatically install LLVM now? [y/N]: ");
    let _ = io::stdout().flush();

    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_ok() {
        let trimmed = answer.trim().to_lowercase();
        if trimmed == "y" || trimmed == "yes" {
            return install_llvm();
        }
    }

    false
}

/// Fallback terminal detection without pulling in external crate.
fn atty_check() -> bool {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
            fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
        }
        unsafe {
            const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            if handle.is_null() || handle as isize == -1 {
                return false;
            }
            let mut mode: u32 = 0;
            GetConsoleMode(handle, &mut mode) != 0
        }
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn isatty(fd: i32) -> i32;
        }
        unsafe { isatty(0) == 1 }
    }
}

/// Automatically downloads and installs LLVM / Clang on the current OS.
pub fn install_llvm() -> bool {
    println!("================================================================================");
    println!(" Datara Toolchain - Automated LLVM / Clang Setup");
    println!("================================================================================");

    if let Some(clang) = find_clang() {
        println!("\n[OK] LLVM / Clang is already installed and ready:");
        println!("     {}", clang.display());
        return true;
    }

    #[cfg(windows)]
    {
        install_llvm_windows()
    }
    #[cfg(target_os = "macos")]
    {
        install_llvm_macos()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        install_llvm_linux()
    }
}

#[cfg(windows)]
fn install_llvm_windows() -> bool {
    println!("\n-> Detecting Windows package managers and download channels...");

    // 1. Try winget if available
    let winget_path = which_local("winget.exe")
        .or_else(|| {
            let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
            let p = PathBuf::from(local_app_data)
                .join(r"Microsoft\WindowsApps\winget.exe");
            if p.exists() { Some(p) } else { None }
        });

    if let Some(winget) = winget_path {
        println!("[1/2] Installing official LLVM via Windows Package Manager (winget)...");
        let status = Command::new(winget)
            .args([
                "install",
                "--id", "LLVM.LLVM",
                "--exact",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ])
            .status();

        if let Ok(st) = status && st.success() {
            println!("\n[SUCCESS] LLVM installed successfully via winget!");
            verify_and_register_llvm_windows();
            return true;
        }
        println!("[Notice] winget install did not succeed; falling back to direct download.");
    }

    // 2. Direct download of official LLVM installer from GitHub Releases
    let version = "18.1.8";
    let installer_url = format!(
        "https://github.com/llvm/llvm-project/releases/download/llvmorg-{}/LLVM-{}-win64.exe",
        version, version
    );
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join(format!("LLVM-{}-win64.exe", version));

    println!("[1/2] Downloading official LLVM {} installer from GitHub...", version);
    println!("      Source: {}", installer_url);
    println!("      Target: {}", installer_path.display());

    // Try curl.exe first (native on Windows 10/11) with clean progress
    let curl_res = Command::new("curl.exe")
        .args([
            "-L",
            "--progress-bar",
            "-o",
            installer_path.to_str().unwrap_or("llvm.exe"),
            &installer_url,
        ])
        .status();

    let download_ok = if let Ok(c_st) = curl_res && c_st.success() && installer_path.exists() {
        true
    } else {
        println!("      Fallback to PowerShell download client...");
        let ps_cmd = format!(
            "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (New-Object Net.WebClient).DownloadFile('{}', '{}')",
            installer_url,
            installer_path.display()
        );
        let ps_res = Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_cmd])
            .status();
        ps_res.map(|s| s.success()).unwrap_or(false) && installer_path.exists()
    };

    if !download_ok {
        eprintln!("\n[ERROR] Failed to download LLVM installer.");
        eprintln!("Please download and install LLVM manually from:");
        eprintln!("  https://github.com/llvm/llvm-project/releases/tag/llvmorg-{}", version);
        return false;
    }

    println!("\n[2/2] Running LLVM installer...");
    println!("      Adding LLVM to system PATH...");

    // 1. Try elevated silent install via PowerShell Start-Process -Verb RunAs
    let ps_elevated = format!(
        "Start-Process -FilePath '{}' -ArgumentList '/S' -Verb RunAs -Wait",
        installer_path.display()
    );
    let elevated_res = Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_elevated])
        .status();

    let mut success = elevated_res.map(|s| s.success()).unwrap_or(false)
        && PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe").exists();

    if !success && !PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe").exists() {
        // 2. Fallback: Launch standard installation wizard
        println!("      Launching LLVM installation wizard...");
        let gui_res = Command::new(&installer_path).status();
        success = gui_res.map(|s| s.success()).unwrap_or(false)
            || PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe").exists();
    }

    let _ = std::fs::remove_file(&installer_path);

    if success || PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe").exists() {
        println!("\n[SUCCESS] LLVM installed successfully!");
        verify_and_register_llvm_windows();
        true
    } else {
        eprintln!("\n[ERROR] LLVM installer did not complete.");
        eprintln!("You can manually install LLVM via: winget install LLVM.LLVM");
        false
    }
}

#[cfg(windows)]
fn verify_and_register_llvm_windows() {
    let default_llvm_bin = PathBuf::from(r"C:\Program Files\LLVM\bin");
    if default_llvm_bin.exists() {
        if let Ok(path) = std::env::var("PATH") {
            let bin_str = default_llvm_bin.to_string_lossy();
            if !path.contains(&*bin_str) {
                let new_path = format!("{};{}", bin_str, path);
                unsafe {
                    std::env::set_var("PATH", new_path);
                }
            }
        }
        let reg_cmd = "$p = [Environment]::GetEnvironmentVariable('PATH', 'User'); if ($p -notlike '*C:\\Program Files\\LLVM\\bin*') { [Environment]::SetEnvironmentVariable('PATH', 'C:\\Program Files\\LLVM\\bin;' + $p, 'User') }";
        let _ = Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", reg_cmd])
            .status();
    }

    if let Some(clang) = find_clang() {
        println!("[OK] Clang verified at: {}", clang.display());
    } else if default_llvm_bin.join("clang.exe").exists() {
        println!("[OK] Clang found at: {}", default_llvm_bin.join("clang.exe").display());
    } else {
        println!("[Notice] LLVM was installed. Please restart your terminal if PATH was updated.");
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_llvm_linux() -> bool {
    println!("\n-> Detecting Linux package manager...");

    if which_local("apt-get").is_some() {
        println!("[1/2] Updating package index and installing clang, lld, llvm via apt...");
        let status = Command::new("sudo")
            .args(["apt-get", "update"])
            .status();
        if status.is_ok() {
            let inst = Command::new("sudo")
                .args(["apt-get", "install", "-y", "clang", "lld", "llvm"])
                .status();
            if let Ok(s) = inst && s.success() {
                println!("[SUCCESS] LLVM / Clang successfully installed via apt-get.");
                return true;
            }
        }
    } else if which_local("dnf").is_some() {
        println!("[1/2] Installing clang, lld, llvm via dnf...");
        let inst = Command::new("sudo")
            .args(["dnf", "install", "-y", "clang", "lld", "llvm"])
            .status();
        if let Ok(s) = inst && s.success() {
            println!("[SUCCESS] LLVM / Clang successfully installed via dnf.");
            return true;
        }
    } else if which_local("pacman").is_some() {
        println!("[1/2] Installing clang, lld, llvm via pacman...");
        let inst = Command::new("sudo")
            .args(["pacman", "-S", "--noconfirm", "clang", "lld", "llvm"])
            .status();
        if let Ok(s) = inst && s.success() {
            println!("[SUCCESS] LLVM / Clang successfully installed via pacman.");
            return true;
        }
    }

    eprintln!("\n[ERROR] Automated installation failed or unsupported package manager.");
    eprintln!("Please install Clang manually, e.g.: sudo apt-get install clang lld llvm");
    false
}

#[cfg(target_os = "macos")]
fn install_llvm_macos() -> bool {
    println!("\n-> Checking macOS development toolchain...");
    if which_local("brew").is_some() {
        println!("[1/2] Installing llvm via Homebrew...");
        let inst = Command::new("brew")
            .args(["install", "llvm"])
            .status();
        if let Ok(s) = inst && s.success() {
            println!("[SUCCESS] LLVM successfully installed via Homebrew.");
            return true;
        }
    } else {
        println!("[1/2] Running xcode-select --install...");
        let _ = Command::new("xcode-select").arg("--install").status();
    }
    eprintln!("Please ensure Clang is installed via 'brew install llvm' or Xcode Command Line Tools.");
    false
}

fn which_local(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for entry in std::env::split_paths(&path_var) {
        let candidate = entry.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
