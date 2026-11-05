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
    # Real C probe (PLAN_V152 N5: real runs, not SKIPs): the test_cimport
    # fixture header + kernel32 — a real FFI call into an OS library.
    ("c",
     'import c "tests/fixtures/test_cimport.h" with link("kernel32.lib");\n\n'
     'fn main() {\n'
     '    unsafe(justification: "bridge smoke: OS FFI via kernel32") {\n'
     '        val pid = GetCurrentProcessId()\n'
     '        out "C_OK"\n'
     '    }\n'
     '}\n',
     "C_OK"),
    ("js",
     'fn main() {\n'
     '    val i = js_eval_int("10 + 25")\n'
     '    out int_to_str(i)\n'
     '}\n',
     "35"),
    ("rust",
     'use rust.serde as serde\n\nfn main() {\n    out "RUST_OK"\n}\n',
     "RUST_OK"),
]

def run(src, name, aot=False):
    tmp = os.path.join(ROOT, ".bridge_smoke_tmp.dtr")
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(src)
    try:
        if aot:
            # C imports need real linking (kernel32 etc.) — the JIT host's
            # symbol lookup only sees the runtime, so build AOT and execute.
            # forgen strips a leading dot from the output stem:
            # .bridge_smoke_tmp.dtr -> bridge_smoke_tmp.exe
            exe = os.path.join(ROOT, "bridge_smoke_tmp.exe")
            if os.path.exists(exe):
                os.remove(exe)
            b = subprocess.run([FORGEN, "build", tmp],
                               capture_output=True, cwd=ROOT, timeout=180)
            if b.returncode != 0 or not os.path.exists(exe):
                return ("BUILD_FAIL: " + b.stderr.decode("utf-8", "replace")[-200:], b.returncode)
            try:
                r = subprocess.run([exe], capture_output=True, cwd=ROOT, timeout=60)
            finally:
                for junk in (exe, exe.replace(".exe", ".obj")):
                    if os.path.exists(junk):
                        os.remove(junk)
            out = r.stdout.decode("utf-8", "replace").strip()
            return out, r.returncode
        r = subprocess.run([FORGEN, "run", tmp],
                           capture_output=True, cwd=ROOT, timeout=120)
    finally:
        if os.path.exists(tmp):
            os.remove(tmp)
    out = r.stdout.decode("utf-8", "replace").strip()
    # Module-binding banner lines (e.g. "[Forgen FFI] Successfully bound
    # Rust crate ...") go to stdout; the program result is the LAST line.
    last = out.splitlines()[-1].strip() if out.strip() else ""
    return (last if last else out), r.returncode

def main():
    print(f"bridge smoke — {FORGEN}")
    rows = []
    for name, src, want in PROBES:
        if src is None:
            rows.append((name, "SKIP (suite-tested)"))
            continue
        try:
            out, code = run(src, name, aot=(name == "c"))
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
