"""A real temporarily missing destination, with the original folder restored."""
import json
import os
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Unavailable fixtures"
    assert root.is_relative_to(session.runtime / "home")
    source = root / "Source"
    destination = root / "Destination"
    source.mkdir(parents=True)
    destination.mkdir()
    (source / "a-completed.txt").write_bytes(b"Confirmed before destination loss\n")
    (source / "b-conflict.txt").write_bytes(b"Skipped incoming version\n")
    (source / "c-waiting.txt").write_bytes(b"Unstarted source remains safe\n")
    (destination / "b-conflict.txt").write_bytes(b"Existing destination version\n")


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Unavailable fixtures"
    source = root / "Source"
    destination = root / "Destination"
    parked = root / "Original destination"
    original = destination.stat().st_ino

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

    def navigate(path):
        chord("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        chord("Return")
        session.wait_for(lambda: title(path.name), "unavailable workflow navigation")
        time.sleep(.3)

    def notices(prefix):
        path = session.output / "notifications.json"
        if not path.exists():
            return []
        return [e for e in json.loads(path.read_text()) if e["title"].startswith(prefix)]

    def waiting(previous=None):
        if notices("Copy"):
            raise AssertionError(f"transfer stopped instead of exposing the waiting question: {notices('Copy')}")
        current = title("Destination unavailable")
        return current if current and current["is_focused"] and current["id"] != previous else None

    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "unavailable Browser focus")
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    navigate(source)
    chord("a", "ctrl")
    chord("c", "ctrl")
    navigate(destination)
    chord("v", "ctrl")
    session.wait_for(lambda: title("File conflict") and title("File conflict")["is_focused"], "conflict after confirmed prefix")
    assert (destination / "a-completed.txt").read_bytes() == b"Confirmed before destination loss\n"
    prefix = (destination / "a-completed.txt").stat().st_ino
    destination.rename(parked)
    assert parked.stat().st_ino == original
    for _ in range(3):
        chord("Tab")
    chord("Return")
    session.capture("unavailable-after-conflict-skip.png")
    question = session.wait_for(waiting, "typed unavailable destination question")
    session.capture("unavailable-first-question.png")
    assert not destination.exists()
    assert not (parked / "c-waiting.txt").exists()
    assert (parked / "a-completed.txt").stat().st_ino == prefix
    # Cancel is the default; Tab chooses Retry. An unsuccessful Retry must ask
    # again with a new generation/native window, without completing or writing.
    chord("Tab")
    chord("Return")
    missing = session.wait_for(lambda: waiting(question["id"]), "Retry while missing remains explicitly waiting")
    session.capture("unavailable-retry-still-missing.png")
    assert not destination.exists()
    assert not (parked / "c-waiting.txt").exists()
    assert (parked / "a-completed.txt").stat().st_ino == prefix
    assert (source / "c-waiting.txt").read_bytes() == b"Unstarted source remains safe\n"
    destination.mkdir()
    (destination / "unrelated.txt").write_bytes(b"An unrelated replacement folder\n")
    assert destination.stat().st_ino != original
    chord("Tab")
    chord("Return")
    session.capture("unavailable-replacement-folder.png")
    session.wait_for(lambda: waiting(missing["id"]), "Retry refuses an unrelated replacement destination")
    assert set(p.name for p in destination.iterdir()) == {"unrelated.txt"}
    assert (destination / "unrelated.txt").read_bytes() == b"An unrelated replacement folder\n"
    assert (parked / "a-completed.txt").stat().st_ino == prefix
    (destination / "unrelated.txt").unlink()
    destination.rmdir()
    parked.rename(destination)
    assert destination.stat().st_ino == original
    chord("Tab")
    chord("Return")
    session.wait_for(lambda: notices("Copy"), "Retry after restoring the original folder completes")
    session.wait_for(lambda: title("Destination unavailable") is None, "completed Retry closes the question")
    assert "2 completed" in notices("Copy")[-1]["body"] and "1 skipped" in notices("Copy")[-1]["body"]
    assert (destination / "a-completed.txt").stat().st_ino == prefix
    assert (destination / "c-waiting.txt").read_bytes() == b"Unstarted source remains safe\n"
    assert (destination / "b-conflict.txt").read_bytes() == b"Existing destination version\n"
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "parent focus after Retry")
    session.capture("unavailable-original-restored.png")
    chord("z", "ctrl")
    session.wait_for(lambda: not (destination / "a-completed.txt").exists() and not (destination / "c-waiting.txt").exists(), "Undo confirmed prefix and resumed copy")
    assert destination.stat().st_ino == original
    assert (destination / "b-conflict.txt").read_bytes() == b"Existing destination version\n"
    assert (source / "a-completed.txt").read_bytes() == b"Confirmed before destination loss\n"
    assert (source / "b-conflict.txt").read_bytes() == b"Skipped incoming version\n"
    assert (source / "c-waiting.txt").read_bytes() == b"Unstarted source remains safe\n"
    assert process.poll() is None
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["a temporarily missing ordinary destination retains its confirmed prefix and asks for attention without writing remaining files",
                       "Retry refuses missing and unrelated replacement folders, restoring the original completes only the unstarted file, and shared Undo reverses the actual copies"]}
