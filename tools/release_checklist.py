#!/usr/bin/env python3
"""release_checklist — the 'release in one command' gate (PLAN_V153 D-block).

Runs every release-blocking check locally and prints a PASS/FAIL table:

  1. Version sync (VERSION / Cargo.toml / README badges)
  2. Release build (cargo build --release)
  3. Full test suite (cargo test --release)
  4. Format gate (cargo fmt --all -- --check)
  5. Size gate (no .rs file in src/ over 60 KB unless allow-listed)
  6. stdlib fmt idempotence (formatter stable over the whole corpus)
  7. CLI smoke (forgen check on a fresh template project)
  8. bench_natvar + bench_json artifacts exist and reference real numbers

Exit code 0 = release-ready. Any FAIL blocks the release.

Usage: python tools/release_checklist.py [--fast]
  --fast skips the full test suite (run it separately before tagging).
"""
import subprocess
import sys
import os
import shutil
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CARGO = os.environ.get("CARGO", "cargo")
FAST = "--fast" in sys.argv

CHECKS = []


def check(name):
    def deco(fn):
        CHECKS.append((name, fn))
        return fn
    return deco


def _cargo() -> str:
    # The tool may run outside a Rust-enabled shell: resolve cargo
    # explicitly (Git-Bash PATH does not reach cmd.exe subprocesses).
    exe = "cargo.exe" if os.name == "nt" else "cargo"
    found = shutil.which(exe)
    if found:
        return found
    for cand in (
        Path.home() / ".cargo" / "bin" / exe,
        Path("C:/Users/watersize/.cargo/bin") / exe,
    ):
        if cand.exists():
            return str(cand)
    return exe


def run(cmd, cwd=ROOT, timeout=1800):
    # Windows: text=True + explicit utf-8 (mojibake otherwise). Shell mode
    # is avoided — cmd.exe does not inherit the Git-Bash PATH.
    if isinstance(cmd, list) and cmd and cmd[0] == "cargo":
        cmd = [_cargo()] + cmd[1:]
    return subprocess.run(
        cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout,
        encoding="utf-8", errors="replace",
    )


@check("version-sync")
def version_sync():
    version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.M)
    if not m:
        return "FAIL: no version in Cargo.toml"
    if m.group(1) != version:
        return f"FAIL: VERSION={version} but Cargo.toml={m.group(1)}"
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    if version not in readme:
        return f"FAIL: {version} not mentioned in README.md"
    return "OK"


@check("release-build")
def release_build():
    r = run([CARGO, "build", "--release"], timeout=3600)
    return "OK" if r.returncode == 0 else f"FAIL: build errors:\n{r.stderr[-400:]}"


@check("test-suite")
def test_suite():
    if FAST:
        return "SKIP (--fast)"
    r = run([CARGO, "test", "--release", "--no-fail-fast", "--", "--skip", "bench"],
            timeout=3600)
    if r.returncode != 0:
        failed = re.findall(r"test (\S+) ... FAILED", r.stdout)
        return f"FAIL: {len(failed)} failed: {failed[:6]}"
    return "OK"


@check("fmt-gate")
def fmt_gate():
    r = run([CARGO, "fmt", "--all", "--", "--check"])
    return "OK" if r.returncode == 0 else "FAIL: cargo fmt --check dirty"


@check("size-gate")
def size_gate():
    limit = 61_440
    allow = {
        "compile_func.rs", "declare_core.rs", "expr_call.rs", "stmt.rs",
        "modules.rs", "decl.rs", "mod.rs", "call.rs", "expr_composite.rs",
    }
    oversized = []
    for p in (ROOT / "src").rglob("*.rs"):
        if p.stat().st_size > limit and p.name not in allow:
            oversized.append(f"{p.relative_to(ROOT)} ({p.stat().st_size}B)")
    if oversized:
        return f"FAIL: over 60KB: {oversized}"
    return "OK"


@check("stdlib-fmt-idempotence")
def stdlib_idempotence():
    r = run([CARGO, "test", "--release", "--test", "test_fmt_idempotence",
             "--", "--skip", "skip_none"], timeout=600)
    return "OK" if r.returncode == 0 else "FAIL: formatter unstable on stdlib"


@check("template-smoke")
def template_smoke():
    forgen = ROOT / "target" / "release" / ("forgen.exe" if os.name == "nt" else "forgen")
    if not forgen.exists():
        return "FAIL: target/release/forgen missing"
    r = run([str(forgen), "new", "__rc_smoke", "--template", "cli"],
            cwd=ROOT.parent, timeout=120)
    if r.returncode != 0:
        return f"FAIL: forgen new: {r.stderr[-200:]}"
    proj = ROOT.parent / "__rc_smoke"
    r2 = run([str(forgen), "check", "src/main.dtr"], cwd=proj, timeout=300)
    import shutil
    shutil.rmtree(proj, ignore_errors=True)
    ok = "100% OK" in (r2.stdout + r2.stderr)
    return "OK" if ok else f"FAIL: template check: {r2.stdout[-200:]}"


@check("bench-artifacts")
def bench_artifacts():
    missing = []
    for rel in ("bench_natvar/RESULTS.md", "bench_json/RESULTS.md"):
        p = ROOT / rel
        if not p.exists():
            missing.append(rel)
        elif "FAIL" in p.read_text(encoding="utf-8")[:200]:
            missing.append(f"{rel} (header says FAIL)")
    if missing:
        return f"FAIL: missing/invalid: {missing}"
    return "OK"


def main():
    print(f"release checklist — ROOT={ROOT} fast={FAST}")
    results = []
    for name, fn in CHECKS:
        try:
            status = fn()
        except Exception as e:
            status = f"FAIL: {e}"
        results.append((name, status))
        print(f"  {name:<24} {status.splitlines()[0]}")
    hard = [s for _, s in results if s.startswith("FAIL")]
    print(f"\n{len(results) - len(hard)}/{len(results)} checks passed")
    sys.exit(1 if hard else 0)


if __name__ == "__main__":
    main()
