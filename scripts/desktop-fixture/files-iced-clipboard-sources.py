"""Owned Copy/Cut preserves source identity before actual native Paste."""
import json
import os
import subprocess
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Clipboard source fixtures"
    assert root.is_relative_to(session.runtime / "home")
    for name in ("Source", "Destination", "Parked"):
        (root / name).mkdir(parents=True)
    (root / "Source/selected.txt").write_bytes(b"Originally selected bytes\n")
    (root / "Source/selection-marker.txt").write_bytes(b"Different clipboard selection\n")


def verify(session, fixture, browser, scheme, scale, process, *, move=False):
    root = fixture / "Clipboard source fixtures"
    source = root / "Source/selected.txt"
    parked = root / "Parked/selected.txt"
    destination = root / "Destination/selected.txt"
    original = b"Originally selected bytes\n"
    replacement = b"Unrelated replacement bytes\n"
    kind = "move" if move else "copy"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def focus():
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()),
                         "clipboard-source Browser focus")

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
                         "clipboard-source navigation")
        time.sleep(.25)

    def clipboard():
        result = subprocess.run(["wl-paste", "--no-newline", "--type", "x-special/gnome-copied-files"],
                                env=session.env, capture_output=True, timeout=3)
        return result.stdout if result.returncode == 0 else None

    def selection(path, cut=False):
        return (("cut\n" if cut else "copy\n") + uri(path) + "\n").encode()

    def notifications():
        path = session.output / "notifications.json"
        if not path.exists():
            return []
        return json.loads(path.read_text())

    def completed():
        return [event for event in notifications() if event["title"].lower().startswith(kind)]

    def undone():
        return [event for event in notifications()
                if event["body"].split(" ·", 1)[0].endswith(" reversed")]

    def assert_refused_source(event):
        body = event["body"]
        assert str(source) in body and "selected source has changed" in body.lower(), event

    focus()
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    navigate(source)
    chord("x" if move else "c", "ctrl")
    expected = selection(source, cut=move)
    session.wait_for(lambda: clipboard() == expected, "original owned source publication")
    session.capture(f"clipboard-source-{kind}-selected.png")
    source.rename(parked)
    source.write_bytes(replacement)
    navigate(destination.parent)
    before = len(completed())
    chord("v", "ctrl")
    session.wait_for(lambda: len(completed()) > before, "changed-source Paste result")
    assert source.exists(), "stale Cut consumed an unrelated replacement at the original source pathname"
    assert source.read_bytes() == replacement
    assert parked.read_bytes() == original
    assert not destination.exists(), "stale clipboard Paste copied an unrelated replacement"
    assert_refused_source(completed()[-1])
    assert clipboard() == expected, "refused source intent was consumed instead of remaining inspectable"
    session.capture(f"clipboard-source-{kind}-refused.png")
    # Completion notification and Cut publication settle independently. A second
    # accepted Paste proves the preceding clipboard post-processing finished; it
    # must still refer to the original selection rather than refresh its identity.
    before = len(completed())
    deadline = time.monotonic() + 8
    while len(completed()) == before and time.monotonic() < deadline:
        assert clipboard() == expected, "failed Cut was cleared during asynchronous post-processing"
        chord("v", "ctrl")
        time.sleep(.6)
    assert len(completed()) > before, "Paste did not become ready after its failed predecessor"
    assert source.exists() and source.read_bytes() == replacement
    assert parked.read_bytes() == original and not destination.exists()
    assert_refused_source(completed()[-1])
    assert clipboard() == expected
    checks = [
        "owned Copy/Cut publishes the real selected source in a narrow native Browser",
        "Paste refuses a different file later placed at the selected pathname and preserves both originals",
        "the refused source is reported and its clipboard intent remains available",
        "a repeated Paste after clipboard post-processing still refuses the replacement",
    ]

    # A different publication makes the subsequent deliberate selection observable
    # even though reselecting the replacement produces the same source URI.
    marker = root / "Source/selection-marker.txt"
    navigate(marker)
    chord("c", "ctrl")
    session.wait_for(lambda: clipboard() == selection(marker), "intervening deliberate selection")
    navigate(source)
    chord("x" if move else "c", "ctrl")
    session.wait_for(lambda: clipboard() == expected, "fresh selection of the replacement")
    navigate(destination.parent)
    before = len(completed())
    chord("v", "ctrl")
    session.wait_for(lambda: len(completed()) > before and destination.exists(), "fresh deliberate Paste")
    assert destination.read_bytes() == replacement
    assert parked.read_bytes() == original
    if move:
        assert not source.exists()
        session.wait_for(lambda: clipboard() is None, "completed fresh Cut release")
    else:
        assert source.read_bytes() == replacement
    before_undo = len(undone())
    chord("z", "ctrl")
    session.wait_for(lambda: source.exists() and not destination.exists() and len(undone()) > before_undo,
                     "Undo of fresh deliberate Paste")
    assert source.read_bytes() == replacement
    assert parked.read_bytes() == original
    assert marker.read_bytes() == b"Different clipboard selection\n"
    session.capture(f"clipboard-source-{kind}-fresh-undo.png")
    checks.append("a fresh deliberate selection transfers the replacement and shared Undo preserves the parked original")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks}
