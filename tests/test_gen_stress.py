import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


GENERATOR_PATH = Path(__file__).resolve().parents[1] / "tools" / "gen_stress.py"
SPEC = importlib.util.spec_from_file_location("gen_stress", GENERATOR_PATH)
assert SPEC is not None and SPEC.loader is not None
GEN_STRESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GEN_STRESS)


class StressGeneratorTests(unittest.TestCase):
    def generate_project(self, output, *options, lines="700"):
        with patch.object(
            sys,
            "argv",
            ["gen_stress.py", "--out", str(output), "--lines", lines, "--seed", "7", *options],
        ):
            GEN_STRESS.main()

    def run_check(self, output):
        repo_root = GENERATOR_PATH.parents[1]
        candidates = [
            repo_root / "target" / "release" / "forgen.exe",
            repo_root / "target" / "release" / "forgen",
        ]
        forgen = next((str(path) for path in candidates if path.is_file()), None)
        if forgen is None:
            forgen = shutil.which("forgen")
        if forgen is None:
            self.skipTest("forgen executable is not available")

        return subprocess.run(
            [forgen, "check", str(output)],
            cwd=repo_root,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )

    def test_module_imports_and_calls_the_next_module(self):
        _, lines, _ = GEN_STRESS.gen_module(0, 2, 1, 42)
        source = "\n".join(lines)

        self.assertIn("use mod0001.fn_0001_000", source)
        self.assertIn("fn_0001_000(local_result)", source)

    def test_last_module_has_no_out_of_range_import(self):
        _, lines, _ = GEN_STRESS.gen_module(1, 2, 1, 42)
        source = "\n".join(lines)

        self.assertNotIn("use mod0002", source)
        self.assertNotIn("fn_0002_000", source)

    def test_generated_project_passes_forgen_check(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "stress_app"
            self.generate_project(output)

            result = self.run_check(output)
            self.assertEqual(
                result.returncode,
                0,
                f"forgen check rejected generated source:\n{result.stdout}\n{result.stderr}",
            )
            output_text = f"{result.stdout}\n{result.stderr}"
            self.assertNotIn(
                "warning[dead_code::",
                output_text,
                "valid stress projects should exercise generated functions instead of linting them as dead",
            )

    def test_duplicate_name_mode_is_rejected_by_forgen_check(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "stress_app"
            self.generate_project(output, "--duplicate-names")

            result = self.run_check(output)
            output_text = f"{result.stdout}\n{result.stderr}".lower()
            self.assertNotEqual(
                result.returncode,
                0,
                "forgen check must reject the intentionally duplicated declaration",
            )
            self.assertIn(
                "duplicate",
                output_text,
                f"forgen check failed for a reason other than the duplicate declaration:\n{output_text}",
            )

    def test_generated_source_matches_requested_line_count_exactly(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "stress_app"
            self.generate_project(output, lines="700")

            actual_lines = sum(
                len(path.read_text(encoding="utf-8").splitlines())
                for path in (output / "src").glob("*.dtr")
            )
            self.assertEqual(actual_lines, 700)

    def test_regeneration_removes_stale_generated_modules(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "stress_app"
            self.generate_project(output, lines="10000")
            self.assertGreater(len(list((output / "src").glob("mod*.dtr"))), 10)

            self.generate_project(output, "--fns-per-module", "1", lines="100")
            self.assertEqual(len(list((output / "src").glob("mod*.dtr"))), 4)

    def test_generation_is_deterministic_for_same_seed(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            first = Path(temp_dir) / "first"
            second = Path(temp_dir) / "second"
            self.generate_project(first, "--fns-per-module", "1", lines="100")
            self.generate_project(second, "--fns-per-module", "1", lines="100")

            first_files = sorted((first / "src").glob("*.dtr"))
            second_files = sorted((second / "src").glob("*.dtr"))
            self.assertEqual([p.name for p in first_files], [p.name for p in second_files])
            self.assertEqual(
                [p.read_text(encoding="utf-8") for p in first_files],
                [p.read_text(encoding="utf-8") for p in second_files],
            )

    def test_generated_project_contains_a_real_chain_from_main(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "stress_app"
            self.generate_project(output, "--fns-per-module", "1", lines="100")

            src_dir = output / "src"
            main_source = (src_dir / "main.dtr").read_text(encoding="utf-8")
            self.assertIn("use mod0000.fn_0000_000", main_source)
            self.assertIn("out fn_0000_000(1)", main_source)

            modules = sorted(src_dir.glob("mod*.dtr"))
            self.assertGreaterEqual(len(modules), 2)
            for current, following in zip(modules, modules[1:]):
                current_source = current.read_text(encoding="utf-8")
                following_name = following.stem
                imported_fn = f"fn_{following_name[3:]}_000"
                self.assertIn(f"use {following_name}.{imported_fn}", current_source)
                self.assertIn(f"{imported_fn}(", current_source)


if __name__ == "__main__":
    unittest.main()
