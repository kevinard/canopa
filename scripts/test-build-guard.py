#!/usr/bin/env python3
"""Exercise build admission and process-tree cancellation without filling a disk."""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("build-guard.py")
spec = importlib.util.spec_from_file_location("build_guard", SCRIPT)
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


def running(pid):
    try:
        return Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1][0] != "Z"
    except FileNotFoundError:
        return False


class BuildGuardTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="canopa-guard-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def run_guard(self, code, *arguments):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--min-free-gib", "0.01",
             "--min-tmp-free-gib", "0.01", "--", sys.executable, "-c", code, *arguments],
            capture_output=True, text=True, timeout=10,
        )

    def test_low_space_refuses_before_command_starts(self):
        marker = self.root / "started"
        with mock.patch.object(guard, "check_space", side_effect=OSError("reserve crossed")), \
                contextlib.redirect_stderr(io.StringIO()) as error:
            code = guard.guarded_run([sys.executable, "-c", f"open({str(marker)!r}, 'w').close()"], [self.root])
        self.assertEqual(code, 75)
        self.assertFalse(marker.exists())
        self.assertIn("refusing to start", error.getvalue())

    def test_running_build_stops_children_even_when_the_leader_exits_first(self):
        child_pid = self.root / "child"
        started = self.root / "started"
        child = ("import os, signal, time; from pathlib import Path; "
                 "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                 f"Path({str(child_pid)!r}).write_text(str(os.getpid())); time.sleep(60)")
        parent = ("import subprocess, sys, time; from pathlib import Path; "
                  f"subprocess.Popen([sys.executable, '-c', {child!r}]); "
                  f"p=Path({str(child_pid)!r})\n"
                  "while not p.exists(): time.sleep(.01)\n"
                  f"Path({str(started)!r}).touch(); time.sleep(60)")

        def check(*_args):
            if started.exists():
                raise OSError("reserve crossed while compiling")

        with mock.patch.object(guard, "check_space", side_effect=check), \
                contextlib.redirect_stderr(io.StringIO()) as error:
            code = guard.guarded_run([sys.executable, "-c", parent], [self.root], interval=.02, grace=.15)
        self.assertEqual(code, 75)
        self.assertIn("stopping command", error.getvalue())
        pid = int(child_pid.read_text())
        deadline = time.monotonic() + 2
        while running(pid) and time.monotonic() < deadline:
            time.sleep(.01)
        self.assertFalse(running(pid), "an uncooperative build child survived cancellation")

    def test_exit_status_arguments_environment_and_output_are_preserved(self):
        with mock.patch.dict(os.environ, {"CARGO_INCREMENTAL": "1", "CARGO_TARGET_DIR": "target/cache with spaces"}):
            result = self.run_guard(
                "import json, os, sys; print(json.dumps([sys.argv[1:], os.environ['CARGO_INCREMENTAL'], "
                "os.environ['CARGO_TARGET_DIR']])); print('compiler diagnostic', file=sys.stderr); sys.exit(23)",
                "argument with spaces", "$(literal)",
            )
        self.assertEqual(result.returncode, 23)
        self.assertEqual(json.loads(result.stdout), [["argument with spaces", "$(literal)"], "1", "target/cache with spaces"])
        self.assertEqual(result.stderr, "compiler diagnostic\n")

    def test_child_signal_has_shell_exit_status(self):
        result = self.run_guard("import os, signal; os.kill(os.getpid(), signal.SIGTERM)")
        self.assertEqual(result.returncode, 128 + signal.SIGTERM)

    def test_external_signals_reach_the_owned_process_group(self):
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            with self.subTest(signal=signum):
                ready = self.root / f"ready-{signum}"
                program = ("import os, subprocess, sys, time; from pathlib import Path; "
                           "p=subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
                           f"Path({str(ready)!r}).write_text(str(p.pid)); time.sleep(60)")
                process = subprocess.Popen([sys.executable, str(SCRIPT), "--min-free-gib", "0.01",
                                            "--min-tmp-free-gib", "0.01", "--", sys.executable, "-c", program],
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                try:
                    deadline = time.monotonic() + 5
                    while not ready.exists() and process.poll() is None and time.monotonic() < deadline:
                        time.sleep(.01)
                    self.assertTrue(ready.exists())
                    process.send_signal(signum)
                    self.assertEqual(process.wait(timeout=8), 128 + signum)
                    self.assertFalse(running(int(ready.read_text())))
                finally:
                    if process.poll() is None:
                        process.terminate()
                        process.wait(timeout=8)

    def test_missing_command_reports_failure(self):
        result = subprocess.run([sys.executable, str(SCRIPT), "--", str(self.root / "missing")],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 127)
        self.assertIn("build-guard:", result.stderr)

    def test_cli_rejects_invalid_reserves_and_missing_commands(self):
        for value in ("0", "-1", "nan", "inf", "invalid"):
            result = subprocess.run([sys.executable, str(SCRIPT), "--min-free-gib", value, "--", "true"],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 2, result.stderr)
        result = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)

    def test_environment_reserve_is_enforced_before_startup(self):
        with mock.patch.dict(os.environ, {"CANOPA_BUILD_MIN_FREE_GIB": "1000000000"}):
            result = subprocess.run([sys.executable, str(SCRIPT), "--", sys.executable, "-c", "print('started')"],
                                    capture_output=True, text=True)
        self.assertEqual(result.returncode, 75)
        self.assertEqual(result.stdout, "")
        self.assertIn("GiB", result.stderr)

    def test_shared_filesystem_uses_larger_reserve_and_missing_target_parent(self):
        with mock.patch.object(guard.shutil, "disk_usage", return_value=mock.Mock(free=2 * guard.GIB)) as usage:
            with self.assertRaisesRegex(OSError, "10 GiB"):
                guard.check_space([self.root / "not-created/target"], 10, self.root, 1)
            self.assertEqual(usage.call_count, 1)

    def test_temporary_filesystem_reserve_is_checked_independently(self):
        with mock.patch.object(guard, "existing_parent", side_effect=lambda path: path), \
                mock.patch.object(guard.shutil, "disk_usage", return_value=mock.Mock(free=guard.GIB / 2)):
            build = mock.Mock()
            build.stat.return_value.st_dev = 1
            temporary = mock.Mock()
            temporary.stat.return_value.st_dev = 2
            with self.assertRaisesRegex(OSError, "1 GiB"):
                guard.check_space([build], .1, temporary, 1)

    def test_configured_and_explicit_targets_and_symlinks_are_observed(self):
        with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(self.root / "env target"),
                                          "CARGO_BUILD_BUILD_DIR": str(self.root / "build cache")}):
            paths = guard.build_paths(["cargo", "build", "--target-dir", str(self.root / "cli target"),
                                       "--", "--target-dir=application-argument"], [self.root / "extra"])
        for name in ("env target", "build cache", "cli target", "extra"):
            self.assertIn(self.root / name, paths)
        self.assertNotIn(Path("application-argument"), paths)
        link = self.root / "link"
        link.symlink_to(self.root, target_is_directory=True)
        self.assertEqual(guard.existing_parent(link / "new/cache"), self.root)


if __name__ == "__main__":
    unittest.main()
