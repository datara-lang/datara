#!/usr/bin/env bash
# msvc-env.sh - put MSVC's linker first on PATH.
#
# Source it, do not run it:
#
#     source scripts/msvc-env.sh
#     cargo test --release
#
# ## Why this exists
#
# On Windows, tests that produce a native binary (anything going through
# `ForgenCompiler::compile_project` with AOT linking, which is most of the
# bridge / capability / exec suites) fail under Git-Bash with:
#
#     Linking failed: status=ExitStatus(1)
#     linker: ...\PortableGit\...\usr\bin\link.exe
#     stderr: /usr/bin/link: extra operand '/DEBUG:NONE'
#
# That reads like a compiler bug and is not one. Git-Bash ships a GNU `link`
# (a coreutils hard-link utility) in /usr/bin, and it shadows MSVC's `link.exe`
# for any process that resolves the bare name `link` off PATH - which is what
# the linker driver does. The GNU tool is handed MSVC flags and rejects the
# first one it does not recognise.
#
# The fix is to prepend the real MSVC bin directory. It must be prepended, not
# appended: appending leaves Git's copy in front and changes nothing.
#
# A bare `cargo test` is therefore not a valid signal for those suites on
# Windows. If you see the message above, you have not sourced this file.
#
# ## Discovery
#
# Versions are discovered rather than hardcoded: a BuildTools update renames
# the `14.xx.xxxxx` directory, and a hardcoded path then fails confusingly. On
# this machine MSVC lives under
# `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools` - note the
# `(x86)` and the `18`, not the `Program Files\...\2022` path one would guess.

_msvc_env() {
  local vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
  local root=""

  if [ -x "$vswhere" ]; then
    # `-products '*'` is required; without it vswhere can return nothing even
    # when a BuildTools installation is present.
    root="$("$vswhere" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 \
            -property installationPath 2>/dev/null | tr -d '\r')"
  fi
  if [ -z "$root" ]; then
    for candidate in "/c/Program Files (x86)/Microsoft Visual Studio"/*/BuildTools \
                     "/c/Program Files/Microsoft Visual Studio"/*/*; do
      if [ -d "$candidate/VC/Tools/MSVC" ]; then root="$candidate"; break; fi
    done
  fi
  if [ -z "$root" ]; then
    echo "msvc-env: no Visual Studio installation found" >&2
    return 1
  fi

  # vswhere reports a Windows path. Git-Bash wants /c/... for its own tools, but
  # INCLUDE and LIB must stay Windows-shaped because link.exe reads them.
  local winroot="$root"
  local msvc_dir
  msvc_dir="$(ls -d "$(cygpath -u "$winroot" 2>/dev/null || echo "$winroot")"/VC/Tools/MSVC/* 2>/dev/null | sort -V | tail -1)"
  local msvc_ver
  msvc_ver="$(basename "$msvc_dir")"

  local sdk="/c/Program Files (x86)/Windows Kits/10"
  local sdk_ver
  # not `xargs basename`: the SDK path contains a space, and xargs would split it
  sdk_ver="$(ls -d "$sdk"/Include/* 2>/dev/null | sort -V | tail -1)"
  sdk_ver="${sdk_ver##*/}"

  local msvc_bin="$msvc_dir/bin/Hostx64/x64"
  if [ ! -d "$msvc_bin" ]; then
    echo "msvc-env: no x64 toolchain under $msvc_dir" >&2
    return 1
  fi

  export PATH="$msvc_bin:$PATH"
  export INCLUDE="$(cygpath -w "$msvc_dir/include" 2>/dev/null || echo "$msvc_dir/include");C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$sdk_ver\\ucrt;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$sdk_ver\\um;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$sdk_ver\\shared"
  export LIB="$(cygpath -w "$msvc_dir/lib/x64" 2>/dev/null || echo "$msvc_dir/lib/x64");C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\$sdk_ver\\ucrt\\x64;C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\$sdk_ver\\um\\x64"

  echo "msvc-env: MSVC $msvc_ver, Windows SDK $sdk_ver"
  echo "msvc-env: link -> $(command -v link.exe)"
}

_msvc_env
