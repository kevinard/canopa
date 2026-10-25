"""Live application Retry/Continue, windowless ownership and exact real files."""
import json
import os
import subprocess
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Continuation fixtures"
    assert root.is_relative_to(session.runtime / "home")
    source = root / "Source/bundle"
    destination = root / "Destination/bundle"
    source.mkdir(parents=True)
    destination.mkdir(parents=True)
    (source / "a-completed.txt").write_bytes(b"Confirmed prefix\n")
    os.mkfifo(source / "b-failed")
    (source / "c-conflict.txt").write_bytes(b"Incoming version\n")
    (source / "d-remaining.txt").write_bytes(b"Remaining bytes\n")
    (destination / "c-conflict.txt").write_bytes(b"Existing version\n")
    config.write_text(config.read_text() + 'files { shortcut "operations" "<Alt>o"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Continuation fixtures"
    source = root / "Source/bundle"
    destination = root / "Destination/bundle"
    binary = process.args[0]

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def chord(key, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", key]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "continuation input focus")

    def navigate(path):
        chord("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        chord("Return")
        session.wait_for(lambda: title(path.name), "continuation navigation")
        time.sleep(.3)

    def notices():
        path = session.output / "notifications.json"
        return [e for e in json.loads(path.read_text()) if e["title"].startswith("Copy")] if path.exists() else []

    def clipboard():
        result = subprocess.run(["wl-paste", "--type", "text/plain"], env=session.env,
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=5)
        return result.stdout.decode().strip() if result.returncode == 0 else None

    def operations():
        focus(browser)
        chord("o", "alt")
        current = session.wait_for(lambda: title("Operations"), "continuation Operations")
        focus(current)
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        time.sleep(.3)
        return current

    focus(browser)
    navigate(source.parent)
    chord("a", "ctrl")
    chord("c", "ctrl")
    navigate(destination.parent)
    chord("v", "ctrl")
    session.wait_for(lambda: title("Folder conflict") and title("Folder conflict")["is_focused"], "folder Merge question")
    chord("Tab")
    chord("Return")
    session.wait_for(lambda: title("File conflict") and title("File conflict")["is_focused"], "child failure followed by file conflict")
    prefix = destination / "a-completed.txt"
    inode = prefix.stat().st_ino
    assert prefix.read_bytes() == b"Confirmed prefix\n"
    chord("Return")
    session.wait_for(lambda: notices(), "saved stopped transfer result")
    assert not (destination / "b-failed").exists()
    assert not (destination / "d-remaining.txt").exists()
    (source / "b-failed").unlink()
    (source / "b-failed").write_bytes(b"Repaired child\n")
    # Remove Files' clipboard lifetime hold. The only remaining ownership must
    # be the backend's exact stopped plans, independent of every window.
    session.start(["wl-copy", "--foreground", "--type", "text/plain", "New external clipboard"], "continuation-external-clipboard.log")
    session.wait_for(lambda: clipboard() == "New external clipboard", "external clipboard owns transport")
    for window in windows():
        if window.get("app_id") == "org.canopa.Files":
            session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
    session.wait_for(lambda: not any(w.get("app_id") == "org.canopa.Files" for w in windows()), "all continuation windows closed")
    session.run([binary, "--quit"])
    time.sleep(.8)
    assert process.poll() is None, "window closure or Quit discarded the only executable plans"
    session.capture("continuation-no-windows.png")
    session.run([binary, str(destination.parent)])
    browser = session.wait_for(lambda: title("Destination"), "same-owner reactivation")
    assert process.poll() is None
    current = operations()
    session.capture("continuation-retained-actions.png")
    order = ["retry", "remaining"] if scheme == "dark" else ["remaining", "retry"]
    for index, part in enumerate(order):
        if index:
            chord("w", "ctrl")
            session.wait_for(lambda: title("Operations") is None, "reset Operations keyboard traversal")
            current = operations()
        # Fresh Operations focus: New Browser, Undo, Recovery, then row controls.
        # A finished nested successor adds only Details ahead of the original
        # row; it has no top-level Open-folder target. The original row now has
        # exactly one executable part. Keep all byte/conflict/Undo oracles strict.
        stops = (5 if part == "retry" else 6) if index == 0 else 6
        for _ in range(stops):
            chord("Tab")
        session.capture(f"continuation-{part}-control.png")
        before = len(notices())
        chord("Return")
        if part == "remaining":
            session.wait_for(lambda: title("File conflict") and title("File conflict")["is_focused"], "remaining exact conflict")
            chord("Tab")
            chord("Tab")
            chord("Return")
        session.wait_for(lambda: len(notices()) > before, f"saved {part} result")
        if part == "retry":
            session.wait_for(lambda: (destination / "b-failed").is_file(), "Retry installs the repaired child")
        else:
            session.wait_for(lambda: (destination / "c-conflict (2).txt").is_file() and (destination / "d-remaining.txt").is_file(), "Continue installs only remaining exact targets")
        session.capture(f"continuation-{part}-finished.png")
        assert prefix.stat().st_ino == inode and prefix.read_bytes() == b"Confirmed prefix\n"
        assert (destination / "c-conflict.txt").read_bytes() == b"Existing version\n"
        assert clipboard() == "New external clipboard"
    assert (destination / "b-failed").read_bytes() == b"Repaired child\n"
    assert (destination / "c-conflict (2).txt").read_bytes() == b"Incoming version\n"
    assert (destination / "d-remaining.txt").read_bytes() == b"Remaining bytes\n"
    focus(current)
    chord("z", "ctrl")
    if order[-1] == "retry":
        session.wait_for(lambda: not (destination / "b-failed").exists(), "Undo only failed-item successor")
    else:
        session.wait_for(lambda: not (destination / "d-remaining.txt").exists(), "Undo only remaining successor")
    assert prefix.stat().st_ino == inode
    chord("z", "ctrl")
    session.wait_for(lambda: not (destination / "b-failed").exists() and not (destination / "d-remaining.txt").exists(), "independent second successor Undo")
    assert prefix.stat().st_ino == inode and prefix.read_bytes() == b"Confirmed prefix\n"
    assert not (destination / "c-conflict (2).txt").exists()
    assert (destination / "c-conflict.txt").read_bytes() == b"Existing version\n"
    session.capture("continuation-independent-undo.png")
    return {"status": "passed", "scheme": scheme, "scale": scale, "order": order,
            "checks": ["saved executable failed/remaining plans survive zero windows and explicit Quit, then reactivate in the same owner after clipboard replacement",
                       "native Retry and Continue in opposite orders preserve the confirmed prefix, exact Keep both target and unrelated external clipboard",
                       "shared Undo reverses each successor independently while preserving the original completed prefix and existing file"]}
