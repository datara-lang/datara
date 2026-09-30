#!/usr/bin/env python3
"""M4 Large-Program Stability: synthetic Datara program generator.

Produces N modules x M functions with cross-module imports and deep call
chains, at an exact source-line count. Used for the 10k/50k/100k/200k stress
matrix (PLAN_V150 M4) and its CI regression gate.

Usage:
    python tools/gen_stress.py --out stress_app --lines 50000
    cd stress_app && forgen build
"""
import argparse
import os
import random
import shutil

HEADER = "# M4 stress module {mod_name} (generated, seed {seed})\n"


def gen_module(mod_idx, n_modules, fns_per_module, seed):
    rng = random.Random(seed + mod_idx)
    mod_name = f"mod{mod_idx:04d}"
    lines = [f"// {mod_name}", ""]
    # Always link to the next module so the project contains a deep call chain;
    # add up to two deterministic forward edges to vary the import graph.
    imported = {mod_idx + 1} if mod_idx + 1 < n_modules else set()
    candidate_start = mod_idx + 2
    candidate_count = max(0, n_modules - candidate_start)
    extra_count = rng.randint(0, min(2, candidate_count))
    imported.update(rng.sample(range(candidate_start, n_modules), extra_count))
    imported_calls = []
    for other in sorted(imported):
        imported_name = f"fn_{other:04d}_000"
        lines.append(f"use mod{other:04d}.{imported_name}")
        imported_calls.append(imported_name)
    lines.append("")

    # Keep each module's first function public so earlier modules can import it.
    # Only that entry function makes cross-module calls, producing a deterministic
    # acyclic call graph because imports always point to a later module.
    if fns_per_module < 1:
        raise ValueError("fns_per_module must be at least 1")

    fn_names = []
    for f in range(fns_per_module):
        fname = f"fn_{mod_idx:04d}_{f:03d}"
        fn_names.append(fname)
        a = rng.randint(1, 97)
        b = rng.randint(1, 89)
        visibility = "pub " if f == 0 else ""
        lines.append(f"{visibility}fn {fname}(x: Int) -> Int {{")
        body_style = f % 3
        if body_style == 0:
            lines.append(f"    mut local_result = x + {a}")
            lines.append("    mut k = 0")
            lines.append(f"    while k < {rng.randint(2, 9)} {{")
            lines.append(f"        local_result = local_result + k * {b}")
            lines.append("        k = k + 1")
            lines.append("    }")
        elif body_style == 1:
            lines.append(f"    val scaled = x * {a} - {b}")
            lines.append(f"    mut local_result = scaled + {a}")
            lines.append(f"    if scaled > {rng.randint(0, 500)} {{")
            lines.append("        local_result = scaled")
            lines.append("    }")
        else:
            lines.append(f"    val local_result = x + {a} - {b}")

        if f + 1 < fns_per_module:
            callee = f"fn_{mod_idx:04d}_{f + 1:03d}"
            lines.append(f"    return {callee}(local_result)")
        else:
            for call_idx, imported_name in enumerate(imported_calls):
                lines.append(f"    val cross_{call_idx} = {imported_name}(local_result)")
            result_expr = " + ".join(
                ["local_result", *(f"cross_{i}" for i in range(len(imported_calls)))]
            )
            lines.append(f"    return {result_expr}")
        lines.append("}")
        lines.append("")
    return mod_name, lines, fn_names


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--lines", type=int, required=True, help="exact total Datara source line count")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--fns-per-module", type=int, default=20)
    ap.add_argument(
        "--duplicate-names",
        action="store_true",
        help="add a duplicate top-level function to exercise duplicate-name diagnostics",
    )
    args = ap.parse_args()
    if args.lines < 1:
        ap.error("--lines must be positive")
    if args.fns_per_module < 1:
        ap.error("--fns-per-module must be at least 1")

    # Each function renders about eight lines; start with that estimate, then
    # count the actual generated source and pad comments to honor --lines exactly.
    per_module = args.fns_per_module * 8 + 14
    n_modules = max(2, args.lines // per_module)
    main_lines = [
        "// M4 stress entry point (generated)",
        "",
        "use mod0000.fn_0000_000",
        "",
        "fn main() {",
        "    out fn_0000_000(1)",
        "}",
    ]

    while True:
        module_lines = []
        for m in range(n_modules):
            mod_name, lines, _ = gen_module(m, n_modules, args.fns_per_module, args.seed)
            if args.duplicate_names and m == 0:
                lines.extend(["pub fn duplicate_probe(x: Int) -> Int => x", ""])
            if args.duplicate_names and m == 1:
                lines.extend(["pub fn duplicate_probe(x: Int) -> Int => x + 1", ""])
            module_lines.append((mod_name, lines))

        total = sum(
            len("\n".join(lines).splitlines()) for _, lines in module_lines
        ) + len("\n".join(main_lines).splitlines())
        if total <= args.lines or n_modules <= 2:
            break
        n_modules -= 1

    if total > args.lines:
        ap.error(
            f"--lines {args.lines} is below the minimum {total} lines for "
            f"{n_modules} modules with {args.fns_per_module} functions each; "
            "use --fns-per-module to lower the minimum"
        )

    padding = [
        f"// stress padding {index + 1}"
        for index in range(args.lines - total)
    ]
    # Preserve the terminal empty item used to keep generated files newline-ended.
    module_lines[-1][1][-1:-1] = padding
    total = args.lines

    os.makedirs(args.out, exist_ok=True)
    src = os.path.join(args.out, "src")
    if os.path.isdir(src):
        shutil.rmtree(src)
    os.makedirs(src, exist_ok=True)
    for mod_name, lines in module_lines:
        with open(os.path.join(src, mod_name + ".dtr"), "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines))
    with open(os.path.join(src, "main.dtr"), "w", encoding="utf-8") as fh:
        fh.write("\n".join(main_lines))

    manifest = (
        "manifest_schema = 1\n\n[package]\nname = \"stress-app\"\nversion = \"0.1.0\"\n"
        "entry = \"src/main.dtr\"\n"
    )
    with open(os.path.join(args.out, "datara.toml"), "w", encoding="utf-8") as fh:
        fh.write(manifest)
    print(f"generated {n_modules} modules, {n_modules * args.fns_per_module} functions, {total} lines in {args.out}")


if __name__ == "__main__":
    main()
