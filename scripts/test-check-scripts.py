#!/usr/bin/env python3
"""Verify check selection and build settings without compiling Rust."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
STUB = '''#!/usr/bin/python3
import json, os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
if name == "bash" and args and args[0] == "scripts/check-files.sh":
    os.execv("/bin/bash", ["bash", *args])
with open(os.environ["CHECK_SCRIPT_LOG"], "a") as stream:
    stream.write(json.dumps({"tool": name, "args": args, "cwd": os.getcwd(),
                             "incremental": os.environ.get("CARGO_INCREMENTAL"),
                             "target_dir": os.environ.get("CARGO_TARGET_DIR")}) + "\\n")
if name == "dbus-run-session":
    assert args[0] == "--"
    os.execvp(args[1], args[1:])
if name == "cargo" and "--lib" in args and os.environ.get("CHECK_SCRIPT_FAIL_LIB"):
    sys.exit(23)
'''


class CheckScripts(unittest.TestCase):
    def invoke(self, *arguments, fail_lib=False, directory=None):
        with tempfile.TemporaryDirectory(prefix="canopa-check-scripts-") as temporary:
            root = Path(temporary)
            tools = root / "bin"
            tools.mkdir()
            for name in ("cargo", "dbus-run-session", "python3", "bash"):
                executable = tools / name
                executable.write_text(STUB)
                executable.chmod(0o755)
            log = root / "commands.jsonl"
            env = {**os.environ, "PATH": str(tools) + os.pathsep + os.environ["PATH"],
                   "CHECK_SCRIPT_LOG": str(log), "CARGO_INCREMENTAL": "0",
                   "CARGO_TARGET_DIR": "target/retained-development-cache"}
            env.pop("CHECK_SCRIPT_FAIL_LIB", None)
            if fail_lib:
                env["CHECK_SCRIPT_FAIL_LIB"] = "1"
            result = subprocess.run(
                ["/bin/bash", str(ROOT / "scripts/check.sh"), *arguments],
                cwd=directory or root, env=env, capture_output=True, text=True, timeout=15,
            )
            commands = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            return result, commands

    def test_default_and_files_select_only_two_copy_test_executables(self):
        for arguments in [(), ("--files",), ("--files", "copy")]:
            with self.subTest(arguments=arguments):
                result, commands = self.invoke(*arguments)
                self.assertEqual(result.returncode, 0, result.stderr)
                cargo = [item["args"] for item in commands if item["tool"] == "cargo"]
                self.assertEqual(cargo, [
                    ["fmt", "--all", "--", "--check"],
                    ["test", "--locked", "-p", "canopa-files", "--lib", "operations::fingerprint::receipt_tests"],
                    ["test", "--locked", "-p", "canopa-files", "--test", "uutils_copy"],
                ])
                self.assertEqual(sum(item["tool"] == "dbus-run-session" for item in commands), 2)
                self.assertTrue(all(item["incremental"] == "1" for item in commands))
                self.assertTrue(all(item["cwd"] == str(ROOT) for item in commands))
                self.assertTrue(all(item["target_dir"] == "target/retained-development-cache" for item in commands))

    def test_library_filter_and_harness_arguments_are_preserved(self):
        result, commands = self.invoke("--files", "--lib", "operations::fingerprint", "--", "--test-threads=1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands[-1]["args"], ["test", "--locked", "-p", "canopa-files",
                                              "--lib", "operations::fingerprint", "--", "--test-threads=1"])

    def test_named_consumers_and_a_filter_stay_in_one_selected_invocation(self):
        result, commands = self.invoke("--files", "--test", "transfers", "--test", "undo_changed_copy",
                                       "filter with spaces", "--", "--nocapture")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands[-1]["args"], ["test", "--locked", "-p", "canopa-files",
                                              "--test", "transfers", "--test", "undo_changed_copy",
                                              "filter with spaces", "--", "--nocapture"])

    def test_broad_or_missing_selectors_are_refused_before_any_checks(self):
        for arguments in [
            ("--files", "bare-filter"), ("--files", "--test"), ("--files", "--test", ""),
            ("--files", "--test", "*"), ("--files", "--test", "undo_*"),
            ("--files", "--workspace"), ("--files", "--lib", "--all-targets"),
            ("--files", "--test", "uutils_copy", "--release"),
            ("--files", "--lib", "--test", "uutils_copy"),
            ("--files", "copy", "extra"), ("--workspace", "extra"), ("--core", "extra"),
        ]:
            with self.subTest(arguments=arguments):
                result, commands = self.invoke(*arguments)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(commands, [])

    def test_selected_test_failure_is_propagated_without_running_later_tests(self):
        result, commands = self.invoke(fail_lib=True)
        self.assertEqual(result.returncode, 23)
        self.assertFalse(any("uutils_copy" in item["args"] for item in commands))

    def test_workspace_gate_remains_explicit_and_complete(self):
        result, commands = self.invoke("--workspace")
        self.assertEqual(result.returncode, 0, result.stderr)
        cargo = [item["args"] for item in commands if item["tool"] == "cargo"]
        self.assertIn(["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"], cargo)
        self.assertIn(["test", "--workspace", "--all-targets", "--locked"], cargo)
        self.assertIn(["build", "--locked", "-p", "canopa-cli"], cargo)
        self.assertIn("scripts/test-check-scripts.py", [item["args"][0] for item in commands if item["tool"] == "python3"])
        self.assertEqual(commands[-1]["args"], ["scripts/test-vendor.py"])
        self.assertTrue(all(item["incremental"] == "0" for item in commands))

    def test_core_gate_stays_available(self):
        result, commands = self.invoke("--core")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(["test", "--locked", "-p", "canopa-files", "--no-default-features"],
                      [item["args"] for item in commands if item["tool"] == "cargo"])
        self.assertEqual(commands[-1]["args"], ["scripts/test-notify.sh", "--no-default-features"])

    def test_help_runs_no_checks(self):
        for arguments in [("--help",), ("--files", "--help")]:
            result, commands = self.invoke(*arguments)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("focused", result.stdout.lower())
            self.assertEqual(commands, [])


if __name__ == "__main__":
    unittest.main()
