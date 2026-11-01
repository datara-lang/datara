#!/usr/bin/env python3
"""bridge_smoke — run every foreign bridge once, print a status table.

Local-only (no CI dependency): python tools/bridge_smoke.py
Expects the forgen binary in target/release (or FORGEN_BIN)."""
import subprocess, sys, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FORGEN = os.environ.get("FORGEN_BIN", os.path.join(ROOT, "target", "release", "forgen.exe"))

PROBES = [
    ("python",
     'import python\n\nfn main() {\n    val py = Py { version: "3" }\n'
     '    out int_to_str(py.eval_int("21 * 2") or -1)\n}\n',
     "42"),
    ("c", None, None),          # import c requires a header file; covered by test_cimport
    ("js", None, None),         # covered by test_js_interop_and_fixes
    ("rust", None, None),       # covered by test_rust_bridge
]

def run(src, name):
    tmp = os.path.join(ROOT, ".bridge_smoke_tmp.dtr")
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(src)
    try:
        r = subprocess.run([FORGEN, "run", tmp],
                           capture_output=True, cwd=ROOT, timeout=120)
    finally:
        if os.path.exists(tmp):
            os.remove(tmp)
    out = r.stdout.decode("utf-8", "replace").strip()
    return out, r.returncode

def main():
    print(f"bridge smoke — {FORGEN}")
    rows = []
    for name, src, want in PROBES:
        if src is None:
            rows.append((name, "SKIP (suite-tested)"))
            continue
        try:
            out, code = run(src, name)
            status = "OK" if code == 0 and out == want else f"FAIL (got {out!r}, rc={code})"
        except Exception as e:
            status = f"FAIL ({e})"
        rows.append((name, status))
    width = max(len(n) for n, _ in rows)
    for name, status in rows:
        print(f"  {name:<{width}}  {status}")
    hard = [s for n, s in rows if s.startswith("FAIL")]
    print(f"\n{len(rows) - len(hard)}/{len(rows)} bridges OK")
    sys.exit(1 if hard else 0)

if __name__ == "__main__":
    main()
