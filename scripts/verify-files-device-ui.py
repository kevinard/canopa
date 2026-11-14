#!/usr/bin/env python3
"""Exercise Iced device controls with actual private GIO requests and windows."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote_from_bytes

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
sys.path.insert(0, str(ROOT / "scripts"))
import native_accessibility  # noqa: E402


def verify(binary, session, scheme, scale, removal):
    home = session.runtime / "home"
    local = home / "Downloads"
    other = local / "Elsewhere"
    other.mkdir(parents=True)
    media = session.runtime / "volumes"
    payload = media / "Fixture USB"
    payload.mkdir(parents=True)
    original = b"Actual Iced device bytes remain unchanged.\n"
    (payload / "notes.txt").write_bytes(original)
    if removal:
        (media / f"drive-{removal}").touch()
    (local / "local.txt").write_bytes(b"Local directory remains usable.\n")
    config = session.runtime / "config/canopa/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n'
                      'files { sidebar "always"; places { place "home"; place "downloads"; };'
                      ' shortcut "operations" "<Alt>o"; }\n')
    modules = session.output / "gio-modules"
    modules.mkdir()
    flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "gio-2.0"], text=True).split()
    # Use the fixture as the native provider so host mounts never enter the menu.
    subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                    "-DCANOPA_FIXTURE_NATIVE_MONITOR",
                    str(ROOT / "scripts/desktop-fixture/files-volume-monitor.c"),
                    "-o", str(modules / "libcanopa-volume-fixture.so"), *flags], check=True, timeout=30)
    session.env.update(HOME=str(home), GIO_EXTRA_MODULES=str(modules), GIO_USE_VOLUME_MONITOR="canopa-private-volume-fixture",
                       GIO_USE_VFS="local", CANOPA_FIXTURE_VOLUMES=str(media))
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\n'
                       'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; };'
                       ' focus-ring { off; }; default-column-width { proportion 1.0; }; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale)
    # Toolbar controls are found through Files' own AT-SPI tree, not by position.
    native_accessibility.start(session)
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    notices = session.output / "notifications.json"
    session.start([sys.executable, str(ROOT / "scripts/desktop-fixture/files-notifications.py"), str(notices)], "notifications.log")
    session.wait_for(notices.with_suffix(".ready").exists, "private notifications")
    inventory = session.run(["gio", "mount", "--list"])
    (session.output / "provider-inventory.txt").write_text(inventory)
    assert inventory.count("Mount(") == 1 and "Fixture USB" in inventory, inventory
    (media / "events").unlink(missing_ok=True)
    process = session.start([str(binary), str(local)], "files.log")
    checks = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((window for window in windows() if window.get("title") == f"{name} — Canopa Files"), None)

    def wait_title(name):
        window = session.wait_for(lambda: title(name), name)
        time.sleep(.2)
        return window

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "device fixture focus")
        time.sleep(.15)

    def key(name, count=1):
        session.run(["wtype", *[arg for _ in range(count) for arg in ("-k", name)]])
        time.sleep(.15)

    def chord(name, modifier="ctrl"):
        session.run(["wtype", "-M", modifier, "-k", name, "-m", modifier])
        time.sleep(.2)

    def navigate(path):
        chord("l")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        key("Return")
        wait_title(path.name)

    def events():
        return (media / "events").read_text()

    def count(event):
        return events().count(event)

    def notified(body, after):
        def delivered():
            try:
                return next((notice for notice in json.loads(notices.read_text())[after:] if notice["body"] == body), None)
            except (FileNotFoundError, json.JSONDecodeError):
                return None
        session.wait_for(delivered, f"device notification {body}")
        time.sleep(.2)

    def notice_count():
        return len(json.loads(notices.read_text())) if notices.exists() else 0

    def locations(*, mounted, remove=False, capture=None):
        # A window without the sidebar offers its places from the toolbar's
        # Places button: configured places, then each device's Open/Mount and
        # removal rows.
        native_accessibility.request(session, "--window", "canopa-files", "--name", "Places", "--click")
        time.sleep(.3)
        key("End")
        if mounted and not remove:
            key("Up")  # Open precedes the current removal capability.
        if capture:
            session.capture(capture)
        key("Return")

    def unmount(browser):
        focus(browser)
        before = notice_count()
        locations(mounted=True, remove=True)
        notified("Device unmounted.", before)
        assert (payload / "notes.txt").read_bytes() == original

    def narrow(window):
        session.run(["niri", "msg", "action", "set-window-width", "340"])
        session.wait_for(lambda: next(w for w in windows() if w["id"] == window["id"])["layout"]["window_size"][0] == 340, "narrow browser")
        time.sleep(.2)

    def close_native(window):
        session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
        session.wait_for(lambda: not any(w["id"] == window["id"] for w in windows()), "exact device window retirement")

    def credentials(dialog):
        focus(dialog)
        for value in ["private-user", "private-domain", "private-secret"]:
            key("Tab")
            chord("a")
            session.run(["wtype", value])
        session.capture("device-password-masked.png")
        key("Return")
        session.wait_for(lambda: not any(w["id"] == dialog["id"] for w in windows()), "answered credential window retirement")

    browser = wait_title("Downloads")
    session.wait_for(lambda: "private provider initialized" in events(), "private volume observation")
    pointer = session.build_pointer()
    session.capture("device-wide-before.png")
    # Two configured locations keep the device section at this known position.
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(output["logical"] for output in outputs.values() if output.get("logical"))
    session.run([str(pointer), str(round(75 * 1600 / logical["width"])), str(round(277 * 1000 / logical["height"]))])
    wait_title("Fixture USB")
    checks.append("wide sidebar opens the prepared device root")
    key("Left")
    wait_title("volumes")
    narrow(browser)
    locations(mounted=True)
    wait_title("Fixture USB")
    key("Left")
    wait_title("volumes")
    checks.append("narrow Locations uses the same root and Left retains parent navigation")
    if removal:
        # Keep the same owner after the operation completes with no windows.
        activated = subprocess.run([str(binary), "--daemon"], env=session.env, capture_output=True, timeout=10)
        assert activated.returncode == 0, activated.stderr
        (media / "hold-drive").touch()
        before = notice_count()
        locations(mounted=True, remove=True, capture=f"device-{removal}-menu.png")
        session.wait_for(lambda: count(f"{removal} requested without force") == 1, "native drive request")
        close_native(browser)
        session.wait_for(lambda: not any(w.get("app_id") == "org.canopa.Files" for w in windows()), "windowless drive operation")
        assert process.poll() is None and count(f"{removal} completed") == 0
        (media / "release-drive").touch()
        body = "Device powered off." if removal == "stop" else "Device ejected."
        notified(body, before)
        assert count(f"{removal} completed") == 1 and process.poll() is None
        checks.append(f"explicit {removal} uses the native no-force callback and completes with every window closed")

        activated = subprocess.run([str(binary), str(local)], env=session.env, capture_output=True, timeout=10)
        assert activated.returncode == 0, activated.stderr
        browser = wait_title("Downloads")
        focus(browser)
        chord("o", "alt")
        operations = wait_title("Operations")
        session.capture(f"device-{removal}-result.png")
        assert count("private provider initialized") == 1, "reactivation replaced the owning runtime"
        assert "UNEXPECTED forced" not in events() and "unmount requested" not in events()
        assert (payload / "notes.txt").read_bytes() == original
        close_native(operations)
        close_native(browser)
        quit_result = subprocess.run([str(binary), "--quit"], env=session.env, capture_output=True, timeout=10)
        assert quit_result.returncode == 0, quit_result.stderr
        assert process.wait(timeout=10) == 0
        checks.append("reactivation uses the same backend/provider and explicit Quit retires it after its result remains inspectable")
        return {"status": "passed", "scheme": scheme, "scale": scale, "removal": removal, "checks": checks,
                "scope": "Iced power-off/eject selection, actual private GDrive callbacks, windowless completion and same-owner reopening; no physical storage proof"}

    unmount(browser)
    session.capture("device-narrow-unmounted.png")
    checks.append("explicit Unmount performs the real private GIO request and preserves old-directory bytes")

    (media / "password-mount").touch()
    before = notice_count()
    locations(mounted=False)
    dialog = wait_title("Unlock device")
    assert dialog["is_floating"]
    key("Return")  # Initial focus must be Cancel.
    notified("Cancelled.", before)
    session.wait_for(lambda: title("Unlock device") is None, "default Cancel retires credentials")
    assert count("credentials accepted without saving") == 0
    checks.append("credential question floats above the narrow tile with Cancel as its default")
    focus(browser)
    locations(mounted=False)
    dialog = wait_title("Unlock device")
    credentials(dialog)
    wait_title("Fixture USB")
    assert count("credentials accepted without saving") == 1
    assert (payload / "notes.txt").read_bytes() == original
    (media / "password-mount").unlink()
    checks.append("keyboard username/domain/masked password reach GIO without saving and navigate the unchanged origin")

    focus(browser)
    navigate(local)
    unmount(browser)
    (media / "hold-mount").touch()
    before = count("mount requested")
    locations(mounted=False)
    session.wait_for(lambda: count("mount requested") > before, "held native mount")
    navigate(other)
    (media / "release-mount").touch()
    session.wait_for(lambda: count("mount completed") >= 2, "held mount completion")
    time.sleep(.5)
    assert title("Elsewhere") and title("Fixture USB") is None
    (media / "hold-mount").unlink()
    (media / "release-mount").unlink()
    checks.append("late mount completion preserves newer navigation")

    unmount(browser)
    (media / "hold-mount").touch()
    before = count("mount requested")
    locations(mounted=False)
    session.wait_for(lambda: count("mount requested") > before, "mount owned by original tab")
    chord("t")
    time.sleep(.3)
    chord("Tab", "ctrl")  # Return to the original tab.
    chord("w")
    (media / "release-mount").touch()
    session.wait_for(lambda: count("mount completed") >= 3, "closed-tab mount completion")
    time.sleep(.5)
    assert title("Elsewhere") and title("Fixture USB") is None
    assert len([w for w in windows() if w.get("app_id") == "org.canopa.Files"]) == 1
    (media / "hold-mount").unlink()
    (media / "release-mount").unlink()
    checks.append("closing the originating tab neither redirects another tab nor opens a replacement window")

    unmount(browser)
    (media / "password-mount").touch()
    locations(mounted=False)
    wait_title("Unlock device")
    cancelled = count("mount interaction cancelled")
    close_native(browser)
    session.wait_for(lambda: not any(w.get("app_id") == "org.canopa.Files" for w in windows()), "all device windows closed")
    time.sleep(.3)
    assert process.poll() is None and count("mount interaction cancelled") == cancelled
    activated = subprocess.run([str(binary), str(other)], env=session.env, capture_output=True, timeout=10)
    assert activated.returncode == 0, activated.stderr
    browser = wait_title("Elsewhere")
    focus(browser)
    narrow(browser)
    chord("o", "alt")
    operations = wait_title("Operations")
    focus(operations)
    key("Tab", 4)  # New browser, Recovery, active Cancel, active Review.
    key("Return")
    dialog = wait_title("Unlock device")
    session.capture("device-windowless-review.png")
    key("Return")
    session.wait_for(lambda: count("mount interaction cancelled") > cancelled, "explicit Cancel after reopening")
    (media / "password-mount").unlink()
    close_native(operations)
    checks.append("closing all windows preserves a pending credential question; Operations reopens the same request for explicit cancellation")

    focus(browser)
    (media / "question-mount").touch()
    (media / "hold-question").touch()
    locations(mounted=False)
    dialog = wait_title("Device question")
    key("Tab")  # Focus the old provider action after the shared Cancel.
    (media / "release-question").touch()
    session.wait_for(lambda: count("current question offered") >= 1, "newer native question")
    time.sleep(.3)
    session.capture("device-newer-question.png")
    key("Return")  # A newer question resets old focus to Cancel.
    session.wait_for(lambda: title("Device question") is None, "updated question cancellation")
    (media / "hold-question").unlink()
    (media / "release-question").unlink()
    focus(browser)
    locations(mounted=False)
    dialog = wait_title("Device question")
    key("Tab")
    key("Return")
    wait_title("Fixture USB")
    (media / "question-mount").unlink()
    checks.append("a superseding question retires old control focus; only the current explicit choice completes mounting")
    assert (payload / "notes.txt").read_bytes() == original
    assert "UNEXPECTED forced unmount" not in events()
    assert "private-secret" not in (session.output / "files.log").read_text()
    assert "private-secret" not in notices.read_text() and "private-secret" not in config.read_text()
    session.capture("device-final.png")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Iced controls, private GIO provider and actual window/input/notification lifecycle; no physical drive or power-off/eject proof"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scheme", choices=("dark", "light"), default="dark")
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--removal", choices=("stop", "eject"))
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    with binary.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    result = {"status": "failed", "binary": str(binary), "sha256": digest}
    with tempfile.TemporaryDirectory(prefix="canopa-files-native-device-ui-") as directory:
        runtime = Path(directory)
        runtime.chmod(0o700)
        session = desktop.Session(runtime, output)
        try:
            result.update(verify(binary, session, args.scheme, args.scale, args.removal))
        except Exception as error:
            result["error"] = str(error)
            try:
                session.capture("device-failure.png")
                (output / "windows.json").write_text(session.run(["niri", "msg", "--json", "windows"]))
            except Exception:
                pass
            raise
        finally:
            if (runtime / "volumes/events").exists():
                (output / "device-events.log").write_bytes((runtime / "volumes/events").read_bytes())
            session.close()
            (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
