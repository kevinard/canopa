#!/usr/bin/env python3
"""Private real legacy parent/worker/compositor keyboard transport acceptance.

This is internal transport acceptance, not public Notify, consent, pointer/touch,
physical-input or distribution acceptance.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def identity(pid):
    try:
        proc = Path("/proc") / str(pid)
        fields = (proc / "stat").read_text().rsplit(") ", 1)[1].split()
        return {"state": fields[0], "ppid": int(fields[1]), "start_ticks": fields[19],
            "command": (proc / "cmdline").read_bytes().replace(b"\0", b" ").decode()}
    except FileNotFoundError:
        return None


def verify(args, session, result):
    from gi.repository import Gio
    session.env.update(PATH=str(args.niri.parent) + os.pathsep + session.env["PATH"],
        CANOPA_INPUT_LEGACY_NATIVE="1", CANOPA_INPUT_WORKER_BINARY=str(args.worker))
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
''')
    for mode, suffix in [("client-header", ".h"), ("private-code", ".c")]:
        subprocess.run(["wayland-scanner", mode, "/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml",
            str(session.output / ("xdg-shell" + suffix))], check=True, timeout=15)
    receiver_binary = session.output / "receiver"
    subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", "-I", str(session.output),
        str(ROOT / "scripts/desktop-fixture/remote-input-receiver.c"), str(session.output / "xdg-shell.c"),
        "-lwayland-client", "-lxkbcommon", "-o", str(receiver_binary)], check=True, timeout=30)
    receiver = session.start([str(receiver_binary)], "receiver.log")
    receiver_log = session.output / "receiver.log"
    session.wait_for(lambda: "focused" in receiver_log.read_text(), "harmless receiver")

    def lines():
        assert receiver.poll() is None
        return receiver_log.read_text().splitlines()

    def received(fragment, before):
        return any(fragment in line for line in lines()[before:])

    def finish(process, status=0):
        if process in session.processes:
            session.processes.remove(process)
        actual = process.wait(timeout=10)
        assert actual == status, (process.args, actual, status)

    for case in ("parent-eof", "worker-sigkill", "parent-sigkill", "caller-loss", "keymap-loss"):
        caller = Gio.DBusConnection.new_for_address_sync(session.env["DBUS_SESSION_BUS_ADDRESS"],
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        before = len(lines())
        log = session.output / (case + "-parent.log")
        with log.open("w") as stream:
            parent = subprocess.Popen([str(args.harness), "--exact", "remote_input::parent::tests::native_legacy_parent_fixture",
                "--ignored", "--nocapture", "--test-threads=1"], stdin=subprocess.PIPE, stdout=stream, stderr=stream,
                env=session.env, start_new_session=True)
        session.processes.append(parent)
        parent.stdin.write(json.dumps({"caller": caller.get_unique_name(), "worker_success": case != "worker-sigkill"}).encode() + b"\n")
        parent.stdin.flush()
        session.wait_for(lambda: "legacy-parent-ready " in log.read_text(), "actual legacy parent and acknowledged symbols", timeout=40)
        info = json.loads(next(line.split("legacy-parent-ready ", 1)[1] for line in log.read_text().splitlines() if "legacy-parent-ready " in line))
        worker = info["worker"]
        owned = identity(worker)
        assert owned and owned["ppid"] == parent.pid and str(args.worker) in owned["command"] and "--remote-input-worker" in owned["command"]
        session.wait_for(lambda: received("key 16 1 A", before) and received("key 48 1 B", before), "real French symbol chords")
        session.wait_for(lambda: received("key 16 0 A", before), "first symbol released while second holds Shift")
        assert not received("key 42 0 ", before), "released Shift while B and raw Shift still own it"
        if case == "parent-eof":
            parent.stdin.close()
        elif case == "worker-sigkill":
            current = identity(worker)
            assert current and current["start_ticks"] == owned["start_ticks"] and current["ppid"] == parent.pid
            os.kill(worker, signal.SIGKILL)
        elif case == "parent-sigkill":
            parent.kill()
            finish(parent, -signal.SIGKILL)
        elif case == "caller-loss":
            caller.close_sync(None)
        else:
            config = session.runtime / "niri.kdl"
            config.write_text(config.read_text().replace('layout "fr"', 'layout "us"'))
        if case != "parent-sigkill":
            finish(parent)
            assert "legacy-parent-retired " + str(case != "worker-sigkill").lower() in log.read_text()
        session.wait_for(lambda: received("key 42 0 Shift_L", before) and received("key 48 0 ", before)
            and received("mods 0 ", before), "all held legacy keys and modifiers released")
        session.wait_for(lambda: (now := identity(worker)) is None or now["start_ticks"] != owned["start_ticks"]
            or now["state"] == "Z", "owned legacy worker terminal")
        if not caller.is_closed():
            caller.close_sync(None)
        result["checks"].append(case + "-real-legacy-parent-worker-symbol-cleanup")
        result.setdefault("workers", []).append({"pid": worker, "start_ticks": owned["start_ticks"], "terminal": True})
        (session.output / "progress.json").write_text(json.dumps(result, indent=2) + "\n")
        print(case + ": passed", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("niri", "worker", "harness"):
        parser.add_argument("--" + name, required=True, type=Path)
        parser.add_argument("--" + name + "-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    for name in ("niri", "worker", "harness"):
        path = getattr(args, name).resolve()
        setattr(args, name, path)
        assert digest(path) == getattr(args, name + "_sha256")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    result = {"status": "running", "scope": __doc__, "checks": [],
        "binaries": {name: {"path": str(getattr(args, name)), "sha256": getattr(args, name + "_sha256")} for name in ("niri", "worker", "harness")},
        "fixture_sources": [{"path": str(path.relative_to(ROOT)), "sha256": digest(path)} for path in
            (Path(__file__), ROOT / "scripts/desktop-fixture/remote-input-receiver.c", ROOT / "scripts/validate-desktop.py")]}
    archive = args.output / "verified-sources"
    for entry in result["fixture_sources"]:
        target = archive / entry["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / entry["path"], target)
        assert digest(target) == entry["sha256"]
    (archive / "receipt.json").write_text(json.dumps(result["fixture_sources"], indent=2) + "\n")
    try:
        with tempfile.TemporaryDirectory(prefix="canopa-legacy-parent-") as directory:
            session = desktop.Session(Path(directory), args.output)
            try:
                verify(args, session, result)
            finally:
                session.close()
        for entry in result["binaries"].values():
            assert digest(Path(entry["path"])) == entry["sha256"]
        result["changed_fixture_sources"] = [entry["path"] for entry in result["fixture_sources"] if digest(ROOT / entry["path"]) != entry["sha256"]]
        assert not result["changed_fixture_sources"]
        result.update(status="passed", binaries_unchanged=True)
    except Exception as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
