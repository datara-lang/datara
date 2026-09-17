# GitHub Release & Deployment Guide

This guide describes the standard, repeatable procedure for publishing official production releases of the Datara programming language and Forgen compiler toolchain to GitHub.

---

## 1. Prerequisites

- **Rust toolchain**: 1.94.0+ (stable).
- **Git**: configured with access to https://github.com/datara-lang/datara.git.
- **PowerShell**: with execution permissions.
- **Operating System**: Windows 10/11 x86_64 for native installer generation.

---

## 2. Release Preparation & Automated Packaging

1. Ensure all integration test suites pass:
   `ash
   cargo test --test test_bridge_decls
   cargo test --test test_capabilities2
   cargo test --test test_dpm_bridge
   cargo test --test test_socket_timeout
   cargo test --test test_fast_input
   cargo test --test test_py_batch
   cargo test --test test_outcome
   `

2. Update version in Cargo.toml:
   `	oml
   [package]
   version = "1.4.2"
   `

3. Run the automated release packager:
   `powershell
   powershell -ExecutionPolicy Bypass -File scripts/package_release.ps1 -Version 1.4.2
   `

   This compiles optimized release binaries, stages packages in dist/, builds Datara-Setup.exe, and computes SHA-256 sums in dist/SHA256SUMS.txt.

4. Sync fresh SHA-256 hashes with package manager manifests:
   - packaging/scoop/datara.json (Windows Scoop)
   - packaging/winget/waters1ze.Datara.yaml (Windows Package Manager)
   - packaging/homebrew/Formula/datara.rb (macOS / Linux Homebrew)
   - packaging/aur/PKGBUILD (Arch Linux AUR)

---

## 3. Git Commit, Tagging, and Push

Execute the following commands from the repository root:

`ash
# 1. Stage all changes
git add -A

# 2. Commit release changes
git commit -m "release: v1.4.2 - bridge hub, capabilities 2.0, dpm registry, socket and fast input ergonomics"

# 3. Create version tag
git tag v1.4.2

# 4. Push code and tag to GitHub
git push origin main
git push origin v1.4.2
`

---

## 4. Publishing the GitHub Release

1. Navigate to the repository releases page:
   https://github.com/datara-lang/datara/releases/new

2. Select existing tag: 1.4.2.
3. Release title: 1.4.2 - Bridge Hub, Capabilities 2.0, DPM Registry, Socket & Fast Input Ergonomics.
4. Copy the release notes from CHANGELOG.md under ## [1.4.2].
5. Attach distribution artifacts from the dist/ folder:
   - Datara-Setup.exe (or Datara-v1.4.2-Setup.exe)
   - orgen-windows-x64.zip (or orgen-v1.4.2-windows-x64.zip)
   - orgen-linux-x64.zip (or orgen-v1.4.2-linux-x64.zip)
   - orgen-darwin-arm64.zip (or orgen-v1.4.2-darwin-arm64.zip)
   - SHA256SUMS.txt
6. Click **Publish release**.

---

## 5. Verification Checklist

- [x] Zero unicode emojis in code, comments, commit messages, and docs.
- [x] All Rust source files in src/ remain strictly below 61,440 bytes.
- [x] SHA-256 checksums verified against binary outputs.
- [x] Git branch main and tag 1.4.2 pushed to remote repository.
