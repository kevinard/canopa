"""Exercise the updater CLI with real Git and stand-ins for Cargo/systemd."""

import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest


PROJECT = Path(__file__).resolve().parents[2]
UPDATER = PROJECT / "scripts/niribar-dev.py"

CARGO = r'''#!/usr/bin/python3
import os, pathlib, shutil, subprocess, sys
root = pathlib.Path(os.environ['NIRIBAR_UPDATE_TEST_ROOT'])
mode = pathlib.Path('src/revision').read_text()
if pathlib.Path('.cargo/config.toml').exists() or pathlib.Path('generated.cfg').exists():
    sys.exit('stale build input')
if mode == 'build-failure':
    sys.exit(1)
if mode == 'superseded':
    (root/'repo/src/revision').write_text('newer')
    subprocess.run(['git','-C',str(root/'repo'),'commit','-am','newer'],check=True)
target = pathlib.Path(sys.argv[sys.argv.index('--target-dir')+1])/'release'
target.mkdir(parents=True,exist_ok=True)
shutil.copy2('/usr/bin/sleep',target/'niribar')
with (target/'niribar').open('ab') as stream:
    stream.write(mode.encode())
if mode == 'startup-failure':
    (target/'niribar').write_text('#!/bin/sh\nif [ "$1" = "--version" ]; then echo test; else exit 1; fi\n')
helper = """#!/usr/bin/python3
import json, struct, sys
sys.stdin.buffer.read()
body=json.dumps({'type':'handshake','protocol_version':1,'content':{'elements':[{'title':'Wi-Fi unavailable'}]}}).encode()
sys.stdout.buffer.write(b'NIRIBAR_EXTERNAL_PANEL_JSON_V1\\n'+struct.pack('>Q',len(body))+body)
"""
(target/'niribar-wifi-provider').write_text(helper)
(target/'niribar-wifi-provider').chmod(0o755)
'''

SYSTEMCTL = r'''#!/usr/bin/python3
import os, pathlib, signal, subprocess, sys
root = pathlib.Path(os.environ['NIRIBAR_UPDATE_TEST_ROOT'])
pidfile = root/'pid'
args = sys.argv[2:]
pid = int(pidfile.read_text()) if pidfile.exists() else 0
if args[0] == 'is-active':
    try:
        state=pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(') ',1)[1].split()[0]
        sys.exit(0 if state not in ('Z','X') else 3)
    except FileNotFoundError:
        sys.exit(3)
if args[0] == 'show':
    if '--property=ActiveState' in args:
        try:
            state=pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(') ',1)[1].split()[0]
            print('failed' if state in ('Z','X') else 'active')
        except FileNotFoundError:
            print('failed' if pid else 'inactive')
    else:
        if (root/'watch-activation').exists() and (root/'data/current').resolve().name != 'original':
            (root/'activation-started').touch()
        print(pid)
if args[0] == 'restart':
    try:
        if pid > 0: os.kill(pid,signal.SIGTERM)
    except ProcessLookupError: pass
    process = subprocess.Popen([str(root/'data/current/niribar'),'120'],
        stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
    pidfile.write_text(str(process.pid))
'''


class UpdateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="niribar-update-test-")
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.env = dict(os.environ, NIRIBAR_UPDATE_TEST_ROOT=str(self.root))
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Updater test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "core.hooksPath", "/dev/null")
        self.git("config", "commit.gpgSign", "false")
        (self.repo / "src").mkdir()
        (self.repo / ".gitignore").write_text("generated.cfg\n")
        for name in ("scripts/package-release.sh", "scripts/smoke-release.sh",
                     "config/default.kdl", "contrib/systemd/niribar.service", "README.md", "LICENSE"):
            destination = self.repo / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(PROJECT / name, destination)
        self.commit("first")
        for name, content in (("cargo", CARGO), ("systemctl", SYSTEMCTL)):
            path = self.root / name
            path.write_text(content)
            path.chmod(0o755)
        for directory in ("cache", "state", "data/releases", "data/original"):
            (self.root / directory).mkdir(parents=True)
        original = self.root / "data/original"
        shutil.copy2("/usr/bin/sleep", original / "niribar")
        (original / "build.json").write_text('{"revision":null}')
        (self.root / "data/current").symlink_to(original)
        self.original_process = subprocess.Popen([str(original / "niribar"), "120"])
        (self.root / "pid").write_text(str(self.original_process.pid))
        self.config = self.root / "config.json"
        self.config.write_text(json.dumps({
            "repository": str(self.repo), "ref": "refs/heads/main",
            "data": str(self.root / "data"), "cache": str(self.root / "cache"),
            "state": str(self.root / "state"), "git": shutil.which("git"),
            "cargo": str(self.root / "cargo"), "systemctl": str(self.root / "systemctl"),
        }))

    def tearDown(self):
        for pid in (self.original_process.pid, int((self.root / "pid").read_text())):
            if pid <= 0:
                continue
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        self.original_process.wait(timeout=5)
        self.temporary.cleanup()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args], text=True,
                                       stderr=subprocess.DEVNULL).strip()

    def commit(self, text):
        (self.repo / "src/revision").write_text(text)
        self.git("add", ".")
        self.git("commit", "-m", text)
        return self.git("rev-parse", "HEAD")

    def cli(self, command, success=True):
        result = subprocess.run([sys.executable, str(UPDATER), command, "--config", str(self.config)],
                                env=self.env, text=True, capture_output=True, timeout=45)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return result

    def status(self):
        return json.loads(self.cli("status").stdout)

    def test_installs_committed_pair_without_touching_dirty_working_files(self):
        wanted = self.git("rev-parse", "HEAD")
        (self.repo / "src/revision").write_text("build-failure")
        self.cli("update")
        state = self.status()
        self.assertEqual(state["installed"]["revision"], wanted)
        self.assertEqual(set(state["installed"]["sha256"]), {"niribar", "niribar-wifi-provider"})
        self.assertTrue(state["running_installed_binary"])
        self.assertEqual((self.repo / "src/revision").read_text(), "build-failure")
        self.cli("update")
        self.assertEqual(self.status()["running_pid"], state["running_pid"])

    def test_failed_build_keeps_running_version(self):
        self.commit("build-failure")
        self.cli("update", success=False)
        state = self.status()
        self.assertEqual(state["update"]["phase"], "failed")
        self.assertIsNone(state["installed"]["revision"])
        self.assertEqual(int(state["running_pid"]), self.original_process.pid)
        self.cli("update")  # cooldown avoids continuously rebuilding a broken commit
        self.assertEqual(int(self.status()["running_pid"]), self.original_process.pid)

    def test_managed_checkout_cannot_redirect_to_the_authors_working_copy(self):
        (self.repo / "src/revision").write_text("uncommitted work")
        (self.root / "cache/checkout").symlink_to(self.repo)
        self.cli("update", success=False)
        self.assertEqual((self.repo / "src/revision").read_text(), "uncommitted work")
        self.assertEqual(int(self.status()["running_pid"]), self.original_process.pid)

    def test_commit_arriving_during_build_is_not_replaced_by_stale_build(self):
        self.commit("superseded")
        self.cli("update")
        state = self.status()
        self.assertEqual(state["update"]["phase"], "superseded")
        self.assertIsNone(state["installed"]["revision"])
        self.assertEqual(int(state["running_pid"]), self.original_process.pid)

    def test_failed_service_start_rolls_back_to_previous_binary(self):
        self.commit("startup-failure")
        self.cli("update", success=False)
        state = self.status()
        self.assertEqual(state["update"]["phase"], "failed")
        self.assertIsNone(state["installed"]["revision"])
        self.assertTrue(state["service_active"] and state["running_installed_binary"])

    def test_generated_and_untracked_inputs_do_not_leak_into_later_builds(self):
        self.cli("update")
        checkout = self.root / "cache/checkout"
        (checkout / ".cargo").mkdir()
        (checkout / ".cargo/config.toml").write_text("stale config")
        (checkout / "generated.cfg").write_text("stale generated input")
        wanted = self.commit("second")
        self.cli("update")
        self.assertEqual(self.status()["installed"]["revision"], wanted)

    def interrupted_activation(self, interruption, stop_bar=False):
        (self.root / "watch-activation").touch()
        with (self.root / "interrupted.log").open("w") as log:
            process = subprocess.Popen(
                [sys.executable, str(UPDATER), "update", "--config", str(self.config)],
                env=self.env, stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 20
                while not (self.root / "activation-started").exists():
                    if process.poll() is not None or time.monotonic() > deadline:
                        self.fail((self.root / "interrupted.log").read_text())
                    time.sleep(0.02)
                if stop_bar:
                    os.kill(int((self.root / "pid").read_text()), signal.SIGTERM)
                    (self.root / "pid").write_text("0")
                if interruption is not None:
                    process.send_signal(interruption)
                self.assertNotEqual(process.wait(timeout=15), 0)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()

    def assert_original_restored(self):
        state = self.status()
        self.assertIsNone(state["installed"]["revision"])
        self.assertTrue(state["service_active"] and state["running_installed_binary"])

    def test_termination_during_activation_restores_original_immediately(self):
        self.interrupted_activation(signal.SIGTERM)
        self.assert_original_restored()

    def test_killed_activation_recovers_before_already_installed_shortcut(self):
        self.interrupted_activation(signal.SIGKILL)
        self.cli("update")
        self.assert_original_restored()

    def test_stopping_bar_during_activation_does_not_restart_it(self):
        self.interrupted_activation(None, stop_bar=True)
        state = self.status()
        self.assertIsNone(state["installed"]["revision"])
        self.assertFalse(state["service_active"])

    def test_recovery_after_forced_kill_respects_a_stopped_bar(self):
        self.interrupted_activation(signal.SIGKILL, stop_bar=True)
        self.cli("update")
        state = self.status()
        self.assertIsNone(state["installed"]["revision"])
        self.assertFalse(state["service_active"])

    def test_install_does_not_start_a_stopped_bar(self):
        self.original_process.terminate()
        self.original_process.wait(timeout=5)
        (self.root / "pid").write_text("0")
        self.cli("update")
        state = self.status()
        self.assertEqual(state["installed"]["revision"], self.git("rev-parse", "HEAD"))
        self.assertFalse(state["service_active"])

    def test_branch_can_return_to_previous_successful_release(self):
        first = self.git("rev-parse", "HEAD")
        self.cli("update")
        second = self.commit("second")
        self.cli("update")
        self.git("update-ref", "refs/heads/main", first, second)
        self.cli("update")
        state = self.status()
        self.assertEqual(state["installed"]["revision"], first)
        self.assertTrue(state["running_installed_binary"])
        self.assertEqual((self.root / "data/previous").resolve().name, second)

    def test_old_releases_are_pruned_but_previous_working_pair_is_retained(self):
        first = self.git("rev-parse", "HEAD")
        self.cli("update")
        second = self.commit("second")
        self.cli("update")
        third = self.commit("third")
        self.cli("update")
        self.assertFalse((self.root / "data/releases" / first).exists())
        self.assertEqual((self.root / "data/previous").resolve().name, second)
        self.assertEqual(self.status()["installed"]["revision"], third)
        self.assertTrue((self.root / "data/original/niribar").exists())


if __name__ == "__main__":
    unittest.main()
