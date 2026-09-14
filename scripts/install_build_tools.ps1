<#
.SYNOPSIS
    Datara & Forgen - Automatic Native C/C++ Toolchain & LLVM Setup for Windows
.DESCRIPTION
    Detects if a C/C++ linker (MSVC link.exe, LLVM lld-link, or MinGW gcc) or LLVM
    toolchain (clang.exe, llc.exe) is installed.
    If missing, automatically installs LLVM or Microsoft Visual Studio C++ Build Tools
    via winget or direct official release download (Node.js style).
#>

param(
    [switch]$LLVM,
    [switch]$BuildTools,
    [switch]$All,
    [switch]$Auto
)

$ErrorActionPreference = "Continue"

Write-Host "========================================================================" -ForegroundColor Cyan
Write-Host "   ____        _                                                        " -ForegroundColor Cyan
Write-Host "  |  _ \  __ _| |_ __ _ _ __ __ _    Datara Systems Language            " -ForegroundColor Cyan
Write-Host "  | | | |/ _` | __/ _` | '__/ _` |   Native Toolchain & Linker Setup    " -ForegroundColor Cyan
Write-Host "  | |_| | (_| | || (_| | | | (_| |   https://github.com/datara-lang/datara" -ForegroundColor Cyan
Write-Host "  |____/ \__,_|\__\__,_|_|  \__,_|                                      " -ForegroundColor Cyan
Write-Host "========================================================================" -ForegroundColor Cyan
Write-Host ""

function Register-LLVMPath {
    $llvmBin = "C:\Program Files\LLVM\bin"
    if (Test-Path $llvmBin) {
        $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($userPath -notlike "*$llvmBin*") {
            [Environment]::SetEnvironmentVariable("PATH", "$llvmBin;$userPath", "User")
            $env:PATH = "$llvmBin;$env:PATH"
            Write-Host "  [OK] Added $llvmBin to User PATH environment variable." -ForegroundColor Green
        }
    }
}

function Test-LLVM {
    $clangCmd = Get-Command clang.exe -ErrorAction SilentlyContinue
    if ($clangCmd) { return @{ Name = "LLVM Clang"; Path = $clangCmd.Source } }
    if (Test-Path "C:\Program Files\LLVM\bin\clang.exe") {
        return @{ Name = "LLVM Clang"; Path = "C:\Program Files\LLVM\bin\clang.exe" }
    }
    return $null
}

function Test-RealLinker {
    # 1. Check for lld-link.exe or clang.exe (LLVM)
    $lldCmd = Get-Command lld-link.exe -ErrorAction SilentlyContinue
    if ($lldCmd) { return @{ Name = "LLVM LLD Linker"; Path = $lldCmd.Source } }
    if (Test-Path "C:\Program Files\LLVM\bin\lld-link.exe") {
        return @{ Name = "LLVM LLD Linker"; Path = "C:\Program Files\LLVM\bin\lld-link.exe" }
    }

    $clangCmd = Get-Command clang.exe -ErrorAction SilentlyContinue
    if ($clangCmd) { return @{ Name = "Clang Linker"; Path = $clangCmd.Source } }
    if (Test-Path "C:\Program Files\LLVM\bin\clang.exe") {
        return @{ Name = "Clang Linker"; Path = "C:\Program Files\LLVM\bin\clang.exe" }
    }

    # 2. Check vswhere for MSVC link.exe
    $pf = ${env:ProgramFiles(x86)}
    if (-not $pf) { $pf = $env:ProgramFiles }
    if (-not $pf) { $pf = "C:\Program Files (x86)" }
    $vswhere = Join-Path $pf "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        try {
            $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
            if ($vsPath -and (Test-Path $vsPath)) {
                $msvcDir = Join-Path $vsPath "VC\Tools\MSVC"
                if (Test-Path $msvcDir) {
                    $newest = Get-ChildItem -Path $msvcDir -Directory | Sort-Object Name -Descending | Select-Object -First 1
                    if ($newest) {
                        $linkCandidates = @(
                            (Join-Path $newest.FullName "bin\Hostx64\x64\link.exe"),
                            (Join-Path $newest.FullName "bin\Hostx86\x64\link.exe"),
                            (Join-Path $newest.FullName "bin\Hostx64\x86\link.exe"),
                            (Join-Path $newest.FullName "bin\link.exe")
                        )
                        foreach ($c in $linkCandidates) {
                            if (Test-Path $c) { return @{ Name = "MSVC Linker"; Path = $c } }
                        }
                    }
                }
            }
        } catch {}
    }

    # 3. Check for real link.exe in PATH (ignoring Git coreutils link.exe)
    $allLinks = Get-Command link.exe -All -ErrorAction SilentlyContinue
    foreach ($cmd in $allLinks) {
        $src = $cmd.Source.ToLower()
        if (-not ($src.Contains("git\usr\bin") -or $src.Contains("git/usr/bin"))) {
            return @{ Name = "MSVC Linker (PATH)"; Path = $cmd.Source }
        }
    }

    # 4. Check for MinGW gcc
    $gccCmd = Get-Command gcc.exe -ErrorAction SilentlyContinue
    if ($gccCmd) { return @{ Name = "MinGW GCC Linker"; Path = $gccCmd.Source } }

    return $null
}

function Install-LLVMToolchain {
    Write-Host "-> [LLVM] Installing LLVM / Clang compiler toolchain..." -ForegroundColor Cyan
    
    # 1. Try winget
    $wingetCmd = Get-Command winget.exe -ErrorAction SilentlyContinue
    if (-not $wingetCmd) {
        $localWinget = "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe"
        if (Test-Path $localWinget) { $wingetCmd = $localWinget }
    }
    if ($wingetCmd) {
        Write-Host "   Installing LLVM.LLVM package via winget..." -ForegroundColor Gray
        try {
            $proc = Start-Process -FilePath "winget" -ArgumentList @(
                "install", "--id", "LLVM.LLVM", "--exact",
                "--accept-package-agreements", "--accept-source-agreements"
            ) -PassThru -Wait -NoNewWindow
            if ($proc.ExitCode -eq 0 -or (Test-Path "C:\Program Files\LLVM\bin\clang.exe")) {
                Register-LLVMPath
                return $true
            }
        } catch {}
    }

    # 2. Direct download
    Write-Host "   Downloading official LLVM 18.1.8 installer from GitHub..." -ForegroundColor Gray
    $llvmUrl = "https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/LLVM-18.1.8-win64.exe"
    $tmpExe = Join-Path $env:TEMP "LLVM-Setup.exe"
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Write-Host "   Source: $llvmUrl" -ForegroundColor Gray
        Invoke-WebRequest -Uri $llvmUrl -OutFile $tmpExe -UseBasicParsing
        Write-Host "   Running LLVM silent setup..." -ForegroundColor Green
        $proc = Start-Process -FilePath $tmpExe -ArgumentList "/S" -Verb RunAs -PassThru -Wait
        if ($proc.ExitCode -eq 0 -or (Test-Path "C:\Program Files\LLVM\bin\clang.exe")) {
            Register-LLVMPath
            Remove-Item $tmpExe -Force -ErrorAction SilentlyContinue
            return $true
        }
    } catch {
        Write-Host "   [WARN] LLVM installation encountered an error: $_" -ForegroundColor Yellow
    }
    Remove-Item $tmpExe -Force -ErrorAction SilentlyContinue
    return $false
}

Write-Host "-> Inspecting system for existing C/C++ linker and LLVM toolchain..." -ForegroundColor Yellow
$foundLinker = Test-RealLinker
$foundLLVM = Test-LLVM

if ($LLVM -or ($All -and -not $foundLLVM)) {
    if ($foundLLVM) {
        Write-Host "  [OK] $($foundLLVM.Name) detected at: $($foundLLVM.Path)" -ForegroundColor Green
    } else {
        $res = Install-LLVMToolchain
        if ($res) {
            Write-Host "  [SUCCESS] LLVM toolchain is now installed and configured!" -ForegroundColor Green
        }
    }
    if (-not $All -and -not $BuildTools) { exit 0 }
}

if ($foundLinker) {
    Write-Host "`n  [OK] $($foundLinker.Name) detected at:" -ForegroundColor Green
    Write-Host "       $($foundLinker.Path)" -ForegroundColor White
    if ($foundLLVM) {
        Write-Host "  [OK] $($foundLLVM.Name) detected at:" -ForegroundColor Green
        Write-Host "       $($foundLLVM.Path)" -ForegroundColor White
    }
    Write-Host "`nDatara can build native executables immediately." -ForegroundColor Green
    Write-Host "No further toolchain installation required." -ForegroundColor Gray
    exit 0
}

Write-Host "`n  [!] No C/C++ linker or LLVM found on this system." -ForegroundColor Yellow
Write-Host "      Datara requires a C/C++ linker or LLVM to produce native Windows (.exe) executables." -ForegroundColor Gray
Write-Host "      We will now install LLVM (compact, fast ~300MB) or Microsoft C++ Build Tools.`n" -ForegroundColor White

# Default to LLVM (much faster, includes lld-link, clang, and full LLVM backend)
$installSucceeded = Install-LLVMToolchain

if (-not $installSucceeded) {
    Write-Host "-> Fallback: Installing Microsoft Visual Studio C++ Build Tools..." -ForegroundColor Cyan
    $bootstrapperUrl = "https://aka.ms/vs/17/release/vs_buildtools.exe"
    $tmpExe = Join-Path $env:TEMP "vs_buildtools.exe"
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $bootstrapperUrl -OutFile $tmpExe -UseBasicParsing
        $proc = Start-Process -FilePath $tmpExe -ArgumentList @(
            "--passive", "--wait", "--norestart",
            "--add", "Microsoft.VisualStudio.Workload.VCTools",
            "--includeRecommended"
        ) -PassThru -Wait -NoNewWindow
        if ($proc.ExitCode -eq 0 -or $proc.ExitCode -eq 3010) {
            $installSucceeded = $true
        }
    } catch {
        Write-Host "  [ERROR] Build Tools installation failed: $_" -ForegroundColor Red
    }
}

Write-Host "`n-> Verifying installed toolchain..." -ForegroundColor Yellow
$verify = Test-RealLinker
if ($verify) {
    Write-Host "`n========================================================================" -ForegroundColor Green
    Write-Host " [SUCCESS] $($verify.Name) is now configured!" -ForegroundColor Green
    Write-Host " Location: $($verify.Path)" -ForegroundColor White
    Write-Host " You can now run:" -ForegroundColor Cyan
    Write-Host "   datara                   (Interactive Console REPL)" -ForegroundColor White
    Write-Host "   forgen run main.dtr      (Compile & run native program)" -ForegroundColor White
    Write-Host "   forgen build main.dtr    (Generate standalone .exe)" -ForegroundColor White
    Write-Host "   forgen build --llvm      (Generate max-optimized .exe)" -ForegroundColor White
    Write-Host "========================================================================" -ForegroundColor Green
    exit 0
} else {
    Write-Host "`n[NOTICE] Installation completed. A system restart or terminal restart may be required" -ForegroundColor Yellow
    Write-Host "         for PATH environment variables to take effect." -ForegroundColor Yellow
    exit 0
}
