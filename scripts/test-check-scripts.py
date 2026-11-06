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
if name == "python3" and args and args[0] == "scripts/select-files-tests.py":
    os.execv("/usr/bin/python3", ["python3", *args])
with open(os.environ["CHECK_SCRIPT_LOG"], "a") as stream:
    stream.write(json.dumps({"tool": name, "args": args, "cwd": os.getcwd(),
                             "incremental": os.environ.get("CARGO_INCREMENTAL"),
                             "target_dir": os.environ.get("CARGO_TARGET_DIR")}) + "\\n")
if name == "dbus-run-session":
    assert args[0] == "--"
    os.execvp(args[1], args[1:])
if name == "cargo" and "--lib" in args and os.environ.get("CHECK_SCRIPT_FAIL_LIB"):
    sys.exit(23)
if name == "cargo" and "--list" in args:
    if os.environ.get("CHECK_SCRIPT_FAIL_LIST"):
        sys.exit(24)
    print(os.environ["CHECK_SCRIPT_TEST_LIST"])
'''

TEST_LIST = "\n".join([
    "cases::transfers::can_copy_a_file: test",
    "cases::undo_changed_copy::keeps_copy_bytes: test",
    "cases::undo_changed_copy::unrelated: test",
    "cases::operation_transfers::can_copy_other: test",
    "cases::transfers::unrelated: test",
])


class CheckScripts(unittest.TestCase):
    def invoke(self, *arguments, fail_lib=False, fail_list=False, directory=None):
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
                   "CHECK_SCRIPT_TEST_LIST": TEST_LIST,
                   "CARGO_TARGET_DIR": "target/retained-development-cache"}
            env.pop("CHECK_SCRIPT_FAIL_LIB", None)
            env.pop("CHECK_SCRIPT_FAIL_LIST", None)
            if fail_lib:
                env["CHECK_SCRIPT_FAIL_LIB"] = "1"
            if fail_list:
                env["CHECK_SCRIPT_FAIL_LIST"] = "1"
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

    def test_function_substrings_are_matched_inside_selected_case_names(self):
        result, commands = self.invoke("--files", "--test", "transfers", "--test", "undo_changed_copy",
                                       "copy", "--", "--nocapture")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands[-1]["args"], ["test", "--locked", "-p", "canopa-files",
                                              "--test", "integration", "--",
                                              "cases::transfers::can_copy_a_file", "cases::undo_changed_copy::keeps_copy_bytes",
                                              "--exact", "--nocapture"])
        cargo = [item["args"] for item in commands if item["tool"] == "cargo" and "test" in item["args"]]
        self.assertEqual(len(cargo), 2)
        self.assertEqual(cargo[0][-3:], ["--", "--list", "--format=terse"])

    def test_module_selection_without_a_function_filter_does_not_run_other_modules(self):
        result, commands = self.invoke("--files", "--test", "transfers", "--test", "undo_changed_copy")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands[-1]["args"], ["test", "--locked", "-p", "canopa-files",
                                              "--test", "integration", "--", "cases::transfers::", "cases::undo_changed_copy::"])

    def test_standalone_and_shared_cases_keep_their_own_filters(self):
        result, commands = self.invoke("--files", "--test", "uutils_copy", "--test", "transfers", "copy")
        self.assertEqual(result.returncode, 0, result.stderr)
        cargo = [item["args"] for item in commands if item["tool"] == "cargo" and "test" in item["args"]]
        self.assertEqual(cargo, [
            ["test", "--locked", "-p", "canopa-files", "--test", "integration", "--", "--list", "--format=terse"],
            ["test", "--locked", "-p", "canopa-files", "--test", "integration", "--", "cases::transfers::can_copy_a_file", "--exact"],
            ["test", "--locked", "-p", "canopa-files", "--test", "uutils_copy", "copy", "--"],
        ])

    def test_exact_filters_keep_case_local_names_and_no_filter_keeps_the_case(self):
        result, commands = self.invoke("--files", "--test", "transfers", "can_copy_a_file", "--", "--exact")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands[-1]["args"][-3:], ["--", "cases::transfers::can_copy_a_file", "--exact"])
        result, commands = self.invoke("--files", "--test", "transfers", "--", "--exact")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands[-1]["args"][-4:], ["--", "cases::transfers::can_copy_a_file", "cases::transfers::unrelated", "--exact"])

    def test_unmatched_filters_do_not_accidentally_run_the_entire_suite(self):
        result, commands = self.invoke("--files", "--test", "transfers", "no match with spaces")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("No ordinary Files tests match", result.stdout)
        cargo = [item["args"] for item in commands if item["tool"] == "cargo" and "test" in item["args"]]
        self.assertEqual(len(cargo), 1)
        self.assertIn("--list", cargo[0])

    def test_a_failed_test_inventory_is_propagated(self):
        result, commands = self.invoke("--files", "--test", "transfers", "copy", fail_list=True)
        self.assertEqual(result.returncode, 24, result.stderr)
        cargo = [item["args"] for item in commands if item["tool"] == "cargo" and "test" in item["args"]]
        self.assertEqual(len(cargo), 1)

    def test_the_whole_integration_suite_can_be_explicitly_selected(self):
        result, commands = self.invoke("--files", "--test", "integration")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands[-1]["args"], ["test", "--locked", "-p", "canopa-files",
                                              "--test", "integration", "--", ""])

    def test_case_filters_do_not_select_similarly_named_modules(self):
        result, commands = self.invoke("--files", "--test", "transfers")
        self.assertEqual(result.returncode, 0, result.stderr)
        arguments = commands[-1]["args"]
        filters = arguments[arguments.index("--") + 1:]
        with tempfile.TemporaryDirectory(prefix="canopa-test-filter-") as temporary:
            source = Path(temporary) / "filters.rs"
            binary = Path(temporary) / "filters"
            source.write_text('#[path = "."] mod cases { mod transfers; mod operation_transfers; }')
            source.with_name("transfers.rs").write_text("#[test] fn can_copy_a_file() {} #[test] fn unrelated() {}")
            source.with_name("operation_transfers.rs").write_text("#[test] fn can_copy_other() {}")
            subprocess.run(["rustc", "--test", str(source), "-o", str(binary)],
                           check=True, capture_output=True, text=True)
            listed = subprocess.check_output([str(binary), *filters, "--list", "--format=terse"], text=True)
            self.assertEqual(listed.splitlines(), ["cases::transfers::can_copy_a_file: test", "cases::transfers::unrelated: test"])
            result, commands = self.invoke("--files", "--test", "transfers", "copy")
            self.assertEqual(result.returncode, 0, result.stderr)
            arguments = commands[-1]["args"]
            filters = arguments[arguments.index("--") + 1:]
            listed = subprocess.check_output([str(binary), *filters, "--list", "--format=terse"], text=True)
            self.assertEqual(listed.splitlines(), ["cases::transfers::can_copy_a_file: test"])

    def test_broad_or_missing_selectors_are_refused_before_any_checks(self):
        for arguments in [
            ("--files", "bare-filter"), ("--files", "--test"), ("--files", "--test", ""),
            ("--files", "--test", "*"), ("--files", "--test", "undo_*"),
            ("--files", "--test", "unknown_case"),
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
        self.assertIn(["test", "--workspace", "--all-targets", "--locked", "--no-fail-fast"], cargo)
        self.assertIn(["build", "--locked", "-p", "canopa-cli"], cargo)
        self.assertIn("scripts/test-check-scripts.py", [item["args"][0] for item in commands if item["tool"] == "python3"])
        self.assertIn("scripts/test-portal-config.py", [item["args"][0] for item in commands if item["tool"] == "python3"])
        self.assertIn("scripts/test-niri-source-bindings.py", [item["args"][0] for item in commands if item["tool"] == "python3"])
        self.assertIn("scripts/test-niri-build-reuse.py", [item["args"][0] for item in commands if item["tool"] == "python3"])
        self.assertIn("scripts/test-remote-touch-core.py", [item["args"][0] for item in commands if item["tool"] == "python3"])
        self.assertIn("scripts/test-remote-desktop-dispatch.py", [item["args"][0] for item in commands if item["tool"] == "python3"])
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
