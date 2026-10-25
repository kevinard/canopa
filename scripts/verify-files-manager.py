#!/usr/bin/env python3
"""Exercise real FileManager1 cold activation in a private desktop and service directory."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import tempfile
from urllib.parse import quote_from_bytes

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


desktop = module("desktop", ROOT / "scripts/validate-desktop.py")
resident = module("resident_fixture", ROOT / "scripts/verify-files-resident.py")
NAME = "org.freedesktop.FileManager1"
OBJECT = "/org/freedesktop/FileManager1"


def uri(path):
    return "file://" + quote_from_bytes(os.fsencode(path), safe="/")


def verify(binary, session, scheme, scale):
    from PIL import ImageChops
    home = session.runtime / "home"
    home.mkdir()
    session.env.update(HOME=str(home), RUST_BACKTRACE="1", GSETTINGS_BACKEND="memory")
    resident.volume_fixture(session)
    config = session.runtime / "config/canopa/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    services = session.runtime / "services"
    services.mkdir()
    service = services / "org.freedesktop.FileManager1.service"
    shutil.copyfile(ROOT / "packaging/file-manager" / service.name, service)
    private_bin = session.runtime / "bin"
    private_bin.mkdir()
    marker = session.runtime / "activated-owner.json"
    # Keep the shipped activation file unchanged. Its PATH executable only
    # records process ownership/logs, then execs the actual application unchanged.
    launcher = private_bin / "canopa-files"
    launcher.write_text(
        "#!/usr/bin/python3\nimport json, os, sys\nfrom pathlib import Path\n"
        f"marker = Path({str(marker)!r})\n"
        "record = marker.with_name('owner-' + str(os.getpid()) + '.json')\n"
        "record.write_text(json.dumps({'pid': os.getpid()}))\nos.replace(record, marker)\n"
        f"log = os.open(str(Path({str(session.output)!r}) / ('files-' + str(os.getpid()) + '.log')), os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)\n"
        "os.dup2(log, 1)\nos.dup2(log, 2)\nos.close(log)\n"
        f"os.execv({str(binary)!r}, [{str(binary)!r}, *sys.argv[1:]])\n"
    )
    launcher.chmod(0o700)
    session.env["PATH"] = str(private_bin) + os.pathsep + session.env["PATH"]
    session.start_niri(
        'animations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; '
        'focus-ring { off; }; default-column-width { proportion 0.5; }; }\n',
        width=round(1280 * scale), height=round(800 * scale), scale=scale,
        service_directories=[services],
    )
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    notifications = session.output / "notifications.json"
    session.start([sys.executable, str(ROOT / "scripts/desktop-fixture/files-notifications.py"), str(notifications)], "notifications.log")
    session.wait_for(notifications.with_suffix(".ready").exists, "private notification receiver")
    bus = ["busctl", f'--address={session.env["DBUS_SESSION_BUS_ADDRESS"]}', "call"]

    def bus_call(destination, path, interface, method, *body):
        return session.run([*bus, destination, path, interface, method, *map(str, body)]).strip()

    def owned():
        return bus_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner", "s", NAME) == "b true"

    def api(method, uris, ok=True):
        command = [*bus, NAME, OBJECT, NAME, method, "ass", str(len(uris)), *uris, ""]
        result = subprocess.run(command, env=session.env, text=True, capture_output=True, timeout=10)
        assert (result.returncode == 0) == ok, (method, result.stdout, result.stderr)
        return result

    # Populate only this private bus's future-activation environment. An already
    # running Files process never imports display variables from later callers.
    keys = ["HOME", "PATH", "XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME",
            "WAYLAND_DISPLAY", "GIO_EXTRA_MODULES", "GIO_USE_VOLUME_MONITOR", "GIO_USE_VFS", "CANOPA_FIXTURE_VOLUMES",
            "DBUS_SYSTEM_BUS_ADDRESS", "PULSE_SERVER", "LIBGL_ALWAYS_SOFTWARE", "WGPU_BACKEND", "RUST_BACKTRACE", "GSETTINGS_BACKEND"]
    environment = {key: session.env[key] for key in keys if key in session.env}
    bus_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "UpdateActivationEnvironment", "a{ss}", len(environment),
             *(part for pair in environment.items() for part in pair))

    first = home / "Reveal café"
    second = home / "Reveal other"
    first.mkdir(); second.mkdir()
    raw = first / os.fsdecode(b"raw-\xff #.txt")
    hidden = first / ".requested"
    sibling = first / ".unrelated"
    for path, data in [(raw, b"FileManager1 original bytes\n"), (hidden, b"explicit hidden selection\n"), (sibling, b"unrelated hidden sibling\n")]:
        path.write_bytes(data)
    raw.chmod(0o640)
    link = first / "dangling link"
    link.symlink_to("unavailable target")
    missing = first / "missing item"
    folder = second / "Folder to select"
    folder.mkdir()
    (folder / "child.txt").write_bytes(b"The folder must be selected, not entered.\n")
    many = []
    for index in range(24):
        path = second / f"properties-{index:02d}.txt"
        path.write_bytes(f"Properties item {index}\n".encode())
        many.append(path)
    originals = {p: p.read_bytes() for p in home.rglob("*") if p.is_file() and not p.is_symlink()}
    checks, owners = [], []
    identity = binary.stat()

    def windows():
        return [w for w in json.loads(session.run(["niri", "msg", "--json", "windows"])) if w.get("app_id") == "org.canopa.Files"]

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "requested window focus")

    def narrow(window):
        session.run(["niri", "msg", "action", "set-window-width", "340"])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["layout"]["window_size"][0] == 340 for w in windows()),
                         "committed 340px window width")

    def settled_capture(name):
        previous, matching = None, 0
        def ready():
            nonlocal previous, matching
            frame = session.capture(name)
            matching = matching + 1 if previous is not None and ImageChops.difference(previous, frame).getbbox() is None else 0
            previous = frame
            return frame if matching >= 2 else None
        return session.wait_for(ready, "stable rendered frame")

    def key(name, *modifiers):
        session.run(["wtype", *(part for m in modifiers for part in ["-M", m]), "-k", name,
                     *(part for m in reversed(modifiers) for part in ["-m", m])])

    def clipboard(mime=None):
        try:
            result = subprocess.check_output(["wl-paste", "--no-newline", *(["--type", mime] if mime else [])], env=session.env, stderr=subprocess.PIPE, timeout=3)
            return result.decode().splitlines()
        except subprocess.SubprocessError:
            return None

    def properties(count):
        return next((w for w in windows() if w["title"] == f'Properties — {count} {"item" if count == 1 else "items"} — Canopa Files'), None)

    def record_owner():
        pid = int(bus_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixProcessID", "s", NAME).split()[1])
        assert json.loads(marker.read_text())["pid"] == pid
        if any(old == pid for old, _ in owners): return pid
        handle = os.pidfd_open(pid)
        actual = os.stat(f"/proc/{pid}/exe")
        if (identity.st_dev, identity.st_ino) != (actual.st_dev, actual.st_ino):
            os.close(handle)
            raise AssertionError("activated FileManager1 owner is not the selected Files executable")
        owners.append((pid, handle))
        return pid

    def exited(handle):
        poll = select.poll(); poll.register(handle, select.POLLIN)
        return bool(poll.poll(0))

    def close_window(window):
        session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
        session.wait_for(lambda: not any(w["id"] == window["id"] for w in windows()), "native window retirement")

    def quit_owner():
        subprocess.run(["wl-copy", "--clear"], env=session.env, check=True, timeout=5)
        result = subprocess.run([str(binary), "--quit"], env=session.env, text=True, capture_output=True, timeout=10)
        assert result.returncode == 0, result.stderr
        session.wait_for(lambda: not owned() and all(exited(fd) for _, fd in owners), "released FileManager1 name and exact owner exit")

    try:
        assert not owned() and not windows()
        api("ShowItemProperties", [uri(p) for p in [raw, link, missing]])
        pid = record_owner()
        inspector = session.wait_for(lambda: properties(3), "cold Properties without a Browser")
        assert len(windows()) == 1, windows()
        session.capture("properties-cold.png")
        checks.append("the shipped service cold-starts the actual owning backend and opens Properties without creating a Browser")
        focus(inspector); key("c", "ctrl")
        session.wait_for(lambda: clipboard() == [uri(p) for p in [raw, link, missing]], "byte-preserving Properties locations")
        checks.append("read-only Properties keeps raw names, dangling links and individual unavailable items; Copy locations preserves exact URIs")
        before = {w["id"] for w in windows()}
        api("ShowItems", [uri(raw), "https://example.test/remote"], ok=False)
        for method in ["ShowFolders", "ShowItems", "ShowItemProperties"]: api(method, [])
        assert {w["id"] for w in windows()} == before
        checks.append("invalid mixed requests are refused and all three empty requests create no extra windows")
        focus(inspector); key("Escape")
        session.wait_for(lambda: not windows(), "Properties Escape closes its only window")
        assert record_owner() == pid
        checks.append("closing the initial Properties window keeps the same resident backend available")

        api("ShowItems", [uri(raw), uri(hidden), uri(folder)])
        browser = session.wait_for(lambda: next((w for w in windows() if w["title"] == f"{first.name} — Canopa Files"), None), "grouped reveal first parent")
        focus(browser)
        narrow(browser)
        key("c", "ctrl")
        session.wait_for(lambda: set(clipboard("text/uri-list") or []) == {uri(raw), uri(hidden)}, "actual selected raw and hidden files")
        settled_capture("reveal-narrow.png")
        checks.append("ShowItems groups raw and explicitly hidden files in their parent; the exact group is selected in a 340px tile")
        key("Tab", "ctrl")
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["title"] == f"{second.name} — Canopa Files" for w in windows()), "inactive parent Tab")
        key("c", "ctrl")
        session.wait_for(lambda: clipboard("text/uri-list") == [uri(folder)], "requested folder selected within its parent")
        checks.append("an inactive parent Tab retains its initial selection and reveals a requested folder without entering it")
        key("Left")
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["title"] == "home — Canopa Files" for w in windows()), "same narrow parent navigation")
        key("c", "ctrl")
        session.wait_for(lambda: clipboard("text/uri-list") == [uri(second)], "parent navigation selects the folder left")
        checks.append("Left retains its parent-navigation meaning after external reveal in a narrow tile")

        before = {w["id"] for w in windows()}
        api("ShowFolders", [uri(first), uri(second)])
        opened = session.wait_for(lambda: next((w for w in windows() if w["id"] not in before and w["title"] == f"{first.name} — Canopa Files"), None), "requested folder contents")
        focus(opened); key("l", "ctrl"); key("c", "ctrl")
        session.wait_for(lambda: clipboard() == [str(first)], "first folder location")
        key("Escape"); key("Tab", "ctrl"); key("l", "ctrl"); key("c", "ctrl")
        session.wait_for(lambda: clipboard() == [str(second)], "second folder location")
        key("Escape")
        checks.append("ShowFolders opens the requested contents in distinct Tabs without selecting the folders in their parents")
        before = {w["id"] for w in windows()}
        cli = subprocess.run([str(binary), str(first)], env=session.env, text=True, capture_output=True, timeout=10)
        assert cli.returncode == 0, cli.stderr
        cli_window = session.wait_for(lambda: next((w for w in windows() if w["id"] not in before), None), "CLI activation into the bus owner")
        assert record_owner() == pid
        close_window(cli_window)
        checks.append("ordinary CLI activation opens a window in the same bus-activated runtime and history writer")

        api("ShowItemProperties", [uri(p) for p in many])
        inspector = session.wait_for(lambda: properties(24), "prepared Properties list")
        for window in list(windows()):
            if window["id"] != inspector["id"]: close_window(window)
        assert len(windows()) == 1
        focus(inspector)
        narrow(inspector)
        key("Home")
        before_frame = settled_capture("properties-narrow-start.png")
        key("End")
        session.wait_for(lambda: ImageChops.difference(before_frame, session.capture("properties-narrow-end.png")).convert("L").point(lambda p: 255 if p > 12 else 0).histogram()[255] > 2000,
                         "End scrolls the prepared Properties list")
        settled_capture("properties-narrow-end.png")
        key("c", "ctrl")
        session.wait_for(lambda: clipboard() == [uri(p) for p in many], "Properties keeps its captured locations after Browsers close")
        checks.append("Properties survives all Browsers closing, wraps a 340px tile, scrolls by End and retains its captured URI list")
        key("w", "ctrl")
        session.wait_for(lambda: not windows(), "Properties Ctrl+W")
        quit_owner()
        checks.append("Ctrl+W closes Properties and explicit Quit releases residency, the bus name and exact activated process")
        api("ShowFolders", [uri(first)])
        new_pid = record_owner()
        assert new_pid != pid
        window = session.wait_for(lambda: next((w for w in windows() if w["title"] == f"{first.name} — Canopa Files"), None), "a new cold activation after Quit")
        close_window(window); quit_owner()
        checks.append("a subsequent cold request starts a new exclusive owner after orderly shutdown")
        assert all(path.read_bytes() == data for path, data in originals.items())
        assert not missing.exists() and os.readlink(link) == "unavailable target"
        checks.append("all presentation requests leave original files, hidden siblings and symlink contents unchanged")
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
                "activated_pids": [pid for pid, _ in owners], "service_sha256": hashlib.sha256(service.read_bytes()).hexdigest(),
                "scope": "actual private-bus activation, local URI presentations, read-only Properties and runtime lifetime; full product/platform/release gates remain separate"}
    except Exception:
        try: session.capture("failure.png")
        except (OSError, subprocess.SubprocessError): pass
        raise
    finally:
        if marker.exists():
            # A failed registration can leave --daemon alive. Capture only the
            # exact fixture launch after confirming its executable and HOME.
            candidate = json.loads(marker.read_text())["pid"]
            handle = None
            try:
                if not any(pid == candidate for pid, _ in owners):
                    handle = os.pidfd_open(candidate)
                    actual = os.stat(f"/proc/{candidate}/exe")
                    private = f"HOME={home}".encode() in Path(f"/proc/{candidate}/environ").read_bytes().split(b"\0")
                    if private and (actual.st_dev, actual.st_ino) == (identity.st_dev, identity.st_ino):
                        owners.append((candidate, handle)); handle = None
            except (FileNotFoundError, ProcessLookupError):
                pass
            finally:
                if handle is not None: os.close(handle)
        for _, handle in owners:
            try:
                if not exited(handle):
                    signal.pidfd_send_signal(handle, signal.SIGTERM)
                    poll = select.poll(); poll.register(handle, select.POLLIN)
                    if not poll.poll(5000):
                        signal.pidfd_send_signal(handle, signal.SIGKILL)
                        assert poll.poll(5000), "private activated owner did not retire after cleanup"
            except ProcessLookupError:
                pass
            finally:
                os.close(handle)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-files")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scheme", choices=("dark", "light"), default="dark")
    parser.add_argument("--scale", type=float, default=1.0)
    args = parser.parse_args()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    result = {"status": "failed"}
    with tempfile.TemporaryDirectory(prefix="canopa-files-native-manager-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            result = verify(args.binary.resolve(), session, args.scheme, args.scale)
        except Exception as error:
            result["error"] = str(error)
            raise
        finally:
            try: session.close()
            finally: (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
