#!/usr/bin/env python3
"""Exercise the updater with real Cargo builds and an isolated systemd substitute."""

import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import unittest


UPDATER = Path(__file__).with_name("update.sh")
LOCAL_PACKAGES = (
    "canopa-panel-protocol", "accesskit_unix", "iced_layershell", "layershellev"
)
SYSTEMCTL = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

path = Path(os.environ["NIRIBAR_TEST_SERVICE"])
state = json.loads(path.read_text())
args = sys.argv[1:]
state["calls"].append(args)
result = 0
if "show" in args:
    values = {"ActiveState": state["active"], "MainPID": str(state["pid"]),
              "LoadState": "loaded"}
    for arg in args:
        if arg.startswith("--property="):
            print(values[arg.split("=", 1)[1]])
elif "is-active" in args:
    try:
        status = Path(f"/proc/{state['pid']}/stat").read_text().split(") ", 1)[1][0]
        if status == "Z":
            state.update(active="failed", pid=0)
    except FileNotFoundError:
        state.update(active="failed", pid=0)
    result = 0 if state["active"] == "active" else 3
elif "reset-failed" in args:
    state["active"] = "inactive"
elif "restart" in args or ("try-restart" in args and state["active"] == "active"):
    if state["pid"]:
        try:
            os.kill(state["pid"], signal.SIGTERM)
        except ProcessLookupError:
            pass
    binary = Path.home() / ".local/bin/niribar"
    if state["mode"] == "wrong-binary":
        binary = Path(os.environ["NIRIBAR_TEST_OLD_BINARY"])
    command = [str(binary)]
    if state["mode"] == "crash":
        command.append("--version")  # Exit after systemd has reported a started process.
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    state.update(active="active", pid=process.pid)
path.write_text(json.dumps(state))
sys.exit(result)
'''


class UpdateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="niribar-update-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "checkout with spaces"
        self.repo.mkdir()
        self.write_project(self.repo, "current")
        scripts = self.repo / "scripts"
        scripts.mkdir()
        shutil.copy2(UPDATER, scripts / "update.sh")
        self.bin_dir = self.root / "user/.local/bin"
        self.bin_dir.mkdir(parents=True)
        self.service = self.root / "service.json"
        self.service.write_text(json.dumps(
            dict(active="inactive", pid=0, mode="normal", calls=[])
        ))
        commands = self.root / "commands"
        commands.mkdir()
        (commands / "systemctl").write_text(SYSTEMCTL)
        (commands / "systemctl").chmod(0o755)
        self.env = os.environ.copy()
        # Preserve the toolchain/cache while isolating the user's installation.
        self.env["CARGO_HOME"] = str(Path(os.environ.get("CARGO_HOME", "~/.cargo")).expanduser())
        self.env["RUSTUP_HOME"] = str(Path(os.environ.get("RUSTUP_HOME", "~/.rustup")).expanduser())
        self.env.update(HOME=str(self.root / "user"),
                        PATH=f"{commands}:{self.env['PATH']}",
                        CARGO_NET_OFFLINE="true",
                        NIRIBAR_TEST_SERVICE=str(self.service),
                        NIRIBAR_TEST_OLD_BINARY=str(self.root / "old-niribar"))
        self.addCleanup(self.stop_service)
        self.run_command(["cargo", "generate-lockfile", "--offline"], self.repo)

    def write_project(self, root, version):
        (root / "src/bin").mkdir(parents=True, exist_ok=True)
        deps = "\n".join(f'{name} = {{ path = "vendor/{name}" }}' for name in LOCAL_PACKAGES)
        (root / "Cargo.toml").write_text(
            '[package]\nname = "niribar"\nversion = "0.1.0"\nedition = "2021"\n'
            f'[dependencies]\n{deps}\n'
        )
        for name in LOCAL_PACKAGES:
            package = root / "vendor" / name
            (package / "src").mkdir(parents=True, exist_ok=True)
            (package / "Cargo.toml").write_text(
                f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n'
            )
            (package / "src/lib.rs").write_text(f'pub const VERSION: &str = "{version}";\n')
        source = (
            'fn main() {\n'
            ' if std::env::args().any(|arg| arg == "--version") {\n'
            f'  println!("{version} {{}}", canopa_panel_protocol::VERSION);\n'
            ' } else { loop { std::thread::sleep(std::time::Duration::from_secs(60)); } }\n'
            '}\n'
        )
        (root / "src/main.rs").write_text(source)
        (root / "src/bin/niribar-wifi-provider.rs").write_text(source)

    def run_command(self, args, cwd=None, check=True):
        result = subprocess.run(args, cwd=cwd or self.root, env=self.env,
                                text=True, capture_output=True, timeout=60)
        if check:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def update(self, check=True):
        return self.run_command(["bash", str(self.repo / "scripts/update.sh")], check=check)

    def state(self, **changes):
        state = json.loads(self.service.read_text())
        state.update(changes)
        self.service.write_text(json.dumps(state))
        return state

    def stop_service(self):
        pid = self.state()["pid"]
        if pid:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass

    def test_rebuilds_current_checkout_after_another_checkout_used_the_cache(self):
        baseline = self.repo / "target/audit-baseline"
        self.write_project(baseline, "old")
        self.run_command(["cargo", "build", "--release", "--bins", "--target-dir",
                          str(self.repo / "target")], baseline)
        self.update()
        for name in ("niribar", "niribar-wifi-provider"):
            result = self.run_command([str(self.bin_dir / name), "--version"])
            self.assertEqual(result.stdout.strip(), "current current")

    def test_recovers_failed_service_and_runs_installed_executable(self):
        self.state(active="failed")
        self.update()
        state = self.state()
        self.assertEqual(state["active"], "active")
        self.assertTrue(os.path.samefile(f"/proc/{state['pid']}/exe", self.bin_dir / "niribar"))

    def test_leaves_intentionally_stopped_service_stopped(self):
        result = self.update()
        self.assertEqual(self.state()["active"], "inactive")
        self.assertIn("inactive", result.stdout)

    def test_restarts_running_service_and_reports_verified_pid(self):
        self.state(active="active")
        result = self.update()
        state = self.state()
        self.assertEqual(state["active"], "active")
        self.assertTrue(os.path.samefile(f"/proc/{state['pid']}/exe", self.bin_dir / "niribar"))
        self.assertIn(f"PID {state['pid']}", result.stdout)

    def test_reports_startup_failure(self):
        self.state(active="active", mode="crash")
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("niribar.service failed during startup", result.stderr)

    def test_rejects_service_running_a_different_executable(self):
        self.run_command(["cargo", "build", "--release", "--target-dir", "target"], self.repo)
        shutil.copy2(self.repo / "target/release/niribar", self.root / "old-niribar")
        self.state(active="active", mode="wrong-binary")
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("executable", result.stderr)

    def test_build_failure_preserves_installed_binaries_and_service(self):
        for name in ("niribar", "niribar-wifi-provider"):
            (self.bin_dir / name).write_text("previous installation")
        (self.repo / "src/main.rs").write_text("invalid Rust")
        self.state(active="active")
        result = self.update(check=False)
        self.assertNotEqual(result.returncode, 0)
        for name in ("niribar", "niribar-wifi-provider"):
            self.assertEqual((self.bin_dir / name).read_text(), "previous installation")
        calls = self.state()["calls"]
        self.assertFalse(any("restart" in call or "try-restart" in call for call in calls))


if __name__ == "__main__":
    unittest.main()
