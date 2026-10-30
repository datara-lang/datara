#!/usr/bin/env python3
"""PLAN_V151 N1: backend parity matrix — Cranelift vs JIT vs LLVM must
produce byte-identical stdout on a fixed probe set. Emits
evidence/parity_matrix.json."""
import subprocess, sys, os, json, glob

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FORGEN = os.path.join(ROOT, "target", "release", "forgen.exe")
PROBES = sorted(glob.glob(os.path.join(ROOT, "evidence", "probes", "*.dtr")))
TIERS = [(), ("--jit",), ("--llvm",)]

def run(path, extra):
    cmd = [FORGEN, "run", path, *extra]
    r = subprocess.run(cmd, capture_output=True, cwd=ROOT, timeout=120)
    return r.returncode, r.stdout.decode("utf-8", errors="replace").replace("\r\n", "\n").strip(), \
        r.stderr.decode("utf-8", errors="replace")

def main():
    results, fails = {}, 0
    for probe in PROBES:
        name = os.path.basename(probe)
        outs = {}
        for tier in TIERS:
            tier_name = tier[0].lstrip("-") if tier else "cranelift"
            try:
                code, out, err = run(probe, list(tier))
                # A backend failure that silently falls back to Cranelift
                # ("Falling back to native Cranelift backend") would make the
                # matrix diff Cranelift against Cranelift — a false OK. Treat
                # the fallback as a hard tier failure instead.
                if "Falling back" in err or "Falling back" in out:
                    code, out = -2, "<backend-fallback>"
                outs[tier_name] = {"exit": code, "stdout": out}
            except subprocess.TimeoutExpired:
                outs[tier_name] = {"exit": -1, "stdout": "<timeout>"}
        ref = outs["cranelift"]
        ok = all(v == ref for v in outs.values()) and ref["exit"] == 0
        results[name] = {"parity": ok, "tiers": outs}
        if not ok:
            fails += 1
        print(("OK   " if ok else "FAIL ") + name)
    os.makedirs(os.path.join(ROOT, "evidence"), exist_ok=True)
    out_path = os.path.join(ROOT, "evidence", "parity_matrix.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump({"probes": results, "fails": fails}, f, indent=2)
    print(f"\n{len(results) - fails}/{len(results)} parity; report: {out_path}")
    sys.exit(1 if fails else 0)

if __name__ == "__main__":
    main()
