#!/usr/bin/env python3
"""Exercise product validation/migration using already-built workspace binaries."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BIN = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve() / "debug"


def main():
    with tempfile.TemporaryDirectory(prefix="canopa-config-") as directory:
        base = Path(directory)
        env = dict(os.environ, XDG_CONFIG_HOME=str(base), HOME=str(base))
        env.pop("WAYLAND_DISPLAY", None)
        env.pop("DBUS_SESSION_BUS_ADDRESS", None)

        def run(*args, success=True):
            result = subprocess.run([str(BIN / "canopa"), "config", *map(str, args)],
                                    env=env, capture_output=True, text=True, timeout=15)
            assert (result.returncode == 0) == success, (args, result.stdout, result.stderr)
            return result

        def reports(result):
            return [json.loads(line) for line in result.stdout.splitlines()]

        default = reports(run("check"))
        assert len(default) == 2 and all(r["source"]["defaults"] for r in default)
        shell = base / "shell.kdl"
        notifications = base / "notifications.kdl"
        shell.write_text('/- kdl-version 2\n// retained shell comment\nbar "main" {}\n')
        notifications.write_text('sound enabled=#false\n')
        output = base / "candidate.kdl"
        run("migrate", "--shell", shell, "--notifications", notifications, "--output", output)
        candidate = output.read_bytes()
        assert b"retained shell comment" in candidate
        assert output.stat().st_mode & 0o777 == 0o600
        checked = reports(run("check", "--file", output))
        assert all(r["source"]["path"] == str(output) for r in checked)
        assert checked[0]["source"]["fingerprint"] == checked[1]["source"]["fingerprint"]
        run("migrate", "--shell", shell, "--output", output, success=False)
        assert output.read_bytes() == candidate
        assert shell.read_text().startswith("/- kdl-version 2\n// retained shell comment")
        invalid = base / "invalid.kdl"
        invalid.write_text('schema-version 1\nshell { private-shell 42; }\nnotifications { private-notify 42; }\n')
        failed = run("check", "--file", invalid, success=False)
        assert all("error" in r for r in reports(failed))
        assert "private-shell" not in failed.stdout + failed.stderr
        assert "private-notify" not in failed.stdout + failed.stderr
        invalid.write_text('schema-version 1\nshell {}\nnotifications { private-notify 42; }\n')
        isolated = reports(run("check", "--file", invalid, success=False))
        assert "source" in isolated[0] and "error" in isolated[1]
        invalid.write_text('schema-version 1\nshell { private-syntax {\n')
        safe = run("check", "--file", invalid, success=False)
        assert "private-syntax" not in safe.stdout + safe.stderr
        rich = run("check", "--file", invalid, "--show-source", success=False)
        assert "private-syntax" in rich.stderr
        assert all(r["error"]["code"] == "syntax" for r in reports(rich))
        shell.write_text('private-shell 42\n')
        rejected = base / "rejected.kdl"
        run("migrate", "--shell", shell, "--output", rejected, success=False)
        assert not rejected.exists()
        oversized = base / "oversized.kdl"
        oversized.write_bytes(b" " * (1024 * 1024 + 1))
        oversize_reports = reports(run("check", "--file", oversized, success=False))
        assert len(oversize_reports) == 2
        assert all(r["error"]["code"] == "read" for r in oversize_reports)
        # A legacy file must not override a present invalid shared file.
        (base / "niribar").mkdir()
        (base / "niribar/config.kdl").write_text('bar "legacy" {}\n')
        (base / "canopa").mkdir()
        (base / "canopa/config.kdl").write_text('schema-version 2\n')
        assert all("error" in r for r in reports(run("check", success=False)))
    print("Canopa configuration: validation, isolation, migration, privacy and no-clobber checks passed.")


if __name__ == "__main__":
    main()
