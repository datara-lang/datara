#!/usr/bin/env python3
"""Benchmark forgen check on reproducible M4 stress projects.

Example:
    python tools/bench_stress.py --sizes 10000 50000 --output m4-results.json
"""
import argparse
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time

try:
    import psutil
except ImportError:  # RSS reporting is optional outside the CI stress gate.
    psutil = None


REPO_ROOT = Path(__file__).resolve().parents[1]
GENERATOR = Path(__file__).with_name("gen_stress.py")
PHASE_RE = re.compile(
    r"\[phases\] so-far total=(\d+)ms parse=(\d+)ms resolve=(\d+)ms typecheck=(\d+)ms"
)
LINT_RE = re.compile(r"\[phases2\] merged lint done at (\d+)ms")
TIMELINE_RE = re.compile(r"\[phases2\] ([a-zA-Z0-9_ +.-]+) done at (\d+)ms")
OWNERSHIP_SLOW_RE = re.compile(r"\[ownership\] slow fn: (.+?) = (\d+)us")
OPTIMIZER_TIMING_RE = re.compile(r"\[optimizer-timing\] ([a-zA-Z0-9_ +.-]+) = (\d+)ms")


def default_forgen():
    # Prefer the compiler built from this checkout over a globally installed one.
    names = ("forgen.exe", "forgen")
    for name in names:
        candidate = REPO_ROOT / "target" / "release" / name
        if candidate.is_file():
            return str(candidate)
    return shutil.which("forgen")


def count_source_lines(src_dir):
    return sum(
        len(path.read_text(encoding="utf-8").splitlines())
        for path in src_dir.glob("*.dtr")
    )


def result_for_size(report, size):
    return next(
        (result for result in report.get("results", []) if result.get("source_lines") == size),
        None,
    )


def regression_ratio(current, baseline, size):
    for key in ("seed", "fns_per_module", "platform", "machine", "backend_verification_required"):
        if current.get(key) != baseline.get(key):
            raise ValueError(f"baseline {key} does not match current run")
    current_result = result_for_size(current, size)
    baseline_result = result_for_size(baseline, size)
    if current_result is None or baseline_result is None:
        raise ValueError(f"both reports must contain the {size}-line workload")
    baseline_seconds = baseline_result.get("check_seconds")
    current_seconds = current_result.get("check_seconds")
    if not isinstance(baseline_seconds, (int, float)) or not math.isfinite(baseline_seconds) or baseline_seconds <= 0:
        raise ValueError("baseline check_seconds must be positive and finite")
    if not isinstance(current_seconds, (int, float)) or not math.isfinite(current_seconds) or current_seconds < 0:
        raise ValueError("current check_seconds must be non-negative and finite")
    return round(current_seconds / baseline_seconds, 3)


def parse_phase_trace(combined_output):
    phase_matches = list(PHASE_RE.finditer(combined_output))
    phases = None
    if phase_matches:
        match = phase_matches[-1]
        phases = {
            "pipeline_total_ms": int(match.group(1)),
            "parse_ms": int(match.group(2)),
            "resolve_ms": int(match.group(3)),
            "typecheck_ms": int(match.group(4)),
        }

    timeline = [
        {"phase": match.group(1).strip(), "elapsed_ms": int(match.group(2))}
        for match in TIMELINE_RE.finditer(combined_output)
        if match.group(1).strip() != "merged lint"
    ]
    previous_elapsed = int(phase_matches[-1].group(1)) if phase_matches else 0
    for item in timeline:
        item["duration_ms"] = max(0, item["elapsed_ms"] - previous_elapsed)
        previous_elapsed = item["elapsed_ms"]

    lint_matches = list(LINT_RE.finditer(combined_output))
    if lint_matches:
        if phases is None:
            phases = {}
        phases["merged_lint_elapsed_ms"] = int(lint_matches[-1].group(1))

    backend_started = "[phases2] pre-backend-verify" in combined_output
    backend_finished = "[phases2] backend-verify done" in combined_output
    if phases is None:
        phases = {}
    phases["backend_verified"] = backend_started and backend_finished
    return phases, timeline


def parse_optimizer_trace(combined_output):
    return [
        {"pass": match.group(1).strip(), "elapsed_ms": int(match.group(2))}
        for match in OPTIMIZER_TIMING_RE.finditer(combined_output)
    ]


def current_tree_rss_bytes(process):
    if psutil is None:
        return None
    try:
        processes = [process, *process.children(recursive=True)]
    except psutil.Error:
        processes = [process]
    rss = 0
    for child in processes:
        try:
            rss += child.memory_info().rss
        except psutil.Error:
            continue
    return rss or None


def benchmark_size(forgen, lines, seed, fns_per_module, timeout_seconds):
    with tempfile.TemporaryDirectory(prefix=f"forgen-m4-{lines}-") as temp_dir:
        output_dir = Path(temp_dir) / "stress_app"
        generated = subprocess.run(
            [
                sys.executable,
                str(GENERATOR),
                "--out",
                str(output_dir),
                "--lines",
                str(lines),
                "--seed",
                str(seed),
                "--fns-per-module",
                str(fns_per_module),
            ],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
        if generated.returncode != 0:
            raise RuntimeError(
                f"generator failed for {lines} lines:\n{generated.stdout}\n{generated.stderr}"
            )

        actual_lines = count_source_lines(output_dir / "src")
        if actual_lines != lines:
            raise RuntimeError(
                f"generator produced {actual_lines} source lines, expected {lines}"
            )

        env = os.environ.copy()
        env["DATARA_PHASE_DEBUG"] = "1"
        env["DATARA_OPTIMIZER_TIMING"] = "1"
        env["FORGEN_CHECK_VERIFY_BACKEND"] = "1"
        start = time.perf_counter()
        process = subprocess.Popen(
            [forgen, "check", str(output_dir)],
            cwd=REPO_ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
        monitor = psutil.Process(process.pid) if psutil is not None else None
        peak_rss_bytes = current_tree_rss_bytes(monitor) if monitor is not None else None
        deadline = start + timeout_seconds
        try:
            while True:
                remaining = deadline - time.perf_counter()
                if remaining <= 0:
                    process.kill()
                    process.communicate()
                    raise RuntimeError(f"forgen check timed out at {lines} lines")
                try:
                    stdout, stderr = process.communicate(timeout=min(0.1, remaining))
                    break
                except subprocess.TimeoutExpired:
                    if monitor is not None:
                        sample = current_tree_rss_bytes(monitor)
                        if sample is not None:
                            peak_rss_bytes = max(peak_rss_bytes or 0, sample)
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()
        elapsed_seconds = time.perf_counter() - start
        combined_output = f"{stdout}\n{stderr}"
        if process.returncode != 0:
            raise RuntimeError(
                f"forgen check failed at {lines} lines (exit {process.returncode}):\n"
                f"{combined_output}"
            )

        phases, timeline = parse_phase_trace(combined_output)
        if not phases or not phases.get("backend_verified"):
            raise RuntimeError(
                f"backend verification did not run at {lines} lines:\n{combined_output}"
            )
        ownership_hotspots = [
            {"function": match.group(1), "elapsed_us": int(match.group(2))}
            for match in OWNERSHIP_SLOW_RE.finditer(combined_output)
        ]
        return {
            "source_lines": actual_lines,
            "modules": len(list((output_dir / "src").glob("mod*.dtr"))),
            "functions": len(list((output_dir / "src").glob("mod*.dtr"))) * fns_per_module,
            "check_seconds": round(elapsed_seconds, 3),
            "peak_rss_mb": round(peak_rss_bytes / (1024 * 1024), 1)
            if peak_rss_bytes is not None
            else None,
            "phases": phases,
            "debug_trace": [
                line.strip()
                for line in combined_output.splitlines()
                if line.strip().startswith("[phases")
            ],
            "phase_timeline": timeline,
            "optimizer_timings": parse_optimizer_trace(combined_output),
            "ownership_hotspots": ownership_hotspots,
            "check_summary": stdout.strip().splitlines()[-1]
            if stdout.strip()
            else "",
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--forgen", default=default_forgen(), help="forgen executable path")
    parser.add_argument(
        "--sizes",
        nargs="+",
        type=int,
        default=[10_000, 50_000, 100_000, 200_000],
        help="exact source line counts to check",
    )
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--fns-per-module", type=int, default=20)
    parser.add_argument("--timeout-seconds", type=int, default=900)
    parser.add_argument("--output", type=Path, help="write the result JSON to this path")
    parser.add_argument("--baseline", type=Path, help="compare the matching workload against this JSON report")
    parser.add_argument("--max-regression", type=float, default=2.0)
    args = parser.parse_args()

    if not args.forgen:
        parser.error("forgen executable not found; pass --forgen PATH")
    if any(size < 1 for size in args.sizes):
        parser.error("all sizes must be positive")
    if args.fns_per_module < 1:
        parser.error("--fns-per-module must be at least 1")
    if not math.isfinite(args.max_regression) or args.max_regression <= 0:
        parser.error("--max-regression must be positive and finite")
    if args.baseline and args.output and args.baseline.resolve() == args.output.resolve():
        parser.error("--baseline and --output must be different files")
    if args.baseline and not args.baseline.is_file():
        parser.error(f"baseline file does not exist: {args.baseline}")

    report = {
        "seed": args.seed,
        "forgen": str(Path(args.forgen).resolve()),
        "platform": platform.system(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "python": platform.python_version(),
        "fns_per_module": args.fns_per_module,
        "backend_verification_required": True,
        "results": [],
    }
    print("lines\tmodules\tcheck seconds\tpeak RSS MB\tparse ms\tresolve ms\ttypecheck ms\tlint ms", flush=True)
    for size in args.sizes:
        result = benchmark_size(
            args.forgen,
            size,
            args.seed,
            args.fns_per_module,
            args.timeout_seconds,
        )
        report["results"].append(result)
        phases = result["phases"] or {}
        print(
            f"{result['source_lines']}\t{result['modules']}\t{result['check_seconds']:.3f}\t"
            f"{result['peak_rss_mb'] if result['peak_rss_mb'] is not None else 'n/a'}\t"
            f"{phases.get('parse_ms', 'n/a')}\t{phases.get('resolve_ms', 'n/a')}\t"
            f"{phases.get('typecheck_ms', 'n/a')}\t{phases.get('merged_lint_elapsed_ms', 'n/a')}",
            flush=True,
        )

    if args.baseline:
        try:
            baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
            ratios = [
                (size, regression_ratio(report, baseline, size))
                for size in args.sizes
            ]
        except (OSError, json.JSONDecodeError, ValueError) as error:
            parser.error(f"invalid stress baseline: {error}")
        report["baseline"] = str(args.baseline)
        report["max_regression"] = args.max_regression
        report["regression_ratios"] = {str(size): ratio for size, ratio in ratios}
        for size, ratio in ratios:
            print(f"regression {size}: {ratio:.3f}x (limit {args.max_regression:.3f}x)")
        if any(ratio > args.max_regression for _, ratio in ratios):
            print("stress performance regression exceeds the configured limit", file=sys.stderr)
            exit_code = 1
        else:
            exit_code = 0
    else:
        exit_code = 0

    serialized = json.dumps(report, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized + "\n", encoding="utf-8")
        print(f"saved {args.output}")
    else:
        print(serialized)
    if not args.baseline:
        print("no baseline supplied; this report is a calibration run")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
