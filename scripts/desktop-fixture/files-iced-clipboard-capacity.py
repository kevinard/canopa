"""Verify owned clipboard replacement at the process source-observation budget.

The harness launches only Files
with a soft RLIMIT_NOFILE of 128; the desktop/harness limits remain unchanged.
"""
import json
import os
import subprocess
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Clipboard capacity fixtures"
    assert root.is_relative_to(session.runtime / "home")
    for name in ("Selection", "Marker", "Destination"):
        (root / name).mkdir(parents=True)
    for index in range(32):
        (root / "Selection" / f"selected-{index:02}.txt").write_bytes(b"Original bulk selection\n")
    (root / "Marker/marker.txt").write_bytes(b"Deliberate replacement selection\n")


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Clipboard capacity fixtures"
    marker = root / "Marker/marker.txt"
    destination = root / "Destination/marker.txt"
    originals = sorted((root / "Selection").iterdir())

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def focus():
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()),
                         "clipboard capacity Browser focus")

    def chord(key, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", key]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.15)

    def uri(path):
        return "file://" + quote_from_bytes(os.fsencode(path), safe="/")

    def navigate(path):
        focus()
        chord("l", "ctrl")
        session.run(["wtype", uri(path)])
        chord("Return")
        directory = path if path.is_dir() else path.parent
        session.wait_for(lambda: any(w["id"] == browser["id"] and
                                    w.get("title") == f"{directory.name} — Canopa Files" for w in windows()),
                         "clipboard capacity navigation")
        time.sleep(.25)

    def clipboard():
        result = subprocess.run(["wl-paste", "--no-newline", "--type", "x-special/gnome-copied-files"],
                                env=session.env, capture_output=True, timeout=3)
        return result.stdout if result.returncode == 0 else None

    def bulk_published():
        payload = clipboard()
        return payload is not None and payload.splitlines()[0:1] == [b"copy"] and \
            len(payload.splitlines()) == 33 and set(payload.splitlines()[1:]) == \
            {uri(path).encode() for path in originals}

    def events():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    focus()
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    navigate(root / "Selection")
    chord("a", "ctrl")
    chord("c", "ctrl")
    session.wait_for(bulk_published, "32-file Copy at the admitted source budget")
    session.capture("clipboard-capacity-bulk.png")
    navigate(marker)
    chord("c", "ctrl")
    expected = ("copy\n" + uri(marker) + "\n").encode()
    try:
        session.wait_for(lambda: clipboard() == expected,
                         "a deliberate one-file Copy replaces an idle full-budget owned selection")
    finally:
        session.capture("clipboard-capacity-replacement.png")
        (session.output / "clipboard-capacity-payload.json").write_text(json.dumps({
            "observed": None if clipboard() is None else clipboard().decode("utf-8", "replace"),
            "expected": expected.decode(), "events": events(),
        }, indent=2))
    assert process.poll() is None
    assert all(path.read_bytes() == b"Original bulk selection\n" for path in originals)
    assert marker.read_bytes() == b"Deliberate replacement selection\n"
    navigate(destination.parent)
    before = len(events())
    chord("v", "ctrl")
    session.wait_for(lambda: destination.exists() and len(events()) > before,
                     "replacement clipboard Paste completes")
    assert destination.read_bytes() == b"Deliberate replacement selection\n"
    assert clipboard() == expected
    before = len(events())
    chord("z", "ctrl")
    session.wait_for(lambda: not destination.exists() and len(events()) > before,
                     "replacement clipboard Paste is undoable")
    assert marker.read_bytes() == b"Deliberate replacement selection\n"
    assert all(path.read_bytes() == b"Original bulk selection\n" for path in originals)
    assert clipboard() == expected
    session.capture("clipboard-capacity-undo.png")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
        "32 real selected files publish from a Files process limited to128 descriptors",
        "a deliberate one-file Copy replaces the idle full-budget owned intent without changing source bytes",
        "the replacement clipboard copies exact marker bytes and shared Undo preserves every original",
    ]}
