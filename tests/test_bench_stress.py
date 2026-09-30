import importlib.util
from pathlib import Path
import unittest


BENCH_PATH = Path(__file__).resolve().parents[1] / "tools" / "bench_stress.py"
SPEC = importlib.util.spec_from_file_location("bench_stress", BENCH_PATH)
assert SPEC is not None and SPEC.loader is not None
BENCH_STRESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BENCH_STRESS)


class StressBenchmarkComparisonTests(unittest.TestCase):
    def make_report(self, seconds, seed=42, fns_per_module=20):
        return {
            "seed": seed,
            "fns_per_module": fns_per_module,
            "platform": "Linux",
            "machine": "x86_64",
            "backend_verification_required": True,
            "results": [{"source_lines": 50_000, "check_seconds": seconds}],
        }

    def test_computes_regression_ratio_for_matching_workload(self):
        ratio = BENCH_STRESS.regression_ratio(
            self.make_report(21.0), self.make_report(10.0), 50_000
        )

        self.assertEqual(ratio, 2.1)

    def test_rejects_baseline_from_different_generated_workload(self):
        with self.assertRaisesRegex(ValueError, "seed"):
            BENCH_STRESS.regression_ratio(
                self.make_report(12.0), self.make_report(10.0, seed=7), 50_000
            )

    def test_rejects_missing_requested_size(self):
        with self.assertRaisesRegex(ValueError, "50000"):
            BENCH_STRESS.regression_ratio(
                self.make_report(12.0),
                {
                    "seed": 42,
                    "fns_per_module": 20,
                    "platform": "Linux",
                    "machine": "x86_64",
                    "backend_verification_required": True,
                    "results": [],
                },
                50_000,
            )

    def test_rejects_nonpositive_baseline_time(self):
        with self.assertRaisesRegex(ValueError, "positive"):
            BENCH_STRESS.regression_ratio(
                self.make_report(12.0), self.make_report(0.0), 50_000
            )

    def test_benchmark_runs_backend_verification(self):
        forgen = BENCH_STRESS.default_forgen()
        if not forgen:
            self.skipTest("forgen executable is not available")

        result = BENCH_STRESS.benchmark_size(
            forgen, 700, seed=42, fns_per_module=20, timeout_seconds=90
        )

        self.assertTrue(result["phases"]["backend_verified"], result["debug_trace"])

    def test_phase_trace_requires_backend_verification(self):
        output = """[phases] so-far total=100ms parse=20ms resolve=30ms typecheck=40ms
[phases2] pre-backend-verify at 151ms
[phases2] backend-verify done at 240ms
"""
        phases, timeline = BENCH_STRESS.parse_phase_trace(output)

        self.assertTrue(phases["backend_verified"])
        self.assertEqual(timeline[-1]["phase"], "backend-verify")

    def test_optimizer_trace_is_sorted_by_elapsed_time(self):
        output = """[optimizer-timing] intraproc = 50ms
[optimizer-timing] mem2reg = 2ms
"""

        self.assertEqual(
            BENCH_STRESS.parse_optimizer_trace(output),
            [{"pass": "intraproc", "elapsed_ms": 50}, {"pass": "mem2reg", "elapsed_ms": 2}],
        )

    def test_rejects_baseline_that_skipped_backend_verification(self):
        invalid_baseline = self.make_report(10.0)
        invalid_baseline["backend_verification_required"] = False
        with self.assertRaisesRegex(ValueError, "backend_verification_required"):
            BENCH_STRESS.regression_ratio(self.make_report(11.0), invalid_baseline, 50_000)

    def test_phase_durations_start_after_parse_resolve_and_typecheck(self):
        output = """[phases] so-far total=100ms parse=20ms resolve=30ms typecheck=40ms
[phases2] effects done at 105ms
[phases2] ownership done at 150ms
[phases2] merged lint done at 10ms
"""
        phases, timeline = BENCH_STRESS.parse_phase_trace(output)

        self.assertEqual(phases["typecheck_ms"], 40)
        self.assertEqual(timeline[0], {"phase": "effects", "elapsed_ms": 105, "duration_ms": 5})
        self.assertEqual(timeline[1], {"phase": "ownership", "elapsed_ms": 150, "duration_ms": 45})
        self.assertEqual([item["phase"] for item in timeline], ["effects", "ownership"])


if __name__ == "__main__":
    unittest.main()
