#!/usr/bin/env python3
"""PLAN_V153 B3: determinism matrix — every example must produce
byte-identical stdout on Cranelift, JIT, and LLVM (examples whose
runtime requires interactive input or net IO are skipped by a
SKIP list). Emits evidence/determinism_matrix.json."""
import subprocess, sys, os, json, glob

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FORGEN = os.path.join(ROOT, "target", "release", "forgen.exe")
TIERS = [(), ("--jit",), ("--llvm",)]

# Examples that need stdin/net/terminal or loop forever by design.
SKIP = {
    "minihttp", "snake", "repl", "shell", "server", "client",
    "pipeline_demo",  # needs fs manifest cwd; covered by its own golden test
    # Polyglot bridges are tier-specific by design (python/zig/csharp
    # runtimes); the LLVM tier's "Falling back" on these is the llvm_gate
    # working, not a parity failure.
    "polyglot", "zig", "csharp",
}

def example_name(p):
    return Path(p).stem

from pathlib import Path

def run(path, extra):
    cmd = [FORGEN, "run", path, *extra]
    r = subprocess.run(cmd, capture_output=True, cwd=ROOT, timeout=60)
    return (r.returncode,
            r.stdout.decode("utf-8", errors="replace").replace("\r\n", "\n").strip(),
            r.stderr.decode("utf-8", errors="replace"))

def main():
    results, fails, tested, skipped = {}, 0, 0, 0
    probes = sorted(glob.glob(os.path.join(ROOT, "examples", "*.dtr")))
    for probe in probes:
        name = example_name(probe)
        if any(s in name.lower() for s in SKIP):
            skipped += 1
            continue
        outs = {}
        crash = False
        for tier in TIERS:
            tier_name = tier[0].lstrip("-") if tier else "cranelift"
            try:
                code, out, err = run(probe, list(tier))
                if "Falling back" in err or "Falling back" in out:
                    code, out, crash = -2, "<backend-fallback>", True
                outs[tier_name] = {"exit": code, "stdout": out}
            except subprocess.TimeoutExpired:
                outs[tier_name] = {"exit": -1, "stdout": "<timeout>"}
        ref = outs["cranelift"]
        ok = all(v == ref for v in outs.values()) and ref["exit"] == 0
        if not ok:
            fails += 1
        else:
            tested += 1
        results[name] = {"parity": ok, "tiers": outs}
    report = {"tested": tested, "failed": fails, "skipped": skipped,
              "examples": results}
    out_path = os.path.join(ROOT, "evidence", "determinism_matrix.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, ensure_ascii=False)
    for name, r in results.items():
        status = "OK  " if r["parity"] else "FAIL"
        print(f"{status} {name}")
    print(f"\n{tested} tested, {fails} failed, {skipped} skipped; "
          f"report: {out_path}")
    sys.exit(1 if fails else 0)

if __name__ == "__main__":
    main()
