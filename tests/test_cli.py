import contextlib
import io
import json
import pathlib
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "tools"))
import atto


class DoctorTests(unittest.TestCase):
    def call(self, args, found):
        stdout = io.StringIO()
        with patch.object(atto.shutil, "which",
                          side_effect=lambda tool: f"/fake/{tool}" if tool in found else None):
            with contextlib.redirect_stdout(stdout):
                status = atto.main(args)
        return status, stdout.getvalue()

    def test_json_success_is_explicitly_source_free(self):
        status, value = self.call(["doctor", "--json", "--require", "rust", "--require", "wasm"],
                                  {"cargo", "rustc", "node"})
        self.assertEqual(status, 0)
        report = json.loads(value)
        self.assertEqual(report["schema_version"], 1)
        self.assertIn("no compiler/build or guest execution", report["evidence"])
        self.assertTrue(report["backends"]["rust"]["all_tools_found"])
        self.assertTrue(report["backends"]["wasm"]["all_tools_found"])
        self.assertFalse(report["backends"]["java"]["all_tools_found"])
        self.assertEqual(report["missing_required"], [])

    def test_missing_requirement_fails_closed(self):
        status, value = self.call(["doctor", "--json", "--require", "java", "--require", "rust"],
                                  {"cargo", "rustc"})
        self.assertEqual(status, 2)
        self.assertEqual(json.loads(value)["missing_required"], ["java"])

    def test_report_does_not_expose_paths_or_run_commands(self):
        status, value = self.call(["doctor", "--json"], {"node"})
        self.assertEqual(status, 0)
        self.assertNotIn("/fake", value)
        self.assertFalse(json.loads(value)["backends"]["llvm"]["all_tools_found"])

    def test_unknown_command_rejected(self):
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as caught:
                atto.main(["run", "program.atto"])
        self.assertEqual(caught.exception.code, 2)

    def test_duplicate_requirements_deduplicated(self):
        status, value = self.call(["doctor", "--json", "--require", "forge", "--require", "forge"], set())
        self.assertEqual(status, 2)
        self.assertEqual(json.loads(value)["missing_required"], ["forge"])


if __name__ == "__main__":
    unittest.main()
