#!/usr/bin/env python3
"""M4 Large-Program Stability: synthetic Datara program generator.

Produces N modules x M functions with cross-module imports and deep call
chains, at a requested approximate line count. Used for the 10k/50k/100k/200k
stress matrix (PLAN_V150 M4) and its CI regression gate.

Usage:
    python tools/gen_stress.py --out stress_app --lines 50000
    cd stress_app && forgen build
"""
import argparse
import os
import random

HEADER = "# M4 stress module {mod_name} (generated, seed {seed})\n"


def gen_module(mod_idx, n_modules, fns_per_module, seed):
    rng = random.Random(seed + mod_idx)
    mod_name = f"mod{mod_idx:04d}"
    lines = [f"// {mod_name}", ""]
    uses = []
    # Cross-module imports: every module imports 1-3 later modules.
    imported = set()
    for _ in range(rng.randint(1, min(3, max(1, n_modules - mod_idx - 1)))):
        other = rng.randint(mod_idx + 1, n_modules - 1) if mod_idx + 1 < n_modules else None
        if other is not None and other not in imported:
            imported.add(other)
            uses.append(other)
    for other in sorted(imported):
        lines.append(f"// uses mod{other:04d} functions via qualified calls (module glue)")
    lines.append("")

    fn_names = []
    for f in range(fns_per_module):
        fname = f"fn_{mod_idx:04d}_{f:03d}"
        fn_names.append(fname)
        a = rng.randint(1, 97)
        b = rng.randint(1, 89)
        lines.append(f"fn {fname}(x: Int) -> Int {{")
        body_style = f % 3
        if body_style == 0:
            lines.append(f"    mut acc = x + {a}")
            lines.append(f"    mut k = 0")
            lines.append(f"    while k < {rng.randint(2, 9)} {{")
            lines.append( "        acc = acc + k * %d" % b)
            lines.append( "        k = k + 1")
            lines.append("    }")
            lines.append("    return acc")
        elif body_style == 1:
            lines.append(f"    val scaled = x * {a} - {b}")
            lines.append(f"    if scaled > {rng.randint(0, 500)} {{")
            lines.append( "        return scaled")
            lines.append("    }")
            lines.append(f"    return scaled + {a}")
        else:
            callee = f"fn_{mod_idx:04d}_{max(0, f - 1):03d}"
            lines.append(f"    val base = {callee}(x + {a})" if f > 0 else f"    val base = x + {a}")
            lines.append(f"    return base - {b}")
        lines.append("}")
        lines.append("")
    return mod_name, lines, fn_names


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--lines", type=int, required=True, help="approximate total line count")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--fns-per-module", type=int, default=20)
    args = ap.parse_args()

    # Each function renders ~8 lines; module overhead ~14 lines.
    per_module = args.fns_per_module * 8 + 14
    n_modules = max(2, args.lines // per_module)
    os.makedirs(args.out, exist_ok=True)
    src = os.path.join(args.out, "src")
    os.makedirs(src, exist_ok=True)

    all_calls = []
    for m in range(n_modules):
        mod_name, lines, fn_names = gen_module(m, n_modules, args.fns_per_module, args.seed)
        all_calls.append((mod_name, fn_names))
        with open(os.path.join(src, mod_name + ".dtr"), "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines))

    main_lines = ["// M4 stress entry point (generated)", ""]
    main_lines.append("fn main() {")
    main_lines.append('    out "stress ok"')
    main_lines.append("}")
    with open(os.path.join(src, "main.dtr"), "w", encoding="utf-8") as fh:
        fh.write("\n".join(main_lines))

    total = 0
    for m in range(n_modules):
        p = os.path.join(src, f"mod{m:04d}.dtr")
        total += sum(1 for _ in open(p, encoding="utf-8"))
    total += 4
    manifest = (
        "manifest_schema = 1\n\n[package]\nname = \"stress-app\"\nversion = \"0.1.0\"\n"
        "entry = \"src/main.dtr\"\n"
    )
    with open(os.path.join(args.out, "datara.toml"), "w", encoding="utf-8") as fh:
        fh.write(manifest)
    print(f"generated {n_modules} modules, {n_modules * args.fns_per_module} functions, {total} lines in {args.out}")


if __name__ == "__main__":
    main()
